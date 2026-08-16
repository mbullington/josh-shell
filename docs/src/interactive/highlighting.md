# Highlighting

<div class="status-coverage">

**Status coverage:** [J-REPL-002](../status/matrix.md#J-REPL-002) — **Implemented**. See [status conventions](../welcome/status-conventions.md).

</div>

<a id="J-REPL-002"></a>
## Lossless token highlighting <span class="status status--implemented" aria-label="Status: Implemented">Implemented</span>

**Availability:** Available in Josh 0.1.0. Evidence: the Reedline highlighter consumes the public lossless parse and completion snapshot.

Josh styles exact token slices, preserving all whitespace and comments. Comments, strings, values, variables, keywords, unsupported tokens, and expression-mode punctuation receive distinct styles. The first literal command is green when the immutable PATH/builtin snapshot knows it and red otherwise.

Highlighting never executes code. It may be stale until the next accepted line if PATH changes during an edit. Nested interpolation semantics are preserved in the AST, but a double-quoted source region remains one concrete token in this slice, so its inner pieces do not receive full independent token colors.

Color is advisory. Parse mode and diagnostics remain authoritative, and no documented distinction depends only on terminal color.
