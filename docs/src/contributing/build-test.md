# Build and test

<div class="status-coverage">

**Status coverage:** [J-CLI-001](../status/matrix.md#J-CLI-001) — **Implemented**; [AT-BUILD-001](../status/matrix.md#AT-BUILD-001) — **Implemented**. See [status conventions](../welcome/status-conventions.md).

</div>

Use separate Cargo target directories to run independent checks concurrently. Otherwise serialize Cargo commands that share `target`, and always serialize terminal scripts that own sockets or process lifecycles.

## Josh evidence

**Host command**
```sh
cd /Users/mbullington/Projects/josh-shell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo build --workspace --all-targets
./target/debug/josh --no-config -c 'printf hello | tr a-z A-Z'
```

The 2026-08-15 verification passed format, warnings-denied Clippy, all-target build, and 44 tests. The tests cover parser recovery; data/functions/snapshot closures/direct recursion/method priority/UFCS; typed unwinding and status chains; structured graph validation, decode errors, stable cardinality, 256-item channels, early producer cancellation, and reap; every redirection and descriptor order; quote-aware sorted globs; startup order/scope/error policy; and a configured prompt through a real PTY. A direct language probe produced `11|x!|2,4|ok`.

## agent-terminal evidence

Ghostty is pinned to revision `d760ee96e54657416eb427b793c7e839f003df7d`, tree `6f245f6c192857e1ee69503c20158e9abf583cee`, and Zig 0.16.0. The flake supplies matching Cargo, Clippy, and rustfmt packages.

**Host command**
```sh
cd /Users/mbullington/Projects/agent-terminal
nix develop --no-write-lock-file -c cargo fmt --all -- --check
nix develop --no-write-lock-file -c cargo clippy --locked --all-targets -- -D warnings
nix develop --no-write-lock-file -c cargo test --locked --all-targets
nix develop --no-write-lock-file -c cargo build --locked --all-targets
scripts/smoke.sh target/debug/agent-terminal
scripts/josh-e2e.sh target/debug/agent-terminal /Users/mbullington/Projects/josh-shell/target/debug/josh
```

The 2026-08-15 verification passed Nix-shell format, warnings-denied Clippy, locked all-target build, 12 unit tests, 4 CLI integration tests, smoke, and the Josh scenario twice. Tests assert linked Ghostty identity, protocol/lifecycle/security boundaries, schema-v2 render facts and malformed-state rejection, response-budget maxima, exact renderer pixels and dimensions, all font faces/cursor variants, OSC 4/10/11/12 colors, DECSCUSR states, omitted screenshot paths, metadata-free encoding, and repeated bytes. The reviewed 128×32 feature fixture has PNG SHA-256 `b1aa287227011a11a50b57dde51d785deac5eaf827cff6a15349a4bc98240c96`. Both Josh runs produced byte-identical repeated renders and the same retained 640×384 RGBA SHA-256 `5956b699529a9d8e3167078e8682339c811c80e7f56cb4d8a69c67abd50d9428`.

## Cross-product evidence

`scripts/josh-e2e.sh` builds an isolated HOME/XDG/PATH/runtime/root and launches Josh at 80×24. Each command must reach a later terminal revision ending at the configured prompt. Side effects distinguish taken command-chain branches from echoed input; exact byte comparisons cover every structured cardinality class, value serialization, all redirections, and complete ordered wildcard/bracket/recursive glob results. The script also checks language/functions/UFCS, semantic styles, deterministic PNG output, and explicit rejection of `&`, `jobs`, `fg`, `bg`, `source`, `import`, `export`, and remote import. Final assertions require `sessions=[]`, no control sockets or temporary runtime, and no producer, external child, Josh, or daemon PID.

## Manual evidence

**Host command**
```sh
cd /Users/mbullington/Projects/josh-shell
python3 docs/tools/check-manual.py
mdbook build docs
```

The checker builds a temporary mdBook and inspects all source and generated pages for SUMMARY reachability, links/fragments, literal fences, duplicate IDs, source-to-HTML code-block preservation, extracted runnable examples, capability/status drift, matrix provenance, table shape, and status-color contrast. The 2026-08-15 run passed 66 pages and 42 capabilities and extracted 26 runnable fences. The persistent build is `docs/book/index.html`; review the retained PNG at `../agent-terminal/target/josh-e2e/screenshot.png`.
