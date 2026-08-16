# Verified 0.1.0 scope

<div class="status-coverage">

**Status coverage:** [J-CLI-001](../status/matrix.md#J-CLI-001) — **Implemented**; [J-STRUCT-001](../status/matrix.md#J-STRUCT-001) — **Implemented**; [J-FILES-001](../status/matrix.md#J-FILES-001) — **Implemented**; [J-CF-001](../status/matrix.md#J-CF-001) — **Implemented**; [J-FUNC-001](../status/matrix.md#J-FUNC-001) — **Implemented**; [J-CONFIG-001](../status/matrix.md#J-CONFIG-001) — **Implemented**; [AT-PNG-001](../status/matrix.md#AT-PNG-001) — **Implemented**. See [status conventions](../welcome/status-conventions.md).

</div>

Josh 0.1.0 now verifies the six-crate workspace end to end: one lossless parser across tolerant interaction and strict execution; lexical data/functions and UFCS; typed non-job control flow; planned external execution; explicit byte/value streams with stable cardinality and bounded cancellation; redirections and sorted globs; dedicated startup scripts; and Reedline interaction.

agent-terminal 0.1.0 verifies a detached daemon per session, real PTY, pinned static `libghostty-vt`, typed NDJSON protocol, semantic schema, input/wait/resize/lifecycle operations, and deterministic client-side PNG rendering from copied semantic state. The isolated-XDG 80×24 scenario drives the complete Josh capability set, checks excluded surfaces, compares repeated 640×384 PNG bytes, exits, closes, and directly proves PID/socket/runtime cleanup.

The canonical Nix development shell completed format, warnings-denied Clippy, locked tests, locked builds, smoke, and the complete scenario with matching Rust tooling. Remaining limits are deliberate: raw byte capture is whole-buffer; unsupported glyphs render as U+FFFD; lifecycle cleanup is not hostile-process containment; and Jobs, modules, and general source loading are unavailable.
