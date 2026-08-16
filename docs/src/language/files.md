# Files, redirections, and globs

<div class="status-coverage">

**Status coverage:** [J-FILES-001](../status/matrix.md#J-FILES-001) — **Implemented**; [J-TILDE-001](../status/matrix.md#J-TILDE-001) — **Implemented**. See [status conventions](../welcome/status-conventions.md).

</div>

<a id="J-FILES-001"></a>
## Redirections and glob expansion <span class="status status--implemented" aria-label="Status: Implemented">Implemented</span>

**Availability:** Available in Josh 0.1.0. Evidence: descriptor-order, pre-spawn validation, glob policy, and cross-product tests.

Redirections attach to the external command stage they follow:

| Form | Stage descriptor action |
|---|---|
| `< path` | Open path for stdin |
| `> path` | Create/truncate path for stdout |
| `>> path` | Create/append path for stdout |
| `2> path` | Create/truncate path for stderr |
| `2>> path` | Create/append path for stderr |
| `2>&1` | Duplicate the stage's current stdout onto stderr |
| `&> path` | Create/truncate one path and use the cloned descriptor for stdout and stderr |

Descriptor actions apply left to right, so `> file 2>&1` sends both streams to the file while `2>&1 > file` leaves stderr on the stdout destination that existed before the file redirect.

Josh first evaluates every stage, expands argv/globs, and resolves every external executable. It then opens redirection files in stage and source order. Only after all preflight succeeds does any command spawn. Preflight is not a filesystem transaction: an earlier output may already be created or truncated when a later open fails.

A target must evaluate to exactly one path. Array/object targets error. An unquoted glob target is allowed only when it matches exactly one path; zero matches is a glob error and multiple matches violate the one-path rule. Redirections on `cd`, `exit`, lexical functions, or in-shell transformer stages are unsupported.

<p class="example-label example-label--implemented"><strong>Runnable example · Implemented</strong></p>

```josh
sh -c 'printf out; printf err >&2' > output.txt 2>&1
sh -c 'printf out; printf err >&2' 2>&1 > stdout-only.txt
```

Unquoted command words expand `*`, `?`, bracket classes, and `**` before invocation. Matching is case-sensitive; path separators and leading dots must be matched literally. Results sort by path bytes, and each match becomes one argv item without word splitting. No match is an error. Quotes and backslash escapes suppress the quoted/escaped metacharacters while leaving other unquoted metacharacters active.

`glob(pattern)` accepts one String, applies the same matching, sorting, and no-match policy, and returns an Array of String paths on valid UTF-8 platforms or Bytes where a Unix path is not valid UTF-8.

<p class="example-label example-label--implemented"><strong>Runnable example · Implemented</strong></p>

```josh
printf '%s\n' **/*.md
printf '%s\n' '*.md'
files = glob("crates/**/*.rs")
```

REPL file completion and execution-time glob expansion are separate features.

<a id="J-TILDE-001"></a>
## Tilde expansion <span class="status status--implemented" aria-label="Status: Implemented">Implemented</span>

**Availability:** Available in the current development snapshot. Evidence: lexer and expansion tests for `~` and `~/` at word starts.

An unquoted leading `~` alone or followed by `/` expands to the session's home directory: the session `HOME` when exported, else the per-process home at startup. Other forms (`~user`, `~+`, mid-word tildes) never expand and stay literal text.

<p class="example-label example-label--implemented"><strong>Runnable example · Implemented</strong></p>

```josh
cd ~/Projects
cfg = "~/config/app.toml"
```
