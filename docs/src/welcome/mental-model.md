# Josh's model

<div class="status-coverage">

**Status coverage:** [J-PARSE-002](../status/matrix.md#J-PARSE-002) — **Implemented**; [J-STRUCT-001](../status/matrix.md#J-STRUCT-001) — **Implemented**. See [status conventions](status-conventions.md).

</div>

Josh classifies each statement as command mode or expression mode from source shape, tokens, and adjacency—not runtime values. A bare command head such as `git` starts command mode. Assignment or an adjacent call/member/index continuation starts expression mode.

Command mode treats words as argv. Expression mode gives operators arithmetic or logical meaning. Parentheses preceded by whitespace insert one evaluated expression into a command argument list.

External commands exchange bytes through OS pipes. Values cross that boundary only through explicit stages: `text`, `json`, `lines`, `jsonl`, `chunks(n)`, function/map/filter/take/first, and `collect`. JSON-looking bytes are never inferred as JSON.

## Shell-first consequences

- There is no implicit word splitting.
- Unquoted glob patterns expand to sorted argv entries; quotes suppress expansion.
- PATH participates in command-call resolution, after visible lexical functions.
- A failing command is an error unless a documented status context handles it.
- Pipeline planning validates every stage and redirection before spawning.
- `$(...)` preserves its declared byte/value cardinality.

Read [Command mode and expression mode](../language/modes.md) and [argv and pipeline boundaries](../reference/argv-boundaries.md).
