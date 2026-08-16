# Josh vertical-slice project brief

## Product

Josh means “JavaScript Object Shell.” It is a Unix-first shell with shell command semantics and a deterministic, JavaScript-shaped data language. It is not JavaScript, POSIX shell, or a JS runtime. There is no event loop, Promise, async/await, npm, `this`, prototypes, classes, coercive `==`, or POSIX compatibility.

The immediate objective is a credible vertical slice, not the full language: prove the dual-mode grammar, execute useful command lines, provide a Reedline REPL, and verify that REPL through a separate libghostty-based automation tool at `~/Projects/agent-terminal`.

## Language principles

- Shell first: command invocation, pipelines, redirections, fast startup, eventually job control.
- JS-shaped expressions: literals, arrays, objects, arrow functions, member/index access, arithmetic, comparisons, strict equality, logical operators, ternary, spread, and destructuring.
- Deterministic grammar: parsing depends only on tokens and mode, never runtime types.
- Structured pipes have explicit byte/value boundaries. No automatic JSON parsing.
- UTF-8 strings and byte strings are first-class.
- Value semantics for ordinary data, implemented with `Arc` and copy-on-write when values need to cross pipeline worker threads.
- Errexit-by-default with structured exceptions and explicit syntactic suppression contexts.
- Fish-quality interaction is a first-class product surface.

## Two lexical modes

The lexer has an explicit mode stack. Command mode is the default at statement position. Expression mode applies inside expression delimiters and expression-bearing constructs.

A statement beginning with a word token enters command mode unless the next token indicates an assignment or expression continuation (`=`, `+=`, `-=`, an immediately adjacent `(`, `.`, `[`, or `=>`), or the word is a language keyword.

In command mode:

- Bare words are string arguments: `git commit -m hello`.
- Flags are ordinary words.
- `$var` splices a value.
- `$(pipeline)` captures a pipeline.
- Double quotes interpolate; single quotes are literal.
- `(expr)` preceded by whitespace evaluates one expression argument.
- Unquoted glob metacharacters eventually glob-expand.
- `|`, newline, `;`, `&&`, `||`, `&`, and redirections delimit commands/pipelines.

In expression mode, parse a strict JS-like subset. `===` and `!==` exist; `==` does not. A statement-position `x - 1` is command `x` with arguments `-` and `1`; `(x - 1)` is arithmetic. `x(1)` is a function call, while `x (1)` is command `x` with one evaluated argument.

Required golden disambiguation examples include:

- `ls -la` → command.
- `x = 5` → assignment.
- `x(1, 2)` → expression call.
- `x (1 + 2)` → command with evaluated argument.
- `items.filter(f)` → expression.
- `x - 1` → command.
- `let y = ls` → undefined expression identifier with a hint to use `$(ls)`.
- `if (n > 3) { ... }` → expression condition.
- `if grep -q foo file { ... }` → command condition.

Use one hand-written recursive-descent parser, with Pratt parsing for expressions. Do not build separate strict and tolerant parsers. The parser always returns a `Parse` result containing the original source, a lossless token stream including trivia, a semantic AST, diagnostics, and an explicit completeness classification. Every token and AST node carries a byte span. The AST can contain explicit `Missing` and `Error` nodes; recovery synchronizes on mode-aware delimiters. Diagnostics carry expected-token sets and primary/secondary labels. Tolerant callers consume this result directly; strict callers reject results containing error diagnostics. This construction makes strict/tolerant agreement automatic and supports highlighting and exact source reconstruction without requiring a red-green tree.

Do not adopt Rowan or a persistent red-green CST in the vertical slice. Interactive shell buffers are small, whole-buffer reparsing is cheap, and Josh does not yet have a formatter or refactoring engine. Keep the evaluator-facing AST independent enough that a lossless green-tree sink and typed AST façade can be introduced later if formatting, refactoring, or large-file incremental parsing demonstrates the need.

## Values

