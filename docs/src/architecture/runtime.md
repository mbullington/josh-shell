# Runtime and pipelines

The workspace separates syntax, runtime values/evaluation, process execution, structured streams, interactive editing, and CLI composition. `josh-runtime` owns the consumer-facing `ExecutionHost` contract; `josh-exec` implements it without a dependency cycle.

`Engine` owns lexical frames, closures, and typed control-flow unwinding. It parses once, applies strict policy, evaluates statements in order, and stops on uncaught errors or explicit exit. Ordinary compound values are Arc-backed; resource execution state stays outside Value.

Pipeline planning resolves stage variants, transitions, argv, globs, redirections, and external paths before any stage spawns. Adjacent external stages use OS pipes. Structured boundaries use bounded channels and joined worker threads. Downstream close cancels workers and external producers. Partial spawn failure kills and reaps started children.

Capture drains final output before status resolution. External bytes without a transformer become String or Bytes. Transformer capture follows declared cardinality. Pipefail preserves ordered outcomes and suppresses only the proven downstream-close SIGPIPE case.

Direct byte capture remains a whole-buffer memory risk. Josh has no user-visible Stream value, async runtime, event loop, background jobs, VM, or garbage collector.
