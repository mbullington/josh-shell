# Modules and configuration

<div class="status-coverage">

**Status coverage:** [J-MOD-001](../status/matrix.md#J-MOD-001) — **Planned**; [J-MOD-002](../status/matrix.md#J-MOD-002) — **Implemented**; [J-CONFIG-001](../status/matrix.md#J-CONFIG-001) — **Implemented**. See [status conventions](../welcome/status-conventions.md).

</div>

<a id="J-MOD-001"></a>
## Modules <span class="status status--planned" aria-label="Status: Planned">Planned</span>

**Availability:** Excluded from the current implementation. Tracking: [Planned work](../roadmap/planned.md#j-mod-001).

`import` and `export` are rejected. URL and remote modules are also unavailable; Josh has no dependency identity, integrity, cache, update, offline, or trust model for them. `source` is the deliberate include mechanism (below) instead of a module system.

<a id="J-MOD-002"></a>
## source statement <span class="status status--implemented" aria-label="Status: Implemented">Implemented</span>

**Availability:** Available in development snapshots since 2026-08-16. Evidence: runtime source tests.

`source path.josh` reads a file, parses it under the strict policy, and evaluates it in the current frame — bash-style, with no new scope, no caching, and no module identity. `let` and `fn` declarations from the file are visible afterwards (aliases share object prototypes), which makes `source` the way to load shared helper files and startup configuration.

- The path is one command word: quoting, `$variable`, captures, and a leading `~` behave like any command argument. Relative paths resolve against the working directory. More than one word is a parse error.
- A file whose path is already being sourced (a cycle) fails with a type error; a missing or unreadable file is a catchable filesystem error; a file that fails the strict parse gate fails before anything runs.
- `return`, `break`, and `continue` cannot escape the sourced file into the caller; they are type errors at the boundary. Loops and functions inside the file behave normally.

<a id="J-CONFIG-001"></a>
## Configuration <span class="status status--implemented" aria-label="Status: Implemented">Implemented</span>

**Availability:** Available in Josh 0.1.0. Evidence: batch startup, fallback path, interactive prompt, error-policy, and cross-product tests.

Josh chooses one config root before running requested source:

1. nonempty `$XDG_CONFIG_HOME`;
2. otherwise nonempty `$HOME` plus `.config`;
3. otherwise no config root and no startup files.

It runs `<root>/josh/env.josh` for every batch, script, and interactive session. Interactive sessions then run `<root>/josh/init.josh`. Missing files are ignored. Files must be readable UTF-8 and run through the ordinary parser/evaluator in the same global lexical scope as the session. `env.josh` bindings are therefore visible to `init.josh`, the prompt function, and user input.

| Startup outcome | Batch or script | Interactive |
|---|---|---|
| Missing file | Continue | Continue |
| Read/UTF-8 error | Diagnose and exit 2 before requested source | Diagnose and continue to the next startup file or prompt |
| Parse/runtime/process error | Diagnose and exit 2 before requested source | Diagnose and continue to the next startup file or prompt |
| `exit N` | Diagnose and exit 2; it does not become the process status | Diagnose, ignore the exit request, and continue |

`--no-config` skips both startup files and is the reproducible automation mode. It does not change `$NAME` environment fallback or `JOSH_HISTORY`.

A visible lexical function named `prompt` controls the interactive primary prompt. It must take zero arguments and return String. Josh calls it before each read. Missing `prompt` uses `josh> `; a nonfunction, wrong arity, thrown error, or non-String result prints a diagnostic and falls back to `josh> `. The multiline prompt remains `...> `.

`JOSH_HISTORY` selects the Reedline history path. Without it, Josh uses `$HOME/.josh_history`, or `./.josh_history` when HOME is absent.

<p class="example-label example-label--implemented"><strong>Runnable example · Implemented</strong></p>

```josh
prefix = "project"
fn prompt() { return prefix + "> " }
```

Dedicated startup files are ordinary Josh source and may use `source` for explicit includes; module loading stays excluded.
