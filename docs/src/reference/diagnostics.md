# Diagnostics and exit behavior

<div class="status-coverage">

**Status coverage:** [J-ERR-001](../status/matrix.md#J-ERR-001) — **Implemented**; [J-PARSE-004](../status/matrix.md#J-PARSE-004) — **Implemented**. See [status conventions](../welcome/status-conventions.md).

</div>

<a id="J-ERR-001"></a>
## Diagnostic and process errors <span class="status status--implemented" aria-label="Status: Implemented">Implemented</span>

**Availability:** Available in Josh 0.1.0. Evidence: parser diagnostic-code assertions, batch CLI tests, and ordered process-outcome tests.

| Code | Meaning |
|---|---|
| `L001` | Unclosed quote at EOF |
| `P101`–`P102` | Unexpected block closer or unparseable token |
| `P110`–`P111` | Missing binding name or `=` |
| `P120`–`P121` | Missing/open `if` block |
| `P130`–`P131` | Trailing pipe or missing command |
| `P140` | Reserved command syntax is unsupported |
| `P150` | Missing member name |
| `P160`–`P163` | Missing/invalid expression, including `==` suggestion |
| `P170`–`P171` | Missing delimiter |
| `P180`–`P181` | Unclosed command/expression interpolation |

Diagnostics carry severity, code, expected strings, primary label, secondary labels, and EOF causation. Display currently prints message, code, byte range, and expected set.

Process errors distinguish command-not-found, spawn, command failure, pipeline failure, and output collection. Outcomes retain zero-based stage, rendered command, code/signal, and success.

| Context | Uncaught parse/command failure | Explicit `exit N` |
|---|---|---|
| REPL | Print; return to prompt | Exit N |
| `-c` or script | Print; exit 1 | Exit N |
| CLI usage/read error | Exit 2 | Not applicable |
