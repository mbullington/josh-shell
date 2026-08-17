#!/usr/bin/env bash
# Hard correctness gates for the interpreter-optimization loop.
# Keep output minimal; exit codes carry the verdict.
set -euo pipefail
cd "$(dirname "$0")"

cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -5
cargo test --workspace --all-targets 2>&1 | grep -E "test result" | tail -12
JOSH=target/release/josh ./scripts/check-share.sh 2>&1 | tail -3
