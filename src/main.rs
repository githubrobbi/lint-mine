//! lint-mine: the strictest posture, applied to nothing but your own diff.
//!
//! Clippy always lints a whole crate. This tool runs it with a strict flag set that
//! lives OUTSIDE the repository (a posture directory with `clippy-flags.txt`,
//! `clippy.toml`, `rustfmt.toml`), then keeps only the diagnostics whose primary span
//! overlaps a line you added or changed. The team's `Cargo.toml`, `clippy.toml` and
//! `rustfmt.toml` are never touched; the artifacts go to `target/lint-mine` so the
//! team's build cache is never invalidated either.
//!
//! Usage:
//!   lint-mine                      changes since the merge-base with upstream (+ working tree)
//!   lint-mine --staged             only what is staged (what a pre-commit hook needs)
//!   lint-mine --base <ref>         diff against an explicit ref
//!   lint-mine --no-fmt             skip the rustfmt pass
//!   lint-mine --posture <dir>      posture directory (default ~/.config/lint-mine)
//!   lint-mine install-hook [--force]   write .git/hooks/pre-commit for this repo
//!   lint-mine install-alias        add `cargo mine` to ~/.cargo/config.toml
//!   lint-mine --version            the build, for bug reports
//!
//! Exit codes: 0 your lines are clean, 1 findings on your lines, 2 the verdict is not
//! possible (the crate does not compile, or clippy never reached a changed crate).
//! "clean" is printed only when every changed crate was actually linted.
//!
//! The posture is handed to clippy at WARN level whatever the flags file says (`-D x`
//! and `-F x` become `-W x`, `-A` stays). Deny is lint-mine's own decision, taken per
//! line: a finding on a changed line fails, a finding elsewhere in the crate is counted
//! and never judged. Passed as -D, one legacy finding would fail the whole crate before
//! the diff filter ever ran.

use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// A changed line range in one file (1-based, inclusive).
#[derive(Debug, Clone)]
struct Hunk {
    lo: usize,
    hi: usize,
}

type Hunks = BTreeMap<String, Vec<Hunk>>;

fn die(msg: &str) -> ! {
    eprintln!("lint-mine: {msg}");
    std::process::exit(2)
}

fn out(cmd: &mut Command) -> Result<String, String> {
    let o = cmd.stderr(Stdio::inherit()).output().map_err(|e| format!("spawn {:?}: {e}", cmd.get_program()))?;
    if !o.status.success() {
        return Err(format!("{:?} failed with {}", cmd.get_program(), o.status));
    }
    Ok(String::from_utf8_lossy(&o.stdout).to_string())
}

fn git(root: &Path, args: &[&str]) -> Result<String, String> {
    out(Command::new("git").arg("-C").arg(root).args(args))
}

fn repo_root() -> PathBuf {
    match out(Command::new("git").args(["rev-parse", "--show-toplevel"])) {
        Ok(s) => PathBuf::from(s.trim()),
        Err(_) => die("not inside a git repository"),
    }
}

/// The merge-base with the branch this one tracks, else origin/HEAD, else HEAD.
fn default_base(root: &Path) -> String {
    for candidate in ["@{upstream}", "origin/HEAD", "origin/main", "origin/master"] {
        if let Ok(s) = out(Command::new("git").arg("-C").arg(root).args(["merge-base", "HEAD", candidate]).stderr(Stdio::null())) {
            let s = s.trim().to_string();
            if !s.is_empty() {
                return s;
            }
        }
    }
    "HEAD".to_string()
}

/// Added/changed line ranges per file from a unified diff with zero context.
fn hunks_from_diff(diff: &str) -> Hunks {
    let mut hunks: Hunks = BTreeMap::new();
    let mut file = String::new();
    for line in diff.lines() {
        if let Some(rest) = line.strip_prefix("+++ b/") {
            file = rest.to_string();
        } else if line.starts_with("+++ /dev/null") {
            file.clear();
        } else if line.starts_with("@@") && !file.is_empty() {
            // @@ -a,b +c,d @@
            let plus = line.split_whitespace().nth(2).unwrap_or("+0,0");
            let plus = plus.trim_start_matches('+');
            let (start, count) = match plus.split_once(',') {
                Some((s, c)) => (s.parse().unwrap_or(0), c.parse().unwrap_or(1)),
                None => (plus.parse().unwrap_or(0), 1usize),
            };
            if count > 0 {
                hunks.entry(file.clone()).or_default().push(Hunk { lo: start, hi: start + count - 1 });
            }
        }
    }
    hunks
}

