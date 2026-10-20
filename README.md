# Josh

Josh is a Unix-first JavaScript Object Shell. It combines deterministic command/expression parsing, lexical functions and objects, explicit byte/value pipelines, and parser-driven interactive editing.

**[▶ Try Josh in your browser](https://mbullington.github.io/josh-shell/)** — no install. The link boots a Linux VM in the tab (via [trynix](https://trynix.dev)) with the latest `main` build on `PATH`; type `josh` at the prompt. Or run it with Nix: `nix run github:mbullington/josh-shell`.

## Build and run

```sh
cargo install patch-crate --version 0.1.13 --locked
cargo patch-crate
cargo build --workspace
cargo run -- -c 'printf hello | tr a-z A-Z'
cargo run -- script.josh
cargo run
```

Run `cargo patch-crate --force` after pulling changes to `patches/`, or `cargo patch-crate` after `cargo clean`. The generated Reedline source lives under `target/patch/`; only the patch is checked in. See [dependency patch maintenance](patches/README.md).

`cargo install --path crates/josh-cli` installs `josh`. Use `josh --check script.josh` or `josh --check -c 'source'` to report syntax errors without running the source or loading startup files. The [manual](https://mbullington.github.io/josh-shell/manual/) is built from `docs/src/` by CI.

For the fully optimized binary, run `./scripts/build-pgo.sh` (requires `rustup component add llvm-tools`): it instruments a build, trains `josh.profdata` on `scripts/regex-bench.josh`, and rebuilds with `-Cprofile-use`. The checked-in `josh.profdata` is the current training result; plain `cargo build --release` does not apply it unless `-Cprofile-use` is passed in `RUSTFLAGS`.

Use `--no-config` for reproducible automation. Otherwise Josh runs `$XDG_CONFIG_HOME/josh/env.josh` (or `~/.config/josh/env.josh`) for every session and `init.josh` for interactive sessions. A zero-argument `prompt` function may return the prompt string. `JOSH_HISTORY` selects the history file.

## Implemented

- Six-crate workspace: syntax, runtime, process execution, structured streams, interactive editing, and CLI composition.
- Lossless tolerant parser with strict policy, spans, diagnostics, and Complete/Incomplete/Invalid classification.
- Null, bool, int, float, string, bytes, array, insertion-ordered object, function, error, and status values.
- Lexical frames; snapshot closures; declarations/arrows; direct recursion; destructured/rest parameters; calls, members, indexes, spread, mutable prototypes, methods, and lexical UFCS.
- `if/else`, `while`, `loop`, `try/catch`, `throw`, `return`, `break`, `continue`, command `&&`/`||`, and `status pipeline`.
- External commands, PATH planning, OS byte pipes, pipefail, capture, redirections, and sorted quote-aware glob expansion.
- Explicit `text`, `json`, `lines`, `jsonl`, `chunks(n)`, function, `map`, `filter`, `take`, `first`, and `collect` stages with bounded channels and stable capture cardinality.
- Reedline REPL with continuation, highlighting, completion, hints, history, configured prompts, and click-to-position in terminals supporting OSC 133 `click_events`.
- `--check` syntax diagnostics for scripts and `-c` source, without execution.

## Excluded

Background `&`, `jobs`, `fg`, `bg`, imports, exports, and remote modules are rejected. `source file.josh` is the deliberate, bash-style include mechanism: explicit paths, current-frame evaluation, no module system. There is no `for` production, VM, event loop, `this`, job table, or user-visible generic Stream value.

## Verification

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo build --workspace --all-targets
python3 docs/tools/check-manual.py
mdbook build docs
./scripts/check-share.sh
```

## Support libraries

`share/` holds [stb-style](https://github.com/nothings/stb) single-file Josh libraries — public domain (MIT-0), no stability promises, intended to be copied into your own config or project. It ships `assert` (assertion helpers) and `regex` (an RE2-syntax-subset engine in pure Josh, also the language's canonical performance benchmark). See [share/README.md](share/README.md); `scripts/check-share.sh` runs the selftests and the golden-output gate; `scripts/regex-bench.josh` is the repeatable performance harness.
