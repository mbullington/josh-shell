# Interactive tests and documentation

<div class="status-coverage">

**Status coverage:** [J-REPL-001](../status/matrix.md#J-REPL-001) — **Implemented**; [AT-JOSH-001](../status/matrix.md#AT-JOSH-001) — **Implemented**. See [status conventions](../welcome/status-conventions.md).

</div>

Unit checks should pin parser-to-Reedline policy: only Incomplete continues; Invalid submits; spans are UTF-8-safe; completion context selects command, file, or variable without evaluation.

PTY checks should wait for semantic output rather than sleep when a terminal harness is available. Fix dimensions, match prompts, send exact text/key events, and inspect final terminal state. Test Ctrl-C on both partial editing and a bounded foreground child. Always close the session and inspect processes directly.

## Write status-safe documentation

- Put a Status coverage panel near every title.
- Give each product capability one stable ID and one status heading.
- Include the required availability sentence and explicit exclusions.
- Label every behavioral fence immediately before the fence.
- Call only Implemented examples runnable.
- Keep Specified, Planned, and Unresolved snippets explicitly unavailable.
- Never call semantic terminal text a screenshot.
- Do not promote a source-defined agent-terminal command until the pinned binary executes.

The deterministic checker validates structure, but reviewers must still check claims against behavior.
