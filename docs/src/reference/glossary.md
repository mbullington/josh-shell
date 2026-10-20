# Glossary

**Josh** — JavaScript Object Shell, a Unix-first shell with a JavaScript-shaped deterministic data language.

**Command mode** — lexical/parser context in which bare words form command argv and shell operators delimit pipelines/statements.

**Expression mode** — lexical/parser context in which literals, identifiers, operators, arrays, and expression continuations are parsed.

**Byte stream** — ordered process bytes, normally carried by an OS pipe between external commands.

**Value stream** — planned ordered Josh values with explicit conversion at byte boundaries.

**Capture** — synchronous `$(pipeline)` execution that collects final stdout as String or Bytes after terminal newline trimming.

**Splice** — insertion of a Josh value into a command word; a sole unquoted array variable expands to multiple argv entries.

**Pipefail** — pipeline policy under which any non-suppressed stage failure fails the pipeline.
