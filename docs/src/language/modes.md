# Command mode and expression mode

<a id="J-PARSE-002"></a>
## Deterministic mode selection

At statement position, a shell word that is not exactly a JavaScript identifier starts a command. An identifier starts assignment when the next significant token is `=`, `+=`, or `-=`. It starts an expression for an adjacent `(`, `.`, or `[`, or for `=>` with or without trivia. `let` and `if` route to dedicated productions. `for` is an ordinary command word, not a keyword.

Command mode makes unquoted `|`, newline, and `;` structural even without spaces. Whitespace followed by `(` starts one evaluated expression argument. `$(` enters a nested command pipeline; single quotes are literal; double quotes support `$name`, `${expression}`, and `$(pipeline)`.

Expression mode implements a strict JavaScript-shaped subset. `===` and `!==` exist; `==` is an Invalid parse with a suggestion to use `===`. Parsing depends only on source, token mode, and adjacency. Name lookup never changes the parse.
