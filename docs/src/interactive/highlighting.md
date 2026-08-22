# Highlighting

<a id="J-REPL-002"></a>
## Lossless token highlighting

Josh styles exact token slices, preserving all whitespace and comments. Comments, strings, values, variables, keywords, unsupported tokens, and expression-mode punctuation receive distinct styles. Each literal command head is green when the immutable PATH/builtin snapshot resolves it and red otherwise. Validity styling covers complete command words such as `local-tool` and executable paths such as `./local-tool`, even when the lexer splits the word into several tokens.

Highlighting never executes code. It may be stale until the next accepted line if PATH changes during an edit. Nested interpolation semantics are preserved in the AST, but a double-quoted source region remains one concrete token in this slice, so its inner pieces do not receive full independent token colors.

Color is advisory. Parse mode and diagnostics remain authoritative, and no documented distinction depends only on terminal color.
