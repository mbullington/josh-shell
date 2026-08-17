# Deterministic PNG screenshots

<a id="AT-PNG-001"></a>
## Pixel renderer

`agent-terminal screenshot [SESSION] [PATH] [--json]` requests schema-v2 semantic state from the selected live daemon, copies it into the client, validates the complete response, and renders there. It does not parse ANSI or scrape formatter text. Omitting PATH writes the platform temporary directory's `agent-terminal-SESSION.png`. Relative paths use the client's working directory. The write truncates an existing file; parent directories are not created.

Default output is the written path. `--json` emits `path`, `session_id`, semantic `revision`, `width`, `height`, encoded `bytes`, `dpi`, `cell_width`, and `cell_height`. Taking a screenshot does not advance the revision.

| Renderer input | Fixed behavior |
|---|---|
| Grid | 8×16 RGBA pixels per cell at reported 96 DPI; an 80×24 grid is 640×384 |
| Fonts | Vendored JetBrains Mono Nerd Font regular, bold, italic, and bold-italic faces under the OFL |
| Glyphs | 13-pixel `fontdue` grayscale CPU rasterization at baseline 13; each scalar in a cell grapheme overlays the same span |
| Missing glyph | U+FFFD from the selected pinned face; never a host-font fallback |
| Default colors | Captured Ghostty effective foreground/background; initial theme RGB 216,222,233 on RGB 15,17,26; OSC 10/11 changes are visible |
| Palette | Captured active Ghostty 256-color palette, including OSC 4 overrides |
| Effects | Explicit foreground/background, face selection, 50% faint blend, inverse, invisible, underline color/shapes, strike, and overline |
| Wide/background cells | A wide-origin glyph is centered across two cells; spacer cells omit glyphs; every cell still paints its background |
| Cursor | Captured bar, block, underline, or hollow-block geometry, optional OSC 12 color, viewport position, and wide-cell width; absent cursor color uses captured foreground |
| Animation | Cursor blink is frozen in the visible phase; text blink attributes do not toggle or hide pixels |
| PNG | 8-bit RGBA, best compression, Sub filter, no time/text/session metadata; `png` and `fontdue` are lockfile-pinned |

Before allocating or painting, the client validates response/request IDs, handshake session and pinned Ghostty identity, snapshot session/schema, dimensions, exact row and row-major cell coverage, unique bounded styles, cursor facts, wide head/tail/spacer consistency, the 256-entry palette, and per-field/aggregate strings. It rejects malformed state instead of clamping. The protocol permits at most 300 columns, 200 rows, 20,000 cells, 1,024 styles, 1,024 hyperlinked cells, and 512 KiB of aggregate snapshot strings. These advertised maxima serialize below the 16 MiB response cap and bound raw RGBA to 10,240,000 bytes.

Determinism means identical semantic state, binary, lockfile, embedded font bytes, and frozen visible blink phase produce identical PNG bytes. A changing terminal revision, captured color/palette/cursor state, cursor location, grid, or build input may produce a different image. Wait for stability before comparing a live session.

<p class="example-label"><strong>CLI sequence · runnable with agent-terminal 0.1.0</strong></p>

```sh
agent-terminal wait "$id" --stable 200ms --timeout 5s
agent-terminal screenshot "$id" terminal.png
agent-terminal screenshot "$id" terminal-repeat.png
cmp terminal.png terminal-repeat.png
```

Semantic `snapshot` output remains the authoritative JSON/text automation surface and is not renamed. PNG pixels contain terminal content and must be handled as sensitive output.
