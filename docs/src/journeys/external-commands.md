# Run external programs

<a id="J-RUN-001"></a>
## External command execution

A command name containing a path separator resolves directly and must name an executable file. Other names search inherited `PATH` in order. Josh resolves all stages before starting a pipeline.

<p class="example-label"><strong>Runnable example</strong></p>

```console
josh> /usr/bin/printf '%s\n' direct
direct
```

Portable `printf` paths vary, so prefer PATH names unless a scenario controls the host. Command arguments are byte-capable on Unix. Quoting controls source interpretation but does not invoke POSIX shell expansion.

Use `sh -c '...'` only when you intentionally delegate a string to a POSIX shell. Josh passes that string as one argument; the child shell, not Josh, interprets it. This distinction matters for error reporting and security review.
