# Build and test

Use separate Cargo target directories to run independent checks concurrently. Otherwise serialize Cargo commands that share `target`, and always serialize terminal scripts that own sockets or process lifecycles.

## Josh evidence

**Host command**
```sh
git clone https://github.com/mbullington/josh-shell
cd josh-shell
cargo install patch-crate --version 0.1.13 --locked
cargo patch-crate
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo build --workspace --all-targets
./target/debug/josh --no-config -c 'printf hello | tr a-z A-Z'
```

The 2026-08-15 verification passed format, warnings-denied Clippy, all-target build, and 44 tests. The tests cover parser recovery; data/functions/snapshot closures/direct recursion/method priority/UFCS; typed unwinding and status chains; structured graph validation, decode errors, stable cardinality, 256-item channels, early producer cancellation, and reap; every redirection and descriptor order; quote-aware sorted globs; startup order/scope/error policy; and a configured prompt through a real PTY. A direct language probe produced `11|x!|2,4|ok`.

## Manual evidence

**Host command**
```sh
cd josh-shell
python3 docs/tools/check-manual.py
mdbook build docs
```

The checker builds a temporary mdBook (when mdbook is on PATH) and checks SUMMARY reachability, links, fragments, code fences, duplicate IDs, and source-to-HTML code-block preservation. The persistent build goes to the ignored `docs/book/` directory. CI publishes it at `/josh-shell/manual/`.

Man pages live in `docs/man/*.scd` (scdoc) with the generated roff committed; `scripts/build-man.sh` regenerates and soft-validates them. Keep the man page a summary — the manual remains the language reference.

`scripts/check-share.sh` runs the `share/*.josh` selftests and the regex golden-output gate; `scripts/regex-bench.josh` prints the fixed-corpus performance table used by interpreter-optimization work (timings vary; the `hits` checksum column must not).
