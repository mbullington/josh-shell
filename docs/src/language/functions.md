# Functions, arrows, and UFCS

<a id="J-EXPR-002"></a>
## Function-shaped syntax

Calls require adjacency: `value(args)`, `value.member`, and `value[index]`. A space before `(` instead makes the parenthesized expression a command argument. Arrows accept one bare parameter or a parenthesized parameter list. Their body is one expression or a block.

<a id="J-FUNC-001"></a>
## Function execution and UFCS

`fn name(params) { ... }` and arrow functions share one Function value. A closure copies all visible lexical bindings when it is created; later reassignment outside the closure does not change that snapshot. Named functions rebind themselves while called, so direct recursion works without cyclic ownership. Declarations are source-ordered, not hoisted, but calls resolve names when invoked, so functions defined later in the same script can call back into earlier ones (mutual recursion included). Scope is lexical: a running function sees its own parameters, its captured snapshot, and shared top-level bindings — it can neither see nor rewrite bindings owned by another active function.

Parameters allow nested array/object destructuring and one trailing rest pattern. A trailing `...name` rest parameter collects every remaining argument into an array bound to `name`; anything after it is a parse error. Missing arguments bind Null, a rest parameter with no remaining arguments binds an empty array, and without a rest parameter extra arguments are ignored. `...array` expands a call argument list. Spreading any other value is a type error. `return` without a value returns Null. `break` and `continue` cannot cross a function boundary.

<p class="example-label"><strong>Runnable example</strong></p>

```console
josh> fn head(first, ...rest) { return [first, rest.length] }
<function head>
josh> head(1, 2, 3)
[1, 2]
```

A visible lexical function in command position runs before PATH lookup and receives evaluated command arguments. Member-call dispatch has this fixed order:

1. a function stored as the object's own field, called with the call's arguments only;
2. a method in the receiver's prototype table (see [Builtin namespaces](../reference/operators-values.md#J-NAMES-001)), called with the receiver as the first argument;
3. a visible lexical function with the member name, called as `name(receiver, ...args)` (UFCS).

A prototype method therefore wins over a same-named lexical function, and an own field wins over a prototype method. Josh has no `this`, classes, or hoisting; the builtin prototype tables are the only shared method registry, and prototype methods declare the receiver explicitly with first-argument parameters (for example `(this, ...)`).

<p class="example-label"><strong>Runnable example</strong></p>

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
| String | `.length` | Count UTF-16 code units (JavaScript semantics); plain member, not callable |
| String | `.contains(part)`, `.includes(part)` | Literal substring test |
| String | `.startsWith(part)`, `.endsWith(part)` | Literal prefix/suffix test |
| String | `.split(separator)` | Array of literal splits; an empty separator splits into Unicode scalar strings |
| String | `.replace(from, to)` | Replace the first literal match |
| String | `.replaceAll(from, to)` | Replace every literal match |
| String | `.trim()` | Remove leading and trailing Unicode whitespace |
| String | `.toUpperCase()`, `.toLowerCase()` | Unicode case conversion |
| String | `.at(index)` | One code-point String at a UTF-16 unit index (pair-interior snaps to the containing code point); negative indexes count from the end; otherwise Null |
| Array | `.length`, `.length()` | Element count |
| Array | `.at(index)` | Element at a signed index, otherwise Null |
| Array | `.contains(value)`, `.includes(value)` | Structural equality search |
| Array | `.map(fn)` | Array of `fn(item, index, array)` results |
| Array | `.filter(fn)` | Original items whose `fn(item, index, array)` result is [truthy](../reference/operators-values.md#J-EXPR-005) |
| Array | `.reduce(fn[, initial])` | Calls `fn(accumulator, item, index, array)`; an empty Array without `initial` errors |
| Array | `.flat([depth])` | Flatten nested Arrays; default depth 1; depth must be a nonnegative Int |
| Array | `.join([separator])` | Scalar-string elements joined with `,` or the String separator |
| Object | `.keys()` | Keys in insertion order |
| Object | `.entries()` | `[key, value]` pairs in insertion order |

Bytes expose `.length` as a byte count but have no methods. Unknown Object members return Null; Array, String, and Bytes indexes accept signed Int indexes. Object indexes accept String keys or Int keys converted to decimal text.
