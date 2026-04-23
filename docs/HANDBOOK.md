# The lint-mine handbook

How to use lint-mine well: for the engineer who wants the strictest Rust posture on
their own work, and for the team member who inherits it. Every command here was run
against the code it describes, on a small bakery workspace (crates `oven`, `proofer`
and `rye`) that the examples keep returning to. Where something is a limit rather
than a feature, it says so.

Version of this handbook: lint-mine 0.2.1.

## How to read this

- **Chapters 1 to 4** take you from nothing to a first run. Do them once, in order.
- **Chapters 5 to 9** are daily use: reading the output, the hook, overlays, legacy
  code, workspaces.
- **Chapters 10 to 13** are reference: caches and toolchains, when something looks
  wrong, undo, every command.
- **Chapter 14** is for whoever changes lint-mine itself. **Chapter 15**, "The words in this book" on the page, is the
  glossary; on the rendered page the first mention of each term in every chapter
  is a dotted link that explains itself on hover.

Anything you type is shown like this:

```bash
cargo mine
```

Paths are written for macOS and Linux. `~` is your home folder.

## Contents

1. [What lint-mine is](#1-what-lint-mine-is)
2. [Install](#2-install)
3. [The posture](#3-the-posture)
4. [Your first run](#4-your-first-run)
5. [Reading the output](#5-reading-the-output)
6. [Daily use and the hook](#6-daily-use-and-the-hook)
7. [Per-repository overlays](#7-per-repository-overlays)
8. [Legacy code and rules that are wrong](#8-legacy-code-and-rules-that-are-wrong)
9. [Workspaces, crates and build scripts](#9-workspaces-crates-and-build-scripts)
10. [Caches, toolchains and speed](#10-caches-toolchains-and-speed)
11. [When something looks wrong](#11-when-something-looks-wrong)
12. [Undo](#12-undo)
13. [Command reference](#13-command-reference)
14. [Working on lint-mine itself](#14-working-on-lint-mine-itself)
15. [Glossary](#glossary)

## 1. What lint-mine is

Clippy lints a whole crate. On a codebase with history, the strictest lint groups
(`clippy::pedantic`, `clippy::nursery`, a few hundred named lints, `missing_docs`)
report thousands of findings in code nobody is touching, so nobody turns them on. The
team's `Cargo.toml` and `clippy.toml` carry the team's compromise, and your own
standards have nowhere to live.

lint-mine gives them a place. It runs clippy and rustfmt with a posture that is yours,
kept outside the repository, and keeps only the diagnostics that land on lines you
added or changed. Everything else is counted and dropped. The verdict is about your
diff and nothing else.

What it never does:

- It never writes into the repository. No attributes, no `clippy.toml`, no `[lints]`
  table, no build artifacts under `target/`. The pre-commit hook it can install
  lives in `.git/hooks`, which git does not track.
- It never touches the team's configuration. Their `Cargo.toml`, `clippy.toml`,
  `rustfmt.toml` and pinned toolchain run as they are.
- It never judges code you did not change. A finding on an untouched line is reported
  as a number in the summary, not as a finding.

What you get: write code, run `cargo mine`, fix what it shows on your lines, commit.
Reviewers see clean code and none of the machinery.

## 2. Install

You need a Rust toolchain with clippy and rustfmt (`rustup component add clippy
rustfmt`) and git. Nothing else: lint-mine is one binary with one dependency
(`serde_json`), and it needs no admin rights.

```bash
git clone https://github.com/githubrobbi/lint-mine ~/src/lint-mine
cd ~/src/lint-mine
cargo build --release
mkdir -p ~/.local/bin
install -m 0755 target/release/lint-mine ~/.local/bin/lint-mine
lint-mine --version
```

`~/.local/bin` must be on your `PATH`. If `lint-mine --version` says "command not
found", add `export PATH="$HOME/.local/bin:$PATH"` to your shell profile and open a
new terminal.

Then the posture, which chapter 3 explains:

```bash
mkdir -p ~/.config/lint-mine
cp -R ~/src/lint-mine/posture/. ~/.config/lint-mine/
```

And the alias, so `cargo mine` works in every repository:

```bash
lint-mine install-alias
```

```
added `cargo mine` to ~/.cargo/config.toml (cargo passes extra args through: cargo mine --staged)
```

The alias is one line, `mine = "lint-mine"`, under `[alias]` in `~/.cargo/config.toml`.
Other aliases in that file are left alone. From here on this handbook writes
`cargo mine`; `lint-mine` with the same arguments is the same thing.

Updating later is the same three steps from inside `~/src/lint-mine`: `git pull`,
`cargo build --release`, `install`.

## 3. The posture

The posture is a folder, `~/.config/lint-mine` unless you say otherwise, with three
files and one subfolder:

| File | What it is |
|---|---|
| `clippy-flags.txt` | The lint flags, one per line, as clippy takes them on the command line: `-D clippy::pedantic`, `-A clippy::module_name_repetitions`, `-W unreachable_pub`. Blank lines and `#` comments are ignored. Later lines win over earlier ones, so groups come first and single lints override them. |
| `clippy.toml` | Clippy's own configuration file (thresholds, test-code relaxations, `avoid-breaking-exported-api`). lint-mine points `CLIPPY_CONF_DIR` here, so this file is used instead of the repository's. |
| `rustfmt.toml` | The formatting rules for the rustfmt pass. Passed with `--config-path`, so again the repository's own file is not used. |
| `repos/` | One `<name>.flags` file per repository, appended to the flags when lint-mine runs inside that repository. Chapter 7. |

The repository ships a starting posture in `posture/`. It is strict: the `cargo`,
`nursery` and `pedantic` groups at deny, 200-odd named lints on top, `missing_docs`
and `unsafe_code` denied, test code relaxed in `clippy.toml` so `unwrap` and `panic`
are allowed in tests. The one rule to look at first is `-D clippy::float_arithmetic`.
It is right for a ledger and wrong for a simulation; chapter 7 shows the one-line way
to relax it per repository, and chapter 8 the one-line way to relax it everywhere.

The posture is yours. Edit the files freely; nobody else is affected, because nothing
in them reaches the repository.

Two ways to point lint-mine at a different posture folder, for trying one out or for
keeping several:

```bash
cargo mine --posture ~/postures/strict
LINT_MINE_POSTURE=~/postures/strict cargo mine
```

The flag wins over the variable. A folder without `clippy-flags.txt` is refused.

**On deny and warn.** Every `-D` and `-F` in your flags reaches clippy as `-W`. This is
deliberate and chapter 9 explains why; the short version is that clippy at `-D` turns a
finding in untouched legacy code into a compile error that stops the whole run. Deny is
lint-mine's decision, made per changed line: a finding on your line fails the run, a
finding elsewhere does not. So write `-D` for the rules you mean, `-A` for the ones
you do not, and leave the level to lint-mine.

## 4. Your first run

The examples use a workspace called `bakery` with three crates: `oven` (temperatures),
`proofer` (proofing times, depends on `oven`) and `rye` (a binary, depends on
`proofer`). Suppose you add a function to each of the two libraries and have not
committed yet:

```rust
// oven/src/lib.rs
pub fn preheat(target: u32) -> u32 {
    target + 10
}

// proofer/src/lib.rs
/// Hours instead of minutes.
pub fn hours(temp: f64) -> f64 {
    let m = minutes(temp);
    (m / 60.0) as f64 * 1 as f64
}
```

Then, anywhere inside the repository:

```bash
cargo mine
```

The first run in a repository compiles it once into lint-mine's own cache, which
takes as long as a `cargo clippy` of the crates you touched. After that it is quick.
The output:

```
lint-mine: 10 changed line(s) in 2 file(s), crate(s): oven, proofer
warning: this function could have a `#[must_use]` attribute
 --> oven/src/lib.rs:8:8
  |
8 | pub fn preheat(target: u32) -> u32 {
  |        ^^^^^^^
  |
  = help: for further information visit https://rust-lang.github.io/rust-clippy/main/index.html#must_use_candidate
...
warning: floating-point arithmetic detected
  --> proofer/src/lib.rs:12:5
   |
12 |     (m / 60.0) as f64 * 1 as f64
   |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^
   |
  = help: for further information visit https://rust-lang.github.io/rust-clippy/main/index.html#float_arithmetic
...

lint-mine: 7 finding(s) inside your diff, 0 formatting spot(s) (6 finding(s) elsewhere in these crates, not judged):
    1  clippy::float_arithmetic
    1  clippy::min_ident_chars
    1  clippy::missing_const_for_fn
    2  clippy::must_use_candidate
    1  clippy::unnecessary_cast
    1  missing_docs
```

Exit code 1: there are findings on your lines. The six findings "elsewhere" are what
the posture thinks of the code that was already there; they are counted so you know
the posture ran over the whole crate, and they never change the exit code.

Fix the lines, run again. When nothing is left:

```
lint-mine: 3 changed line(s) in 1 file(s), crate(s): oven
lint-mine: clean. Your 3 line(s) pass the strict posture (6 finding(s) elsewhere in these crates, not judged).
```

Exit code 0.

## 5. Reading the output

Every run prints the same parts in the same order.

**The header line.** `lint-mine: 10 changed line(s) in 2 file(s), crate(s): oven,
proofer`. The lines come from `git diff -U0`, so a changed line is one that git
shows as added or modified. Deleted lines have no line to judge. Only `.rs` files
count. The crates are the workspace packages whose folders contain the changed files.

**The overlay line.** `lint-mine: overlay ~/.config/lint-mine/repos/bakery.flags (2
flag(s))`, when this repository has one. Chapter 7.

**The findings.** Clippy's own rendering, unchanged, for each diagnostic whose primary
span starts on one of your lines. The same finding at the same place is printed once
even when clippy reports it for several targets (lib, tests, benches).

**Enclosing items.** A diagnostic whose span starts before your lines and merely
contains them: `too_many_lines` on a 200-line function you added one line to,
`missing_docs` on a module you touched. These are listed under their own heading,
`N pre-existing item(s) enclose your change (not yours, not blocking)`, with file,
line and lint, and they do not count as findings. You did not make the function long.

**Formatting spots.** `rustfmt: proofer/src/lib.rs:7 is not formatted to the posture
(rustfmt --config-path ~/.config/lint-mine/rustfmt.toml proofer/src/lib.rs)`. One
line per place where rustfmt, with the posture's `rustfmt.toml`, would change one of
your lines. The command in brackets formats the file; chapter 10 says why that is a
whole-file command.

**The summary.** Either

```
lint-mine: clean. Your 3 line(s) pass the strict posture (6 finding(s) elsewhere in these crates, not judged).
```

or

```
lint-mine: 7 finding(s) inside your diff, 1 formatting spot(s) (6 finding(s) elsewhere in these crates, not judged):
    1  clippy::float_arithmetic
    ...
```

followed by a count per lint. The number in brackets is omitted when it is zero.

**Exit codes.**

| Exit | Meaning |
|---|---|
| 0 | Your lines are clean. `clean` is printed only when every changed crate was actually linted. |
| 1 | Findings or formatting spots on your lines. Also: the hook refused a commit that touches the team's lint configuration (chapter 6). |
| 2 | No verdict is possible. The crate does not compile, clippy never reached one of your crates, another crate failed, a build script failed, or the posture folder is missing. The message says which. Chapter 11 lists them. |

The hook treats 1 and 2 alike: the commit does not go through.

## 6. Daily use and the hook

**What is compared.** Without arguments, lint-mine diffs the working tree (staged and
unstaged) against the merge-base with the branch you track: `@{upstream}` if the
branch has one, else `origin/HEAD`, `origin/main`, `origin/master`, in that order, and
finally `HEAD` when none exists. So on a feature branch you see everything the branch
adds, plus what you have not committed yet.

```bash
cargo mine                      # everything since you branched, plus the working tree
cargo mine --base origin/main   # against an explicit ref
cargo mine --base HEAD          # only what is not committed yet
cargo mine --staged             # only what is staged: what a commit would contain
cargo mine --no-fmt             # skip the rustfmt pass
```

**The pre-commit hook.** In each repository where you want it:

```bash
lint-mine install-hook
```

```
installed ~/src/bakery/.git/hooks/pre-commit
```

The hook is three lines and calls `lint-mine --staged` by its absolute path:

```sh
#!/bin/sh
# lint-mine: the strict posture, on the staged diff only (local hook, not committed)
exec "/Users/you/.local/bin/lint-mine" --staged
```

`.git/hooks` is not tracked, so the hook is yours alone. If a pre-commit hook already
exists, lint-mine refuses to overwrite it; `--force` replaces it. A commit with
findings on the staged lines stops with the same output as a run, and exit 1:

```
lint-mine: 3 finding(s) inside your diff, 0 formatting spot(s) (6 finding(s) elsewhere in these crates, not judged):
    1  clippy::missing_const_for_fn
    2  clippy::must_use_candidate
```

`git commit --no-verify` skips the hook when you must commit anyway.

**The guard against leaking scaffolding.** In hook mode a commit that touches
`clippy.toml`, `rustfmt.toml`, `rust-toolchain.toml`, `.rustfmt.toml`,
`.cargo/config.toml`, or adds a `[lints]` or `[workspace.lints]` table to a
`Cargo.toml`, is refused:

```
lint-mine: this commit touches the TEAM's lint/format/toolchain configuration (rustfmt.toml).
lint-mine: your posture belongs in ~/.config/lint-mine, not in the repo. Re-run with --allow-team-config if this is intended.
```

This exists because the easiest way to make a strict rule pass is to add it to the
repository, and that is exactly what lint-mine is for avoiding. When the change to the
team's configuration is genuinely your task, `lint-mine --staged --allow-team-config`
runs the check without the guard; for the hook, commit with `--no-verify` that once.

**Nothing to judge.** A run with no changed Rust lines prints `lint-mine: no changed
Rust lines; nothing to judge` and exits 0.

## 7. Per-repository overlays

A rule can be right in general and wrong for one repository. Floats are the usual
case: denied in a ledger, everyday in a simulation. The overlay is a file of extra
flags that applies in one repository only:

```bash
cargo mine relax floats
```

```
relaxed 1 lint(s) in ~/.config/lint-mine/repos/bakery.flags
  -A clippy::float_arithmetic
```

From now on, inside `bakery`, every run prints `lint-mine: overlay
~/.config/lint-mine/repos/bakery.flags (1 flag(s))` and the float findings are gone.
The opposite is `cargo mine tighten floats`, which writes `-D` and removes the earlier
`-A` line for the same lint, so the file stays readable. `cargo mine overlay` shows the
file and the presets:

```
overlay ~/.config/lint-mine/repos/bakery.flags:
  -A clippy::float_arithmetic
presets: floats (1), float-casts (4), docs (4), size (3), indexing (1), prints (3), std (2)
```

`relax` and `tighten` take lint names or presets, several at once. A name without
`::` gets `clippy::` in front (`too_many_lines` is `clippy::too_many_lines`) unless it
is one of rustc's own lints that people name often (`missing_docs`, `unsafe_code`,
`unreachable_pub`, `unused_results`, `unused_qualifications`, `trivial_casts`,
`trivial_numeric_casts`, `unused_import_braces`, `unused_lifetimes`,
`variant_size_differences`, `non_ascii_idents`) or starts with `rustdoc`.

| Preset | Lints |
|---|---|
| `floats` | `clippy::float_arithmetic` |
| `float-casts` | `clippy::cast_precision_loss`, `clippy::cast_possible_truncation`, `clippy::cast_sign_loss`, `clippy::cast_lossless` |
| `docs` | `missing_docs`, `clippy::missing_docs_in_private_items`, `clippy::missing_errors_doc`, `clippy::missing_panics_doc` |
| `size` | `clippy::too_many_lines`, `clippy::too_many_arguments`, `clippy::cognitive_complexity` |
| `indexing` | `clippy::indexing_slicing` |
| `prints` | `clippy::print_stdout`, `clippy::print_stderr`, `clippy::use_debug` |
| `std` | `clippy::std_instead_of_core`, `clippy::std_instead_of_alloc` |

`--global` writes the same lines to `clippy-flags.txt` instead, for everywhere.

**Which name.** The overlay file is named after the repository, and the repository's
name is the origin remote's: `git@host:org/sourdough.git`, `https://host/org/sourdough`
and `/srv/sourdough.git` all mean `sourdough.flags`. The main checkout and every
worktree of it share that one file, which is the point: a worktree folder called
`sourdough-4711` still gets `sourdough`'s settings. Without an origin remote the
checkout's folder name is the key. A file under the folder name is still found when
no file under the remote's name exists, so overlays made before this rule keep
working; when both exist, the remote's wins. `cargo mine overlay` always says which
file is in effect.

You can also write the file by hand. Same syntax as `clippy-flags.txt`, later lines
win, and the overlay is appended after the global flags, so it wins over them.

## 8. Legacy code and rules that are wrong

Three different situations, three different answers.

**The rule is wrong for you, everywhere.** Change its line in `clippy-flags.txt` from
`-D` to `-A`, or `cargo mine relax <lint> --global`. The posture is yours and nobody
else sees it.

**The rule is wrong for this repository.** `cargo mine relax <lint or preset>`,
chapter 7.

**The rule is right, and the code around your change breaks it.** You added a line to
a function that is already too long, or wrote a float expression in a crate full of
them. Two honest options:

- Fix the surrounding code when the fix is small and safe. The findings "elsewhere"
  count in the summary tells you how much of that there is.
- Put `#[expect(clippy::too_many_lines, reason = "...")]` on the item. `expect`, not
  `allow`: it is visible in review, it carries a reason, and the compiler reports it
  as unfulfilled the day someone fixes the code, so it cannot outlive its excuse.

The one thing not to do is a bare `#[allow]` or a change to the team's `clippy.toml`
to make the number go to zero. The guard in chapter 6 catches the second.

**Enclosing items** (chapter 5) need nothing from you. They are listed so you see
them, and they do not block.

## 9. Workspaces, crates and build scripts

**Which crates run.** lint-mine maps each changed file to the workspace package whose
folder contains it (the longest matching folder wins) and runs clippy for those
packages only, with `-p <name>` for each and `--all-targets`, so tests, examples and
benches are linted too. Changed files outside every package are an error: `changed
files belong to no workspace package`.

**Dependencies are built, not linted.** Clippy runs with `--no-deps`. The workspace
crates your crates depend on are compiled as usual, but the posture is not applied to
them. Without `--no-deps` the strict flags would hit every dependency, one of them
would fail first, and cargo would stop before reaching your crate at all. That was
lint-mine 0.1.0's bug: it then found no diagnostics on your lines and said `clean`.

**Why the posture runs at warn level.** Even with `--no-deps`, a crate you did change
usually has legacy findings. At `-D` each of those is a compile error: cargo stops at
the first failing crate, your other changed crates are never linted, and the run has
no verdict. So lint-mine rewrites every `-D x` and `-F x` to `-W x` before clippy sees
them (`-A` stays). Every changed crate then gets linted to the end, and lint-mine
applies deny itself, per changed line. That was 0.2.0's bug and 0.2.1's fix.

**The trust check.** Before saying anything about your lines, lint-mine checks that
clippy actually reached every changed crate (an artifact or a diagnostic from a real
target of it; a build script does not count) and that no crate failed. If a changed
crate was never reached, or a crate you did not change reported errors, or clippy
failed without a single lint diagnostic, the run exits 2 with the reason and never
says `clean`. Chapter 11 has the messages.

**Compile errors.** rustc's own errors (no lint code, or an `E0xxx` code) are not
findings. They are printed and the run exits 2: `the crate does not compile; fix that
first`.

**Build scripts and the project's environment.** Some repositories have build scripts
that need the project's environment: a virtualenv, a `direnv` setup, a tool on the
`PATH`. Without it the build script fails before clippy sees any Rust. lint-mine
recognises that case and says so:

```
lint-mine: a build script failed before clippy could lint anything; this is the project's build
lint-mine: environment, not your code. Run it the way the project builds, e.g. `direnv exec . cargo mine`
lint-mine: or after sourcing the project's env setup, then try again.
```

The hook inherits the environment of the shell that runs `git commit`, so a commit
from an editor or a GUI client can hit this when the same commit from a project shell
does not.

**Per-crate lints.** Lints that fire on the crate as a whole (the `clippy::cargo`
group, `missing_docs` on the crate root) have a span at the top of the crate. They
show up as enclosing items at most, never as findings.

## 10. Caches, toolchains and speed

**The cache.** Build artifacts go to `~/.cache/lint-mine/<folder name>-<hash>/target`,
one per repository path, never into the repository's own `target/`. The team's build
cache is not invalidated by lint-mine's flags, and lint-mine's is not invalidated by
theirs. The first run per repository is a full compile of the changed crates and their
dependencies; later runs recompile only what changed. A change to the posture's flags
recompiles the linted crates once.

`lint-mine reset` deletes a repository's cache (chapter 12). The cache can be large;
it is safe to delete at any time.

**The toolchain.** Whatever `cargo` resolves to in the repository runs: a
`rust-toolchain.toml` is honoured, as it would be for `cargo clippy`. A lint the
toolchain does not know is ignored (the posture starts with `-A unknown_lints` and
`-A renamed_and_removed_lints`), so one posture works across toolchain versions, at
the price that a renamed lint silently stops applying until you rename it in the
posture.

**rustfmt.** The posture's `rustfmt.toml` uses nightly-only options
(`unstable_features = true`, comment wrapping, import grouping). On a stable rustfmt
those options are reported as warnings and ignored; the rest applies. rustfmt's
`--file-lines` is nightly-only and lint-mine does not use it, so formatting is judged
by a whole-file check whose results are filtered to your lines: the spot is yours, the
suggested command formats the whole file. Format your hunks by hand, or format the
file once if the team accepts that. When rustfmt is not installed, the pass is skipped
with `lint-mine: rustfmt not found; skipping format check`.

**Speed.** A run is a `cargo clippy` of your changed crates in a warm cache plus a
`rustfmt --check` per changed file. On a workspace with heavy dependencies the first
run is minutes; from then on seconds, and the hook is bearable.

## 11. When something looks wrong

Each message, what it means, what to do.

| Message | Meaning | What to do |
|---|---|---|
| `clippy did not lint <crate>: cargo clippy exited with exit status: 101; not a clean result` | Clippy stopped before reaching a changed crate, with no lint diagnostic to explain it. The last lines of clippy's stderr are printed above. | Read the stderr lines. Usually a compile error in a dependency, or a build script (next row). |
| `a build script failed before clippy could lint anything` | A `build.rs` failed; the project needs its environment. | Run inside the project's environment: `direnv exec . cargo mine`, or source the env setup first. |
| `clippy did not lint <crate>: <other> failed (N diagnostic(s)); not a clean result` | A crate you did not change reported errors, and yours was never reached. | That crate does not compile on this toolchain or with this environment. Fix the build, or run after the team's `cargo build` passes. |
| `clippy reported errors outside your crates: <crate> failed (N diagnostic(s))` | Your crates were reached, but another crate errored. | Same as above; the verdict is withheld because the build is not whole. |
| `the crate does not compile; fix that first` | rustc errors in a changed crate. They are printed above. | Fix them. |
| `changed files belong to no workspace package` | A changed `.rs` file is outside every package folder (a script, a file under `target/`). | Nothing to do unless it should be in a package. |
| `not inside a git repository` | lint-mine was run outside a checkout. | `cd` into one. |
| `posture directory ~/.config/lint-mine has no clippy-flags.txt` | The posture is missing. | Chapter 2: copy `posture/` from the source, or point `--posture` at yours. |
| `this commit touches the TEAM's lint/format/toolchain configuration` | The guard of chapter 6. | Keep your posture local, or `--allow-team-config` / `--no-verify` when the change is the task. |
| `rustfmt not found; skipping format check` | No rustfmt on the `PATH`. | `rustup component add rustfmt`. Not an error. |
| Findings on lines you did not touch | The diff base is not what you think: a stale `origin/main`, a branch without upstream. | `cargo mine --base <ref>` with the ref you mean; `git fetch` first. |
| The same finding appears after you fixed it | The cache is from before the edit, or the editor has not saved. | Save; run again. `lint-mine reset` if in doubt. |
| `clean` with a suspiciously small count of findings elsewhere | The count is for the changed crates only, and only for lines the diff does not cover. Dependencies are not counted. | Nothing; that is the design. |

Always include `lint-mine --version` and the full output in a bug report.

## 12. Undo

Nothing lint-mine does reaches the repository, so undo is about your own machine.

**One repository.**

```bash
lint-mine reset
```

```
removed ~/src/bakery/.git/hooks/pre-commit
removed ~/.cache/lint-mine/bakery-0946637886d7cdfe
reset done: this repository carries nothing of lint-mine
```

It removes the pre-commit hook only if it is lint-mine's own (a hook that does not
mention lint-mine is left in place with a message), and the repository's build cache.
`lint-mine uninstall-hook` removes just the hook.

**Everything.**

```bash
lint-mine uninstall
```

Removes `~/.cache/lint-mine`, `~/.config/lint-mine`, the `mine = "lint-mine"` line
from `~/.cargo/config.toml` (other aliases stay; an empty `[alias]` table is removed
too), and `~/.local/bin/lint-mine`. Run `lint-mine uninstall-hook` in each repository
first, or delete `.git/hooks/pre-commit` by hand afterwards; a leftover hook only
points at a binary that is gone, and `git commit` tells you so.

The source in `~/src/lint-mine` is a normal clone; delete it when you like.

## 13. Command reference

**Running.**

| Command | What it does |
|---|---|
| `lint-mine` | Judge the working tree against the merge-base with the tracked branch (chapter 6). |
| `lint-mine --base <ref>` | Judge against an explicit ref. `--base HEAD` is "what I have not committed". |
| `lint-mine --staged` | Judge the staged diff only. What the hook runs. |
| `lint-mine --no-fmt` | Skip the rustfmt pass. |
| `lint-mine --posture <dir>` | Use this posture folder. Default `$LINT_MINE_POSTURE`, else `~/.config/lint-mine`. |
| `lint-mine --allow-team-config` | In `--staged` mode, do not refuse a commit that touches the team's lint/format/toolchain files. |
| `lint-mine --version`, `-V` | Print `lint-mine <version>`. |
| `lint-mine --help`, `-h` | The one-screen summary. |

**Setting up and undoing.**

| Command | What it does |
|---|---|
| `lint-mine install-alias` | Add `mine = "lint-mine"` to `~/.cargo/config.toml`. |
| `lint-mine install-hook [--force]` | Write `.git/hooks/pre-commit` in this repository. `--force` replaces an existing hook. |
| `lint-mine uninstall-hook` | Remove the hook, if it is lint-mine's. |
| `lint-mine reset` | This repository: the hook and the build cache. |
| `lint-mine uninstall` | Everything on this machine. |

**Overlays.**

| Command | What it does |
|---|---|
| `lint-mine overlay` | Show this repository's overlay file and the presets. |
| `lint-mine relax <lint or preset>... [--global]` | Append `-A` lines. `--global`: to `clippy-flags.txt` instead of the overlay. |
| `lint-mine tighten <lint or preset>... [--global]` | Append `-D` lines, removing earlier `-A` lines for the same lints. |

**Environment.**

| Variable | Effect |
|---|---|
| `LINT_MINE_POSTURE` | The posture folder, when `--posture` is not given. |
| `HOME` | Where `.config/lint-mine`, `.cache/lint-mine`, `.cargo/config.toml` and `.local/bin` are. |
| `CLIPPY_CONF_DIR`, `CARGO_TARGET_DIR` | Set by lint-mine for the clippy run (to the posture and the cache). Do not set them yourself. |

**Files on your machine.**

| Path | What it is |
|---|---|
| `~/.local/bin/lint-mine` | The binary. |
| `~/.config/lint-mine/clippy-flags.txt` | The flags. |
| `~/.config/lint-mine/clippy.toml` | Clippy's configuration for the run. |
| `~/.config/lint-mine/rustfmt.toml` | rustfmt's configuration for the run. |
| `~/.config/lint-mine/repos/<name>.flags` | Per-repository overlays. |
| `~/.cache/lint-mine/<folder>-<hash>/target` | Per-repository build caches. |
| `~/.cargo/config.toml` | Holds the `cargo mine` alias. |
| `<repo>/.git/hooks/pre-commit` | The hook, where installed. Untracked. |

**Exit codes.** 0 clean, 1 findings on your lines, 2 no verdict (chapter 5).

**What clippy is run with.** `cargo clippy --all-targets --message-format=json --quiet
-p <crate>... --no-deps -- <flags at warn level>`, in the repository root, with
`CLIPPY_CONF_DIR` set to the posture and `CARGO_TARGET_DIR` to the cache. rustfmt:
`rustfmt --check --edition <crate edition> --config-path <posture>/rustfmt.toml
<file>` per changed file.

## 14. Working on lint-mine itself

The source is one file, `src/main.rs`, with the end-to-end tests in
`tests/regression.rs`. Build and test with

```bash
cargo build --release
cargo test --release
```

The tests build throwaway two-crate workspaces (`dep` and `app`) under the system
temp folder with a one-lint posture, commit a base, change `app` or `dep`, and run the
real binary against them with `HOME` pointed into the fixture so your own cache and
posture are never touched. They take a few seconds. Each test is named for the
behaviour it protects; the two bugs of 0.1.0 and 0.2.0 each have one that fails
without the fix.

Clippy on lint-mine itself is clean (`cargo clippy --release --all-targets`), and the
code is meant to stay readable from top to bottom: the diff, the packages, the posture,
the clippy run and its trust check, the rustfmt pass, the overlays, the commands, `main`.

**Adding a preset.** `PRESETS` in `src/main.rs` is a list of `(name, lints)`; add a
row and mention it in chapter 7's table.

**This handbook.** `docs/HANDBOOK.md` is the source. `docs/render.sh` renders it to
`docs/handbook.html` with `tools/md2html`, a small Markdown renderer in this repository
(standard library only; it builds with the same toolchain). The page is one
self-contained file with a table of contents, search, a theme switch and the live
glossary; open it in any browser. Edit the Markdown, run the script, commit both.

```bash
docs/render.sh
```

## 15. Glossary

| Term | Meaning |
|---|---|
| Posture | Your lint and format rules: `clippy-flags.txt`, `clippy.toml`, `rustfmt.toml` and the `repos/` overlays, in `~/.config/lint-mine`. Yours alone; never in a repository. |
| Flags | The lines of `clippy-flags.txt`: `-D`, `-W`, `-A` or `-F` followed by a lint name, exactly as clippy takes them after `--`. |
| Overlay | A per-repository flags file, `repos/<name>.flags`, appended to the global flags when lint-mine runs in that repository. |
| Preset | A word that stands for a few lints in `relax` and `tighten`: `floats`, `docs`, `size`, `float-casts`, `indexing`, `prints`, `std`. |
| Hunk | A range of added or changed lines in one file, as `git diff -U0` reports it. Your lines are the union of your hunks. |
| Finding | A clippy diagnostic whose primary span starts inside one of your hunks. Findings fail the run. |
| Enclosing item | A diagnostic whose span starts before your hunk and contains it: a long function you added a line to. Listed, never blocking. |
| Elsewhere | Diagnostics in the changed crates that are not in your hunks. Counted in the summary, never judged. |
| Formatting spot | A line of yours that rustfmt, with the posture's `rustfmt.toml`, would change. |
| Changed crate | A workspace package whose folder contains a changed file. These are the crates clippy lints. |
| Trust check | The check that every changed crate was actually reached by clippy and no crate failed, before any verdict is given. Fails with exit 2. |
| Warn level | The level the posture reaches clippy at: `-D` and `-F` become `-W`, so legacy findings never stop the run. Deny is then lint-mine's per-line decision. |
| Base | The commit the working tree is diffed against: the merge-base with the tracked branch by default, or what `--base` names. |
| Staged | The part of your changes added with `git add`; what a commit would contain, and what the hook judges. |
| Hook | The `.git/hooks/pre-commit` script that runs `lint-mine --staged` before each commit. Untracked, local, optional. |
| Alias | The `mine = "lint-mine"` line in `~/.cargo/config.toml` that makes `cargo mine` run lint-mine. |
| Cache | lint-mine's build directory for one repository, `~/.cache/lint-mine/<folder>-<hash>/target`, separate from the repository's `target/`. |
| Team configuration | The repository's own `Cargo.toml` lint tables, `clippy.toml`, `rustfmt.toml`, `rust-toolchain.toml` and `.cargo/config.toml`. lint-mine never changes them and refuses to commit changes to them from the hook. |
| Expect | `#[expect(lint, reason = "...")]`: the attribute to use when a rule is right and the surrounding code is not. Visible, reasoned, and reported when it stops being needed. |
| Build script | A crate's `build.rs`, run by cargo before compiling the crate. One that needs the project's environment fails outside it, before clippy runs. |
| Verdict | The outcome of a run: clean (exit 0), findings (exit 1), or none possible (exit 2). |
