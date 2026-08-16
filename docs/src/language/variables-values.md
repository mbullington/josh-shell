# Variables, environment, and values

<div class="status-coverage">

**Status coverage:** [J-RUN-005](../status/matrix.md#J-RUN-005) — **Implemented**; [J-ENV-001](../status/matrix.md#J-ENV-001) — **Implemented**; [J-ENV-002](../status/matrix.md#J-ENV-002) — **Implemented**. See [status conventions](../welcome/status-conventions.md).

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
In `$NAME` command/string interpolation, Josh checks lexical bindings, then the session environment (the inherited process environment plus any `env` mutations). Plain assignment does not export.

<a id="J-ENV-002"></a>
## The `env` namespace <span class="status status--implemented" aria-label="Status: Implemented">Implemented</span>

**Availability:** Available in Josh 0.1.0. Evidence: `environment_namespace_is_dynamic_exported_and_scalar_canonical`, `environment_bytes_path_views_and_validation_preserve_os_values`, and `startup_environment_mutation_persists_for_path_lookup_and_children`.

`env.NAME` reads and writes the session environment; writes stay scoped to the shell's snapshot (no process-global `setenv`), and child pipeline processes inherit the member state at spawn. Values are text-centric: reads decode lossy, invalid UTF-8 preserved through the bytes view. Closures read `env` dynamically, so a later write is visible inside previously created functions. `env` cannot be shadowed by lexical bindings of the same name.

PATH is first-class: `env.PATH` reads the PATH string; `env.PATH` exposes an array view of one entry per path segment when assigned an array (and PATH lookups, completion, and `cd` all follow it). Startup `env.josh` mutations (for example `env.PATH = env.PATH + [someDir]`, `env.EDITOR = ...`) persist for both command resolution and child processes. Plain `let`/assignment never exports.

<p class="example-label example-label--implemented"><strong>Runnable example · Implemented</strong></p>

```josh
env.FOO = "visible to children"
```
