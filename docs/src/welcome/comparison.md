# Josh compared with other shells

Josh borrows familiar command invocation, pipelines, redirections, globbing, status chains, and functions from Unix shells, but it does not claim POSIX compatibility. The grammar and expansion rules differ, and background job control is unavailable, so POSIX shell scripts do not automatically run in Josh.

Its expression surface looks JavaScript-shaped, but Josh has no JavaScript event loop, Promise, `async`/`await`, npm, prototypes, classes, `this`, or coercive `==`. Strict equality uses `===` and `!==`, and [truthiness](../reference/operators-values.md#J-EXPR-005) diverges too: empty arrays and objects are falsy and `NaN` is truthy, each the opposite of JavaScript. Runtime semantics are designed for deterministic shell data rather than browser or Node.js objects.

Compared with data-oriented shells, Josh plans explicit byte/value boundaries. It does not inspect output and guess JSON. External-to-external pipelines stay kernel byte pipes; `text`, `json`, `lines`, `jsonl`, `chunks`, functions, and collection stages opt into documented value transitions and cardinality.

Compared with terminal screen scraping, agent-terminal treats a parsed semantic terminal grid as its automation surface. Schema-v2 snapshots remain authoritative structured state. The separate `screenshot` command deterministically renders that copied state to pixels with pinned fonts, metrics, colors, rasterizer, and encoder.
