# Lexical modes and grammar

This descriptive EBNF summarizes the implemented slice; the hand-written parser is authoritative. Newline separates command statements and is trivia where expression parsing permits it.

<p class="example-label"><strong>Grammar summary</strong></p>

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
## Completeness and strict policy

`Complete` means no error diagnostics. `Incomplete` means every error is caused by EOF after appendable missing syntax. Any hard error makes the result `Invalid`. `Parse::strict_program()` accepts only Complete/no-error parses and returns references to the same AST or diagnostics; it never reparses.
