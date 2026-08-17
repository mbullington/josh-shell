# Josh command-line interface

<div class="status-coverage">

**Status coverage:** [J-CLI-001](../status/matrix.md#J-CLI-001) — **Implemented**; [J-CONFIG-001](../status/matrix.md#J-CONFIG-001) — **Implemented**. See [status conventions](../welcome/status-conventions.md).

</div>

<a id="J-CLI-001"></a>
## CLI entry points <span class="status status--implemented" aria-label="Status: Implemented">Implemented</span>

**Availability:** Available in Josh 0.1.0. Evidence: help/version, command/script, config, error, and exit-policy tests.

**Host command**
```text
Josh — JavaScript Object Shell

Usage:
  josh [--no-config]
  josh [--no-config] -c <source>
  josh [--no-config] <script.josh>
  josh lsp

Options:
  --no-config  Skip env.josh and interactive init.josh startup files
  -h, --help   Show this help
  -V, --version
               Show the version

Josh supports external commands and structured pipelines, redirections and globs,
variables, functions/closures/UFCS, and non-job control flow. Jobs and modules are unavailable.

The lsp subcommand serves the errors-only language server over stdin/stdout
for editor integrations (see editors/vscode/).
```

| Invocation | Behavior |
|---|---|
| `josh` | Run startup files and start the Reedline REPL |
| `josh -c SOURCE` | Run startup environment and one source argument |
| `josh PATH` | Run startup environment and one UTF-8 script |
| `josh --no-config ...` | Skip both startup files |
| `josh -h`, `josh --help` | Print help, exit 0 without loading config |
| `josh -V`, `josh --version` | Print `josh 0.1.0`, exit 0 without loading config |
| `josh lsp` | Serve the errors-only language server over stdin/stdout |

`josh lsp` starts no REPL session, creates no `ProcessHost`, and loads no startup files: stdout carries only Language Server Protocol frames. The server is stateless (full-document sync, parse diagnostics only, no file I/O); closing stdin exits it. The VSCode extension in `editors/vscode/` spawns this subcommand by default.

`-c` accepts no trailing argument. Script mode accepts no positional arguments. Usage/read/startup errors exit 2; parse/runtime errors exit 1; `exit N` returns N within the process exit-code range.
