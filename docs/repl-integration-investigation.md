# Reedline integration investigation (2026-08-16)

Method: drove the Josh REPL (`target/debug/josh`, current build incl. namespace
migration) inside `agent-terminal` sessions (isolated XDG/HOME/JOSH_HISTORY,
80x24 ghostty grid) across ~40 interaction states. Harness + scenario scripts:
`/tmp/josh-repl-test/{driver.py,scen_a.py,scen_b.py,scen_b2.py,scen_c.py,scen_d.py,scen_f.py,scen_g.py}`,
PNG artifacts in `/tmp/josh-repl-test/artifacts/`.

## Confirmed bugs

> **Status 2026-08-16:** B1 fixed (evaluator cancellation polling in
> `Engine::run_program`; verified live under agent-terminal against a 16kRPM
> `loop { }` — Ctrl-C now returns to the prompt), B2/B3/B4 fixed
> (`CompletionSnapshot` is built from the session snapshot: session PATH for
> command discovery and highlight classification, session environment keys for
> variable completion, session cwd for file completion — empty-prefix scans the
> cwd), B5 fixed (`editor.sync_history()` after each accepted line), B6 fixed
> in the agent-terminal repo (`int`/`string` → `Number`/`String`, `fg` back to
> its reserved negative after the fg slot was reverted in josh@df1c73d); the
> full `scripts/josh-e2e.sh` passes clean end-to-end again.
>
> Repro details retained below for regression context.

### B1. SIGINT cannot interrupt pure-Josh evaluation (REPL hard-hangs)

Repro: run `loop { }`, press Ctrl+C.

Observed: `^C` is echoed by the cooked-mode line discipline, no prompt ever
returns; further typed input raw-echoes (`^C1 + 1`) and is never evaluated.
The session is unrecoverable short of killing josh (agent-terminal `close` →
SIGKILL works).

Root cause: `Engine::execution_cancellation` is only consumed by josh-exec
pipeline paths (`is_cancelled()` in `crates/josh-exec/src/lib.rs`). Nothing in
the evaluator's statement/expression loop polls it. `run_repl`
(`crates/josh-interactive/src/lib.rs`) resets the flag around `run_source` but
the interpreter never observes it. External commands DO interrupt (child death
propagates `error: uncaught value: command: command failed ... (signal 2)`),
as does `$(capture)` (`error: pipeline was cancelled`), so the missing piece
is only the pure-evaluation path.

Note: DAILY_DRIVER_BRIEF item 2 covers Ctrl+C for *external* foreground
pipelines but does not obviously cover evaluator polling; worth an explicit
line in that work.

### B2. File completion broken for bare names in cwd

Repro: `cat comp_a` + Tab → "NO RECORDS FOUND". `cat ./comp_a` works.

Root cause (`file_completions`, crates/josh-interactive/src/lib.rs):
`Path::new("comp_a").parent()` is `Some("")`, so `fs::read_dir("")` errors and
the function returns `vec![]`. The `display_parent` filter already special-cases
`Path::new("")` but the read path does not. Fix: map empty parent to `.`.

### B3. File completion ignores the session cwd

Repro: `cd subdir` (external `/bin/pwd` confirms children land there), then
`cat ./unique` + Tab in `subdir` → "NO RECORDS FOUND" even though
`unique-nested-file.txt` exists there.

Root cause: completion runs `fs::read_dir` relative to the josh *process* cwd,
which is fixed at launch; `cd` only mutates `ShellContext` state. Same
split-brain as below; covered by the daily-driver brief's session-cwd item.

### B4. Command completion/highlighting ignore session PATH mutations

Repro: `env.PATH = "$PATH:/path/with/newbin"` then `brandnewcmd99` *executes*
(session PATH works) but `brandnew` + Tab → "NO RECORDS FOUND", and the
highlighter paints resolvable-but-unindexed commands red.

