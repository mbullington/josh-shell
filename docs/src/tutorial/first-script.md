# Write and run a script

<div class="status-coverage">

**Status coverage:** [J-CLI-001](../status/matrix.md#J-CLI-001) — **Implemented**; [J-RUN-005](../status/matrix.md#J-RUN-005) — **Implemented**. See [status conventions](../welcome/status-conventions.md).

</div>

Create a UTF-8 file. Scripts, `-c`, and accepted REPL lines all pass through `Engine::run_source`; they do not have separate parsers or evaluators.

**Host command**
```sh
cat > greeting.josh <<'JOSH'
name = $(printf Josh)
printf 'hello %s\n' $name
printf 'ready\n' | tr a-z A-Z
JOSH
./target/debug/josh greeting.josh
```

Expected output is `hello Josh`, then `READY`. Script expression results are not printed automatically; commands print through inherited stdout.

Josh 0.1.0 accepts exactly one script path and no script arguments. A parse or command failure stops execution and exits 1. An explicit standalone `exit N` returns `N`, clamped by the operating-system exit-code boundary. File read or CLI-usage errors exit 2.

Use [Diagnostics and exit behavior](../reference/diagnostics.md) when making scripts suitable for automation.
