# Hints, history, and completion

<a id="J-REPL-003"></a>
## Completion

Syntax highlighting in the REPL resolves colors from a TextMate theme when `JOSH_THEME` points at a `.tmTheme` file, falling back to a built-in ANSI palette. Theme colors are looked up with TextMate scope selectors (the same scope names the VS Code grammar in `editors/vscode` emits), and each line's command heads are colored by whether the command resolves on the session `PATH`. `JOSH_THEME` failures print a warning and use the fallback palette.

At a statement or post-pipe command head, completion searches builtins, visible lexical functions, and executable names indexed from the session's `env.PATH`. A command-position prefix that contains `/` uses file completion for executable paths. Relative PATH entries resolve against the session cwd, so `cd` and PATH edits apply on the next Tab. After `$`, completion searches Josh bindings and inherited environment names. Elsewhere, it lists matching files from the session cwd and appends `/` to directories. `./`, `../`, and deeper relative prefixes remain in the inserted candidate. Dotfiles appear only after the current path component starts with `.`. A leading `~` (bare or followed by `/…`) is resolved against the session's `HOME` while the inserted text keeps the tilde form, mirroring command execution. File suggestions close an open quote or backslash-escape shell punctuation and whitespace. Results are prefix-based, sorted where file traversal permits, capped at 200, and never evaluate source.

The replacement span uses UTF-8 byte offsets and covers the whole word when the cursor is in the middle, including an existing closing quote. Completion does not add a second space when whitespace already follows the word. Command and file completions request trailing whitespace except for directories (which end in `/`); variable completions do not.

Tab accepts a single match immediately. With multiple matches for a typed prefix, the first candidate stays inline while the menu opens, matching Fish's completion preview; alternatives use muted gray, with the selection bold and underlined. Further Tab presses or the arrow keys move the menu selection, Enter accepts it, and Escape dismisses the menu. Without an inline candidate, Tab inserts the common prefix before opening the menu. If nothing matches, Tab leaves the line unchanged.

### Typeahead hints

Behind the cursor, muted-gray ghost text offers the likely rest of the line, accepted whole with Right/Ctrl+F or one word with Alt+Right/Ctrl+Right. History and native completion hints use the same subdued treatment as menu alternatives. The most recent history entry with the typed prefix wins, except that a stale history path cannot override file candidates from the current session cwd. When no usable history entry matches, the remainder of the first native Tab candidate is offered instead. Completion hints never spawn carapace, and they are suppressed mid-line, inside comments, and without a word prefix.

### Command-specific completion via carapace

When an external `carapace` binary resolves on the session PATH, argument completion for an external command asks `carapace <name> export <name> <args…current-word>` from the session cwd and environment, then uses its JSON values (with descriptions, and `nospace` suffixes suppressing the trailing space). Every failure — the binary is absent, a spec for the command does not exist, the call errors, or the answer is empty — falls back silently to native file completion; there are no diagnostics. `JOSH_CARAPACE=0` disables the bridge entirely, and `JOSH_CARAPACE=/path/to/carapace` pins a specific binary. First words that are paths, `$variables`, or assignments never reach carapace.

<a id="J-REPL-004"></a>
## Hints and history

Prefix hints come from up to 10,000 plain file-backed history entries. Set `JOSH_HISTORY` to select a path and `JOSH_HISTORY_SIZE` to change the cap. Otherwise Josh uses `$HOME/.josh_history`, falling back to `.josh_history` in the current directory when HOME is absent.

Ctrl+R opens the history menu: a reverse-chronological, deduplicated list of entries containing the typed substring. Typing refilters the list, arrows navigate it, and Enter copies the selected entry into the buffer.

There is no SQLite database, remote sync, history schema, or semantic ranking in this slice. Treat the history file as sensitive because commands may contain paths or literal data.
