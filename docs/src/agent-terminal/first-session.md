# Your first automated terminal session

<div class="status-coverage">

**Status coverage:** [AT-CLI-001](../status/matrix.md#AT-CLI-001) — **Implemented**; [AT-WAIT-001](../status/matrix.md#AT-WAIT-001) — **Implemented**; [AT-LIFE-001](../status/matrix.md#AT-LIFE-001) — **Implemented**. See [status conventions](../welcome/status-conventions.md).

</div>

Build and verify the binary before starting a session.

<p class="example-label example-label--implemented"><strong>Implemented CLI sequence · Runnable with agent-terminal 0.1.0</strong></p>

```sh
id=$(agent-terminal launch --cols 80 --rows 24 -- /bin/sh)
agent-terminal wait "$id" --stable 200ms
agent-terminal type "$id" 'printf "hello\n"'
agent-terminal key "$id" enter
agent-terminal wait "$id" --text hello --timeout 5s
agent-terminal snapshot "$id"
agent-terminal snapshot "$id" --json
agent-terminal resize "$id" 100 31
agent-terminal close "$id"
```

`launch` prints a 32-hex-character UUID without hyphens. A full ID is authoritative; a unique prefix needs at least eight hexadecimal characters. Omitting the ID works only when exactly one healthy session exists.

Always install an EXIT/INT/TERM cleanup trap in automation. The one-hour idle timeout is a backstop, not normal cleanup.
