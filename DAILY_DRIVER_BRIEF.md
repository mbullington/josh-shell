# Josh daily-driver readiness brief

## Objective

Implement the four daily-driver gaps approved in the originating thread:

1. Session-owned exported environment mutation, including an array-valued PATH view.
2. Complete foreground terminal process-group handoff for external commands, with one suspended foreground slot and `fg`, but no general background jobs.
3. Optional Carapace command-specific completion with native fallback.
4. Bounded materializing capture and collect operations.

The user will handle installation/packaging. Preserve the existing language/runtime/stream/files/config/screenshot behavior and the deliberate exclusion of general jobs and modules.

## Session context and environment

Introduce a session-owned context shared coherently by evaluator, process execution, streams, completion, and REPL. It owns at least:

- logical current working directory;
- environment map preserving OS-string values;
- foreground terminal state/suspended slot;
- capture limits or access to their constants.

Do not mutate process-global environment from Josh evaluation. Stream execution uses threads, and Rust 2024 process-environment mutation is not a safe ownership model. Every external process receives an explicit cwd and environment snapshot.

Environment expression contract:

- `env.NAME` and `env["NAME"]` read the current session environment dynamically.
- Valid UTF-8 values become String; non-UTF-8 Unix values become Bytes; missing variables become null.
- `env.NAME = value` updates the session environment and affects later child processes.
- Assigning null unsets a variable.
- String, Bytes, Int, Float, and Bool use documented canonical environment conversion; compound Objects/Functions/Errors are rejected.
- `env.PATH` reads as an Array of path components split with `std::env::split_paths` and writes from an Array joined with `std::env::join_paths`. A direct String/Bytes assignment is accepted as an explicit raw PATH where platform-safe.
- Environment names and values reject NUL and invalid names with structured errors.
- `env` is a reserved runtime namespace and cannot be accidentally shadowed into losing environment access.
- Closures read `env` dynamically at call time; environment is session state, not snapshot-captured ordinary data.
- `typeof env` is `object`; object-style keys/entries may use a snapshot if implemented, but writes always target session state.
- PATH resolution, command resolvability highlighting, command completion, Carapace, globs/redirections, startup scripts, and children must use the same session environment/cwd facts.
- `env.josh` can persistently change PATH and exported values for the session; `--no-config` remains authoritative.

Prefer moving cwd into the same session context instead of continuing to mutate process-global cwd. `cd` updates the session cwd; command spawn, redirection, globbing, config resolution where relevant, file completion, and Carapace use it. This removes cwd races with stream worker threads.

## Capture limits

All materializing boundaries are bounded, not just raw `$()` bytes:

- maximum aggregate capture/materialization: 64 MiB;
- maximum captured value items: 1,000,000;
- limits apply to raw external capture, `text`, `json`, `lines`, `jsonl`, `chunks`, capture of value streams, and `collect`/value-to-text internal materialization;
- use incremental bounded readers/builders and fallible allocation;
- never call an unbounded `read_to_end` or allocate based solely on input-provided size;
- on overflow, cancel the graph, terminate/reap external process groups, join workers/readers, discard the partial assignment, and return a structured actionable error naming the limit and suggesting streaming `filter`/`take`/external consumers rather than `$()`/`collect`;
- exact-limit input succeeds; limit+1 fails deterministically;
- existing chunks-size limits remain independent;
- keep the constants documented and exposed for focused tests.

A fixed limit is intentional for 0.1.0. Do not add a speculative configuration schema in this task.

## Foreground terminal ownership

Implement Unix foreground control for external-only foreground pipelines and terminal applications:

- initialize an interactive shell terminal controller from `/dev/tty`/stdin as appropriate;
- place one pipeline's external processes into one process group (first child establishes PGID; all later stages join it);
- transfer foreground terminal ownership to the pipeline PGID with `tcsetpgrp`;
- the shell ignores or safely handles SIGTTOU, SIGTTIN, SIGTSTP, and interactive SIGINT while it is the controller; children reset normal dispositions before exec;
- restore the shell PGID and terminal modes through an RAII guard on normal exit, error, signal, stop, panic-safe unwind boundaries available in safe Rust, and shell shutdown;
- wait for process state with stop/continue awareness and preserve pipefail/outcomes;
- terminal resize reaches the foreground process group under normal tty semantics;
- Ctrl-C interrupts the foreground group and returns Josh to a usable prompt;
- foreground `vim`, `less`, `ssh`-style and `fzf`-style programs receive a controlling foreground terminal and can exit normally.

