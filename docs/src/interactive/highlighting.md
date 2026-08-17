# Highlighting

<a id="J-REPL-002"></a>
## Lossless token highlighting

Josh styles exact token slices, preserving all whitespace and comments. Comments, strings, values, variables, keywords, unsupported tokens, and expression-mode punctuation receive distinct styles. The first literal command is green when the immutable PATH/builtin snapshot knows it and red otherwise.

Highlighting never executes code. It may be stale until the next accepted line if PATH changes during an edit. Nested interpolation semantics are preserved in the AST, but a double-quoted source region remains one concrete token in this slice, so its inner pieces do not receive full independent token colors.

Color is advisory. Parse mode and diagnostics remain authoritative, and no documented distinction depends only on terminal color.
