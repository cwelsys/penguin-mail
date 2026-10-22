#!/usr/bin/env bash
# The gate on Arch under a tiling compositor. Pass --ui after touching the UI.
#
# a11y-names.sh looks for the accessibility registry where Ubuntu keeps it,
# so the name check runs from a copy that looks in /usr/lib.
#
# The Docker suites call plain `docker`, which follows the active context.
# Pinning DOCKER_CONTEXT keeps them on this computer when another context,
# such as a remote server, is selected.
set -euo pipefail

cd "$(dirname "$0")/.."

unset DOCKER_HOST
export DOCKER_CONTEXT=default

GDK_BACKEND=x11 xvfb-run -a cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
if [ "${1:-}" = --ui ]; then
    copy=scripts/a11y-names.arch.sh
    trap 'rm -f "$copy"' EXIT
    sed 's#/usr/libexec/#/usr/lib/#' scripts/a11y-names.sh >"$copy"
    bash "$copy"
fi
