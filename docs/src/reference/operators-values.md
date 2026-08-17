# Operators and values

<a id="J-EXPR-001"></a>
## Expression values

Values are Null, Bool, Int, Float, String, Bytes, Array, insertion-ordered Object, Function, Error, and Status. Arrays/objects are Arc-backed. Object overwrite retains the original key position. Data, errors, and statuses compare structurally; user functions compare by identity.

False, Null, numeric zero, empty String/Bytes/Array/Object, and failed Status are falsey. True, nonempty data, Function, Error, and successful Status are truthy. `typeof` returns `null`, `bool`, `int`, `float`, `string`, `bytes`, `array`, `object`, `function`, `error`, or `status`.

| Operators | Behavior |
|---|---|
| `!`, unary `-` | Truthiness negation; numeric negation |
| `+` | Same-kind numeric addition or String concatenation |
| `-`, `*`, `/`, `//`, `%` | Checked numeric operations; `/` may produce Float |
| `<`, `<=`, `>`, `>=` | Numeric/string ordering where defined |
| `===`, `!==` | Runtime value equality |
| expression `&&`, `||` | Short-circuit and return an operand |
| `condition ? left : right` | Short-circuit ternary |

Arrays/objects support literals and spread. `let` and parameters support nested array/object destructuring with trailing rest; missing values bind Null. Arrays, Bytes, and strings accept signed Int indexes; Objects accept String keys or Int keys converted to decimal text. Missing and out-of-range indexes fall back to the type's prototype table before returning Null.

`if` and `try` are value-producing in expression position: `let x = if 40 < 42 { "yes" } else { "no" }` binds `"yes"`, and `try { ... }` produces the body's value or the thrown error object (the status `e` variable still binds inside `catch`).

`object.key = value` and `object[index] = value` assign existing or new fields on object values in place and evaluate to the assigned value; they are errors on other kinds.

<a id="J-NAMES-001"></a>
## Builtin namespaces and prototypes

Scalar converters live as namespace callables: `String(1)`, `Number("42")`, `Boolean([])`, `Array(...)`, `error(...)`, and `glob("pattern")`. Calling `Object(...)` is a type error; objects come from literals. `Number(value)` keeps Josh's conversion rules (`int`/`float` arguments; strings choose Int when the text is integral, otherwise Float, then error) and rejects bool, null, object, function, error, and status inputs.

Methods sit on prototype tables owned by the namespaces (full method lists live in [prototypes and namespaces](../language/prototypes-namespaces.md)). `String.prototype` carries `at`/`contains`/`startsWith`/`endsWith`/`split`/`replace`/`replaceAll`/`trim`/`toUpperCase`/`toLowerCase`, `Number.prototype` carries `abs`/`ceil`/`floor`/`round`/`norm`, and `Array.prototype` carries `at`/`contains`/`map`/`filter`/`reduce`/`flat`/`join` plus `push`/`pop`/`reverse`/`sort`, which edit the array in place JavaScript-style — every alias observes the edit; `push` returns the new length, `pop` the removed element (null when empty), and `reverse`/`sort` the array itself. `.length` answers directly on arrays, strings, and bytes as a builtin member read (not a prototype function). Extending PATH is three steps because `env.PATH` reads materialize a fresh array: `paths = env.PATH; paths.push("/new/dir"); env.PATH = paths`. Method lookup is own fields first, then the receiver's custom prototype chain, then its type prototype, then Null. Prototype entries receive the receiver as the first argument (creators use `(this, ...)`-style parameters); `+` does not dispatch through tables. Object literals chain off the root `Object.prototype`, which currently starts empty — enumeration goes through the `Object.keys`/`Object.values`/`Object.entries` statics until the root table grows members.

`Object.keys(o)`, `Object.values(o)`, `Object.entries(o)`, `Object.freeze(o)`, and `Object.isFrozen(o)` cover statics; `Object.freeze` seals an object against member assignment, and `Object.isFrozen` reports it. `Object.prototype` itself is immutable, so anonymous objects cannot mutate the shared tables.

`Date.now()` returns epoch-milliseconds Int, `Date.fromTimestamp(value)` parses epoch numbers or RFC3339 timestamps into `{timestamp millis, epochSeconds, timezone, text, datetimeComponents}`, and `Date.toTimestamp(value)` formats them back to ISO-8601 UTC (Int/Float/text input). `File.exists(path)` / `File.stat(path)` report basic cwd-relative file metadata as `{exists, file, directory, byteSize}`; null/empty paths stay false. `Math` exposes `PI`, `E`, `TAU`, abs/sign/truncate/floor/ceil/round (Int-preserving), sqrt/cbrt/exp/log/log2/log10/pow (Float), min/max variadic, and `random()` (Float [0,1)).

<a id="J-UNICODE-001"></a>
## String indexing unit

String indexing counts UTF-16 code units (JavaScript semantics), not UTF-8 bytes or grapheme clusters: `"😀".length` is 2. Josh strings are Rust strings and can only hold whole code points, so a position inside a surrogate pair resolves outward to the full code point — `"😀ab"[0]` and `"😀ab"[1]` are both `"😀"` instead of JS's lone surrogates. Negative indexes count from the end. This policy applies to `length`, `at`, bracket indexing, and range slices; unsupported positions return Null.

<a id="J-EXPR-004"></a>
## Range slices

`a[b..c]` slices an array or a string with JavaScript `slice()` bound semantics:

<p class="example-label"><strong>Runnable example</strong></p>

```josh
letters = ["a", "b", "c", "d", "e"]
letters[0..2]    # ["a", "b"] — end-exclusive
letters[2..]     # ["c", "d", "e"] — open end
letters[..2]     # ["a", "b"] — open start
letters[..]      # full copy (mutating it cannot touch the original)
letters[-2..]    # ["d", "e"] — negative counts from the end
"hello"[1..3]    # "el"
"😀ab"[0..1]     # "😀" — bounds snap outward to whole code points
```

Out-of-range bounds clamp to the edges, and an inverted pair produces an empty slice. Slice bounds must be numbers (floats truncate toward zero); bytes do not support slicing yet. Inclusive `..=` and stride ranges are deliberately rejected — `a[0..2]` already covers exactly indices 0 and 1, and `Array.prototype.slice` (or the range form) stays the only slicing mechanism.
