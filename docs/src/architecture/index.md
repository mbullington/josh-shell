# Architecture and security

<div class="status-coverage">

**Status coverage:** [J-PARSE-001](../status/matrix.md#J-PARSE-001) — **Implemented**; [J-RUN-003](../status/matrix.md#J-RUN-003) — **Implemented**; [AT-PTY-001](../status/matrix.md#AT-PTY-001) — **Implemented**; [AT-SEC-001](../status/matrix.md#AT-SEC-001) — **Implemented**. See [status conventions](../welcome/status-conventions.md).

</div>

Josh is a six-crate Rust workspace organized by ownership: syntax, runtime/evaluation, process planning, structured streams, Reedline integration, and CLI composition. REPL, `-c`, scripts, and startup files converge on `Engine::run_source`; the runtime depends on an execution-host contract rather than OS details.

agent-terminal is a separate source tree. Each session owns one daemon, PTY master, direct child, and pinned Ghostty terminal. A current-thread event loop serializes terminal access. The typed local protocol returns copied semantic state; the invoking client owns deterministic PNG rendering and file output.

Security boundaries follow ownership. Josh executes commands with the user's authority. agent-terminal launches arbitrary local commands and trusts same-user socket clients. Neither product is a sandbox. Planned modules, remote dependencies, background jobs, and network exposure require separate threat-model review.
