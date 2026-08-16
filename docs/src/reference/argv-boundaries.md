# argv and pipeline boundaries

<div class="status-coverage">

**Status coverage:** [J-ARGV-001](../status/matrix.md#J-ARGV-001) — **Implemented**; [J-RUN-003](../status/matrix.md#J-RUN-003) — **Implemented**; [J-STRUCT-001](../status/matrix.md#J-STRUCT-001) — **Implemented**. See [status conventions](../welcome/status-conventions.md).

</div>

<a id="J-ARGV-001"></a>
## Command-word conversion <span class="status status--implemented" aria-label="Status: Implemented">Implemented</span>

**Availability:** Available in Josh 0.1.0. Evidence: argv, glob, structured-stream, and invalid-UTF-8 tests.

| Source shape | Result |
|---|---|
| Literal, quote, scalar interpolation, capture, evaluated expression | Parts concatenate into one argv entry |
| Sole unquoted `$array` word | One argv entry per array element |
| Array inside double quotes | Elements joined with one space into one argv entry |
| Array combined with other unquoted parts | Type error |
| Null/bool/int/float | Canonical scalar text |
| Bytes on Unix | Original bytes |
| Object | Type error outside a structured value stream |
| Unquoted glob | One sorted argv entry per match; no match is an error |

There is no shell word splitting. Quotes suppress glob expansion.

## Pipeline boundary table

| Producer → consumer | Result |
|---|---|
| external bytes → external | Direct OS pipe |
| bytes → `text` | One String, or Bytes for invalid UTF-8 |
| bytes → `json` | One parsed JSON value |
| bytes → `lines`/`jsonl`/`chunks` | Streaming values with many cardinality |
| bytes → function/`map` | Pre-spawn planning error with an explicit-transformer hint |
| values → function/`map`/`filter` | One call per item; map emits return values and filter emits retained input values |
| values → `take`/`first` | Bounded output and upstream cancellation |
| values → `collect` | One Array value |
| values → `text` | LF-separated bytes with no final LF |
| values → external | Strings plus LF; other JSON-compatible data as JSONL |

Capture follows the final declared cardinality, not observed item count. `lines`, `jsonl`, `chunks`, `filter`, and `take` return Array for zero or one item; function/map preserve upstream cardinality; `first` returns one value or Null; `collect` returns its one Array. JSON-looking bytes remain text unless `json` or `jsonl` is explicit. See [Capture and pipelines](../language/capture-pipelines.md#J-STRUCT-001) for decoding and serialization errors.
