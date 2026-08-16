# Your first session

<div class="status-coverage">

**Status coverage:** [J-REPL-001](../status/matrix.md#J-REPL-001) — **Implemented**; [J-RUN-002](../status/matrix.md#J-RUN-002) — **Implemented**. See [status conventions](../welcome/status-conventions.md).

</div>

Start the shell from the repository. Josh shows `josh> ` for a complete input and `...> ` when the parser needs more source.

**Host command**
```sh
./target/debug/josh
```

<p class="example-label example-label--implemented"><strong>Runnable example · Implemented</strong></p>

```console
josh> printf 'hello\n'
hello
josh> (1 + 2)
3
```

The first line is a command. The second starts with an expression delimiter, so Josh evaluates arithmetic and prints the resulting non-null value.

<a id="J-RUN-002"></a>
## Standalone builtins <span class="status status--implemented" aria-label="Status: Implemented">Implemented</span>

**Availability:** Available in Josh 0.1.0. Evidence: `builtins_are_dispatched_from_evaluated_argv_and_validate_arity` and CLI exit-status tests.

`cd [path]` changes Josh's process directory; without a path it uses `HOME`, then `.`. `exit [status]` leaves the REPL or returns that status from batch mode. Both must be standalone. A pipeline or capture containing either builtin returns an unsupported-feature error.

<p class="example-label example-label--implemented"><strong>Runnable example · Implemented</strong></p>

```console
josh> cd /tmp
josh> pwd
/tmp
josh> exit 0
```

Ctrl-D on an empty edit also exits with status 0. Ctrl-C clears a partial edit rather than terminating Josh.
