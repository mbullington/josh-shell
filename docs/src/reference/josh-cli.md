# Josh command-line interface

<a id="J-CLI-001"></a>
## CLI entry points

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

`josh lsp` execs the sibling `josh-lsp` binary (found next to the `josh` binary first, then on `PATH`) before any REPL session, `ProcessHost`, or startup file: stdout carries only Language Server Protocol frames. The server is stateless (full-document sync, parse diagnostics only, no file I/O); closing stdin exits it. It is a separate binary because the editor protocol stack's dependencies measurably slow the shell when linked in. The VSCode extension in `editors/vscode/` spawns this subcommand by default.

`-c` accepts no trailing argument. Script mode accepts no positional arguments. Usage/read/startup errors exit 2; parse/runtime errors exit 1; `exit N` returns N within the process exit-code range.
