# Capture and pipelines

<div class="status-coverage">

**Status coverage:** [J-RUN-004](../status/matrix.md#J-RUN-004) — **Implemented**; [J-RUN-003](../status/matrix.md#J-RUN-003) — **Implemented**; [J-STRUCT-001](../status/matrix.md#J-STRUCT-001) — **Implemented**; [J-STREAM-002](../status/matrix.md#J-STREAM-002) — **Implemented**. See [status conventions](../welcome/status-conventions.md).

</div>

<a id="J-RUN-004"></a>
## String and byte capture <span class="status status--implemented" aria-label="Status: Implemented">Implemented</span>

**Availability:** Available in Josh 0.1.0. Evidence: trailing-newline, invalid-UTF-8, stderr, and failed-assignment capture tests.

Without a terminal transformer, `$(pipeline)` collects final stdout, removes every trailing LF and an immediately preceding CR, returns valid UTF-8 as String, and preserves invalid UTF-8 as Bytes. Stderr stays inherited unless redirected. Capture does not inspect JSON-looking content and remains whole-buffer.

<p class="example-label example-label--implemented"><strong>Runnable example · Implemented</strong></p>

```josh
text_value = $(printf '{"answer":42}\n')
```

<a id="J-STRUCT-001"></a>
## Structured transformers <span class="status status--implemented" aria-label="Status: Implemented">Implemented</span>

**Availability:** Available in Josh 0.1.0. Evidence: graph-validation, capture-cardinality, bounded-termination, process-serialization, and cross-product tests.

Josh evaluates stages and validates the complete byte/value graph before spawning. Adjacent external commands retain direct OS byte pipes. Structured stages use bounded channels with capacity 256.

| Input → stage | Output and errors |
|---|---|
| bytes → `text` | Collect all bytes into one String; invalid UTF-8 becomes Bytes; unlike raw capture, no trailing newline is removed |
| bytes → `json` | Collect and parse exactly one JSON document into one Josh value; empty, malformed, trailing, non-finite, or out-of-range numeric input errors |
| bytes → `lines` | Stream UTF-8 String values; remove one LF and a preceding CR per line; invalid UTF-8 reports its line number |
| bytes → `jsonl` | Apply the `lines` boundary, then parse every line as one JSON value; blank or malformed lines error with a line number |
| bytes → `chunks(n)` or `chunks n` | Stream Bytes blocks up to positive Int `n`, with a planning-time maximum of 65,536 bytes; allocation is fallible and the final block may be shorter |
| bytes → function/`map` | Planning error with a hint to add `text`, `json`, `lines`, `jsonl`, or `chunks(n)` |
| values → function or `map fn` | Call once per item and emit each return value |
| values → `filter fn` | Keep an item when the function result is truthy |
| values → `take n` | Emit at most nonnegative Int `n`, then cancel upstream |
| values → `first` | Emit the first item, then cancel upstream |
| values → `collect` | Collect all items and emit one Array |
| values → `text` | Emit bytes with one LF between values and no final LF; String/Bytes stay raw and other data uses JSON |
| values → external | Write each String literally plus LF; serialize other JSON-compatible values as JSONL |

Object key insertion order is preserved through JSON parsing and serialization. Bytes, Function, Error, and Status values cannot use JSON serialization. Non-finite Float values also fail. These errors cancel the graph, join workers, and reap external children.

The terminal stage determines capture shape:

| Final shape | `$(...)` result |
|---|---|
| external bytes or values → `text` → bytes | Raw String/Bytes capture with trailing newline trimming |
| `json` or bytes → `text` | One value |
| `lines`, `jsonl`, `chunks`, `filter`, or `take` | Always Array, including zero or one item |
| function or `map` | Preserve upstream one/many cardinality |
| `first` | First value, or Null for empty input |
| `collect` | One Array, including an empty Array |

No content-based JSON inference or observed-item-count collapse occurs.

<p class="example-label example-label--implemented"><strong>Runnable example · Implemented</strong></p>

```josh
values = $(printf '1\n2\n3\n' | lines | map (x => Number(x) * 2) | filter (x => x > 2) | take 2)
```

A downstream close, including inherited stdout closing, is graceful for in-shell writers. `take` and `first` acknowledge each demanded item before allowing function workers to invoke again, then stop and join in-shell workers and external producers. Cancellation reaches external commands called by those functions. External outcomes remain ordered and use pipefail; only a proven non-final SIGPIPE caused by normal downstream close is treated as success. A deliberate SIGPIPE remains a failure.

<a id="J-STREAM-002"></a>
## Pipelines from values <span class="status status--implemented" aria-label="Status: Implemented">Implemented</span>

**Availability:** Available in the current development snapshot. Evidence: value-pipeline parse, evaluation, and focused-error tests.

An array, map-shaped object, or scalar can start a pipeline instead of a command. Each item streams through the usual structured stages, and a bare closure is a map stage: its parameter is the item and its return value is emitted. An object becomes one `{key, value}` record per entry in insertion order; any other scalar is a single item.

A one-stage pipeline whose stage is a standalone expression — a literal, a variable, a member read, a call — evaluates directly to that value with no stream at all, so `$(shared.nested) | $((x + 1))`-style capture of a computed scalar works without ceremony.

<p class="example-label example-label--implemented"><strong>Runnable example · Implemented</strong></p>

```josh
doubled = $([1, 2, 3] | map (x => x * 2))
joined = $(["a", "b"] | x => x + "!" | collect)
count = $([10, 20, 30] | take 2)
```
