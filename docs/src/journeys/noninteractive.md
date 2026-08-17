# Use Josh noninteractively

Use `-c` for one source argument and a path for a UTF-8 script. Both stop on the first error. Neither accepts trailing positional arguments in Josh 0.1.0.

**Host command**
```sh
josh -c 'printf hello | tr a-z A-Z'
printf 'printf hello | tr a-z A-Z\n' > smoke.josh
josh smoke.josh
```

Both commands write exactly `HELLO` and exit 0 on the verified Unix host. Parse and runtime errors exit 1. Missing `-c` source, extra arguments, or unreadable scripts exit 2. `exit 7` returns 7.

For automation, treat stdout as program output and stderr as Josh diagnostics plus inherited child stderr. Capture redirects only the final pipeline stdout internally; child stderr remains inherited and visible.