fn overlaps(h: &[Hunk], lo: usize, hi: usize) -> bool {
    h.iter().any(|x| lo <= x.hi && hi >= x.lo)
}

/// Package name and edition for every workspace member, keyed by manifest directory.
fn packages(root: &Path) -> Vec<(PathBuf, String, String)> {
    let meta = out(Command::new("cargo").current_dir(root).args(["metadata", "--no-deps", "--format-version", "1"]))
        .unwrap_or_else(|e| die(&format!("cargo metadata: {e}")));
    let v: Value = serde_json::from_str(&meta).unwrap_or_else(|e| die(&format!("cargo metadata json: {e}")));
    let mut pkgs = Vec::new();
    for p in v["packages"].as_array().cloned().unwrap_or_default() {
        let manifest = PathBuf::from(p["manifest_path"].as_str().unwrap_or_default());
        let dir = manifest.parent().map(Path::to_path_buf).unwrap_or_default();
        let name = p["name"].as_str().unwrap_or_default().to_string();
        let edition = p["edition"].as_str().unwrap_or("2021").to_string();
        pkgs.push((dir, name, edition));
    }
    pkgs
}

/// The packages whose directories contain the changed files (longest prefix wins).
fn packages_for(root: &Path, hunks: &Hunks, pkgs: &[(PathBuf, String, String)]) -> BTreeMap<String, String> {
    let mut hit: BTreeMap<String, String> = BTreeMap::new();
    for file in hunks.keys() {
        let abs = root.join(file);
        let mut best: Option<&(PathBuf, String, String)> = None;
        for p in pkgs {
            if abs.starts_with(&p.0) && best.is_none_or(|b| p.0.as_os_str().len() > b.0.as_os_str().len()) {
                best = Some(p);
            }
        }
        if let Some(p) = best {
            hit.insert(p.1.clone(), p.2.clone());
        }
    }
    hit
}

/// The names a repository's own files go by in the posture, best first: the origin
/// remote's repository name, which every checkout and worktree of it shares, then the
/// checkout's folder name (the only key before 0.2.0, and the fallback without a remote).
fn repo_names(root: &Path) -> Vec<String> {
    let mut names = Vec::new();
    let url = Command::new("git").arg("-C").arg(root).args(["remote", "get-url", "origin"]).stderr(Stdio::null()).output();
    if let Ok(o) = url {
        if o.status.success() {
            if let Some(n) = repo_name_from_url(String::from_utf8_lossy(&o.stdout).trim()) {
                names.push(n);
            }
        }
    }
    if let Some(f) = root.file_name() {
        let f = f.to_string_lossy().to_string();
        if !names.contains(&f) {
            names.push(f);
        }
    }
    if names.is_empty() {
        names.push("repo".into());
    }
    names
}

/// `git@host:org/sourdough.git`, `https://host/org/sourdough/`, `/srv/sourdough.git`
/// all name `sourdough`.
fn repo_name_from_url(url: &str) -> Option<String> {
    let tail = url.trim_end_matches('/').rsplit(['/', ':']).next()?;
    let name = tail.strip_suffix(".git").unwrap_or(tail);
    (!name.is_empty() && name != "." && name != "..").then(|| name.to_string())
}

/// This repository's overlay: the first of its names that has a file, else the file a new
/// overlay goes to (keyed by the remote's name, so worktrees share it).
fn overlay_path(posture: &Path, root: &Path) -> PathBuf {
    let dir = posture.join("repos");
    let names = repo_names(root);
    names
        .iter()
        .map(|n| dir.join(format!("{n}.flags")))
        .find(|p| p.exists())
        .unwrap_or_else(|| dir.join(format!("{}.flags", names[0])))
}

