# Interactive tests and documentation

Unit checks should pin parser-to-Reedline policy: only Incomplete continues; Invalid submits; spans are UTF-8-safe; completion context selects command, file, or variable without evaluation.

PTY checks should wait for semantic output rather than sleep when a terminal harness is available. Fix dimensions, match prompts, send exact text/key events, and inspect final terminal state. Test Ctrl-C on both partial editing and a bounded foreground child. Always close the session and inspect processes directly.

## Write behavior-first documentation

- Label every behavioral fence immediately before the fence (`Runnable example` versus `**Host command**`).
- Only examples exercised against the current build carry the `Runnable example` label; keep planned or hypothetical snippets out of runnable fences.
- Never call semantic terminal text a screenshot.
- Do not document a source-defined agent-terminal command until the pinned binary executes it.

The deterministic checker validates structure, but reviewers must still check claims against behavior.
