# Type, press keys, resize, and wait

<div class="status-coverage">

**Status coverage:** [AT-INPUT-001](../status/matrix.md#AT-INPUT-001) — **Implemented**; [AT-WAIT-001](../status/matrix.md#AT-WAIT-001) — **Implemented**. See [status conventions](../welcome/status-conventions.md).

</div>

<a id="AT-INPUT-001"></a>
## Input and resize <span class="status status--implemented" aria-label="Status: Implemented">Implemented</span>

**Availability:** Available in agent-terminal 0.1.0. Evidence: the real PTY CLI smoke test verifies Unicode input, encoded Enter, and resize.

A key chord is `modifier+...+key`. Modifiers are `shift`, `ctrl`/`control`, `alt`/`option`, and `super`/`cmd`/`meta`. Named keys include enter/return, tab, backspace, escape/esc, arrows, home/end, page up/down, insert/delete, and F1–F12. One printable US-layout ASCII character is also accepted. Key encoding synchronizes application-cursor and Kitty keyboard modes before each event. Alt produces a distinct terminal encoding. A chord, including Super, that has no encoding in the current mode returns `InvalidKey` instead of acknowledging empty input.

Grids are limited to 300 columns, 200 rows, and 20,000 total cells. The current contract uses deterministic synthetic 8×16 cell metrics for winsize pixel fields.

<a id="AT-WAIT-001"></a>
## Wait conditions <span class="status status--implemented" aria-label="Status: Implemented">Implemented</span>

**Availability:** Available in agent-terminal 0.1.0. Evidence: the CLI smoke test and Josh scenario exercise daemon-side text and stability waits.

`--text` matches a substring of current Ghostty-formatted visible text. `--stable 200ms` succeeds after revision remains unchanged for that interval. PTY reads continue while clients wait. Timeout exits 124 and carries last snapshot metadata; child exit triggers one final check before an exited-before-condition error.

Reads are not paints: a stable wait issued immediately after submitted input can succeed against the last fully painted pre-input frame. See the [errata](errata.md) for the input-then-wait rule and measured settle budgets.