/// The global posture, then the per-repository overlay `repos/<repo name>.flags`
/// (later flags win on clippy's command line, so an overlay can relax or tighten any lint).
fn load_flags(posture: &Path, root: &Path) -> Vec<String> {
    let parse = |text: &str| -> Vec<String> {
        text.lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .flat_map(|l| l.split_whitespace().map(String::from).collect::<Vec<_>>())
            .collect()
    };
    let text = fs::read_to_string(posture.join("clippy-flags.txt"))
        .unwrap_or_else(|_| die(&format!("no clippy-flags.txt in {}", posture.display())));
    let mut flags = parse(&text);
    let overlay = overlay_path(posture, root);
    if let Ok(extra) = fs::read_to_string(&overlay) {
        let more = parse(&extra);
        println!("lint-mine: overlay {} ({} flag(s))", overlay.display(), more.len());
        flags.extend(more);
    }
    flags
}

/// The posture at warn level: `-D`/`-F` (and their long forms) become `-W`, so legacy
/// code never fails the crate; the deny decision is made per changed line afterwards.
fn at_warn_level(flags: &[String]) -> Vec<String> {
    let mut out = Vec::with_capacity(flags.len());
    let mut i = 0;
    while i < flags.len() {
        let f = &flags[i];
        match f.as_str() {
            "-D" | "-F" | "--deny" | "--forbid" => {
                out.push("-W".into());
                if let Some(name) = flags.get(i + 1) {
                    out.push(name.clone());
                    i += 1;
                }
            }
            _ if f.starts_with("-D") || f.starts_with("-F") => out.push(format!("-W{}", &f[2..])),
            _ if f.starts_with("--deny=") || f.starts_with("--forbid=") => {
                out.push(format!("-W{}", f.split_once('=').map(|x| x.1).unwrap_or_default()));
            }
            _ => out.push(f.clone()),
        }
        i += 1;
    }
    out
}

struct Finding {
    file: String,
    line: usize,
    lint: String,
    rendered: String,
    /// The span starts outside your hunks and merely encloses them (a crate-level or
    /// item-level lint on legacy code you touched): reported, never blocking.
    enclosing: bool,
}

/// Build artifacts live under ~/.cache/lint-mine/<repo-name>-<hash>/, never in the repository.
fn cache_target_dir(root: &Path) -> PathBuf {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    root.hash(&mut h);
    let name = root.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "repo".into());
    let home = PathBuf::from(std::env::var("HOME").unwrap_or_default());
    home.join(".cache/lint-mine").join(format!("{name}-{:016x}", h.finish())).join("target")
}

/// What one clippy run established besides the findings: which changed crates clippy
/// actually reached, and everything that keeps the run from supporting a verdict.
struct ClippyRun {
    findings: Vec<Finding>,
    /// Rendered compile errors (no lint code, or an `E0xxx` code) in the changed crates.
    compile_errors: Vec<String>,
    /// Error-level lint diagnostics in the changed crates, inside the diff or not.
    lint_errors: usize,
    /// Posture findings in the changed crates on lines you did not touch: counted, not judged.
    elsewhere: usize,
    /// A build script of some crate failed: clippy never ran on the sources.
    build_script_failed: bool,
    /// Changed crates with an artifact or a diagnostic from a real target: proof of a lint.
    checked: BTreeSet<String>,
    /// Error-level diagnostics in crates nobody changed, per crate, and the first one.
    foreign_errors: BTreeMap<String, usize>,
    foreign_first: Option<String>,
    exit: std::process::ExitStatus,
    stderr_tail: String,
}

/// A compile error rather than a lint: rustc's own errors carry no code or an `E0xxx` code.
fn is_compile_error(code: &str) -> bool {
    code.is_empty() || (code.len() == 5 && code.starts_with('E') && code[1..].bytes().all(|b| b.is_ascii_digit()))
}

/// The workspace package a cargo JSON message belongs to, by its manifest path.
fn package_of(v: &Value, all: &[(PathBuf, String, String)]) -> String {
    let manifest = PathBuf::from(v["manifest_path"].as_str().unwrap_or_default());
    let dir = manifest.parent().map(Path::to_path_buf).unwrap_or_default();
    all.iter()
        .find(|p| p.0 == dir)
        .map(|p| p.1.clone())
        .or_else(|| v["target"]["name"].as_str().map(String::from))
        .unwrap_or_else(|| "?".into())
}

