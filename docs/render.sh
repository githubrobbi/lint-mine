#!/bin/sh
# Render docs/HANDBOOK.md to docs/handbook.html with md2html, which is a
# repository of its own: https://github.com/githubrobbi/md2html
#
# The renderer is looked for, in this order:
#   1. $MD2HTML, a path to the binary;
#   2. a checkout beside this repository (../md2html), built on first use;
#   3. `md2html` on the PATH.
set -eu
here="$(cd "$(dirname "$0")" && pwd)"
beside="${here}/../../md2html"
if [ -n "${MD2HTML:-}" ]; then
    tool="${MD2HTML}"
elif [ -f "${beside}/Cargo.toml" ]; then
    ( cd "${beside}" && cargo build --release --quiet )
    tool="${beside}/target/release/md2html"
elif command -v md2html >/dev/null 2>&1; then
    tool="md2html"
else
    echo "render.sh: md2html not found." >&2
    echo "  Clone https://github.com/githubrobbi/md2html beside this repository," >&2
    echo "  or set MD2HTML to the binary." >&2
    exit 1
fi
"${tool}" "${here}/HANDBOOK.md" "${here}/handbook.html"
