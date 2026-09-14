# Editing and multiline input

<a id="J-REPL-001"></a>
## Parser-driven editing

The main prompt is `josh> ` and the continuation prompt is `...> `. Reedline handles cursor movement and ordinary editing. On validation, Josh parses the whole buffer once and requests continuation only for `Completeness::Incomplete`.

<p class="example-label"><strong>Runnable example</strong></p>

```console
josh> (1 +
...> 2)
3
```

An open quote, delimiter, interpolation, block, trailing pipe, or missing EOF operand can be Incomplete. A hard error such as an unexpected closer is Invalid and submits immediately so the normal diagnostic path can report it; the REPL does not trap the user in multiline input.

Whole-buffer reparsing is intentional for short interactive buffers. Every span is a UTF-8 byte range, and completion floors an incoming cursor to a character boundary before constructing replacement spans.

## Click to position the cursor

In terminals supporting Kitty's OSC 133 `click_events` extension, including compatible Ghostty builds, click command text without modifiers to move the editing cursor. Josh advertises `click_events=1` and lets Reedline map the terminal coordinates to the buffer. This handles wrapped lines, Unicode text, indentation, and the `...> ` continuation indicator. Clicks on prompt text do not move the cursor.

Josh closes the input region before executing a command and finishes the prompt on cancellation or exit. It does not enable terminal-wide mouse capture. Terminals without this extension retain their existing mouse behavior.

The PTY tests `prompt_clicks_move_cursor` and `prompt_markers_bound_execution` cover these interactions and the OSC 133 command boundaries.