Root cause: `CompletionSnapshot::build` scans `std::env::var_os("PATH")`
(process-global launch env) instead of the session environment. Variable
completion is already correct because `Engine::variable_names()` merges
`context.environment_names()`. `docs/src/interactive/highlighting.md`
documents one-line staleness, but this never catches up — the rebuild source
is wrong.

### B5. History only persists on clean exit

`FileBackedHistory` syncs to disk only in `Drop` (reedline 0.49
`file_backed.rs`). Verified: a session killed via daemon close (SIGTERM/KILL)
leaves `JOSH_HISTORY` untouched; all session entries are lost. Clean Ctrl+D /
`exit` persists, and two clean-exited sessions merge correctly.

Options: sync after each accepted line (extra fsync cost), or accept and
document. Related: Ctrl+C-abandoned buffers are not recorded (reedline
default; bash records them).

### B6. Official cross-project e2e is stale ( masks regressions )

`agent-terminal/scripts/josh-e2e.sh` uses `int(...)` / `string(...)`,
removed by commit 46bbd3c ("Conversion namespaces replace string/int/float/
bool"). It currently fails at the `structured` stage. With the josh line
patched (`int(`→`Number(`, `string(`→`String(`), the entire e2e passes
(exit 0; language/chains/structured/redirections/globs/negatives/schema-v2
snapshot/deterministic PNG/cleanup all green). Patched copy:
`/tmp/josh-repl-test/josh-e2e-patched.sh`.

### agent-terminal harness caveat (not a Josh bug)

`agent-terminal wait --stable` immediately after `type` returns stale grids:
the daemon defers PTY processing while a wait request is in flight, so the
post-wait snapshot can show the pre-type frame (reproduced 3/3). Use
sleep-based settling between input and snapshot. This produced several
phantom "torn frame" readings mid-investigation.

## Verified-good states

- Startup prompt, value printing (`null` prints nothing), parse-error
  recovery, exit codes (`exit`, `exit 7`, Ctrl+D → 0).
- Continuation: `{`, trailing `&&`/`|`/`(`, unterminated quote, multiline
  paste-as-block, Ctrl+C abort from continuation.
- Signals: Ctrl+C at prompt (abandons buffer), during external command
  (child reaped, prompt returns), during `$(capture)` (cancelled), Ctrl+Z on
  external command (deliberate "suspended jobs are not supported" error;
  stopped child verified reaped — no leak), Ctrl+D with text (no-op).
- Editing: arrows/backspace/delete/ctrl+a/ctrl+e; 100-col input wraps and
  executes; CJK input with correct cursor column (x=25 for
  `printf '界面テスト'`); typeahead during running command.
- Geometry: resize idle/during output/in continuation/with menu open; 10x4
  survives; menu at bottom row; 40-line scroll ends at prompt with cursor on
  row 23; `printf '\033[2J\033[H'` clears to a fresh prompt.
- Completion mechanics: menu opens/cycles/escape, `$` variable completion
  sees `env.X` assignments, fresh lexical variables appear after one command,
  `ctrl+r` search works. The `| ` prompt swap while a menu is open is stock
  reedline `MenuSettings::default().marker`, cosmetic only.
- Prompt function: custom/dynamic/empty/wrong-type all fine; throwing
  `prompt()` logs one clear error per iteration and falls back to `josh> `.
  Note: snapshot-closure semantics mean `prompt()` sees stale ordinary
  variables after reassignment — `env.*` reads are the dynamic escape hatch;
  worth a doc line for prompt authors.

## Suggested fix order

1. e2e names (one-line; un-red the cross-project proof).
2. `file_completions` empty-parent → `.` (one-line; un-breaks everyday Tab).
3. Poll the cancellation token in the evaluator loop (un-hangs Ctrl+C).
4. Route completion + highlighting + file pane through `ShellContext`
   session env/cwd (already the daily-driver plan).
5. Decide on history sync cadence.
