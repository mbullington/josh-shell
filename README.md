# Josh

Josh is a Unix-first JavaScript Object Shell. It combines deterministic command/expression parsing, lexical functions and objects, explicit byte/value pipelines, and parser-driven interactive editing.

## Build and run

```sh
cargo build --workspace
cargo run -- -c 'printf hello | tr a-z A-Z'
cargo run -- script.josh
cargo run
```

The editor language server is a separate binary: `cargo install --path crates/josh-cli` installs `josh`, `cargo install --path crates/josh-lsp` installs `josh-lsp`, and `josh lsp` looks for the server next to the `josh` binary first, then on `PATH`.

Use `--no-config` for reproducible automation. Otherwise Josh runs `$XDG_CONFIG_HOME/josh/env.josh` (or `~/.config/josh/env.josh`) for every session and `init.josh` for interactive sessions. A zero-argument `prompt` function may return the prompt string. `JOSH_HISTORY` selects the history file.

## Implemented

- Seven-crate workspace: syntax, runtime, process execution, structured streams, interactive editing, errors-only LSP server (separate `josh-lsp` binary, launched via `josh lsp`), and CLI composition.
- Lossless tolerant parser with strict policy, spans, diagnostics, and Complete/Incomplete/Invalid classification.
- Null, bool, int, float, string, bytes, array, insertion-ordered object, function, error, and status values.
- Lexical frames; snapshot closures; declarations/arrows; direct recursion; destructured/rest parameters; calls, members, indexes, spread, finite methods, and lexical UFCS.
- `if/else`, `while`, `loop`, `try/catch`, `throw`, `return`, `break`, `continue`, command `&&`/`||`, and `status pipeline`.
- External commands, PATH planning, OS byte pipes, pipefail, capture, redirections, and sorted quote-aware glob expansion.
- Explicit `text`, `json`, `lines`, `jsonl`, `chunks(n)`, function, `map`, `filter`, `take`, `first`, and `collect` stages with bounded channels and stable capture cardinality.
- Reedline REPL with continuation, highlighting, completion, hints, history, and configured prompts.
- VSCode extension (`editors/vscode/`): TextMate highlighting plus live parser diagnostics from `josh lsp`; `npm run package` builds a local `.vsix`.

## Excluded

Background `&`, `jobs`, `fg`, `bg`, imports, exports, and remote modules are rejected. `source file.josh` is the deliberate, bash-style include mechanism: explicit paths, current-frame evaluation, no module system. There is no `for` production, VM, event loop, prototype system, `this`, job table, or user-visible generic Stream value.

## Verification

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo build --workspace --all-targets
python3 docs/tools/check-manual.py
```
./scripts/check-share.sh
```

## Support libraries

`share/` holds [stb-style](https://github.com/nothings/stb) single-file Josh libraries — public domain (MIT-0), no stability promises, intended to be copied into your own config or project. It ships `assert` (assertion helpers) and `regex` (an RE2-syntax-subset engine in pure Josh, also the language's canonical performance benchmark). See [share/README.md](share/README.md); `scripts/check-share.sh` runs the selftests and the golden-output gate; `scripts/regex-bench.josh` is the repeatable performance harness.

The cross-project proof is `../agent-terminal/scripts/josh-e2e.sh`; it uses an isolated XDG config and fixed 80×24 grid, exercises the implemented language/stream/file surfaces, records semantic JSON and a deterministic PNG, rejects excluded jobs/modules, exits, and proves process/socket/runtime cleanup.
