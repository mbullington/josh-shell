# Hints, history, and completion

<a id="J-REPL-003"></a>
## Completion

Syntax highlighting in the REPL resolves colors from a TextMate theme when `JOSH_THEME` points at a `.tmTheme` file, falling back to a built-in ANSI palette. Theme colors are looked up with TextMate scope selectors (the same scope names the VS Code grammar in `editors/vscode` emits), and each line's command heads are colored by whether the command resolves on the session `PATH`. `JOSH_THEME` failures print a warning and use the fallback palette.

At a statement or post-pipe command head, completion searches builtins and executable names indexed from the session's `env.PATH` — including entries relative to the session cwd — so `cd` and PATH edits apply on the very next Tab. After `$`, it searches Josh bindings and inherited environment names. Elsewhere, it lists matching files and appends `/` to directories. A leading `~` (bare or followed by `/…`) is resolved against the session's `HOME` while the inserted text keeps the tilde form, mirroring command execution. Results are prefix-based, sorted where file traversal permits, capped at 200, and never evaluate source.

The replacement span uses UTF-8 byte offsets. Command and file completions request trailing whitespace except for directories (which end in `/`); variable completions do not.

### Typeahead hints

Behind the cursor, ghost text offers the likely rest of the line, accepted whole with Right/Ctrl+F or one word with Alt+Right/Ctrl+Right. The most recent history entry with the typed prefix wins (fish-style autosuggestion); when no history entry matches, the remainder of the first native Tab candidate is offered instead. Completion hints never spawn carapace, and they are suppressed mid-line, inside comments, and without a word prefix.

### Command-specific completion via carapace

When an external `carapace` binary resolves on PATH, argument completion for an external command asks `carapace <name> export <name> <args…current-word>` and uses its JSON values (with descriptions, and `nospace` suffixes suppressing the trailing space). Every failure — the binary is absent, a spec for the command does not exist, the call errors, or the answer is empty — falls back silently to native file completion; there are no diagnostics. `JOSH_CARAPACE=0` disables the bridge entirely, and `JOSH_CARAPACE=/path/to/carapace` pins a specific binary. First words that are paths, `$variables`, or assignments never reach carapace.

<a id="J-REPL-004"></a>
## Hints and history

Prefix hints come from up to 10,000 plain file-backed history entries. Set `JOSH_HISTORY` to select a path and `JOSH_HISTORY_SIZE` to change the cap. Otherwise Josh uses `$HOME/.josh_history`, falling back to `.josh_history` in the current directory when HOME is absent.

Ctrl+R opens the history menu: a reverse-chronological, deduplicated list of entries containing the typed substring. Typing refilters the list, arrows navigate it, and Enter copies the selected entry into the buffer.

There is no SQLite database, remote sync, history schema, or semantic ranking in this slice. Treat the history file as sensitive because commands may contain paths or literal data.
