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
  josh --check <script.josh>
  josh --check -c <source>

Options:
  --no-config  Skip env.josh and interactive init.josh startup files
  --check      Parse only: report diagnostics, exit nonzero on errors
  -h, --help   Show this help
  -V, --version
               Show the version

Josh supports external commands and structured pipelines, redirections and globs,
variables, functions/closures/UFCS, and non-job control flow. Jobs and modules are unavailable.
```

| Invocation | Behavior |
|---|---|
| `josh` | Run startup files and start the Reedline REPL |
| `josh -c SOURCE` | Run startup environment and one source argument |
| `josh PATH` | Run startup environment and one UTF-8 script |
| `josh --no-config ...` | Skip both startup files |
| `josh -h`, `josh --help` | Print help, exit 0 without loading config |
| `josh -V`, `josh --version` | Print `josh 0.1.0`, exit 0 without loading config |
| `josh --check PATH` | Parse one script and print diagnostics without executing it or loading startup files |
| `josh --check -c SOURCE` | Parse source text without executing it or loading startup files |

`-c` accepts no trailing argument. Script mode accepts no positional arguments. Usage/read/startup errors exit 2; parse/runtime errors and invalid `--check` source exit 1; `exit N` returns N within the process exit-code range.
