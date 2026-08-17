# Errors

Josh is error-first. A completed nonzero external command raises a structured Error by default. Pipelines use pipefail. Batch execution stops and exits nonzero; interactive execution prints the error and returns to the prompt.

Errors cover parsing, undefined identifiers, type failures, excluded capabilities, command lookup, spawn, command/pipeline status, stream decode/JSON, redirection planning, glob no-match, capture, and `cd`. Process failures retain ordered stage outcomes.

`try/catch` consumes thrown values and evaluator/process errors inside its already-parsed body; a parse error rejects the whole source before `try` can run. Caught Error values expose `kind`, `message`, and optional Status-valued `status`.

`status pipeline` handles completed command outcomes, including nonzero status, but does not convert lookup, interpolation, spawn, decode, redirection-open, glob, or type failures. Command conditions and command-mode `&&`/`||` likewise handle only completed nonzero status. Those planning/evaluation failures propagate unless an enclosing `try/catch` catches them.

See [Diagnostics and exit behavior](../reference/diagnostics.md) for stable parse codes and process exit policy.
