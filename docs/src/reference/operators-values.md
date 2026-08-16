# Operators and values

<div class="status-coverage">

**Status coverage:** [J-EXPR-001](../status/matrix.md#J-EXPR-001) — **Implemented**; [J-NAMES-001](../status/matrix.md#J-NAMES-001) — **Implemented**; [J-UNICODE-001](../status/matrix.md#J-UNICODE-001) — **Implemented**. See [status conventions](../welcome/status-conventions.md).

</div>

<a id="J-EXPR-001"></a>
## Implemented expression values <span class="status status--implemented" aria-label="Status: Implemented">Implemented</span>

**Availability:** Available in Josh 0.1.0. Evidence: parser/evaluator tests for values, operators, access, spread, destructuring, `if`/`try` expressions, member assignment, and namespace converters.

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
## Builtin namespaces and prototypes <span class="status status--implemented" aria-label="Status: Implemented">Implemented</span>

**Availability:** Available in Josh 0.1.0. Evidence: `prototype_namespaces_methods_and_statics_are_first_class`, `file_date_and_math_namespaces_cover_the_flattened_surface`, and the namespace runtime tests.

Scalar converters live as namespace callables: `String(1)`, `Number("42")`, `Boolean([])`, `Array(...)`, `error(...)`, and `glob("pattern")`. Calling `Object(...)` is a type error; objects come from literals. `Number(value)` keeps Josh's conversion rules (`int`/`float` arguments; strings choose Int when the text is integral, otherwise Float, then error) and rejects bool, null, object, function, error, and status inputs.

Methods sit on prototype tables: `String.prototype`, `Number.prototype`, `Array.prototype`, `Boolean.prototype`, and `Function.prototype` cover case/trim/split/join/slices/parse/push/pop/reverse/sort/length/iterate/each/map/filter/reduce/toString and the other runtime helpers. Method lookup is own fields first, then the type's prototype, then Null. Prototype entries receive the receiver as the first argument (creators use `(this, ...)`-style parameters); `+` does not dispatch through tables. Object literals chain off `Object.prototype` so objects that have not shadowed them get `keys`/`values`/`iterate`/`map`/`each`/`toString`.

`Object.keys(o)`, `Object.values(o)`, `Object.entries(o)`, `Object.freeze(o)`, and `Object.isFrozen(o)` cover statics; `Object.freeze` seals an object against member assignment, and `Object.isFrozen` reports it. `Object.prototype` itself is immutable, so anonymous objects cannot mutate the shared tables.

`Date.now()` returns epoch-milliseconds Int, `Date.fromTimestamp(value)` parses epoch numbers or RFC3339 timestamps into `{timestamp millis, epochSeconds, timezone, text, datetimeComponents}`, and `Date.toTimestamp(value)` formats them back to ISO-8601 UTC (Int/Float/text input). `File.exists(path)` / `File.stat(path)` report basic cwd-relative file metadata as `{exists, file, directory, byteSize}`; null/empty paths stay false. `Math` exposes `PI`, `E`, `TAU`, abs/sign/truncate/floor/ceil/round (Int-preserving), sqrt/cbrt/exp/log/log2/log10/pow (Float), min/max variadic, and `random()` (Float [0,1)).

<a id="J-UNICODE-001"></a>
## String indexing unit <span class="status status--implemented" aria-label="Status: Implemented">Implemented</span>

**Availability:** Available in Josh 0.1.0. Evidence: Unicode string `length`, `at`, and signed-index tests.

String indexing counts Unicode scalar values, not UTF-8 bytes or grapheme clusters. Negative indexes count from the end. This policy applies to `length` and `at`; unsupported scalar positions return Null.
