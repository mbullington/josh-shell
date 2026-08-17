# Type, press keys, resize, and wait

<a id="AT-INPUT-001"></a>
## Input and resize

A key chord is `modifier+...+key`. Modifiers are `shift`, `ctrl`/`control`, `alt`/`option`, and `super`/`cmd`/`meta`. Named keys include enter/return, tab, backspace, escape/esc, arrows, home/end, page up/down, insert/delete, and F1–F12. One printable US-layout ASCII character is also accepted. Key encoding synchronizes application-cursor and Kitty keyboard modes before each event. Alt produces a distinct terminal encoding. A chord, including Super, that has no encoding in the current mode returns `InvalidKey` instead of acknowledging empty input.

Grids are limited to 300 columns, 200 rows, and 20,000 total cells. The current contract uses deterministic synthetic 8×16 cell metrics for winsize pixel fields.

<a id="AT-WAIT-001"></a>
## Wait conditions

`--text` matches a substring of current Ghostty-formatted visible text. `--stable 200ms` succeeds after revision remains unchanged for that interval. PTY reads continue while clients wait. Timeout exits 124 and carries last snapshot metadata; child exit triggers one final check before an exited-before-condition error.

Stability is measured from the later of the last painted revision and the last submitted `type`/`key` input, so a wait admitted immediately after input cannot satisfy until a full quiet interval passes with no new input or output. See the [errata](errata.md) for measured settle budgets when polling snapshots without waits.
