# Variables, environment, and values

<a id="J-RUN-005"></a>
## Bindings and assignment

`let` binds in the current lexical frame. Plain assignment updates the nearest visible binding or creates one in the current frame. `+=` and `-=` apply the corresponding checked operator. Blocks create child frames. A failed right-hand capture does not commit assignment.

Array/object destructuring is available in `let` and function parameters, including nested and trailing rest patterns. Destructuring assignment is not implemented.

Reserved words are valid member names and property keys wherever a name is expected rather than a reference: `value.status`, `{ status: "ok" }`, and `let { status: s } = error` all parse. The shorthand forms remain parse errors — `{ status }` would have to mean a reference or binding named `status`.

<p class="example-label"><strong>Runnable example</strong></p>

```josh
let {name, ...rest} = {name: "Josh", version: 1}
let [first, ...tail] = [1, 2, 3]
```

<a id="J-ENV-001"></a>
## Environment fallback

<a id="environment-boundary"></a>
In `$NAME` command/string interpolation, Josh checks lexical bindings, then the session environment (the inherited process environment plus any `env` mutations). Plain assignment does not export.

<a id="J-ENV-002"></a>
## The `env` namespace

`env.NAME` reads and writes the session environment; writes stay scoped to the shell's snapshot (no process-global `setenv`), and child pipeline processes inherit the member state at spawn. Values are text-centric: reads decode lossy, invalid UTF-8 preserved through the bytes view. Closures read `env` dynamically, so a later write is visible inside previously created functions. `env` cannot be shadowed by lexical bindings of the same name.

PATH is first-class: `env.PATH` reads the PATH string; `env.PATH` exposes an array view of one entry per path segment when assigned an array (and PATH lookups, completion, and `cd` all follow it). Startup `env.josh` mutations (for example `env.PATH = env.PATH + [someDir]`, `env.EDITOR = ...`) persist for both command resolution and child processes. Plain `let`/assignment never exports.

<p class="example-label"><strong>Runnable example</strong></p>

```josh
env.FOO = "visible to children"
```
