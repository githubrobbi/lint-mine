# Per-repository overlays

Drop `<repo directory name>.flags` here, e.g. `sourdough.flags`, with clippy flags that
apply only when lint-mine runs inside that repository. Later flags win, so an overlay can
relax (`-A clippy::float_arithmetic`) or tighten any rule without touching the global posture
or the repository.