Ctrl-Z policy approved by the user:

- retain exactly one suspended external foreground pipeline slot;
- when the foreground external pipeline stops, reclaim the terminal, retain the process group/pids/command and necessary pipe/capture ownership, print a concise stopped notice, and return to the prompt;
- `fg` with no arguments transfers the terminal back, sends SIGCONT to that group, and resumes waiting;
- a second stopped foreground pipeline must have a deterministic policy that avoids leaking the first. Prefer refusing to launch/stop over silently replacing; document the exact rule;
- shell exit with a suspended slot sends bounded HUP/CONT/TERM/KILL as needed and reaps it;
- repeated `fg` after completion reports an actionable “no suspended foreground pipeline” error;
- no `&`, `bg`, `jobs`, `%N`, job IDs, or multiple-job table;
- this minimal suspended slot does not promote J-JOBS-001 to generally Implemented; document it as foreground control.

Structured pipelines contain in-process Rust workers and are not safely suspendable. Ctrl-Z during a structured graph cancels/reaps/joins it and returns an explicit diagnostic explaining that only external foreground pipelines can be resumed with `fg`. Do not pretend worker threads are suspended.

Design foreground state as an explicit state machine, not scattered booleans. Terminal ownership and suspended-process cleanup must be idempotent.

## Carapace completion

Keep native Josh-aware completion for command position, variables, members/UFCS, and fallback files. In external-command argument position, optionally query Carapace.

Current upstream interface verified from primary docs/source and a real current binary:

- invocation shape for carapace-bin: `carapace <command> export <args... current-prefix>`;
- JSON export includes `version`, `messages`, `noprefix`, `nospace`, `usage`, and `values` with `value`, `display`, optional `description`, `style`, and `tag`;
- reference: https://carapace-sh.github.io/carapace/carapace/export.html.

Requirements:

- discover `carapace` through the session PATH; do not require it for startup;
- `JOSH_CARAPACE=0` disables the bridge; other absent/invalid values use normal optional discovery;
- derive the current external command stage and argv prefix from Josh's parser/token context, not POSIX shell splitting;
- pass session cwd/environment to Carapace;
- enforce a short synchronous completion deadline (target 200 ms) and a bounded JSON output size;
- on missing binary, timeout, nonzero exit, malformed/oversized JSON, unsupported version shape, or no usable values, silently return native file completion; completion must never damage the edit buffer or print provider errors into the prompt;
- terminate and reap a timed-out Carapace child;
- preserve Carapace descriptions/display/tag where Reedline supports them, and honor no-space suffix semantics without copying ANSI style strings blindly;
- suggestions use UTF-8-safe replacement spans and deterministic ordering/deduplication;
- tests use a fake Carapace executable for argv/cwd/env/description/no-space/timeout/malformed-output/fallback contracts; add one optional real smoke when Carapace is available, but do not make normal tests depend on it;
- document installation as optional and link the primary Carapace docs.

## Verification

1. Format, warnings-denied Clippy, all tests, debug and release builds across the workspace.
2. Environment tests for read/write/unset, UTF-8/Bytes, PATH array/raw assignment, dynamic closure reads, startup persistence, child export, PATH lookup/completion, and no process-global leakage between independent Engine sessions.
3. Session cwd tests for `cd`, child cwd, globs/redirections/completion/Carapace, and stream-thread safety.
4. Capture exact-limit and limit+1 tests for bytes and values; overflow must cancel, reap, join, and leave assignment untouched.
5. PTY tests for terminal ownership, Ctrl-C, resize, `vim` or an available full-screen fixture, pager behavior, Ctrl-Z stop, prompt return, `fg` resume, normal exit, repeated fg error, shell exit cleanup, and structured Ctrl-Z cancellation policy.
6. Carapace fake tests and one real current Carapace export smoke if available; verify descriptions and fallback.
7. Extend agent-terminal Josh E2E with isolated environment/PATH mutation, command-specific completion, bounded-capture failure, a foreground TUI fixture, Ctrl-Z/fg, and complete cleanup.
8. Update the 66-page mdBook, status matrix, architecture, CLI/reference, completion, environment, capture, signal/troubleshooting, roadmap, and release evidence. Keep general jobs/modules excluded.
9. Directly inspect final sessions, process groups, PIDs, sockets, terminal state, and temporary roots. No long-running/background development process may remain.
