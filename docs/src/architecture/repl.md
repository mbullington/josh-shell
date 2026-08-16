# Reedline integration

<div class="status-coverage">

**Status coverage:** [J-REPL-001](../status/matrix.md#J-REPL-001) — **Implemented**; [J-REPL-002](../status/matrix.md#J-REPL-002) — **Implemented**; [J-REPL-003](../status/matrix.md#J-REPL-003) — **Implemented**; [J-REPL-004](../status/matrix.md#J-REPL-004) — **Implemented**. See [status conventions](../welcome/status-conventions.md).

</div>

## Reedline ownership

The REPL adapter is outside parser and runtime modules. `ReplAnalyzer` exposes parse and completion-context operations. Validator maps only Incomplete to continuation. Highlighter and completer independently parse immutable buffers and use UTF-8 byte spans.

A completion snapshot owns sorted command and variable sets. It combines builtins, PATH directory entries, lexical engine names, and inherited environment names. Reedline helpers borrow an `Arc` snapshot behind a short read lock; the main loop replaces it only after accepted input. Callbacks do not evaluate or mutate the engine.

History is a plain file through Reedline's file-backed implementation, with default prefix hints. This minimizes shared state and keeps SQLite outside the slice.

SIGINT installs a parent-side caught handler on the engine's root cancellation flag. Reedline Ctrl-C discards editing. During execution, the same flag reaches the active structured graph; its child cancellation scope stops and joins workers and kills and reaps external process groups without poisoning the next command. This seam is not terminal process-group transfer or job control.
