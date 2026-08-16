# Interactive shell

<div class="status-coverage">

**Status coverage:** [J-REPL-001](../status/matrix.md#J-REPL-001) — **Implemented**; [J-REPL-002](../status/matrix.md#J-REPL-002) — **Implemented**; [J-REPL-003](../status/matrix.md#J-REPL-003) — **Implemented**; [J-REPL-004](../status/matrix.md#J-REPL-004) — **Implemented**; [J-REPL-005](../status/matrix.md#J-REPL-005) — **Implemented**. See [status conventions](../welcome/status-conventions.md).

</div>

Josh embeds Reedline for line editing. The parser, not a separate delimiter counter, drives continuation, highlighting, and completion context. A completion snapshot contains lexical names, inherited environment names, builtins, and a PATH index; it is replaced between accepted lines.

The current surface includes a primary prompt, multiline prompt, token highlighting, file-backed history, prefix hints, command/file/variable completion, Ctrl-C recovery, and Ctrl-D exit. It does not include job control, SQLite history, rich output, or third-party completion specifications.
