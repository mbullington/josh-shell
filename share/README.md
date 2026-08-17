# share/ — vendored Josh support libraries

Files here are **stb-style**: single-file libraries you can read, copy, and own.
They follow https://github.com/nothings/stb conventions adapted to Josh:

- Each `*.josh` is self-contained. Its dependencies, if any, are listed in its
  header (in dependency order for `source`).
- Each file carries a fixed header block: name, purpose, one-line usage,
  license, and caveat.
- **License:** public domain (MIT-0). No attribution required; a "vendored from
  josh-shell share/" credit is appreciated.
- **No stability promises.** Copy the file into your own config or project
  (e.g. `~/.config/josh/lib/`); do not reference these paths across machines.
- Every library exposes a `<name>_selftest()` function returning `true` or
  throwing an assertion. `scripts/check-share.sh` runs them all headless.

## Libraries

| File | Purpose |
|---|---|
| `assert.josh` | Assertion helpers with assert-kinded errors: `assert`, `assert_eq`, `assert_ne`, `assert_throws`, and friends. |
| `regex.josh` | RE2-syntax-subset regular expressions (Thompson NFA / Pike VM) in pure Josh: `regex("a+").test("saa")`. Also the dialect's performance benchmark; its golden output is checked in CI. |
