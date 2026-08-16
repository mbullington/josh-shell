# Sessions and process lifecycle

<div class="status-coverage">

**Status coverage:** [AT-LIFE-001](../status/matrix.md#AT-LIFE-001) — **Implemented**; [AT-PTY-001](../status/matrix.md#AT-PTY-001) — **Implemented**. See [status conventions](../welcome/status-conventions.md).

</div>

<a id="AT-LIFE-001"></a>
## One daemon per session <span class="status status--implemented" aria-label="Status: Implemented">Implemented</span>

**Availability:** Available in agent-terminal 0.1.0. Evidence: cross-invocation smoke and Josh scenarios launch, inspect, close, and directly verify daemon and child reaping.

`launch` starts the same executable in hidden daemon mode under `setsid()`, waits up to five seconds for a protocol handshake, then sends argv, cwd, dimensions, and inherited environment. It executes argv directly; no shell expansion occurs unless the caller explicitly launches a shell.

`list` handshakes with sockets instead of trusting metadata PIDs. It reports starting, running, exited, incompatible, or unhealthy. Stale unreachable directories older than the startup grace period may be removed, but metadata alone never authorizes a signal.

`close` targets the PTY foreground and child leader process groups with HUP, then TERM and KILL after bounded waits, closes the PTY, waits for the direct child, removes socket/metadata, and ends the daemon. Natural child exit leaves final terminal state available until close or idle expiry.

Cleanup strongly covers ordinary terminal process trees. A hostile child can create a new session or escape known process groups; this is not containment.
