# Signals and troubleshooting

<div class="status-coverage">

**Status coverage:** [J-REPL-005](../status/matrix.md#J-REPL-005) — **Implemented**; [J-JOBS-001](../status/matrix.md#J-JOBS-001) — **Planned**. See [status conventions](../welcome/status-conventions.md).

</div>

<a id="J-REPL-005"></a>
## Clean interruption <span class="status status--implemented" aria-label="Status: Implemented">Implemented</span>

**Availability:** Available in Josh 0.1.0. Evidence: Ctrl-C PTY probes for partial input and a foreground child.

Ctrl-C while editing discards the current input and redraws a fresh prompt. Ctrl-D on an empty buffer exits. While a command or structured graph runs, Josh's caught SIGINT marks that active execution as cancelled. Josh stops and joins in-shell workers, terminates and reaps external child process groups—including commands called by stream functions—and then returns to the prompt.

This is not full job control. Josh does not transfer terminal ownership among process groups, manage background jobs, or expose job tables.

## Troubleshooting checklist

- **Unexpected continuation:** the parser found only EOF-caused missing syntax. Close quotes, parentheses, brackets, captures, or blocks; complete a trailing operator or pipe.
- **Immediate error instead of continuation:** a hard error made the parse Invalid. Fix the first reported span.
- **Red command name:** it was absent from the PATH snapshot when the prompt began. Run it by path or accept another line after changing PATH.
- **No history:** check `JOSH_HISTORY`, HOME, parent-directory permissions, and whether the history file is writable.
- **Ctrl-C does not stop a program:** a hostile descendant can escape its inherited process group because this slice is cleanup, not containment. Stop an escaped process using host process tools.
