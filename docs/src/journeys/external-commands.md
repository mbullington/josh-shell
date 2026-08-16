# Run external programs

<div class="status-coverage">

**Status coverage:** [J-RUN-001](../status/matrix.md#J-RUN-001) — **Implemented**; [J-ARGV-001](../status/matrix.md#J-ARGV-001) — **Implemented**. See [status conventions](../welcome/status-conventions.md).

</div>

<a id="J-RUN-001"></a>
## External command execution <span class="status status--implemented" aria-label="Status: Implemented">Implemented</span>

**Availability:** Available in Josh 0.1.0. Evidence: `command_and_script_modes_share_observable_behavior` and process planner tests.

A command name containing a path separator resolves directly and must name an executable file. Other names search inherited `PATH` in order. Josh resolves all stages before starting a pipeline.

<p class="example-label example-label--implemented"><strong>Runnable example · Implemented</strong></p>

```console
josh> /usr/bin/printf '%s\n' direct
direct
```

Portable `printf` paths vary, so prefer PATH names unless a scenario controls the host. Command arguments are byte-capable on Unix. Quoting controls source interpretation but does not invoke POSIX shell expansion.

Use `sh -c '...'` only when you intentionally delegate a string to a POSIX shell. Josh passes that string as one argument; the child shell, not Josh, interprets it. This distinction matters for error reporting and security review.
