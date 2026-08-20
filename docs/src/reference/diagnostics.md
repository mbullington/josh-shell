# Diagnostics and exit behavior

<a id="J-ERR-001"></a>
## Diagnostic and process errors

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

Diagnostics carry severity, code, expected strings, primary label, secondary labels, and EOF causation. Interactive and command-line entry points render each diagnostic with the offending source line, the origin (`<input>` for one-liners and the REPL, otherwise the file path), a 1-based line and char column, and a caret under the primary span. The compact `Display` form (message, code, byte range, expected set) remains for tooling.

Process errors distinguish command-not-found, spawn, command failure, pipeline failure, and output collection. Outcomes retain zero-based stage, rendered command, code/signal, and success.

| Context | Uncaught parse/command failure | Explicit `exit N` |
|---|---|---|
| REPL | Print; return to prompt | Exit N |
| `-c` or script | Print; exit 1 | Exit N |
| CLI usage/read error | Exit 2 | Not applicable |
