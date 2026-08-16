# Hints, history, and completion

<div class="status-coverage">

**Status coverage:** [J-REPL-003](../status/matrix.md#J-REPL-003) — **Implemented**; [J-REPL-004](../status/matrix.md#J-REPL-004) — **Implemented**. See [status conventions](../welcome/status-conventions.md).

</div>

<a id="J-REPL-003"></a>
## Completion <span class="status status--implemented" aria-label="Status: Implemented">Implemented</span>

**Availability:** Available in Josh 0.1.0. Evidence: UTF-8-safe completion-context tests and Reedline adapter construction.

At a statement or post-pipe command head, completion searches builtins and executable names indexed from the session's `env.PATH` — including entries relative to the session cwd — so `cd` and PATH edits apply on the very next Tab. After `$`, it searches Josh bindings and inherited environment names. Elsewhere, it lists matching files and appends `/` to directories. Results are prefix-based, sorted where file traversal permits, capped at 200, and never evaluate source.

The replacement span uses UTF-8 byte offsets. Command/file completions request trailing whitespace; variable completions do not.

### Command-specific completion via carapace

When an external `carapace` binary resolves on PATH, argument completion for an external command asks `carapace <name> export <name> <args…current-word>` and uses its JSON values (with descriptions, and `nospace` suffixes suppressing the trailing space). Every failure — the binary is absent, a spec for the command does not exist, the call errors, or the answer is empty — falls back silently to native file completion; there are no diagnostics. `JOSH_CARAPACE=0` disables the bridge entirely, and `JOSH_CARAPACE=/path/to/carapace` pins a specific binary. First words that are paths, `$variables`, or assignments never reach carapace.

<a id="J-REPL-004"></a>
## Hints and history <span class="status status--implemented" aria-label="Status: Implemented">Implemented</span>

**Availability:** Available in Josh 0.1.0. Evidence: Reedline is configured with `FileBackedHistory` and `DefaultHinter`.

Prefix hints come from up to 1,000 plain file-backed history entries. Set `JOSH_HISTORY` to select a path. Otherwise Josh uses `$HOME/.josh_history`, falling back to `.josh_history` in the current directory when HOME is absent.

There is no SQLite database, remote sync, history schema, or semantic ranking in this slice. Treat the history file as sensitive because commands may contain paths or literal data.
