# Diagnose a failed command

<div class="status-coverage">

**Status coverage:** [J-ERR-001](../status/matrix.md#J-ERR-001) — **Implemented**. See [status conventions](../welcome/status-conventions.md).

</div>

Read failures from the outside inward:

1. **Parse diagnostics** include a stable code, byte span, expected-token set when applicable, and an EOF-causation flag used by the REPL.
2. **Command not found** identifies the zero-based pipeline stage and command.
3. **Spawn errors** identify the resolved stage that the operating system could not start.
4. **Command/pipeline failures** retain ordered stage outcomes with exit code or signal.

<p class="example-label example-label--implemented"><strong>Runnable example · Implemented</strong></p>

```console
$ josh --no-config -c 'sh -c "exit 9"'
error: uncaught value: command: command failed: `sh -c 'exit 9'` (exit 9)
```

The exact quoting in rendered diagnostics is for display, not a command to copy. Batch failure exits 1; the REPL prints the error and remains available.

`let y = ls` resolves `ls` only as an expression identifier. If no binding exists, Josh reports ``undefined identifier `ls`; use $(ls) to capture a command``. It does not consult PATH to change the parse after the fact.
