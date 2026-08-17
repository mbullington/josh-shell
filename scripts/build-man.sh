#!/usr/bin/env bash
# Regenerate the committed man pages from docs/man/*.scd with scdoc.
#
# scdoc is intentionally not vendored; install it via nix (scdoc), brew, or
# your package manager. The generated roff output is committed so readers
# (and `man josh`) never need the tool.
set -euo pipefail
cd "$(dirname "$0")/.."

if ! command -v scdoc >/dev/null 2>&1; then
    for candidate in /nix/store/*scdoc-1.1*/bin/scdoc; do
        if [ -x "$candidate" ]; then
            PATH="$(dirname "$candidate"):$PATH"
            break
        fi
    done
fi
command -v scdoc >/dev/null 2>&1 || {
    echo "build-man: scdoc not found (install scdoc 1.11+)" >&2
    exit 2
}

for source in docs/man/*.scd; do
    output="${source%.scd}"
    scdc_tmp="$output.tmp"
    scdoc <"$source" >"$scdc_tmp"
    man --warnings -E UTF-8 -l "$scdc_tmp" >/dev/null 2>&1 || {
        # groff-loaded mans report warnings on some macOS builds; keep the
        # failure soft but visible.
        echo "build-man: warning: groff reported issues for $source" >&2
    }
    mv "$scdc_tmp" "$output"
    echo "built $output"
done
