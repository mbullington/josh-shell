# Approved implementation brief: all planned capabilities except jobs and modules

## Objective

Implement every capability currently classified Planned in the Josh manual, except `J-JOBS-001` and `J-MOD-001`, following the design approved in the originating thread. Promote behavior to Implemented only after executable evidence.

Included capability IDs:

- `J-STRUCT-001`: structured streams and explicit transformers.
- `J-FILES-001`: redirections and glob expansion.
- `J-CF-001`: complete non-job control flow and status chaining.
- `J-FUNC-001`: functions, closures, calls, member/index access, builtin methods, and lexical UFCS.
- `J-CONFIG-001`: dedicated startup scripts.
- `AT-PNG-001`: deterministic PNG rendering in agent-terminal.

Excluded:

- `J-JOBS-001`: no background `&`, Job values, job table, process-group job control, `jobs`, `fg`, or `bg`.
- `J-MOD-001`: no `import`, `export`, `source`, remote modules, or standard-library module resolver.
- Unresolved background-job expression syntax remains unresolved.

The implementation may add prerequisite expression/value behavior (objects, ternary, spread, destructuring, methods) required to make included capabilities coherent. It must not silently implement excluded surfaces.

## Architecture

Converge from the current single package to a Rust workspace with ownership-based crates:

- `josh-syntax`: lossless lexer, recursive-descent/Pratt parser, AST, spans, diagnostics, completeness; OS-free and `#![forbid(unsafe_code)]`.
- `josh-runtime`: Value, lexical frames, closures, builtins/method dispatch, UFCS, evaluator and typed control-flow unwinding; `#![forbid(unsafe_code)]`.
- `josh-exec`: argv/PATH, globs, redirections, external processes and status collection.
- `josh-streams`: typed stage graph, byte/value boundaries, transformers, bounded channels, cancellation.
- `josh-interactive`: Reedline prompt/highlight/hint/completion/history integration.
- `josh-cli`: `josh` binary, `-c`, scripts, REPL, startup configuration.

Do not introduce a bytecode VM, async user semantics, event loop, Rowan/red-green tree, compatibility wrappers around obsolete ownership, or a user-visible generic Stream value. Whole-buffer interactive reparsing remains deliberate.

Temporary breakage during migration is acceptable. The verification boundary is the final end state.

## Runtime values and functions

Ordinary values use `Arc` and copy-on-write where mutation exists:

- Null, Bool, Int(i64), Float(f64), String, Bytes, Array, insertion-ordered Object, Function, Error.
- Resource execution state remains outside ordinary `Value` copy semantics.
- Closures capture lexical bindings as snapshots. No `this`, prototypes, classes, GC, Promises, or shared mutable global dispatch tables.
- Use lexical frames/scopes rather than one global map.

Evaluation uses typed unwinding such as:

- ordinary success Value;
- Throw(Value);
- Return(Value);
- Break;
- Continue.

Implement:

- `fn name(params) { ... }` declarations and arrow functions using one closure representation.
- Calls, member access, and index access.
- Builtin-method-first dispatch, then lexical UFCS: `value.name(args)` → builtin method or `name(value, ...args)`.
- Objects and arrays sufficient for JSON, member/index access, spread, destructuring, and functions.
- Ternary expressions.
- Common JS-shaped non-mutating methods required by project examples and useful structured scripting: string length/contains/includes/startsWith/endsWith/split/replace/replaceAll/trim/toUpperCase/toLowerCase/at; array length/at/contains/includes/map/filter/reduce/flat/join/slice; object keys/entries. Keep method names and exact behavior documented and tested. Do not add speculative prototype machinery.
- Explicit scalar conversions and `typeof` where needed for coherent examples.

Implement control flow:

- `if`/`else` and else-if.
- `while` and unconditional `loop`.
- `try`/`catch`, `throw`, `return`, `break`, and `continue`.
- No `for` production; `for` remains an ordinary command word.
- External command failures become structured error values at the evaluator boundary.
- Catch binds the thrown/error value. `if command` suppresses only a completed nonzero status; planning, interpolation, spawn, and type failures propagate.
- `&&` and `||` chain on completed exit status only. `status command` returns status without throwing. Plan errors are never converted to status success/failure.

## Structured streams

Pipeline stages are resolved/evaluated and the full graph validated before anything spawns. Model stage and port variants explicitly; do not use generic records or runtime type feedback in parsing.

Stream kinds:

- Byte stream from external commands and byte-producing builtins.
- Value stream from explicit transformers and functions.

Required transitions:

- bytes → external: direct OS pipe when adjacent.
- bytes → `text`: collect/decode to one String or Bytes according to the documented policy.
- bytes → `json`: collect and parse exactly one JSON value.
- bytes → `lines`: stream UTF-8 lines as String values.
- bytes → `jsonl`: stream one parsed JSON value per line.
- bytes → `chunks(n)`: stream Bytes chunks of bounded positive size.
- bytes → function directly: planning error with transformer hint.
- values → function/lambda: apply once per item.
- values → external: strings serialize one line per item; all other data values serialize as JSONL.
- values → `collect`: emit one Array value.
- values → `text`: join/serialize to bytes under one documented rule.
- unsupported transitions fail before spawn.

Streaming builtins/stages:

- function stage (`| (x => ...)`).
- `map`, `filter`, `take`, `first`, and `collect`.
- `text`, `json`, `lines`, `jsonl`, `chunks(n)`.

