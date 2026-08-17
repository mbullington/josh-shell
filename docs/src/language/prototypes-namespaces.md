# Prototypes and namespaces

<a id="J-OBJ-001"></a>
## Prototypal method lookup and builtin namespaces

Josh has no classes or implicit `this`. Objects are ordered key/value maps with an optional prototype link, and method lookup walks: the object's own fields → the object's prototype chain → the value's *type prototype*. A method found on a prototype is called with the receiver passed as the first argument.

<p class="example-label"><strong>Runnable example</strong></p>

```josh
Animal = { legs: 4 }
Animal.speak = (this) => "legs:" + String(this.legs)
cat = Object.create(Animal)   # prototype chain, not a copy
cat.name = "Miso"
cat.speak()                    # "legs:4" — receiver travels as `this`
```

Every value has a type prototype the namespaces own: `"text".toUpperCase()` resolves through `String.prototype`, `[1,2].map(...)` through `Array.prototype`, and so on. The namespaces themselves are first-class values:

| Namespace | Surface |
|---|---|
| `Object` | `keys`, `entries`, `values`, `create(proto)`, `fromEntries`, `getPrototype`, `setPrototype`, `seal`, `isSealed` |
| `String` | conversion `String(x)`; `String.prototype`: `at`, `contains`, `startsWith`, `endsWith`, `split`, `replace`, `replaceAll`, `trim`, `toUpperCase`, `toLowerCase` |
| `Number` | conversion `Number(x)`; `Number.prototype`: `abs`, `ceil`, `floor`, `round`, `norm`; constants `NaN`, `MAX_VALUE`, `MIN_VALUE`, `MAX_INT`, `MIN_INT`; `Number.isNaN(x)` (JavaScript semantics: true only for actual NaN, never coerced). `MIN_VALUE` follows JavaScript: the smallest positive denormal (5e-324), not the most-negative finite value |
| `Boolean` | conversion `Boolean(x)` |
| `Array` | conversion `Array(x)`; `Array.prototype`: `at`, `contains`, `map`, `filter`, `reduce`, `flat`, `join`, `slice`, plus `push`/`pop`/`reverse`/`sort`, which edit the array in place (JavaScript semantics: `push` returns the new length, `pop` the removed element or null, `reverse`/`sort` return the array itself) |
| `Function` | never callable: constructing functions goes through `=>` |
| `File` | `exists`, `stat` |
| `Date` | `now` (epoch milliseconds), `toLocaleString` |
| `Math` | `abs`, `cbrt`, `ceil`, `exp`, `floor`, `log`, `log2`, `log10`, `max`, `min`, `pow`, `random`, `round`, `sign`, `sqrt`, `trunc`; constants `PI`, `E` |

Rules that stay constant:

- Reading a missing member is `null`, never an error. Typo'd method calls then fail at the call with a "not callable" error pointing at the resolved null.
- Object literals start with the root prototype shared by all objects, while `Object.create(null)` opts out entirely; the root table is empty today, so object enumeration goes through `Object.keys`/`values`/`entries`.
- Arrays are shared mutable values like objects: aliases observe in-place edits, while callback methods (`map`/`filter`/`reduce`) iterate a snapshot so callbacks may push/pop freely. `sort` needs all numbers or all strings, and `.length` is a builtin member read, not a method. Extending PATH is `paths = env.PATH; paths.push("/new/dir"); env.PATH = paths` — `env.PATH` reads materialize fresh arrays, so the assignment writes PATH back.
- `Object.setPrototype` refuses cycles; `Object.seal` prevents adding new keys to that object while existing keys stay writable.
- `o.name = value` and `o[key] = value` mutate objects in place anywhere an expression position was already a statement; assignments target objects only.

<p class="example-label"><strong>Runnable example</strong></p>

```josh
cfg = { retries: 0 }
cfg.retries = 3
cfg["source"] = "user"
String.prototype.shout = (this) => this.toUpperCase() + "!"
"ok".shout()                  # "OK!"
```
