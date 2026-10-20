# Release verification

Run from a clean checkout. Record the revision, lockfile, toolchain, and check date. A build or test summary is not a substitute for running the CLI.

1. Run format, Clippy with warnings denied, workspace tests, and the all-target build.
2. Exercise objects, prototypes, insertion order, spread, destructuring, closures, recursion, methods, UFCS, conversions, and Unicode string indexing.
3. Check expression and command conditions, control-flow unwinding, error catches, status chains, and planning errors.
4. Check byte/value transitions and cardinality, malformed UTF-8/JSON/JSONL, bounded cancellation, redirections, globs, startup files, and the prompt in a PTY.
5. Confirm that `&`, `jobs`, `fg`, `bg`, `import`, `export`, and remote imports fail.
6. Run `josh --check` on valid and invalid source; confirm that it neither runs commands nor loads startup files.
7. Build the manual and check its links and generated pages.

**Host command**
```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo build --workspace --all-targets
python3 docs/tools/check-manual.py
mdbook build docs
test -f docs/book/index.html
./target/debug/josh --check -c 'echo ready'
```
