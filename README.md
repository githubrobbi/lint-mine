# lint-mine

The strictest Rust posture you own, applied to nothing but the lines you changed.
For the contributor who works in a shared Rust codebase and wants `clippy::pedantic`,
`clippy::nursery` and a few hundred named lints on their own code without asking the team
to adopt any of it: lint-mine runs clippy with your flags, keeps the diagnostics that
land on lines in your diff, and drops the rest.
Nothing of it ever reaches the repository: no attributes, no config files, no hook in
the tree, no build artifacts. Upstream sees only the result.

## How it works

1. `git diff -U0` (against the merge-base with upstream, or `--staged`) gives the lines
   you added or changed, per file.
2. `cargo clippy --all-targets --message-format=json -p <the crates you touched> --no-deps`
   runs with the posture's flags (`-D clippy::pedantic`, `-D clippy::nursery`, 200+ named
   lints, `-D missing_docs`, ...) and `CLIPPY_CONF_DIR` pointing at the posture's
   `clippy.toml`. The team's `Cargo.toml`, `clippy.toml` and `rustfmt.toml` stay as they are.
   Artifacts go to `~/.cache/lint-mine/<repo>-<hash>/target`, never into the repo.
   `--no-deps` keeps the posture on your crates: the workspace crates they depend on are
   built, not linted. Without it the flags hit every dependency, one of those fails first,
   and cargo stops before it ever reaches your crate.
   The flags reach clippy at **warn** level: every `-D x` / `-F x` in the posture is
   passed as `-W x` (`-A` stays). Deny is lint-mine's decision, made per line in step 3.
   Passed as `-D`, a single legacy finding in the crate is a compile error, cargo stops,
   and the diff filter never gets its input; the posture's scope would be the whole
   crate, not your lines.
3. A diagnostic is **yours** when its primary span starts inside your hunks. It is
   **enclosing** when the span starts before your hunk and merely contains it (a
   crate-level `missing_docs`, a `too_many_lines` on a function you added one line to):
   listed, never blocking.
4. `rustfmt --check --config-path <posture>/rustfmt.toml` runs on each changed file;
   only differences on your lines count.
5. Exit 0 when your lines are clean, 1 when not, 2 when no verdict is possible: the crate
   does not compile, clippy never reached a changed crate, or another crate failed. `clean`
   is printed only when every changed crate was actually linted; otherwise lint-mine says
   which crate it could not lint and why, for example
   `lint-mine: clippy did not lint brioche: proofer_macros failed (541 diagnostic(s)); not a clean result`.
   The summary names what was left unjudged:
   `lint-mine: clean. Your 34 line(s) pass the strict posture (1,004 finding(s) elsewhere in these crates, not judged).`
   A build script that fails (a project that needs `direnv exec .` or its env setup
   sourced) is reported as the environment, not as your code.

## Setup on a machine

- `~/.local/bin/lint-mine`
- `~/.config/lint-mine/{clippy-flags.txt, clippy.toml, rustfmt.toml}`: the posture,
  generated from `rust-forge-template` (`Cargo.toml` lint tables flattened to CLI
  flags). Edit it freely; it is yours.
- `lint-mine install-alias` adds `mine = "lint-mine"` to `~/.cargo/config.toml`,
  so `cargo mine` works in every repo.

## Per repository, once you know which repos

```
cd <repo>
lint-mine install-hook        # .git/hooks/pre-commit -> lint-mine --staged (untracked)
cargo mine --base origin/main --no-fmt   # first run warms the cache; see where you stand
```

Daily: write code, `cargo mine`, fix what it shows on your lines, commit (the hook
re-checks the staged diff), push. Reviewers see clean code and none of the machinery.

## Per-repository overlay

`<posture>/repos/<repo name>.flags` is appended to the global flags when lint-mine runs in
that repository; later flags win. The repo name is the origin remote's repository name
(`git@host:org/sourdough.git` is `sourdough`), so the main checkout and every worktree
of it share one overlay. Without a remote, the checkout's folder name is the key; an overlay
file under the folder name is still found when no file under the remote's name exists. This is how a domain decision (floats
allowed in a simulation crate, denied in a ledger crate) stays local and per repo, with the
global posture untouched and nothing in the repository.

Automated: inside the repo, `cargo mine relax floats` writes the overlay line for you,
`cargo mine tighten floats` reverses it, `cargo mine overlay` shows the file. Arguments are
lint names (`too_many_lines`, `missing_docs`, `clippy::indexing_slicing`) or presets:
`floats`, `float-casts`, `docs`, `size`, `indexing`, `prints`, `std`. `--global` edits the
global posture instead of the overlay.

## When a strict rule is wrong for the team's code

- A rule you do not want at all: change its line in `clippy-flags.txt` (`-D` to `-A`).
  Posture is local; nobody else is affected.
- A rule that is right but the legacy around your change violates it: fix it in
  place when small and safe, otherwise `#[expect(clippy::..., reason = "...")]` on the
  item, which is honest, visible and expires when someone fixes it. Never a bare `allow`.
- `float_arithmetic` is denied in the forge posture because the forge is a trading
  codebase; for general work switch it to `-A` or you will fight it every day.

## Undo, at any level

- One repo: `lint-mine reset` removes the pre-commit hook (only if it is lint-mine's own)
  and that repo's build cache under `~/.cache/lint-mine`. The repo is untouched because
  nothing was ever written into it.
- Everything: `lint-mine uninstall` removes `~/.cache/lint-mine`, `~/.config/lint-mine`,
  the `cargo mine` alias line (other aliases stay) and `~/.local/bin/lint-mine` itself.
  Run `lint-mine uninstall-hook` in each repo first, or delete `.git/hooks/pre-commit`
  by hand afterwards.

## Guard against leaking scaffolding

In hook mode (`--staged`) a commit that touches `clippy.toml`, `rustfmt.toml`,
`rust-toolchain.toml`, `.cargo/config.toml`, or adds `[lints]` tables to a `Cargo.toml`
is refused with a message. `--allow-team-config` overrides it when such a change is
genuinely part of your work.

## Limits

- Lints that are per crate (the `clippy::cargo` group) show up only as enclosing items.
- rustfmt's `--file-lines` is nightly-only, so formatting is judged by whole-file
  check and filtered to your lines; the fix is still `rustfmt` on your hunks by hand,
  or a one-time whole-file format if the team accepts it.
- The repo's pinned toolchain is what runs. A lint unknown to that toolchain is
  silently ignored (`-A unknown_lints`).

## Versions

`lint-mine --version` names the build.

- **0.2.1** (2026-04-22): the posture reaches clippy at warn level (`-D`/`-F` rewritten to
  `-W`); deny is applied per changed line by lint-mine itself, so legacy findings in a
  changed crate never fail the run (as `-D` they were compile errors: the first legacy crate
  stopped cargo and the other changed crates were "not linted", exit 2). The summary counts
  the findings left unjudged. A failing build script is named as the project's environment.
- **0.2.0** (2026-04-21): `--no-deps`, so strict flags lint only the changed crates; a run
  that never reached a changed crate, or failed elsewhere, exits 2 and is never `clean`;
  `E0xxx` compiler errors count as compile errors, not findings; per-repo overlays keyed by
  the origin remote's name; `--version`. Regression tests in `tests/regression.rs`
  (`cargo test --release`).
- **0.1.0** (2026-04-10): first cut. Printed `clean` when a workspace
  dependency failed the posture first and the changed crate was never linted.