fn run_clippy(root: &Path, posture: &Path, pkgs: &BTreeMap<String, String>, all: &[(PathBuf, String, String)], hunks: &Hunks) -> ClippyRun {
    let flags = load_flags(posture, root);
    let mut cmd = Command::new("cargo");
    cmd.current_dir(root)
        .env("CLIPPY_CONF_DIR", posture)
        .env("CARGO_TARGET_DIR", cache_target_dir(root))
        .args(["clippy", "--all-targets", "--message-format=json", "--quiet"]);
    for p in pkgs.keys() {
        cmd.arg("-p").arg(p);
    }
    // The posture is for the crates you changed. Without --no-deps clippy applies the
    // flags to every workspace dependency too; one of those fails first, cargo stops, and
    // the changed crate is never linted at all.
    cmd.arg("--no-deps");
    cmd.arg("--").args(at_warn_level(&flags));
    // The JSON stream on stdout carries every diagnostic. stderr is kept only to explain
    // a run that failed without one.
    let o = cmd.stderr(Stdio::piped()).output().unwrap_or_else(|e| die(&format!("cargo clippy: {e}")));
    let text = String::from_utf8_lossy(&o.stdout);
    let err = String::from_utf8_lossy(&o.stderr);
    let tail: Vec<&str> = err.lines().rev().take(15).collect();
    let mut run = ClippyRun {
        findings: Vec::new(),
        compile_errors: Vec::new(),
        lint_errors: 0,
        elsewhere: 0,
        build_script_failed: err.contains("failed to run custom build command") || err.contains("could not compile") && err.contains("build_script_build"),
        checked: BTreeSet::new(),
        foreign_errors: BTreeMap::new(),
        foreign_first: None,
        exit: o.status,
        stderr_tail: tail.into_iter().rev().collect::<Vec<_>>().join("\n"),
    };
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for line in text.lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
        let reason = v["reason"].as_str().unwrap_or("");
        if reason != "compiler-message" && reason != "compiler-artifact" {
            continue;
        }
        let pkg = package_of(&v, all);
        let ours = pkgs.contains_key(&pkg);
        let build_script = v["target"]["kind"].as_array().is_some_and(|k| k.iter().any(|x| x == "custom-build"));
        if ours && !build_script {
            run.checked.insert(pkg.clone());
        }
        if reason == "compiler-artifact" {
            continue;
        }
        let m = &v["message"];
        let level = m["level"].as_str().unwrap_or("");
        let code = m["code"]["code"].as_str().unwrap_or("").to_string();
        let rendered = m["rendered"].as_str().unwrap_or("").to_string();
        if level == "error" && m["message"].as_str().unwrap_or("").starts_with("aborting due to") {
            continue;
        }
        if !ours {
            // Warnings from crates you did not change are not yours to judge; an error
            // there means clippy may never have reached yours.
            if level == "error" {
                *run.foreign_errors.entry(pkg).or_default() += 1;
                run.foreign_first.get_or_insert(rendered);
            }
            continue;
        }
        if level == "error" && is_compile_error(&code) {
            run.compile_errors.push(rendered);
            continue;
        }
        if level == "error" {
            run.lint_errors += 1;
        }
        if level != "warning" && level != "error" {
            continue;
        }
        for span in m["spans"].as_array().cloned().unwrap_or_default() {
            if span["is_primary"] != true {
                continue;
            }
            let file = span["file_name"].as_str().unwrap_or("").to_string();
            let lo = span["line_start"].as_u64().unwrap_or(0) as usize;
            let hi = span["line_end"].as_u64().unwrap_or(lo as u64) as usize;
            let key = format!("{file}:{lo}:{code}");
            if !seen.insert(key) {
                continue;
            }
            match hunks.get(&file) {
                Some(h) if overlaps(h, lo, hi) => {
                    let enclosing = !overlaps(h, lo, lo);
                    run.findings.push(Finding { file, line: lo, lint: code.clone(), rendered: rendered.clone(), enclosing });
                }
                _ => run.elsewhere += 1,
            }
        }
    }
    run.findings.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));
    run
}

