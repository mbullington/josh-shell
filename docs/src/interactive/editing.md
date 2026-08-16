# Editing and multiline input

<div class="status-coverage">

**Status coverage:** [J-REPL-001](../status/matrix.md#J-REPL-001) — **Implemented**. See [status conventions](../welcome/status-conventions.md).

</div>

<a id="J-REPL-001"></a>
## Parser-driven editing <span class="status status--implemented" aria-label="Status: Implemented">Implemented</span>

**Availability:** Available in Josh 0.1.0. Evidence: Reedline validator tests and PTY multiline probes.

The main prompt is `josh> ` and the continuation prompt is `...> `. Reedline handles cursor movement and ordinary editing. On validation, Josh parses the whole buffer once and requests continuation only for `Completeness::Incomplete`.

<p class="example-label example-label--implemented"><strong>Runnable example · Implemented</strong></p>

```console
josh> (1 +
...> 2)
3
```

An open quote, delimiter, interpolation, block, trailing pipe, or missing EOF operand can be Incomplete. A hard error such as an unexpected closer is Invalid and submits immediately so the normal diagnostic path can report it; the REPL does not trap the user in multiline input.

Whole-buffer reparsing is intentional for short interactive buffers. Every span is a UTF-8 byte range, and completion floors an incoming cursor to a character boundary before constructing replacement spans.