Long-term value types are Null, Bool, Int(i64), Float(f64), String(UTF-8), Bytes, Array, Object (insertion ordered), Function, Error, plus resource handles such as Stream and Job. Ordinary data has copy-on-write value semantics. Resource handles are explicitly shared/affine capabilities and are not described as ordinary copy-on-write data.

Use `Arc`, not `Rc`, for values that cross worker threads. Assignment and argument passing conceptually copy ordinary data. Mutation uses `Arc::make_mut`. Closures capture snapshots. Explicit shared mutation, if eventually needed, uses a dedicated `ref()` cell. Recursive functions require a design that does not claim cycles are impossible without proof.

Truthiness: false, null, numeric zero, empty string, empty array, and empty object are false. `/` produces Float; `//` performs integer division; `%` is integer remainder. String indexing and slicing are by Unicode scalar/grapheme policy that must be made explicit before full implementation.

## Name resolution and interpolation

Command-call position resolves lexical scope, builtins, then PATH. Expression position resolves lexical scope and builtins only; PATH is never consulted. Environment variables live under `env`, with command/string `$NAME` convenience resolving scope before environment. Plain assignment does not export; `env.FOO = "bar"` does.

Double quotes support `$var`, `${expr}`, and `$(pipeline)`. Single quotes are literal. There is no word splitting. A string is one argv entry; an array splats to multiple argv entries; scalar primitives stringify canonically; objects are errors unless serialized explicitly. Arrays inside double quotes join with spaces.

## Capture and pipelines

`$(pipeline)` runs to completion. External byte output is collected, decoded as UTF-8 when valid, trailing newlines stripped, and otherwise returned as Bytes. It never automatically parses JSON.

Named transformers make stream boundaries explicit:

- `text` and `json` collect and produce one value.
- `lines`, `jsonl`, and `chunks(n)` stream and capture as arrays even for zero or one item.

Two stream kinds are planned: byte streams and value streams. Adjacent external commands use OS pipes. Byte-to-function is an error that asks for a transformer. Value-to-function maps per element. Value-to-external serializes strings one per line and other values as JSONL. In-shell stages eventually use bounded channels for backpressure. Classification errors occur at planning time after resolving stage expressions, before spawning the pipeline; they are not parser type checks.

The vertical slice should implement only a coherent subset and expose a status matrix. Prefer external byte pipelines and string capture first. Never fake unimplemented structured stream behavior.

## Errors

A failing external command in ordinary statement position throws a structured command error. Planned suppression contexts are `&&`/`||`, command conditions, `try/catch`, and `status command`. Pipelines use pipefail by default; downstream-close SIGPIPE is not an upstream failure. Uncaught script errors exit nonzero; interactive errors print and return to the prompt.

The vertical slice must at minimum report command-not-found, parse errors, failed commands, and pipeline failures clearly without killing the interactive shell.

## Redirections and control flow

Planned redirections are `>`, `>>`, `<`, `2>`, `2>>`, `2>&1`, and `&>`, command mode only. Planned control flow includes `if`, `while`, `loop`, `try/catch`, function return, break, continue, and throw. There are intentionally no `for` loops; remove `for` from keyword and grammar lists.

Background-job syntax in expression assignment is unresolved. Do not document `j = sleep 100 &` as valid until a coherent job expression is chosen. Background command statements and full terminal job control are later phases.

## Functions and UFCS

Function values use `fn name(args) { ... }` declarations and arrow expressions. There is no `this`. UFCS resolves `value.name(args)` first as a builtin method for the type, then as a lexically scoped function called as `name(value, ...args)`. There are no mutable global prototypes.

## Modules

Planned imports run in their own scope and expose only exports. `source` evaluates in the current scope. URLs and a hosted standard library are future design work and need a secure dependency model; they are not part of the vertical slice.

## Interactive vertical slice

Use Reedline. The slice should provide:

- Prompt and line editing.
- Tolerant-parser-driven multiline continuation.
- Syntax/token highlighting; command resolvability coloring when feasible.
- History-backed prefix hints.
- Completion for commands, files, and variables to the extent supported by the implemented parser.
- Clean Ctrl-C behavior that does not kill the shell.
- Noninteractive `josh -c '...'` and `josh script.josh` paths sharing the same parser/runtime.

