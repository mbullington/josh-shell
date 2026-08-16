# agent-terminal

<div class="status-coverage">

**Status coverage:** [AT-CLI-001](../status/matrix.md#AT-CLI-001) — **Implemented**; [AT-PTY-001](../status/matrix.md#AT-PTY-001) — **Implemented**; [AT-SNAP-001](../status/matrix.md#AT-SNAP-001) — **Implemented**; [AT-PNG-001](../status/matrix.md#AT-PNG-001) — **Implemented**. See [status conventions](../welcome/status-conventions.md).

</div>

`agent-terminal` is a Unix terminal automation CLI. One detached daemon owns one real PTY, child process, pinned `libghostty-vt` terminal, revision counter, and private Unix socket. Separate client invocations launch, send input, resize, wait, inspect semantic state, render deterministic PNGs, and close the process tree.

Semantic snapshots are the authoritative automation state. Screenshots render copied schema-v2 cell/style/render state client-side with pinned fonts, metrics, rasterizer, and PNG encoding. They do not replace semantic snapshots.

The verified 0.1.0 binary passes static-link/build-identity checks, real PTY/protocol/security/lifecycle tests, exact renderer tests, and the complete Josh scenario. See [Installation and build](install.md) and [Deterministic PNG screenshots](screenshots.md).
