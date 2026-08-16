# Welcome to Josh

<div class="status-coverage">

**Status coverage:** [J-CLI-001](status/matrix.md#J-CLI-001) — **Implemented**; [AT-CLI-001](status/matrix.md#AT-CLI-001) — **Implemented**. See [status conventions](welcome/status-conventions.md).

</div>

Josh is a Unix-first JavaScript Object Shell. It combines external commands with lexical objects/functions, explicit structured pipelines, redirections/globs, non-job control flow, startup scripts, and a Reedline REPL. It is not JavaScript, a JavaScript runtime, POSIX shell, or compatibility layer.

`agent-terminal` is a separate Unix automation harness. Its cross-process CLI uses a real PTY and pinned `libghostty-vt`, exposes semantic snapshots, and renders deterministic PNG screenshots with pinned assets.

Background jobs and modules/source remain Planned and are rejected. The [capability matrix](status/matrix.md) is the availability authority.

## Choose a path

- New to Josh: [Your first session](tutorial/first-session.md).
- Exact syntax: [Reference](reference/index.md).
- Terminal automation: [agent-terminal](agent-terminal/index.md).
- Availability: [Capability matrix](status/matrix.md).
- Implementation work: [Contributor guide](contributing/index.md).

The manual does not rely on badge color, icons, or code-fence language to communicate availability.
