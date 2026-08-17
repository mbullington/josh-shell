# Release verification

Run from clean source trees. Record both product revisions, lockfiles, toolchain identity, check date, and retained artifact hashes. Compilation and test summaries are necessary evidence, not substitutes for the real CLI/artifact paths.

1. **Josh quality:** format, Clippy with warnings denied, workspace all-target tests, and workspace all-target build.
2. **Josh language:** direct objects, insertion order, spread/destructuring, closures, recursion, exact methods, lexical UFCS, conversions, and Unicode scalar indexing.
3. **Josh control:** expression/command conditions, while/loop, typed return/break/continue/throw, catch of runtime/process errors, Status members, chain short-circuiting, and planning-error propagation.
4. **Josh streams/files/config:** every documented byte/value transition and cardinality; malformed UTF-8/JSON/JSONL; bounded producer cancellation; every redirection and ordering; quoted/unquoted/no-match globs; XDG/fallback startup order, scope, `--no-config`, error policy, and prompt through a PTY.
5. **Excluded Josh surfaces:** prove `&`, `jobs`, `fg`, `bg`, `source`, `import`, `export`, and remote imports fail; keep background assignment Unresolved.
6. **agent-terminal prerequisite:** exact Zig 0.16.0, Ghostty SHA/tree, matching Rust tools, renderer lockfile, four vendored font faces/license, and static `libghostty-vt` linkage.
7. **agent-terminal behavior:** locked format/Clippy/tests/build; real PTY/input/key/resize/wait/protocol/security/lifecycle tests; schema-v2 semantic cells/styles/render facts; screenshot omitted/explicit paths; exact pixels/dimensions; metadata-free repeated PNG bytes.
8. **Cross-product:** run the isolated-XDG fixed-grid Josh scenario twice; inspect semantic JSON and PNG; compare each run's repeated screenshot; exit/close; directly inspect process, socket, session, and temporary-runtime cleanup.
9. **Manual:** run the checker and persistent mdBook build; inspect generated links/fragments, literal fences, duplicate IDs, code blocks, wait-table shape, index/schema pages, and the retained PNG.

**Host command**
```sh
cd /Users/mbullington/Projects/josh-shell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo build --workspace --all-targets
python3 docs/tools/check-manual.py
mdbook build docs

test -f docs/book/index.html
test -f docs/book/agent-terminal/screenshots.html
```

**Host command**
```sh
cd /Users/mbullington/Projects/agent-terminal
test "$(nix develop --no-write-lock-file -c zig version)" = 0.16.0
nix develop --no-write-lock-file -c cargo fmt --all -- --check
nix develop --no-write-lock-file -c cargo clippy --locked --all-targets -- -D warnings
nix develop --no-write-lock-file -c cargo test --locked --all-targets
nix develop --no-write-lock-file -c cargo build --locked --all-targets
scripts/smoke.sh target/debug/agent-terminal
scripts/josh-e2e.sh target/debug/agent-terminal /Users/mbullington/Projects/josh-shell/target/debug/josh
scripts/josh-e2e.sh target/debug/agent-terminal /Users/mbullington/Projects/josh-shell/target/debug/josh
shasum -a 256 target/josh-e2e/screenshot.png
```

The 2026-08-15 release evidence was 53 Josh tests, 12 agent-terminal unit tests, 4 agent-terminal CLI integration tests, 66 manual pages, 42 capability rows, 26 extracted runnable fences, a 640×384 RGBA PNG, and SHA-256 `5956b699529a9d8e3167078e8682339c811c80e7f56cb4d8a69c67abd50d9428`. Recompute rather than copying those values into a later release record.
