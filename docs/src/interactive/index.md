# Interactive shell

Josh embeds Reedline for line editing. The parser, not a separate delimiter counter, drives continuation, highlighting, and completion context. A completion snapshot contains lexical names, inherited environment names, builtins, and a PATH index; it is replaced between accepted lines.

The current surface includes a primary prompt, multiline prompt, token highlighting, file-backed history, prefix hints, command/file/variable completion, Ctrl-C recovery, and Ctrl-D exit. It does not include job control, SQLite history, rich output, or third-party completion specifications.