Do not implement job control, SQLite history, carapace, modules, rich output, or full structured streams merely to check boxes. Leave stable seams and document them as planned.

## agent-terminal project

Build a separate Rust project at `/Users/mbullington/Projects/agent-terminal`. It is “agent-browser for terminal applications,” with a semantic terminal state as the terminal analogue of a DOM.

Use a real PTY for the child process and `libghostty-vt` for VT parsing/state. Pin the Ghostty revision because its C API is explicitly unstable. Relevant references are cached at:

- `/Users/mbullington/.cache/checkouts/github.com/ghostty-org/ghostty`
- `/Users/mbullington/.cache/checkouts/github.com/ghostty-org/ghostling`
- `/Users/mbullington/.cache/checkouts/github.com/jasonkneen/agent-terminal`
- `/Users/mbullington/.cache/checkouts/github.com/vercel-labs/agent-browser`

The machine has Nix, CMake, Ninja, Rust, Clang, Node, and pnpm. Zig is not on the default PATH; Ghostling’s Nix development environment or an explicit pinned Zig provision is available as a build strategy. Do not silently replace libghostty with a toy ANSI parser.

MVP commands:

- `launch -- <command> [args...]` → session ID.
- `snapshot [session]` → agent-friendly text by default and structured JSON on request, including size, cursor, title when available, rows/cells, text, styles where practical, and a monotonic revision.
- `type [session] <text>` → text input.
- `key [session] <key chord>` → encoded special/modifier key.
- `resize [session] <cols> <rows>`.
- `wait [session] --text ...`, `--stable ...`, and timeout behavior.
- `list`.
- `close [session]`; cleanup must terminate/reap children.

An auto-started local daemon over a Unix socket is acceptable for cross-invocation sessions, but tests and verification must always close it. Do not leave background processes after development commands. Keep protocol types explicit and versionable. Restrict socket permissions. Clearly document that running arbitrary commands is powerful and should not be exposed to untrusted callers.

Pixel screenshots are a second milestone. `libghostty-vt` does not render pixels. The MVP must not call plain text a screenshot; use `snapshot`. Document a deterministic renderer plan: pinned font, theme, DPI, viewport, disabled cursor blink, and PNG output.

## Static user manual

Build an extensive mdBook-compatible manual in `docs/`. It must build to static HTML. Use `SUMMARY.md` as a deliberately editable information architecture. Include:

- Welcome and installation/build instructions.
- Tutorial and first session.
- Command mode and expression mode.
- Variables, values, strings, interpolation, splicing, capture, pipelines, errors, functions/UFCS, files/globs, control flow, jobs, modules, and configuration.
- Interactive editing, highlighting, hints, completion, history, and troubleshooting.
- agent-terminal concepts, installation, CLI reference, semantic snapshot schema, scenario/e2e examples, security, and deterministic screenshot roadmap.
- Language grammar/disambiguation appendix.
- Architecture and contributor guide.
- Implementation status/roadmap.

Every page or feature block must distinguish **Implemented**, **Specified**, and **Planned/Unresolved**. Examples for planned syntax must not be presented as currently runnable. Add minimal custom theme assets for status badges/callouts and readable code examples without turning the docs into a JS application.

## Verification contract

The final workflow must exercise the closest real behavior:

1. Build and test Josh.
2. Build and test agent-terminal against the pinned libghostty-vt.
3. Build the static manual.
4. Launch the Josh REPL through agent-terminal at a fixed grid size.
5. Wait for the prompt, type a command, inspect semantic snapshots, exercise at least one completion/history or multiline interaction, and cleanly exit.
6. Assert no agent-terminal daemon/session/child is left behind.

If an environment/toolchain blocker prevents a component, report it exactly and leave a deterministic setup/check command; do not claim completion from compilation proxies or agent summaries.
