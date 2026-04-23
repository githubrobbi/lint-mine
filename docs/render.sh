#!/bin/sh
# Render docs/HANDBOOK.md to docs/handbook.html with tools/md2html (built on first use).
set -eu
here="$(cd "$(dirname "$0")" && pwd)"
tool="${here}/../tools/md2html"
( cd "${tool}" && cargo build --release --quiet )
"${tool}/target/release/md2html" "${here}/HANDBOOK.md" "${here}/handbook.html"
