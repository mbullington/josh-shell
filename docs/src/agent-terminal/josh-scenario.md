# Automate the complete Josh scenario

<a id="AT-JOSH-001"></a>
## Cross-product scenario

**Host command**
```sh
cd agent-terminal
scripts/josh-e2e.sh target/debug/agent-terminal "$(command -v josh)"
```

`agent-terminal` comes from the companion repository (crate `agent-terminal-cli`, binary `agent-terminal`); the scenario accepts any pair of `agent-terminal` and `josh` binaries, from a build tree or `PATH`.

The script creates isolated HOME, XDG config, PATH, runtime, and file roots. `env.josh` and `init.josh` establish shared lexical state and `cfg:env-init>` as the prompt. The 80×24 session exercises:

- objects, declarations/arrows, snapshot closures, calls/member/index access, and lexical UFCS;
- if/else, while, return, break, continue, throw/catch, command errors, status, and command chains, with side effects proving both taken and short-circuited branches;
- exact values and cardinalities for `text`, `json`, `lines`, `jsonl`, `chunks`, function, map/filter/take/first/collect, value-to-external serialization, and value-to-text serialization;
- every redirection family with left-to-right descriptor behavior;
- complete sorted wildcard, bracket, and recursive glob outputs, quoted literals, and no-match errors;
- explicit rejection of `&`, `jobs`, `fg`, `bg`, `import`, `export`, and remote imports.

Before each input, the script records the current terminal revision. It accepts completion only after a later revision whose last nonblank row is exactly the configured prompt, so echoed command text cannot satisfy an output assertion. Structured and glob results are compared as exact host-file bytes.

It records `target/josh-e2e/snapshot.json` and `screenshot.png`. The schema-v2 snapshot asserts semantic style, wide-cell, background-only, captured palette/default/cursor state, prompt, running-process, and 80×24 grid facts. Each run renders the same revision twice and requires byte-identical 640×384 RGBA PNGs with no text/time metadata. The 2026-08-15 retained artifact has SHA-256 `5956b699529a9d8e3167078e8682339c811c80e7f56cb4d8a69c67abd50d9428`; both complete verification runs reproduced it.

After `exit 0`, the script requires an exact exited/code-0 process state. `close` must then leave `sessions=[]`, no control socket or session directory, and no live producer, external child, Josh child, or daemon PID. The temporary HOME/XDG/PATH/runtime/root is removed; only the semantic snapshot and PNG review artifacts remain.
