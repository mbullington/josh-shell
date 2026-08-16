# Functions, arrows, and UFCS

<div class="status-coverage">

**Status coverage:** [J-EXPR-002](../status/matrix.md#J-EXPR-002) — **Implemented**; [J-FUNC-001](../status/matrix.md#J-FUNC-001) — **Implemented**. See [status conventions](../welcome/status-conventions.md).

</div>

<a id="J-EXPR-002"></a>
## Function-shaped syntax <span class="status status--implemented" aria-label="Status: Implemented">Implemented</span>

**Availability:** Available in Josh 0.1.0. Evidence: AST and evaluator tests for calls, member/index access, arrows, declarations, spread, and destructuring.

Calls require adjacency: `value(args)`, `value.member`, and `value[index]`. A space before `(` instead makes the parenthesized expression a command argument. Arrows accept one bare parameter or a parenthesized parameter list. Their body is one expression or a block.

<a id="J-FUNC-001"></a>
## Function execution and UFCS <span class="status status--implemented" aria-label="Status: Implemented">Implemented</span>

**Availability:** Available in Josh 0.1.0. Evidence: closure, recursion, method, command-position, and UFCS tests.

`fn name(params) { ... }` and arrow functions share one Function value. A closure copies all visible lexical bindings when it is created; later reassignment outside the closure does not change that snapshot. Named functions rebind themselves while called, so direct recursion works without cyclic ownership. Declarations are source-ordered, not hoisted, and mutual forward recursion is unavailable.

Parameters allow nested array/object destructuring and one trailing rest pattern. Missing arguments bind Null; extra arguments are ignored. `...array` expands a call argument list. Spreading any other value is a type error. `return` without a value returns Null. `break` and `continue` cannot cross a function boundary.

A visible lexical function in command position runs before PATH lookup and receives evaluated command arguments. Member-call dispatch has this fixed order:

1. a builtin method for the receiver's type;
2. a visible lexical function with the member name, called as `name(receiver, ...args)` (UFCS);
3. a callable object member.

A builtin method therefore wins over a same-named lexical function. Josh has no `this`, prototypes, classes, hoisting, or mutable global method table.

<p class="example-label example-label--implemented"><strong>Runnable example · Implemented</strong></p>

```josh
x = 10
snapshot = y => x + y
x = 100
fn suffix(value, tail) { return value + tail }
printf '%s %s\n' (snapshot(1)) ("ok".suffix("!"))
```

## Finite builtin method set

Methods are nonmutating. An exact argument-count or argument-type mismatch is a type error. `contains` and `includes` are aliases.

| Receiver | Member or method | Exact result |
|---|---|---|
| String | `.length`, `.length()` | Count Unicode scalar values |
| String | `.contains(part)`, `.includes(part)` | Literal substring test |
| String | `.startsWith(part)`, `.endsWith(part)` | Literal prefix/suffix test |
| String | `.split(separator)` | Array of literal splits; an empty separator splits into Unicode scalar strings |
| String | `.replace(from, to)` | Replace the first literal match |
| String | `.replaceAll(from, to)` | Replace every literal match |
| String | `.trim()` | Remove leading and trailing Unicode whitespace |
| String | `.toUpperCase()`, `.toLowerCase()` | Unicode case conversion |
| String | `.at(index)` | One Unicode scalar String; negative indexes count from the end; otherwise Null |
| Array | `.length`, `.length()` | Element count |
| Array | `.at(index)` | Element at a signed index, otherwise Null |
| Array | `.contains(value)`, `.includes(value)` | Structural equality search |
| Array | `.map(fn)` | Array of `fn(item, index, array)` results |
| Array | `.filter(fn)` | Original items whose `fn(item, index, array)` result is truthy |
| Array | `.reduce(fn[, initial])` | Calls `fn(accumulator, item, index, array)`; an empty Array without `initial` errors |
| Array | `.flat([depth])` | Flatten nested Arrays; default depth 1; depth must be a nonnegative Int |
| Array | `.join([separator])` | Scalar-string elements joined with `,` or the String separator |
| Array | `.slice([start[, end]])` | Half-open signed/clamped range copied into a new Array |
| Object | `.keys()` | Keys in insertion order |
| Object | `.entries()` | `[key, value]` pairs in insertion order |

Bytes expose `.length` as a byte count but have no methods. Unknown Object members return Null; Array, String, and Bytes indexes accept signed Int indexes. Object indexes accept String keys or Int keys converted to decimal text.
