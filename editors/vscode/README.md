# Josh VSCode Extension

Syntax highlighting and parser diagnostics for Josh scripts (the shell and
language live in the repository root). Diagnostics come from the `josh` binary
itself (`josh lsp`), so the editor always agrees with the shell.

## Features

- TextMate syntax highlighting for `.josh` files (comments, strings with
  `${}` interpolation and `$()` captures, keywords, operators, redirects).
- Errors and warnings from `josh_syntax::parse` as you type, including
  incomplete-syntax (unclosed quote/brace/capture) squiggles.

## Requirements

The `josh` binary must be on your `PATH` (build it with `cargo build --release`
in the Josh repo and install or link it somewhere on `PATH`).

To override how the server starts, set `josh.server.command` to an argv array,
e.g. `["josh", "lsp"]` (the default) or an absolute path such as
`["/usr/local/bin/josh", "lsp"]`.

## Install from source

In this directory:

```sh
npm install
npm run package     # typecheck + esbuild bundle + vsce package
code --install-extension josh-0.1.0.vsix
```

## Development loop

- `npm run typecheck` — `tsc --noEmit`.
- `npm run build` — esbuild bundle to `dist/`.
- `../../scripts/lsp-smoke.sh` — stdio smoke test of the bundled server
  (against `../../target/debug/josh`, or set `JOSH`).

After editing, rebuild, repackage, and reinstall the `.vsix` (or use the
Extension Development Host with F5 from a VSCode window open on this folder).
