# Architecture and security

Josh is a six-crate Rust workspace organized by ownership: syntax, runtime/evaluation, process planning, structured streams, Reedline integration, and CLI composition. REPL, `-c`, scripts, and startup files converge on `Engine::run_source`; the runtime depends on an execution-host contract rather than OS details.

Josh executes commands with the user's authority; it is not a sandbox. Planned modules, remote dependencies, and background jobs require separate threat-model review.
