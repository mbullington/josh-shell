# Lexical modes and grammar

<div class="status-coverage">

**Status coverage:** [J-PARSE-001](../status/matrix.md#J-PARSE-001) — **Implemented**; [J-PARSE-002](../status/matrix.md#J-PARSE-002) — **Implemented**; [J-PARSE-004](../status/matrix.md#J-PARSE-004) — **Implemented**. See [status conventions](../welcome/status-conventions.md).

</div>

This descriptive EBNF summarizes the implemented slice; the hand-written parser is authoritative. Newline separates command statements and is trivia where expression parsing permits it.

<p class="example-label example-label--implemented"><strong>Implemented grammar summary · Runnable forms only where runtime capability also exists</strong></p>

```text
program       = { separator | statement } ;
statement     = let | assignment | function | if | while | loop | try
              | throw | return | break | continue | status
              | expression-statement | command-chain ;
command-chain = pipeline { ("&&" | "||") pipeline } ;
pipeline      = stage { "|" stage } ;
stage         = command | transformer | function-expression ;
command       = command-word { command-word | redirection | ws "(" expression ")" } ;
redirection   = (">" | ">>" | "<" | "2>" | "2>>" | "2>&1" | "&>") target ;
expression    = primary { postfix | binary expression | "?" expression ":" expression } ;
primary       = scalar | array | object | identifier | arrow | "(" expression ")" | capture | unary expression ;
postfix       = adjacent-call | adjacent-member | adjacent-index ;
```

Expression precedence, low to high: ternary; `||`; `&&`; equality; ordering; `+`/`-`; `*`/`/`/`//`/`%`; unary; adjacent call/member/index. Arrow parsing applies at the lowest expression level.

`for`, jobs, modules, and `source` are not productions. Reserved/excluded forms produce diagnostics rather than argv.

<a id="J-PARSE-004"></a>
## Completeness and strict policy <span class="status status--implemented" aria-label="Status: Implemented">Implemented</span>

**Availability:** Available in Josh 0.1.0. Evidence: EOF-classification and strict/tolerant identity tests.

`Complete` means no error diagnostics. `Incomplete` means every error is caused by EOF after appendable missing syntax. Any hard error makes the result `Invalid`. `Parse::strict_program()` accepts only Complete/no-error parses and returns references to the same AST or diagnostics; it never reparses.