Use bounded channels (capacity around 256 values unless measurement indicates otherwise) and internal threads; no async runtime is needed. Cancellation/downstream close must stop and join in-shell workers and external children. Adjacent external stages retain raw OS pipes. In-shell writers treat downstream close as graceful. Pipeline status preserves pipefail and the proven SIGPIPE distinction.

Capture semantics remain explicit and cardinality-stable:

- external bytes with no terminal transformer: String with trailing newline trimming, or Bytes for invalid UTF-8.
- `text`/`json`: one value.
- `lines`/`jsonl`/`chunks(n)`: always Array, including zero or one item.
- `collect`: one Array value.
- Never infer JSON from content or collapse cardinality based on observed item count.

Verification must include bounded-memory/early-termination behavior using a bounded test producer and prove no worker/process leaks.

## Files and globbing

Implement command-mode redirections:

- `> file`, `>> file`, `< file`, `2> file`, `2>> file`, `2>&1`, `&> file`.
- Attach redirections to the command stage they follow and apply them left-to-right.
- Targets evaluate to exactly one path; arrays/objects are errors.
- Open/validate redirection targets during full pipeline planning before spawning any stage.
- Preserve expression-mode comparison parsing.

Globbing:

- Unquoted command words containing `*`, `?`, bracket classes, or `**` expand before invocation.
- Sort results deterministically.
- No match is an error.
- Quoting suppresses expansion.
- Each match is one argv item; no word splitting.
- Add expression builtin `glob(pattern)` with the same policy, returning an Array.

## Configuration

Configuration is deliberately independent of modules/source:

- Resolve config root from `XDG_CONFIG_HOME`, otherwise `~/.config`.
- Run `josh/env.josh` for every session.
- Additionally run `josh/init.josh` for interactive sessions.
- Execute startup files in the shell global scope through the ordinary parser/evaluator.
- Do not expose general `source` or import syntax.
- Add `--no-config` for reproducible automation.
- Missing files are ignored.
- Batch startup errors fail startup.
- Interactive startup errors print diagnostics but still open a usable shell.
- A lexical zero-argument `prompt` function may return the interactive prompt string; invalid prompt values/errors fall back with a diagnostic.
- Preserve `JOSH_HISTORY` behavior unless the new config explicitly supersedes it with a documented implemented setting.

## agent-terminal deterministic PNG renderer

Implement `agent-terminal screenshot [SESSION] [PATH]` with agent-browser-like ergonomics. If PATH is omitted, write to a deterministic documented temporary/output location and print it. Add JSON result mode if consistent with existing command conventions.

Rendering requirements:

- Consume the actual libghostty-derived semantic/render state; do not use a toy ANSI parser or plain-text screenshot.
- Render client-side from the semantic snapshot unless a stronger ownership reason requires daemon rendering.
- Vendor and license a pinned JetBrains Mono font (the Ghostling reference includes one under OFL).
- Fixed 96 DPI, fixed cell metrics, pinned font bytes, fixed default theme, deterministic CPU rasterizer settings, frozen cursor blink/animations, and deterministic metadata-free PNG encoding.
- Render foreground/background, bold, italic (or a documented deterministic synthetic policy), underline, strike, inverse, wide cells, background-only cells, and cursor.
- Unsupported glyphs use a deterministic replacement glyph; never host font fallback.
- Bound dimensions and output allocation under existing grid limits.
- Snapshot remains semantic JSON/text and is not renamed; screenshot is pixel output.
- Add exact dimension/pixel/byte-stability tests and a real TUI/Josh screenshot scenario. Keep fixtures small and explain the concrete regression protected.

## Documentation and status

Update the extensive mdBook manual after actual behavior exists:

- Promote included capability rows to Implemented only with executable evidence.
- Split broad status rows when independently verifiable behavior needs separate claims.
- Keep `J-JOBS-001` and `J-MOD-001` Planned/Excluded and ensure examples are not runnable.
- Keep unresolved background assignment unresolved.
- Update tutorials, references, architecture, CLI help, snapshot/screenshot schema, troubleshooting, security, status matrix, and release verification.
- Every code fence and status block must satisfy the existing deterministic manual checker.
- Build generated HTML and inspect links, duplicate IDs, literal fences, and status consistency.

## Final proof

Run independent checks concurrently where safe using GNU Parallel; serialize commands that contend for Cargo targets, terminal sockets, or process lifecycle.

Required final evidence:

1. Format, Clippy with warnings denied, tests, and builds for every Josh workspace crate.
2. Parser/evaluator behavior for objects/functions/closures/UFCS/control flow/throws/chains.
3. Structured byte/value pipeline behavior, cardinality, JSON errors, bounded backpressure, early cancellation, and cleanup.
4. Redirection ordering and all required forms; quoted/unquoted/no-match glob behavior.
5. Config file ordering, scope, `--no-config`, startup error behavior, and configured prompt through a real PTY.
6. agent-terminal format/Clippy/tests/build against the pinned actual libghostty-vt.
7. Byte-identical deterministic PNG test across repeated renders plus visual/manual inspection of one rendered artifact.
8. Build and check the static manual.
9. Drive Josh through agent-terminal at a fixed grid and exercise a function, loop/control flow, structured stream, redirection, glob, configured prompt, semantic snapshot, and PNG screenshot.
10. Exit/close and directly prove no Josh child, pipeline worker, agent-terminal daemon, control socket, or temporary runtime directory remains.
11. Explicit negative checks prove `&`, jobs/fg/bg, import/export/source, and remote modules remain unavailable.

Compilation and agent summaries are evidence only, not completion proof.
