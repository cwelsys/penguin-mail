#!/usr/bin/env bash
# The gate on Arch under a tiling compositor. Pass --ui after touching the UI.
#
# a11y-names.sh looks for the accessibility registry where Ubuntu keeps it,
# so the name check runs from a copy that looks in /usr/lib.
set -euo pipefail

cd "$(dirname "$0")/.."

GDK_BACKEND=x11 xvfb-run -a cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
if [ "${1:-}" = --ui ]; then
    copy=scripts/a11y-names.arch.sh
    trap 'rm -f "$copy"' EXIT
    sed 's#/usr/libexec/#/usr/lib/#' scripts/a11y-names.sh >"$copy"
    bash "$copy"
fi
