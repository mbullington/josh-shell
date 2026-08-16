# Strings, interpolation, and splicing

<div class="status-coverage">

**Status coverage:** [J-ARGV-001](../status/matrix.md#J-ARGV-001) — **Implemented**; [J-UNICODE-001](../status/matrix.md#J-UNICODE-001) — **Implemented**. See [status conventions](../welcome/status-conventions.md).

</div>

Single quotes are literal command words. In double quotes, a backslash removes itself and makes the next Unicode scalar literal. Expression double-quoted strings and quoted Object keys use the same decoding. Double-quoted command words also support variable, expression, and capture parts. Unquoted `$name` splices a value; `${...}` is available inside double quotes, not as a general command-mode delimiter.

<p class="example-label example-label--implemented"><strong>Runnable example · Implemented</strong></p>

```console
josh> n = 6
6
josh> printf '%s\n' 'literal $n' "value=${n * 7}"
literal $n
value=42
```

Source strings are UTF-8. Captured process output may become Bytes when decoding fails, and Unix argv preserves those bytes. Displaying Bytes as an expression prints a size marker rather than decoding it.

String length, signed indexing, and `at` count Unicode scalar values. They do not count UTF-8 bytes or extended grapheme clusters.
