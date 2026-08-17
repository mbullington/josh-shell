# Welcome to Josh

Josh is a Unix-first JavaScript Object Shell. It combines external commands with lexical objects/functions, explicit structured pipelines, redirections/globs, non-job control flow, startup scripts, and a Reedline REPL. It is not JavaScript, a JavaScript runtime, POSIX shell, or compatibility layer.

`agent-terminal` is a separate Unix automation harness. Its cross-process CLI uses a real PTY and pinned `libghostty-vt`, exposes semantic snapshots, and renders deterministic PNG screenshots with pinned assets.

Background jobs and `import`/`export` modules do not exist yet and are rejected with diagnostics; `source` is the implemented include mechanism. The [roadmap](roadmap/index.md) lists what is planned.

## Choose a path

- New to Josh: [Your first session](tutorial/first-session.md).
- Exact syntax: [Reference](reference/index.md).
- Terminal automation: [agent-terminal](agent-terminal/index.md).
- Implementation work: [Contributor guide](contributing/index.md).
