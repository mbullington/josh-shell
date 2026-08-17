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

The sibling `/Users/mbullington/Projects/agent-terminal` tree separates CLI/client, daemon, protocol, runtime paths, PTY, semantic Ghostty FFI, and client-side PNG renderer. `vendor/ghostty`, Zig 0.16.0, renderer crates, and four font faces are pinned build inputs. `scripts/smoke.sh` proves the core terminal slice; `scripts/josh-e2e.sh` proves the complete cross-product scenario.

The manual is a conventional mdBook under `docs`: hand-edited `src/SUMMARY.md`, Markdown source, and deterministic `tools/check-manual.py`. The checker validates page reachability, links/fragments, fence labeling/extraction, generated code blocks, duplicate IDs, literal fences, and the agent-terminal wait row. It uses no custom JavaScript.
