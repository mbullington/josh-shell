# Glossary

<div class="status-coverage">

**Status coverage:** This page makes no product-availability claims. Definitions do not imply implementation.

</div>

**Josh** — JavaScript Object Shell, a Unix-first shell with a JavaScript-shaped deterministic data language.

**Command mode** — lexical/parser context in which bare words form command argv and shell operators delimit pipelines/statements.

**Expression mode** — lexical/parser context in which literals, identifiers, operators, arrays, and expression continuations are parsed.

**Byte stream** — ordered process bytes, normally carried by an OS pipe between external commands.

**Value stream** — planned ordered Josh values with explicit conversion at byte boundaries.

**Capture** — synchronous `$(pipeline)` execution that collects final stdout as String or Bytes after terminal newline trimming.

**Splice** — insertion of a Josh value into a command word; a sole unquoted array variable expands to multiple argv entries.

**Semantic snapshot** — terminal text plus structured grid, cursor, row, cell, style, title, process, and revision facts. It is not a screenshot.

**Session** — one agent-terminal daemon's ownership unit: one PTY, child, Ghostty state, socket, and lifecycle.

**Protocol version** — compatibility discriminator for agent-terminal request/response envelopes.

**Revision** — monotonic per-session counter advanced by processed output batches and resize; not a timestamp.

**Pipefail** — pipeline policy under which any non-suppressed stage failure fails the pipeline.
