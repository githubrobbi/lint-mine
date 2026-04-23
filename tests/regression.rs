//! End-to-end checks of lint-mine against throwaway two-crate workspaces.
//!
//! Each test builds `app` (changed) depending on `dep` (not changed), commits a base,
//! edits `app`, and runs the real binary with a one-lint posture. HOME points into the
//! fixture so lint-mine's cache never touches the real one; cargo and rustup keep theirs.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

const FLOAT: &str = "-D clippy::float_arithmetic\n";
const CLEAN_FN: &str = "/// Double.\npub fn double(x: i64) -> i64 { x.saturating_mul(2) }\n";
const FLOAT_FN: &str = "/// Third.\npub fn third(x: f64) -> f64 { x / 3.0 }\n";

static N: AtomicUsize = AtomicUsize::new(0);

struct Fixture {
    dir: PathBuf,
    real_home: PathBuf,
}

impl Fixture {
    /// `dep_src` and `app_src` are the committed bodies; `folder` is the checkout's name.
    fn new(folder: &str, dep_src: &str, app_src: &str) -> Self {
        let n = N.fetch_add(1, Ordering::SeqCst);
        let base = std::env::temp_dir().join(format!("lint-mine-test-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let dir = base.join(folder);
        for d in ["dep/src", "app/src", "posture", "home"] {
            fs::create_dir_all(dir.join(d)).unwrap();
        }
        fs::write(dir.join("Cargo.toml"), "[workspace]\nmembers = [\"dep\", \"app\"]\nresolver = \"2\"\n").unwrap();
        fs::write(dir.join("dep/Cargo.toml"), "[package]\nname = \"dep\"\nversion = \"0.1.0\"\nedition = \"2021\"\n").unwrap();
        fs::write(
            dir.join("app/Cargo.toml"),
            "[package]\nname = \"app\"\nversion = \"0.1.0\"\nedition = \"2021\"\n[dependencies]\ndep = { path = \"../dep\" }\n",
        )
        .unwrap();
        fs::write(dir.join("dep/src/lib.rs"), format!("//! dep\n{dep_src}")).unwrap();
        fs::write(dir.join("app/src/lib.rs"), format!("//! app\npub use dep;\n{app_src}")).unwrap();
        fs::write(dir.join("posture/clippy-flags.txt"), FLOAT).unwrap();
        fs::write(dir.join(".gitignore"), "posture/\nhome/\ntarget/\n").unwrap();
        let real_home = PathBuf::from(std::env::var("HOME").unwrap());
        let f = Fixture { dir, real_home };
        f.git(&["init", "-q"]);
        f.git(&["add", "-A"]);
        f.git(&["-c", "commit.gpgsign=false", "-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "base"]);
        f
    }

    fn git(&self, args: &[&str]) {
        let ok = Command::new("git").current_dir(&self.dir).args(args).status().unwrap().success();
        assert!(ok, "git {args:?}");
    }

    /// Append to app's lib.rs without committing: that is the diff lint-mine judges.
    fn change_app(&self, text: &str) {
        self.append("app/src/lib.rs", text);
    }

    fn change_dep(&self, text: &str) {
        self.append("dep/src/lib.rs", text);
    }

    fn append(&self, rel: &str, text: &str) {
        let p = self.dir.join(rel);
        let mut cur = fs::read_to_string(&p).unwrap();
        cur.push_str(text);
        fs::write(p, cur).unwrap();
    }

    /// Commit a file on top of the base, so it is part of what lint-mine diffs against.
    fn commit_file(&self, rel: &str, text: &str) {
        fs::write(self.dir.join(rel), text).unwrap();
        self.git(&["add", "-A"]);
        self.git(&["-c", "commit.gpgsign=false", "-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", rel]);
    }

    fn run(&self, extra: &[&str]) -> (i32, String) {
        let cargo_home = std::env::var("CARGO_HOME").map(PathBuf::from).unwrap_or_else(|_| self.real_home.join(".cargo"));
        let rustup_home = std::env::var("RUSTUP_HOME").map(PathBuf::from).unwrap_or_else(|_| self.real_home.join(".rustup"));
        let posture = self.dir.join("posture");
        let o: Output = Command::new(env!("CARGO_BIN_EXE_lint-mine"))
            .current_dir(&self.dir)
            .env("HOME", self.dir.join("home"))
            .env("CARGO_HOME", cargo_home)
            .env("RUSTUP_HOME", rustup_home)
            .env_remove("LINT_MINE_POSTURE")
            .args(["--base", "HEAD", "--no-fmt", "--posture"])
            .arg(&posture)
            .args(extra)
            .output()
            .unwrap();
        let text = format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
        (o.status.code().unwrap_or(-1), text)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if let Some(base) = self.dir.parent() {
            let _ = fs::remove_dir_all(base);
        }
    }
}

fn path(p: &Path) -> String {
    p.display().to_string()
}

/// The bug of 2026-04-21: the dependency and the changed crate both break the posture.
/// Before 0.2.0 the dependency failed first, the app was never linted, and the verdict was "clean".
#[test]
fn a_failing_dependency_does_not_hide_the_changed_crate() {
    let f = Fixture::new("ws", FLOAT_FN, CLEAN_FN);
    f.change_app("/// Quarter.\npub fn quarter(x: f64) -> f64 { x / 4.0 }\n");
    let (code, out) = f.run(&[]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("app/src/lib.rs"), "the finding in app is reported: {out}");
    assert!(!out.contains("dep/src/lib.rs"), "nothing is reported for dep: {out}");
    assert!(!out.contains("clean"), "{out}");
}

#[test]
fn clean_dependency_and_clean_change_are_clean() {
    let f = Fixture::new("ws", CLEAN_FN, CLEAN_FN);
    f.change_app("/// Triple.\npub fn triple(x: i64) -> i64 { x.saturating_mul(3) }\n");
    let (code, out) = f.run(&[]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("lint-mine: clean."), "{out}");
}

/// A dependency that does not compile at all: no verdict, and certainly not "clean".
#[test]
fn a_dependency_that_does_not_compile_is_never_clean() {
    let f = Fixture::new("ws", "/// Broken.\npub fn broken() -> u8 { \"not a number\" }\n", CLEAN_FN);
    f.change_app("/// Triple.\npub fn triple(x: i64) -> i64 { x.saturating_mul(3) }\n");
    let (code, out) = f.run(&[]);
    assert_eq!(code, 2, "{out}");
    assert!(out.contains("clippy did not lint app"), "{out}");
    assert!(out.contains("not a clean result"), "{out}");
    assert!(!out.contains("lint-mine: clean."), "{out}");
}

/// The tool's purpose (case A of the 2026-04-22 report): the changed crate's legacy code
/// trips the posture, the new lines are clean. That is "clean", exit 0, and the summary
/// says how much was left unjudged.
#[test]
fn legacy_findings_outside_the_diff_do_not_block() {
    let f = Fixture::new("ws", CLEAN_FN, FLOAT_FN);
    f.change_app("/// Triple.\npub fn triple(x: i64) -> i64 { x.saturating_mul(3) }\n");
    let (code, out) = f.run(&[]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("lint-mine: clean. Your 2 line(s) pass the strict posture (1 finding(s) elsewhere in these crates, not judged)."), "{out}");
    assert!(!out.contains("did not lint"), "{out}");
}

/// Case B: same legacy crate, and the new line breaks the posture too. Exactly one
/// finding, on the new line, exit 1; the legacy line is counted, not reported.
#[test]
fn a_finding_on_the_new_line_is_reported_the_legacy_one_is_not() {
    let f = Fixture::new("ws", CLEAN_FN, FLOAT_FN);
    f.change_app("/// Quarter.\npub fn quarter(x: f64) -> f64 { x / 4.0 }\n");
    let (code, out) = f.run(&[]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("app/src/lib.rs:6:"), "the finding is on the new line 6: {out}");
    assert!(!out.contains("app/src/lib.rs:4:"), "the legacy line 4 is not reported: {out}");
    assert!(out.contains("1 finding(s) inside your diff, 0 formatting spot(s) (1 finding(s) elsewhere in these crates, not judged)"), "{out}");
    assert!(!out.contains("clean"), "{out}");
}

/// The bug of 2026-04-22 (a workspace with crates rye, brioche and pretzel): two changed
/// crates, one of them with legacy findings. Passed as -D, the legacy crate failed to
/// compile, cargo stopped, and 0.2.0 said "did not lint" for the other one, exit 2.
#[test]
fn legacy_findings_in_one_changed_crate_do_not_stop_the_other() {
    let f = Fixture::new("ws", FLOAT_FN, CLEAN_FN);
    f.change_dep("/// Triple.\npub fn triple(x: i64) -> i64 { x.saturating_mul(3) }\n");
    f.change_app("/// Quadruple.\npub fn quadruple(x: i64) -> i64 { x.saturating_mul(4) }\n");
    let (code, out) = f.run(&[]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("crate(s): app, dep"), "{out}");
    assert!(out.contains("lint-mine: clean."), "{out}");
    assert!(!out.contains("did not lint"), "{out}");
}

/// A build script that needs the project's environment and fails without it: no verdict,
/// and the message names the environment, not the code.
#[test]
fn a_failing_build_script_names_the_environment() {
    let f = Fixture::new("ws", CLEAN_FN, CLEAN_FN);
    f.commit_file("dep/build.rs", "fn main() { panic!(\"Is bazel installed?\") }\n");
    f.change_app("/// Triple.\npub fn triple(x: i64) -> i64 { x.saturating_mul(3) }\n");
    let (code, out) = f.run(&[]);
    assert_eq!(code, 2, "{out}");
    assert!(out.contains("a build script failed before clippy could lint anything"), "{out}");
    assert!(out.contains("direnv exec . cargo mine"), "{out}");
    assert!(!out.contains("lint-mine: clean."), "{out}");
}

/// A worktree's folder name differs from the repository's; the overlay is found by the
/// origin remote's name, so `sourdough-4711` gets the settings of `sourdough`.
#[test]
fn the_overlay_is_keyed_by_the_remote_not_the_folder() {
    let f = Fixture::new("sourdough-4711", CLEAN_FN, CLEAN_FN);
    f.git(&["remote", "add", "origin", "git@github.example:org/sourdough.git"]);
    let repos = f.dir.join("posture/repos");
    fs::create_dir_all(&repos).unwrap();
    fs::write(repos.join("sourdough.flags"), "-A clippy::float_arithmetic\n").unwrap();
    f.change_app(FLOAT_FN);
    let (code, out) = f.run(&[]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains(&path(&repos.join("sourdough.flags"))), "{out}");

    // `relax` from the worktree writes to the same, remote-keyed file.
    let (_, out) = f.run(&["relax", "size"]);
    assert!(out.contains("sourdough.flags"), "{out}");
    assert!(!repos.join("sourdough-4711.flags").exists());
}

/// Without a remote, the folder name is the key, as before 0.2.0.
#[test]
fn without_a_remote_the_folder_name_is_the_key() {
    let f = Fixture::new("plain", CLEAN_FN, CLEAN_FN);
    let repos = f.dir.join("posture/repos");
    fs::create_dir_all(&repos).unwrap();
    fs::write(repos.join("plain.flags"), "-A clippy::float_arithmetic\n").unwrap();
    f.change_app(FLOAT_FN);
    let (code, out) = f.run(&[]);
    assert_eq!(code, 0, "{out}");
}

#[test]
fn version_names_the_build() {
    let o = Command::new(env!("CARGO_BIN_EXE_lint-mine")).arg("--version").output().unwrap();
    assert!(o.status.success());
    assert_eq!(String::from_utf8_lossy(&o.stdout).trim(), format!("lint-mine {}", env!("CARGO_PKG_VERSION")));
}
