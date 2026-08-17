#!/bin/sh
# Regenerate josh.profdata by training on the regex benchmark, then build the
# PGO-optimized release binary. Requires llvm-tools (rustup component add llvm-tools).
set -eu
cd "$(dirname "$0")/.."
PGO_DIR="$(mktemp -d)"
trap 'rm -rf "$PGO_DIR"' EXIT
RUSTFLAGS="-Cprofile-generate=$PGO_DIR" cargo build --release
target/release/josh --no-config scripts/regex-bench.josh > /dev/null
target/release/josh --no-config scripts/regex-bench.josh > /dev/null
"$(rustc --print sysroot)"/lib/rustlib/*/bin/llvm-profdata merge -o josh.profdata "$PGO_DIR"/*.profraw
RUSTFLAGS="-Cprofile-use=$PWD/josh.profdata" cargo build --release
printf 'wrote %s and built PGO binary\n' "$PWD/josh.profdata"