/// Refuse a verdict the run cannot support: a changed crate clippy never reached, errors
/// in crates nobody changed, or a failed run without a single diagnostic. Exits 2.
fn require_trustworthy(run: &ClippyRun, pkgs: &BTreeMap<String, String>) {
    let unchecked: Vec<&str> = pkgs.keys().filter(|p| !run.checked.contains(*p)).map(String::as_str).collect();
    let foreign: Vec<String> = run.foreign_errors.iter().map(|(p, n)| format!("{p} failed ({n} diagnostic(s))")).collect();
    let silent_failure = !run.exit.success() && run.lint_errors == 0 && run.foreign_errors.is_empty();
    if unchecked.is_empty() && foreign.is_empty() && !silent_failure {
        return;
    }
    if let Some(first) = &run.foreign_first {
        eprintln!("{}", first.trim_end());
    }
    if (silent_failure || (!unchecked.is_empty() && foreign.is_empty())) && !run.stderr_tail.trim().is_empty() {
        eprintln!("{}", run.stderr_tail);
    }
    if run.build_script_failed {
        eprintln!("lint-mine: a build script failed before clippy could lint anything; this is the project's build");
        eprintln!("lint-mine: environment, not your code. Run it the way the project builds, e.g. `direnv exec . cargo mine`");
        eprintln!("lint-mine: or after sourcing the project's env setup, then try again.");
    }
    let why = if !foreign.is_empty() {
        foreign.join(", ")
    } else {
        format!("cargo clippy exited with {}", run.exit)
    };
    if unchecked.is_empty() {
        eprintln!("lint-mine: clippy reported errors outside your crates: {why}; not a clean result");
    } else {
        eprintln!("lint-mine: clippy did not lint {}: {why}; not a clean result", unchecked.join(", "));
    }
    std::process::exit(2);
}

/// rustfmt --check per changed file; a "Diff in <file> at line N" counts only if N is in a hunk.
fn run_fmt(root: &Path, posture: &Path, hunks: &Hunks, edition: &str) -> Vec<(String, usize)> {
    let cfg = posture.join("rustfmt.toml");
    let mut hits = Vec::new();
    for (file, h) in hunks {
        if !file.ends_with(".rs") {
            continue;
        }
        let abs = root.join(file);
        let mut cmd = Command::new("rustfmt");
        cmd.current_dir(root).args(["--check", "--edition", edition]);
        if cfg.exists() {
            cmd.arg("--config-path").arg(&cfg);
        }
        cmd.arg(&abs);
        let o = match cmd.stderr(Stdio::null()).output() {
            Ok(o) => o,
            Err(_) => {
                eprintln!("lint-mine: rustfmt not found; skipping format check");
                return hits;
            }
        };
        let text = String::from_utf8_lossy(&o.stdout);
        for line in text.lines() {
            // "Diff in /abs/path/file.rs:123:" (newer) or "Diff in /abs/path/file.rs at line 123:" (older)
            if let Some(rest) = line.strip_prefix("Diff in ") {
                let n = rest
                    .rsplit(':')
                    .nth(1)
                    .and_then(|s| s.trim().rsplit(' ').next())
                    .and_then(|s| s.trim().parse::<usize>().ok());
                if let Some(n) = n {
                    if overlaps(h, n, n + 2) {
                        hits.push((file.clone(), n));
                    }
                }
            }
        }
    }
    hits
}

fn install_hook(root: &Path, force: bool) {
    let hook = root.join(".git/hooks/pre-commit");
    if hook.exists() && !force {
        die(&format!("{} exists; use --force to replace it", hook.display()));
    }
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("lint-mine"));
    let body = format!("#!/bin/sh\n# lint-mine: the strict posture, on the staged diff only (local hook, not committed)\nexec \"{}\" --staged\n", exe.display());
    fs::write(&hook, body).unwrap_or_else(|e| die(&format!("write hook: {e}")));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&hook, fs::Permissions::from_mode(0o755));
    }
    println!("installed {}", hook.display());
}

/// Remove the hook, but only when it is the one lint-mine wrote.
fn uninstall_hook(root: &Path) {
    let hook = root.join(".git/hooks/pre-commit");
    match fs::read_to_string(&hook) {
        Ok(body) if body.contains("lint-mine") => { let _ = fs::remove_file(&hook); println!("removed {}", hook.display()); }
        Ok(_) => println!("{} is not lint-mine's hook; left in place", hook.display()),
        Err(_) => println!("no pre-commit hook in this repository"),
    }
}

