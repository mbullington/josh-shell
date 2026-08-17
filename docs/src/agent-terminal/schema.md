# Snapshot and protocol schemas

## Semantic snapshot schema v2

A snapshot has `schema_version`, `session_id`, `revision`, tagged `process`, `cols`, `rows`, captured `default_foreground`, `default_background`, the active 256-entry `palette`, `cursor`, optional `title`, `active_screen`, formatter `text`, `row_data`, `cells`, and deduplicated `styles`. The color fields are Ghostty render state copied by the daemon, not client theme substitutions.

<p class="example-label"><strong>JSON shape · emitted by agent-terminal 0.1.0</strong></p>

```json
{
  "schema_version": 2,
  "session_id": "0123456789abcdef0123456789abcdef",
  "revision": 7,
  "process": { "type": "running", "pid": 1234 },
  "cols": 80,
  "rows": 24,
  "default_foreground": { "r": 216, "g": 222, "b": 233 },
  "default_background": { "r": 15, "g": 17, "b": 26 },
  "palette": [],
  "cursor": {
    "x": 6,
    "y": 0,
    "visible": true,
    "pending_wrap": false,
    "visual_style": "block",
    "blinking": true,
    "color": null,
    "viewport": { "x": 6, "y": 0, "wide_tail": false }
  },
  "title": null,
  "active_screen": "primary",
  "text": "josh> ",
  "row_data": [],
  "cells": [],
  "styles": []
}
```

The example abbreviates `palette`, `row_data`, `cells`, and `styles`; live snapshots contain exactly 256 palette entries and complete bounded grid data.

Rows carry `index`, `wrap`, `wrap_continuation`, `semantic_prompt` (`none`, `prompt`, or `prompt_continuation`), and `text`. Cells carry row/column, optional grapheme, `wide` state (`narrow`, `wide`, `spacer_tail`, `spacer_head`), semantic content, protection, hyperlink, background-only color, and style ID. Colors are tagged palette indices or RGB values. Styles include foreground/background/underline colors and bold, italic, faint, blink, inverse, invisible, strikethrough, overline, and underline shape. Cursor `x`/`y` remain active-area facts; optional `viewport` is the visible render position and records whether it is on a wide tail. `visual_style` is `bar`, `block`, `underline`, or `hollow_block`; `blinking` records terminal mode even though PNG rendering freezes it visibly on.

Screenshot JSON is a command result, not schema v2. It contains `path`, `session_id`, `revision`, `width`, `height`, `bytes`, `dpi`, `cell_width`, and `cell_height`. PNG bytes carry no session metadata.

<a id="AT-PROTO-001"></a>
## NDJSON protocol v1

Requests carry `protocol_version: 1`, `request_id`, and a tagged command. Responses repeat the version and request ID and contain either a tagged result or `{code, message, details?}` error. Handshake returns CLI version, protocol version, Ghostty SHA, daemon PID, session ID, and process state.

Error codes include invalid request/message size/version, lifecycle state, dimensions/key, spawn/I/O, timeout/early exit, session selection/health, and internal failure. A version mismatch is explicit; clients do not signal metadata PIDs to reconcile it.
