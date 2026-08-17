# Autoresearch: speed up the Josh interpreter against the regex benchmark

## Objective and Workload

`share/regex.josh` is Josh's canonical pure-computation benchmark: a Pike-VM
regex engine written in the language, dominated by interpreter hot loops —
bytecode dispatch (`re_run`'s per-step thread loop), closure calls, array
push/pop, object field lookup, string length/index, and array slices.

The workload is the committed fixed corpus: `scripts/regex-bench.josh`,
8 cases with fixed iteration counts and subjects. **The benchmark, corpus,
iteration counts, and `share/regex.josh` itself are frozen.** We optimize the
Rust interpreter, not the benchmark or the Josh code.

Symptom: tree-walking interpreter overhead; every regex char step costs
microseconds of Josh evaluation on this machine.

## Metric Contract

- **Primary**: `bench_total_ms` (ms, lower is better) — sum of the 8 per-case
  `total_ms` values from one run of `scripts/regex-bench.josh` under the
  release binary.
- **Secondary guardrails**:
  - `hits_total` must stay exactly 315 (semantic checksum across all cases);
  - `case_count` must stay exactly 8;
  - all hard checks in `autoresearch.checks.sh` must pass.
- **Secondary tie-breakers**: none. Per-case `METRIC <case>_ms=` lines are
  printed for diagnostics and hypothesis selection only.

## How to Run

`./autoresearch.sh` — rebuilds release, runs the benchmark once, prints
`METRIC` lines. Checks run automatically after each passing benchmark via
`./autoresearch.checks.sh`.

## Files in Scope

- `crates/josh-runtime/src/**` — evaluator, Value repr, frames, natives
- `crates/josh-streams/src/**`, `crates/josh-exec/src/**` — pipeline plumbing
- `crates/josh-cli/src/main.rs` + `Cargo.toml` files — allocator/build-level tuning
- `crates/josh-syntax/src/**` — lexer/parser (only if they show on the profile)
- `autoresearch.md`, `autoresearch.ideas.md` (working notes/backlog only)

## Off Limits

- `share/**` (the benchmark's Josh code and golden output)
- `scripts/regex-bench.josh`, `scripts/check-share.sh`, `share/regex.golden.txt`
  (harness and correctness gates), `autoresearch.sh`, `autoresearch.checks.sh`
- Language semantics: `docs/src` (manual) is the spec and is frozen; behavior
  changes require manual + matrix updates and are out of scope for this loop
- `../agent-terminal` (user's live workspace)
- No caching/memoization of benchmark answers at the semantics level; every
  benchmark call must still do the real work (smoking-gun example: no skipping
  `re_compile` by keying on pattern text).

## Hard Constraints

`autoresearch.checks.sh` runs after every passing benchmark:

1. `cargo test --workspace --all-targets`
2. `cargo clippy --workspace --all-targets -- -D warnings`
3. `JOSH=target/release/josh ./scripts/check-share.sh` (selftests + golden diff)
4. `cargo fmt --all -- --check`

A result keeps only if checks pass, the improvement is credibly outside noise
(confidence ≥ 2.0 when available), and both guardrails hold.

## Stop Conditions

- Target: **≥ 20% total reduction** (bench_total_ms ≤ ~1410 from a ~1762ms
  baseline), or
- 25 iterations, or ~90 minutes wall clock, whichever comes first, or
- hypothesis exhaustion (backlog empty), user interruption, or tooling dead end.
