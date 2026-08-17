#!/usr/bin/env bash
# Autoresearch benchmark: one release run of the fixed regex corpus.
# Outputs METRIC lines: total (primary), per-case diagnostics, guardrails.
set -euo pipefail
cd "$(dirname "$0")"

cargo build --release --quiet

out="$(target/release/josh --no-config scripts/regex-bench.josh)"
total=0
hits_total=0
case_count=0
while IFS= read -r line; do
    case "$line" in
        BENCH\ *)
            # fields: BENCH <name> iters= <n> ms= <n> hits= <n> len= <n>
            name="$(awk '{print $2}' <<<"$line")"
            ms="$(awk '{print $6}' <<<"$line")"
            hits="$(awk '{print $8}' <<<"$line")"
            total=$((total + ms))
            hits_total=$((hits_total + hits))
            case_count=$((case_count + 1))
            echo "METRIC ${name}_ms=$ms"
            ;;
    esac
done <<<"$out"

echo "METRIC bench_total_ms=$total"
echo "METRIC hits_total=$hits_total"
echo "METRIC case_count=$case_count"
