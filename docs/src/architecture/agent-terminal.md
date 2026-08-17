# agent-terminal daemon

<a id="AT-PTY-001"></a>
## PTY and Ghostty ownership

The child receives three duplicated PTY slave descriptors. Pre-exec creates a session, sets the controlling terminal, and establishes the child process group. Environment preserves caller entries while setting conservative terminal identity variables and fixed initial dimensions.

PTY reads are drained in bounded batches and fed directly to `ghostty_terminal_vt_write`. Ghostty effects append bytes to a pending queue during the synchronous call; the event loop flushes them afterward to avoid callback reentrancy. Grid references, render colors, and borrowed strings never survive an await or terminal mutation.

Snapshot code uses the pinned formatter, viewport grid/cell/style APIs, and narrow render-state APIs for active colors and cursor facts. Key encoding updates from current terminal modes before each event. Resize applies PTY winsize and Ghostty grid state within one serialized command.

`screenshot` sends the ordinary Snapshot protocol request, validates and renders copied schema-v2 cells/styles/render facts in the short-lived client, and writes the PNG there. The daemon does not own fonts, pixels, or arbitrary output paths. The canonical Nix development shell built and executed the pinned library, renderer tests, smoke script, and complete Josh scenario on this host.
