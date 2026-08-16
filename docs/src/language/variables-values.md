# Variables, environment, and values

<div class="status-coverage">

**Status coverage:** [J-RUN-005](../status/matrix.md#J-RUN-005) — **Implemented**; [J-ENV-001](../status/matrix.md#J-ENV-001) — **Implemented**; [J-ENV-002](../status/matrix.md#J-ENV-002) — **Specified**. See [status conventions](../welcome/status-conventions.md).

</div>

<a id="J-RUN-005"></a>
## Bindings and assignment <span class="status status--implemented" aria-label="Status: Implemented">Implemented</span>

**Availability:** Available in Josh 0.1.0. Evidence: lexical-frame, destructuring, assignment, closure, and capture-commit tests.

`let` binds in the current lexical frame. Plain assignment updates the nearest visible binding or creates one in the current frame. `+=` and `-=` apply the corresponding checked operator. Blocks create child frames. A failed right-hand capture does not commit assignment.

Array/object destructuring is available in `let` and function parameters, including nested and trailing rest patterns. Destructuring assignment is not implemented.

<p class="example-label example-label--implemented"><strong>Runnable example · Implemented</strong></p>

```josh
let {name, ...rest} = {name: "Josh", version: 1}
let [first, ...tail] = [1, 2, 3]
```

<a id="J-ENV-001"></a>
## Environment fallback <span class="status status--implemented" aria-label="Status: Implemented">Implemented</span>

**Availability:** Available in Josh 0.1.0. Evidence: interpolation resolves lexical bindings before inherited environment variables.

<a id="environment-boundary"></a>
In `$NAME` command/string interpolation, Josh checks lexical bindings, then the inherited process environment. Plain assignment does not export.

<a id="J-ENV-002"></a>
## The `env` namespace <span class="status status--specified" aria-label="Status: Specified">Specified</span>

**Availability:** Accepted contract; not available in the current build. `env.FOO` will read or write process environment while plain assignment remains local.

<p class="example-label example-label--specified"><strong>Specified syntax · Not implemented</strong></p>

```josh
env.FOO = "visible to children"
```
