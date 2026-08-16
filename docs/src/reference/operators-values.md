# Operators and values

<div class="status-coverage">

**Status coverage:** [J-EXPR-001](../status/matrix.md#J-EXPR-001) — **Implemented**; [J-UNICODE-001](../status/matrix.md#J-UNICODE-001) — **Implemented**. See [status conventions](../welcome/status-conventions.md).

</div>

<a id="J-EXPR-001"></a>
## Implemented expression values <span class="status status--implemented" aria-label="Status: Implemented">Implemented</span>

**Availability:** Available in Josh 0.1.0. Evidence: parser/evaluator tests for values, operators, access, spread, destructuring, and conversions.

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

Arrays/objects support literals and spread. `let` and parameters support nested array/object destructuring with trailing rest; missing values bind Null. Arrays, Bytes, and strings accept signed Int indexes; Objects accept String keys or Int keys converted to decimal text. Missing and out-of-range indexes return Null.

<a id="J-UNICODE-001"></a>
## String indexing unit <span class="status status--implemented" aria-label="Status: Implemented">Implemented</span>

**Availability:** Available in Josh 0.1.0. Evidence: Unicode string `length`, `at`, and signed-index tests.

String indexing counts Unicode scalar values, not UTF-8 bytes or grapheme clusters. Negative indexes count from the end. This policy applies to `length` and `at`; unsupported scalar positions return Null.