/// The full undo: every cache, the posture, the cargo alias, the binary itself.
fn uninstall_everything() {
    let home = PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| die("HOME unset")));
    for d in [home.join(".cache/lint-mine"), home.join(".config/lint-mine")] {
        if d.exists() { let _ = fs::remove_dir_all(&d); println!("removed {}", d.display()); }
    }
    let cfg = home.join(".cargo/config.toml");
    if let Ok(cur) = fs::read_to_string(&cfg) {
        let kept: Vec<&str> = cur.lines().filter(|l| l.trim() != "mine = \"lint-mine\"").collect();
        let mut text = kept.join("\n");
        if cur.ends_with('\n') { text.push('\n'); }
        if text.trim() == "[alias]" { text.clear(); }
        if text != cur { let _ = fs::write(&cfg, text); println!("removed the `cargo mine` alias from {}", cfg.display()); }
    }
    let bin = home.join(".local/bin/lint-mine");
    if bin.exists() { let _ = fs::remove_file(&bin); println!("removed {}", bin.display()); }
    println!("uninstall done. Hooks in repositories: run `lint-mine uninstall-hook` in each before this, or delete .git/hooks/pre-commit by hand (it only references a binary that is now gone).");
}

/// Presets: a word for a domain decision, expanded to the lints it governs.
const PRESETS: &[(&str, &[&str])] = &[
    ("floats", &["clippy::float_arithmetic"]),
    ("float-casts", &["clippy::cast_precision_loss", "clippy::cast_possible_truncation", "clippy::cast_sign_loss", "clippy::cast_lossless"]),
    ("docs", &["missing_docs", "clippy::missing_docs_in_private_items", "clippy::missing_errors_doc", "clippy::missing_panics_doc"]),
    ("size", &["clippy::too_many_lines", "clippy::too_many_arguments", "clippy::cognitive_complexity"]),
    ("indexing", &["clippy::indexing_slicing"]),
    ("prints", &["clippy::print_stdout", "clippy::print_stderr", "clippy::use_debug"]),
    ("std", &["clippy::std_instead_of_core", "clippy::std_instead_of_alloc"]),
];

fn posture_dir(p: Option<PathBuf>) -> PathBuf {
    p.unwrap_or_else(|| PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".config/lint-mine"))
}

fn expand(names: &[String]) -> Vec<String> {
    let mut v = Vec::new();
    for n in names {
        if let Some((_, lints)) = PRESETS.iter().find(|(k, _)| *k == n.as_str()) {
            v.extend(lints.iter().map(|l| (*l).to_string()));
        } else if n.contains("::") || n.starts_with("rustdoc") || ["missing_docs", "unsafe_code", "unused_results", "unreachable_pub", "unused_qualifications", "trivial_casts", "trivial_numeric_casts", "unused_import_braces", "unused_lifetimes", "variant_size_differences", "non_ascii_idents"].contains(&n.as_str()) {
            v.push(n.clone());
        } else {
            v.push(format!("clippy::{n}"));
        }
    }
    v
}

/// `relax`/`tighten`: append -A/-D lines to the repo overlay (or the global flags with --global).
fn adjust(root: &Path, posture: &Path, relax: bool, names: &[String], global: bool) {
    if names.is_empty() { die("name at least one lint or preset (lint-mine overlay lists the presets)"); }
    let lints = expand(names);
    let target = if global {
        posture.join("clippy-flags.txt")
    } else {
        let _ = fs::create_dir_all(posture.join("repos"));
        overlay_path(posture, root)
    };
    let mut cur = fs::read_to_string(&target).unwrap_or_default();
    if !cur.is_empty() && !cur.ends_with('\n') { cur.push('\n'); }
    let level = if relax { "-A" } else { "-D" };
    let mut added = 0;
    for l in &lints {
        let line = format!("{level} {l}");
        // drop an earlier opposite decision for the same lint so the file stays readable
        cur = cur.lines().filter(|x| x.split_whitespace().nth(1) != Some(l.as_str())).map(|x| format!("{x}\n")).collect();
        cur.push_str(&format!("{line}\n"));
        added += 1;
    }
    fs::write(&target, cur).unwrap_or_else(|e| die(&format!("write {}: {e}", target.display())));
    println!("{} {added} lint(s) in {}", if relax { "relaxed" } else { "tightened" }, target.display());
    for l in &lints { println!("  {level} {l}"); }
}

fn show_overlay(root: &Path, posture: &Path) {
    let f = overlay_path(posture, root);
    match fs::read_to_string(&f) {
        Ok(t) => { println!("overlay {}:", f.display()); for l in t.lines() { println!("  {l}"); } }
        Err(_) => println!("no overlay for this repository yet ({})", f.display()),
    }
    println!("presets: {}", PRESETS.iter().map(|(k, v)| format!("{k} ({})", v.len())).collect::<Vec<_>>().join(", "));
}

