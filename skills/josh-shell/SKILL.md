---
name: josh-shell
description: Write, edit, and debug Josh scripts (.josh), including share/*.josh libraries and Josh configuration (env.josh, init.josh). Use whenever a task involves the Josh language or its shell runtime — writing regex.josh-style pure-computation libraries, wiring pipelines and captures, fixing [P1xx] parse diagnostics, adding a source-include, or benchmarking Josh performance. Even for small edits, Josh's dual command/expression mode bites; read this before writing the file.
---

# Josh: the JavaScript Object Shell

Josh is a Unix-first shell with a capable scripting language: tree-walking interpreter in Rust (`crates/josh-*`), documented by the mdBook manual in `docs/src` (rendered HTML committed in `docs/book`; planned/unresolved work lives in `docs/src/roadmap/`). The manual is the source of truth; this skill is the fast path. **When code and docs disagree, run the code and believe the run.**

## Ground rules

- **Always execute what you write.** From the repo root: `cargo run -q -- -c 'code'` (or an installed `josh -c 'code'`). Multi-file: `source` from a scratch dir; do not guess at parse behavior.
- The strict parse gate rejects the whole program before anything runs. Diagnostics read as `message[P123] at start..end; expected ...`.
- Josh is typed and strict. There are almost no silent JS coercions. Read errors literally; they usually say exactly what is unsupported.

## The dual mode (the #1 source of mistakes)

Every statement head is classified deterministically:

- `name =`, `+=`, `-=` → assignment to nearest visible binding.
- `name` followed ADJACENT (no trivia) by `(`, `.`, `[` → expression statement.
- `name` followed by `=>` (spaced or not) → expression.
- `let` / `fn` / `if` / `while` / `loop` / `try` / `throw` / `return` / `break` / `continue` / `status` / `source` → dedicated productions.
- Anything else at statement head is a **command**, parsed as command words until `|`, `;`, newline, or a brace.

Consequences that bite:

- `f(1)` calls; `f (1)` is command `f` with one evaluated argument `(1)`.
- `a[0..2]` at statement start is a range-slice expression; inside a command word, `[`/`]` are glob syntax.
- `for` is an ordinary command word — there is no `for` loop. Use `while`/`loop` or pipeline stages.
- `==` is a hard parse error with a suggestion: use `===` / `!==`. There is no truthiness-based coercion equality.

### Evaluating values inside commands

- `$(pipeline)` runs a command pipeline (output capture), `$((expr))` — note TWO parens — splices one evaluated expression. `echo $(expr)` does NOT evaluate `expr`; it greps for a command named `expr`.
- `$name` splices a binding. `$e.message` splices ONLY `$e` and appends the literal text `.message` — write `$((e.message))` or `${e.message}` inside double quotes.
- An array value splices as separate argv words ONLY as a sole unquoted `$var` word: `echo $lines` works; `echo $((lines))` errors with "an array expands only as a sole unquoted `$variable` command word".
- Error values do not display implicitly ("errors require explicit string conversion") — echo `e.message` via `$((e.message))` or `String(e)`.
- `$name` does NOT exist inside expression syntax (`P163 expected an expression`). `$`-splicing lives only in command words; inside `$(( ))` and other expression contexts reference bindings bare.

## Language cheat-sheet (verified)

- Values: `null`, booleans, `Int` (i64; `+`/`-`/`*` are checked — overflow is a type-kind error; `% 0` errors as "integer remainder by zero"),, `Float`, `String`, `Bytes`, `Array`, `Object`, `Function`, `Error`, `Status`, plus `env` and namespace values.
- `/` is ALWAYS float division (`10 / 3` → `3.3333333333333335`, `1 / 0` → `inf`); `%` is integer remainder.
- Int and Float do NOT mix: `2.5 > 0` errors ("operator Greater does not accept these values"); compare Floats with `0.0`. `Number(x)` converts strings/ints.
- `fn name(params) { ... }` and arrows `x => x * 2` / `(x, y) => x + y` / block bodies `x => { return x }`. Closures SNAPSHOT visible bindings at creation. Direct recursion works; nothing is hoisted; no `this`/classes.
- Destructuring in `let` and params (nested + one trailing `...rest`); NOT in assignment. A trailing `...args` rest PARAMETER collects remaining call arguments into an array (`fn z(...args) { ... }`, `(a, ...rest) => rest`); it must be last and an identifier ([P184]/[P185] otherwise).
- `let` for declarations; plain `=` assigns or creates in the current frame. `+=`/`-=` checked.
- Control: `if (cond) { } else { }`, `while (cond) { }`, `loop { break }`, `try { } catch e { }`, `throw value`, `return`, `break`, `continue`. **Parenthesize expression conditions.** An unparenthesized `if`/`while` condition is parsed as a COMMAND PIPELINE run for its exit status (shell semantics, J-RUN-006): `while i < 5 { }` runs command `i` with input redirected from file `5`, it is NOT a comparison. `if` and `try` are also value-producing expressions when parenthesized: `x = if (cond) { 1 } else { 2 }`.
- `a && b`, `a || b` do double duty: at command position they chain processes on success; inside expressions they are short-circuit boolean operators (`(a && b)`).
- `source path.josh` evaluates a file in the CURRENT frame (bash-style). One command word path (quoting, `$var`, `$()`, leading `~` OK), cwd-relative, strict parse gate, cycle guard, `return`/`break`/`continue` cannot escape the file. `import`/`export`/remote modules are deliberately rejected.
- Builtins: `cd`, `exit`, `status`. `command name …` skips lexical functions and builtins, resolving `name` on PATH — verb-hiding helpers call it to avoid recursing (`fn mkdir(...args) { command mkdir -v $args }`).
- Semicolons AND newlines separate statements. `++`/`--` are deliberately rejected.

## Data model for computation (share-library style)

- Strings: `'…'` and `"…"` are IDENTICAL processed strings — JavaScript escape decoding (`\n`, `\t`, `\xNN`, `\uNNNN`, `\u{…}`, identity escapes) plus, in command words, `$var`/`${…}`/`$(…)` interpolation. `r'…'`/`r"…"` are fully raw. Bare words keep POSIX backslash-quote semantics. Immutable UTF-8 with **UTF-16 code-unit positions** (JS semantics, since 2026-08-16): `"😀".length == 2`. `s[i]`/`s.at(i)` return one code point; a position inside a surrogate pair snaps to the whole code point (Rust strings cannot hold lone surrogates). Negative indexes count from the end; out-of-range gives `null`.
- **Range slices are the one slicing mechanism:** `a[b..c]` on strings AND arrays, end-exclusive, negative/omitted/clamped bounds, inverted pair → empty. `a[..]` makes an independent copy. No `..=`, no stride ranges; no `String.prototype.slice/substring` (arrays have `.slice(a, b)` — prefer ranges).
- Arrays are SHARED MUTABLE values: aliases observe in-place edits from `push`/`pop`/`reverse`/`sort` (JS return semantics). Callback methods (`map`/`filter`/`reduce`/`flat`/`join`) iterate a snapshot. `a.push(x)` returns the new length.
- Objects are string-keyed and mutable: `o.x = v`, `o[k] = v`, `o.x += 1` all work. `Object.keys(o)`, `Object.entries`, `Object.freeze`.
- Prototypes: every value dispatches through its prototype table (`"T".toUpperCase()`, `[1,2].map(f)`, `(-3).abs()`). You CAN add methods: `String.prototype.shout = (str) => str + "!"` — receiver is explicit; no `this`. Member-call order: own field → prototype method → lexical function (UFCS, called `name(receiver, ...args)`).
- Useful namespaces: `Math` (`max`, `min`, `floor`, ...), `Date.now()`, `File` (exists/read/etc.), `length(x)` builtin-ish via `.length`, `String(x)` explicit conversion.
- Char iteration: `s.split("")` → array of code-point strings; `Array(chars.length)` etc. Pure-computation libraries (NFA engines, CSV) work on char arrays.

## Errors

- `throw value` throws anything; `try/catch` catches thrown values and evaluator/process errors raised inside the parsed body. `catch e`: `e.kind` (e.g. `"type"`, `"filesystem"`, `"undefined"`, `"parse"`, `"process"`), `e.message`, optional `e.status`.
- A parse error in the source REJECTS THE WHOLE FILE before `try` can run — you cannot `try` a syntax error.
- `status cmd ...` runs a command and captures a Status value (`.code`, `.success`, `.outcomes`) instead of failing on nonzero.

## Pipelines and capture

- Byte streams: `producer | consumer`, captures `$( ... )` return String (or Bytes on invalid UTF-8) with ALL terminal CR/LF trimmed.
- Structured transforms: `text`, `json`, `lines`, `jsonl`, `chunks`, `map f` / `filter f`, closures as stages (`x => x + 1`), `take n`, `takeLast n` (bounded tail), `first`, `collect`.
- Value pipelines: `[1,2,3] | x => x * 2`; single-value sources too: `5 | x => x * 2`; unit `()` runs a pure expression stage.
- Capturing streams gives arrays ALWAYS for `lines/jsonl/chunks/filter/take/takeLast` (even 0 or 1 items); `text`/`json` give a scalar; `first` gives value-or-null.
- Internal stages and functions in pipelines are concurrency-limited execution; keep hot loops in one process instead of spawning a stage per item when you benchmark.

## Interactive (REPL)

- Completion: commands from session PATH/builtins, `$variables`, files (leading `~` resolved against session HOME, tilde preserved on insert). Optional carapace bridge (`JOSH_CARAPACE=0` disables).
- Ghost-text typeahead: history-prefix hint first, else first native completion candidate; Right/Ctrl+F accepts whole, Alt+Right/Ctrl+Right one word.
- Ctrl+R opens the history MENU: deduplicated substring search, newest first, refilters while typing, arrows navigate, Enter copies to buffer.
- History: plain text at `$JOSH_HISTORY` or `~/.josh_history`, cap `$JOSH_HISTORY_SIZE` (default 10,000), synced after each accepted line.
- Highlighting: per-stage command-validity coloring; `JOSH_THEME=/path/to/theme.tmTheme` resolves TextMate theme colors (fallback ANSI palette).

## Verification

- `cargo test --workspace --all-targets`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check`.
- Docs (if you touch `docs/src`): `mdbook build docs` then `python3 docs/tools/check-manual.py`; the rendered `docs/book` is committed.
- The manual's capability matrix is enforced: new user-visible capabilities need a matrix row, a status panel in the page, and a test named in the Evidence column.

## share/ libraries (vendored Josh code)

`share/` follows the stb single-file-library model: vendored, public-domain (MIT-0), no stability promises, copy-into-your-own-config to use. Each file has a fixed header block (name, purpose, one-line usage, license, caveat) and a `*_selftest()` function exercised by `scripts/check-share.sh`. Read `share/README.md` and `share/regex.josh`'s header as the canonical examples before authoring one. Style: pure functions + prototype extensions, small public API, local aliases for hot globals (`let String = ...` patterns are unnecessary — closures snapshot; prototype extension happens at top level once).
