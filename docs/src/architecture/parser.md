# Parser and parse result

<a id="J-PARSE-001"></a>
## One lossless tolerant parse

The public operation returns `Parse { source, tokens, program, diagnostics, completeness }`. Source is shared `Arc<str>`. Concrete tokens include trivia, lexical mode, and UTF-8 byte span; token slices partition source exactly. Semantic AST nodes have spans and can represent zero-width Missing or source-covering Error nodes.

One lexer and recursive-descent/Pratt parser serve tolerant callers and strict execution. Statement-head lookahead inspects the same token sequence. The parser commits each consumed token's mode as productions choose Command, Expression, SingleQuote, or DoubleQuote.

Recovery preserves owning delimiters. EOF-only appendable errors classify Incomplete; a hard error makes the whole parse Invalid. Diagnostics encode stable code, severity, expected set, primary/secondary labels, and whether EOF caused the error.

The slice intentionally has no Rowan tree, token-text copies, incremental reparse, formatter CST, or separate strict parser. Short REPL buffers are reparsed whole; future formatting/refactoring evidence may justify a lossless green-tree sink later.
