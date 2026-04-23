# lint-mine

The strictest Rust posture you own, applied to nothing but the lines you changed.

For the contributor who works in a shared Rust codebase and wants `clippy::pedantic`,
`clippy::nursery` and a few hundred named lints on their own code without asking the team
to adopt any of it: lint-mine runs clippy and rustfmt with your flags, kept outside the
repository, keeps the diagnostics that land on lines in your diff, and drops the rest.
Nothing of it ever reaches the repository: no attributes, no config files, no hook in the
tree, no build artifacts. Upstream sees only the result.

**The handbook** is the complete guide, from install to every command and message:
[docs/HANDBOOK.md](docs/HANDBOOK.md), or the same as one self-contained page with a
table of contents, search and a live glossary, [docs/handbook.html](docs/handbook.html)
(download it and open it in a browser).

## Install

A Rust toolchain with clippy and rustfmt, and git. No admin rights.

```bash
git clone https://github.com/githubrobbi/lint-mine ~/src/lint-mine
cd ~/src/lint-mine
cargo build --release
install -m 0755 target/release/lint-mine ~/.local/bin/lint-mine   # ~/.local/bin on your PATH
mkdir -p ~/.config/lint-mine && cp -R posture/. ~/.config/lint-mine/   # the starting posture; edit freely
lint-mine install-alias                                             # `cargo mine` everywhere
```

## Use

```bash
cd <repo>
cargo mine                    # judge your changes since you branched, plus the working tree
cargo mine --staged           # what a commit would contain
lint-mine install-hook        # .git/hooks/pre-commit runs that before each commit (untracked)
cargo mine relax floats       # this repo allows float arithmetic; `tighten` reverses it
```

Exit 0 when your lines are clean, 1 when there are findings on them, 2 when no verdict
is possible (the crate does not compile, clippy never reached a changed crate, a build
script needs the project's environment). `clean` is printed only when every changed crate
was actually linted:

```
lint-mine: clean. Your 34 line(s) pass the strict posture (1,004 finding(s) elsewhere in these crates, not judged).
```

## How it works

1. `git diff -U0` against the merge-base with the tracked branch (or `--staged`, or
   `--base <ref>`) gives the lines you added or changed, per file.
2. `cargo clippy --all-targets --message-format=json -p <the crates you touched> --no-deps`
   runs with the posture's flags and `CLIPPY_CONF_DIR` pointing at the posture's
   `clippy.toml`; artifacts go to `~/.cache/lint-mine/<repo>-<hash>/target`. Dependencies
   are built, not linted. The flags reach clippy at **warn** level (`-D`/`-F` rewritten to
   `-W`); deny is lint-mine's own decision, made per line in step 3, so legacy findings in a
   changed crate never fail the run.
3. A diagnostic is **yours** when its primary span starts inside your hunks; **enclosing**
   when the span starts before your hunk and merely contains it (listed, never blocking);
   **elsewhere** otherwise (counted, never judged).
4. `rustfmt --check --config-path <posture>/rustfmt.toml` runs on each changed file; only
   differences on your lines count.
5. Before any verdict, lint-mine checks that clippy actually reached every changed crate and
   that no crate failed; otherwise it exits 2 and says which crate it could not lint and why.

## The posture

`~/.config/lint-mine/{clippy-flags.txt, clippy.toml, rustfmt.toml}` plus
`repos/<repo name>.flags` overlays, keyed by the origin remote's repository name so every
worktree shares them. `cargo mine overlay` shows what is in effect; `relax` and `tighten`
take lint names or the presets `floats`, `float-casts`, `docs`, `size`, `indexing`,
`prints`, `std`; `--global` edits the global flags instead. The repository ships a strict
starting posture in `posture/`; `-D clippy::float_arithmetic` is the line to look at first.

## Undo

`lint-mine reset` (this repo: hook and cache), `lint-mine uninstall` (everything on this
machine). The repository is untouched because nothing was ever written into it.

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
- **0.1.0** (2026-04-10): first cut. Printed `clean` when a workspace dependency failed the
  posture first and the changed crate was never linted.

## Repository

| Path | What it is |
|---|---|
| `src/main.rs` | lint-mine, one file |
| `tests/regression.rs` | end-to-end tests on throwaway workspaces (`cargo test --release`) |
| `posture/` | the starting posture |
| `docs/HANDBOOK.md`, `docs/handbook.html` | the handbook, source and rendered page (`docs/render.sh`) |
| `tools/md2html/` | the Markdown renderer the page is made with; standard library only |

MIT license.
