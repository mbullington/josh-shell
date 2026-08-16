# Commands and expressions

<div class="status-coverage">

**Status coverage:** [J-PARSE-002](../status/matrix.md#J-PARSE-002) — **Implemented**; [J-EXPR-001](../status/matrix.md#J-EXPR-001) — **Implemented**; [J-CF-001](../status/matrix.md#J-CF-001) — **Implemented**. See [status conventions](../welcome/status-conventions.md).

</div>

At statement position, `printf hello` is a command: every bare word is argv text. `(20 + 22)` begins an expression. Assignment also selects expression mode for its right-hand side.

<p class="example-label example-label--implemented"><strong>Runnable example · Implemented</strong></p>

```console
josh> total = 20 + 22
42
josh> printf '%s\n' (total * 2)
84
```

Whitespace matters in the second command. A parenthesized expression preceded by whitespace supplies one command argument. By contrast, `name(1)` is an implemented function call because the parenthesis is adjacent.

Expression mode evaluates Null, booleans, numbers, strings, arrays, objects, functions, member/index access, calls, spread, ternary expressions, unary `!` and `-`, arithmetic, ordering, strict equality, and short-circuit `&&`/`||`. Expression `&&` and `||` return one operand according to truthiness.

At command level, `&&` and `||` form a status chain rather than argv. They run the right pipeline after success or failure respectively. A completed nonzero status is handled by the chain; lookup, interpolation, spawn, and type errors still propagate.

<p class="example-label example-label--implemented"><strong>Runnable example · Implemented</strong></p>

```console
josh> sh -c 'exit 7' || printf 'recovered\n'
recovered
josh> sh -c 'exit 0' && printf 'continued\n'
continued
```

Use parentheses when arithmetic at statement position could look like a command. `x - 1` invokes command `x` with two arguments; `(x - 1)` performs subtraction after resolving expression variable `x`.
