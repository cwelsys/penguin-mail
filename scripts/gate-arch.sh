#!/usr/bin/env bash
# The gate on Arch under a tiling compositor. Pass --ui after touching the UI.
# Pass --quick while iterating: it tests and lints only the crates with
# uncommitted changes and leaves out the Docker suites. Run the full gate
# before a commit.
#
# a11y-names.sh looks for the accessibility registry where Ubuntu keeps it,
# so the name check runs from a copy that looks in /usr/lib.
#
# The Docker suites call plain `docker`, which follows the active context.
# Pinning DOCKER_CONTEXT keeps them on this computer when another context,
# such as a remote server, is selected.
set -euo pipefail

cd "$(dirname "$0")/.."

quick=false
ui=false
for arg in "$@"; do
    case $arg in
    --quick) quick=true ;;
    --ui) ui=true ;;
    *)
        echo "usage: $0 [--quick] [--ui]" >&2
        exit 2
        ;;
    esac
done

unset DOCKER_HOST
export DOCKER_CONTEXT=default
run=(nice -n 19 ionice -c2 -n7)

scope=(--workspace)
if $quick; then
    export DOCKER_HOST=unix:///nonexistent
    unset DOCKER_CONTEXT
    scope=()
    while read -r dir; do
        case $dir in
        Cargo.toml | Cargo.lock | vendor | .cargo)
            scope=(--workspace)
            break
            ;;
        esac
        if [ -f "$dir/Cargo.toml" ]; then
            scope+=(-p "$(sed -n 's/^name = "\(.*\)"/\1/p' "$dir/Cargo.toml" | head -1)")
        fi
    done < <( (git diff --name-only HEAD; git ls-files -o --exclude-standard) | cut -d/ -f1 | sort -u)
    if [ ${#scope[@]} -eq 0 ]; then
        echo "gate --quick: no uncommitted changes to a crate"
        exit 0
    fi
    echo "gate --quick: ${scope[*]}"
fi

GDK_BACKEND=x11 "${run[@]}" xvfb-run -a cargo test "${scope[@]}"
"${run[@]}" cargo clippy "${scope[@]}" --all-targets -- -D warnings
if $ui; then
    copy=scripts/a11y-names.arch.sh
    trap 'rm -f "$copy"' EXIT
    sed 's#/usr/libexec/#/usr/lib/#' scripts/a11y-names.sh >"$copy"
    "${run[@]}" bash "$copy"
fi