/// 1004 -> "1,004", for the summary line.
fn thousands(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

fn install_alias() {
    let home = PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| die("HOME unset")));
    let cfg = home.join(".cargo/config.toml");
    let cur = fs::read_to_string(&cfg).unwrap_or_default();
    if cur.contains("mine = ") {
        println!("alias `cargo mine` already present in {}", cfg.display());
        return;
    }
    let mut s = cur;
    if !s.contains("[alias]") {
        s.push_str("\n[alias]\n");
    }
    s = s.replacen("[alias]\n", "[alias]\nmine = \"lint-mine\"\n", 1);
    let _ = fs::create_dir_all(cfg.parent().unwrap());
    fs::write(&cfg, s).unwrap_or_else(|e| die(&format!("write {}: {e}", cfg.display())));
    println!("added `cargo mine` to {} (cargo passes extra args through: cargo mine --staged)", cfg.display());
}

fn main() {
    let mut args = std::env::args().skip(1).peekable();
    let mut base: Option<String> = None;
    let mut staged = false;
    let mut fmt = true;
    let mut force = false;
    let mut allow_team_config = false;
    let mut posture: Option<PathBuf> = std::env::var_os("LINT_MINE_POSTURE").map(PathBuf::from);
    let mut sub: Option<String> = None;
    let mut sub_args: Vec<String> = Vec::new();
    let mut global = false;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--base" => base = args.next(),
            "--staged" => staged = true,
            "--no-fmt" => fmt = false,
            "--force" => force = true,
            "--allow-team-config" => allow_team_config = true,
            "--global" => global = true,
            "-V" | "--version" => {
                println!("lint-mine {}", env!("CARGO_PKG_VERSION"));
                return;
            }
            w if sub.as_deref() == Some("relax") || sub.as_deref() == Some("tighten") => sub_args.push(w.to_string()),
            "--posture" => posture = args.next().map(PathBuf::from),
            "install-hook" | "install-alias" | "uninstall-hook" | "uninstall" | "reset" | "overlay" => sub = Some(a),
            "relax" | "tighten" => { sub = Some(a); }
            "-h" | "--help" => {
                println!("lint-mine [--base REF | --staged] [--no-fmt] [--posture DIR] [--allow-team-config]");
                println!("lint-mine install-hook [--force] | uninstall-hook | install-alias | reset | uninstall");
                println!("  reset      this repo: remove the hook (only if it is ours) and this repo's build cache");
                println!("  uninstall  everything: every repo cache, ~/.config/lint-mine, the cargo alias, ~/.local/bin/lint-mine");
                println!("lint-mine relax <lint|preset>... [--global]   e.g. relax floats, relax clippy::too_many_lines");
                println!("lint-mine tighten <lint|preset>... [--global] the opposite (-D)");
                println!("lint-mine overlay                            show this repo's overlay and the presets");
                println!("lint-mine --version                          the build, for bug reports");
                return;
            }
            other => die(&format!("unknown argument {other}")),
        }
    }
    match sub.as_deref() {
        Some("install-alias") => { install_alias(); return; }
        Some("uninstall") => { uninstall_everything(); return; }
        _ => {}
    }
    let root = repo_root();
    match sub.as_deref() {
        Some("relax") | Some("tighten") => { adjust(&root, &posture_dir(posture.clone()), sub.as_deref() == Some("relax"), &sub_args, global); return; }
        Some("overlay") => { show_overlay(&root, &posture_dir(posture.clone())); return; }
        Some("install-hook") => { install_hook(&root, force); return; }
        Some("uninstall-hook") => { uninstall_hook(&root); return; }
        Some("reset") => { uninstall_hook(&root); let t = cache_target_dir(&root); let dir = t.parent().unwrap_or(&t); let _ = fs::remove_dir_all(dir); println!("removed {}", dir.display()); println!("reset done: this repository carries nothing of lint-mine"); return; }
        _ => {}
    }
    let posture = posture.unwrap_or_else(|| {
        PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".config/lint-mine")
    });
    if !posture.join("clippy-flags.txt").exists() {
        die(&format!("posture directory {} has no clippy-flags.txt", posture.display()));
    }

    let diff = if staged {
        git(&root, &["diff", "--cached", "-U0", "--no-color", "--diff-filter=AM", "--", "*.rs"])
    } else {
        let b = base.unwrap_or_else(|| default_base(&root));
        git(&root, &["diff", &b, "-U0", "--no-color", "--diff-filter=AM", "--", "*.rs"])
    }
    .unwrap_or_else(|e| die(&e));
    if staged && !allow_team_config {
        let touched = git(&root, &["diff", "--cached", "--name-only"]).unwrap_or_default();
        let bad: Vec<&str> = touched.lines().filter(|f| {
            let base = f.rsplit('/').next().unwrap_or(f);
            matches!(base, "clippy.toml" | "rustfmt.toml" | "rust-toolchain.toml" | ".rustfmt.toml") || f.ends_with(".cargo/config.toml") || f.ends_with(".cargo/config")
        }).collect();
        let lints_in_cargo = git(&root, &["diff", "--cached", "-U0", "--", "Cargo.toml", "*/Cargo.toml"]).unwrap_or_default()
            .lines().any(|l| l.starts_with('+') && (l.contains("[lints") || l.contains("[workspace.lints")));
        if !bad.is_empty() || lints_in_cargo {
            eprintln!("lint-mine: this commit touches the TEAM's lint/format/toolchain configuration ({}{}).",
                bad.join(", "), if lints_in_cargo { " and [lints] tables in Cargo.toml" } else { "" });
            eprintln!("lint-mine: your posture belongs in ~/.config/lint-mine, not in the repo. Re-run with --allow-team-config if this is intended.");
            std::process::exit(1);
        }
    }
    let hunks = hunks_from_diff(&diff);
    if hunks.is_empty() {
        println!("lint-mine: no changed Rust lines; nothing to judge");
        return;
    }
    let changed: usize = hunks.values().map(|v| v.iter().map(|h| h.hi - h.lo + 1).sum::<usize>()).sum();
    let all = packages(&root);
    let pkgs = packages_for(&root, &hunks, &all);
    if pkgs.is_empty() {
        die("changed files belong to no workspace package");
    }
    println!(
        "lint-mine: {changed} changed line(s) in {} file(s), crate(s): {}",
        hunks.len(),
        pkgs.keys().cloned().collect::<Vec<_>>().join(", ")
    );

    let run = run_clippy(&root, &posture, &pkgs, &all, &hunks);
    if !run.compile_errors.is_empty() {
        for e in &run.compile_errors {
            eprintln!("{}", e.trim_end());
        }
        eprintln!("lint-mine: the crate does not compile; fix that first");
        std::process::exit(2);
    }
    require_trustworthy(&run, &pkgs);
    let findings = &run.findings;
    let edition = pkgs.values().next().cloned().unwrap_or_else(|| "2021".into());
    let fmt_hits = if fmt { run_fmt(&root, &posture, &hunks, &edition) } else { Vec::new() };

    let (mine, enclosing): (Vec<&Finding>, Vec<&Finding>) = findings.iter().partition(|f| !f.enclosing);
    for f in &mine {
        println!("{}", f.rendered.trim_end());
    }
    if !enclosing.is_empty() {
        println!("\nlint-mine: {} pre-existing item(s) enclose your change (not yours, not blocking):", enclosing.len());
        for f in &enclosing {
            println!("  {}:{}  {}", f.file, f.line, f.lint);
        }
    }
    for (file, line) in &fmt_hits {
        println!("rustfmt: {file}:{line} is not formatted to the posture (rustfmt --config-path {} {file})", posture.join("rustfmt.toml").display());
    }
    let lints: BTreeMap<&str, usize> = mine.iter().fold(BTreeMap::new(), |mut m, f| {
        *m.entry(f.lint.as_str()).or_default() += 1;
        m
    });
    let elsewhere = if run.elsewhere == 0 {
        String::new()
    } else {
        format!(" ({} finding(s) elsewhere in these crates, not judged)", thousands(run.elsewhere))
    };
    if mine.is_empty() && fmt_hits.is_empty() {
        println!("lint-mine: clean. Your {changed} line(s) pass the strict posture{elsewhere}.");
        return;
    }
    println!("\nlint-mine: {} finding(s) inside your diff, {} formatting spot(s){elsewhere}:", mine.len(), fmt_hits.len());
    for (lint, n) in lints {
        println!("  {n:3}  {lint}");
    }
    std::process::exit(1);
}
