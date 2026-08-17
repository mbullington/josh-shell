# Pipelines and failures

<a id="J-RUN-003"></a>
## External byte pipelines

Adjacent external stages use kernel pipes. Josh evaluates command words and resolves every executable before spawning any stage. A missing later executable therefore cannot leave an earlier side-effecting stage running.

<p class="example-label"><strong>Runnable example</strong></p>

```console
josh> printf hello | tr a-z A-Z
HELLO
```

Pipeline status uses pipefail: any failed stage fails the pipeline. SIGPIPE from a non-final stage is ignored when a downstream command closes normally, so `yes | head -n 1` succeeds. A final signal or any downstream nonzero exit still fails.

In batch mode, an uncaught pipeline error prints to stderr and exits 1. In the REPL, Josh prints the same structured error and returns to the prompt.

Add an explicit transformer before applying functions to command output. Streaming terminals capture as Arrays even when they emit zero or one item.

<p class="example-label"><strong>Runnable example</strong></p>

```console
josh> doubled = $(printf '1\n2\n' | lines | map (x => Number(x) * 2) | collect)
[2, 4]
josh> doubled.join(",")
2,4
```

Redirections belong to the external stage before them and are planned before spawn. The two descriptor orders below differ.

<p class="example-label"><strong>Runnable example</strong></p>

```josh
sh -c 'printf out; printf err >&2' > combined.txt 2>&1
sh -c 'printf out; printf err >&2' 2>&1 > stdout-only.txt
```

Background execution and builtins inside pipelines remain unavailable. Structured transitions and every documented redirection have explicit implemented paths; unsupported transitions report planning errors rather than becoming argv.
