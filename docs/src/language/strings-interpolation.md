# Strings, interpolation, and splicing

Quoted strings come in one processing form and one raw form. Both `'…'` and `"…"` are identical processed strings: they decode JavaScript escape sequences, and inside command words they interpolate variables and run captures. Use `r'…'` (or `r"…"`) when nothing may be processed — raw strings never decode escapes, never interpolate variables, and never run captures.

Escape decoding follows JavaScript: `\n`, `\t`, `\r`, `\b`, `\f`, `\v`, `\0`, `\\`, `\"`, `\'`, `\xNN`, `\uNNNN`, and `\u{…}` code points, plus a line continuation after a trailing backslash. Any other escaped character stands for itself, so `\$` and `\"` quote those delimiters literally in command words. Expression strings, quoted Object keys, and command words all share this decoding.

Inside command words, processed strings also interpolate: `$name` splices a value, `${…}` evaluates an expression, and `$(…)` runs a capture. `$name` splices only the name — write `${e.message}`, not `$e.message`, for member access. Unquoted `$name` splices a value; `${...}` is available inside both quote kinds, not as a general command-mode delimiter.

<p class="example-label"><strong>Runnable example</strong></p>

```console
josh> n = 6
6
josh> printf '%s\n' r'literal $n' "value=${n * 7}"
literal $n
value=42
```

Source strings are UTF-8. Captured process output may become Bytes when decoding fails, and Unix argv preserves those bytes. Displaying Bytes as an expression prints a size marker rather than decoding it.

String length, signed indexing, and `at` count UTF-16 code units (JavaScript semantics): `"😀".length` is 2. A position inside a surrogate pair resolves to the whole code point, since Josh strings cannot hold lone surrogates. See [String indexing unit](../reference/operators-values.md#J-UNICODE-001) and [Range slices](../reference/operators-values.md#J-EXPR-004).
