# Development setup and repository map

Josh is a six-crate Rust workspace. Rust/Cargo and common Unix test utilities are required.

| Path | Ownership |
|---|---|
| `crates/josh-syntax` | Lossless lexer/parser, AST, spans, diagnostics, and completeness |
| `crates/josh-runtime` | Values, lexical frames, closures, evaluator, typed unwinding, and the execution-host contract |
| `crates/josh-exec` | PATH/argv planning, globbing, redirection opens, and process-host composition |
| `crates/josh-streams` | Typed byte/value stage graph, bounded channels, external processes, cancellation, and reap |
| `crates/josh-interactive` | Reedline prompt, validator, highlighter, completion, hints, history, and signal behavior |
| `crates/josh-cli` | `josh` argument routing, startup files, scripts, and REPL composition |
| `scripts/structured-large-producer.sh` | Bounded-memory stream test producer |

The manual is an mdBook under `docs`: `src/SUMMARY.md`, Markdown source, and `tools/check-manual.py`. The checker validates page reachability, links and fragments, code fences, generated code blocks, and duplicate IDs.
