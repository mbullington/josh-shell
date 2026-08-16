# Semantic snapshots

<div class="status-coverage">

**Status coverage:** [AT-SNAP-001](../status/matrix.md#AT-SNAP-001) — **Implemented**; [AT-PNG-001](../status/matrix.md#AT-PNG-001) — **Implemented**. See [status conventions](../welcome/status-conventions.md).

</div>

<a id="AT-SNAP-001"></a>
## Visible semantic state <span class="status status--implemented" aria-label="Status: Implemented">Implemented</span>

**Availability:** Available in agent-terminal 0.1.0. Evidence: real CLI tests assert Ghostty title, text, fixed grids, row/cell/style facts, wide cells, and background-only cells.

Default `snapshot` output starts with session, grid, cursor, revision, and process metadata, followed by Ghostty-formatted visible plain text. `--json` emits semantic schema v2.

Rows include wrapping, prompt semantics, and text. Cells include grapheme, wide state, semantic content, protection, hyperlink, optional background-only color, and style ID. The deduplicated style table carries colors and text attributes. Render facts include Ghostty's effective foreground/background, active 256-color palette, and optional cursor color plus cursor style, blink, viewport, and wide-tail state. OSC 4/10/11/12 updates therefore remain authoritative in later screenshots.

Revision is monotonic within a live session. Each nonempty PTY batch and resize advances it; snapshots and screenshots do not. It is not a clock or cross-session sequence.

Cell and row traversal uses Ghostty viewport coordinates. Active cursor coordinates remain available separately; the optional cursor viewport position determines whether and where a screenshot draws it. There is no client viewport-scrolling command. The live daemon is authoritative. `screenshot` consumes this copied semantic state client-side; it does not change or rename the schema.
