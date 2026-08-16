# agent-terminal errata

<div class="status-coverage">

**Status coverage:** [AT-WAIT-001](../status/matrix.md#AT-WAIT-001) — **Implemented** (qualified by erratum 1 below); [AT-INPUT-001](../status/matrix.md#AT-INPUT-001) — **Implemented**. See [status conventions](../welcome/status-conventions.md).

</div>

Known behavioral defects and empirical guidance, recorded from real automation
runs against the Josh REPL. Each entry states the observed behavior, the rule
to follow until it is fixed, and the evidence. Full investigation writeup lives
outside the book at `docs/repl-integration-investigation.md` (repo root
relative), with scenarios and harness under `/tmp/josh-repl-test/`.

## 1. (Fixed) Stable waits could satisfy against a pre-input frame

**Affected:** agent-terminal development builds before `ec1d958`
("daemon: gate stable waits on input submission time", 2026-08-16).

**Observed:** `wait --stable` issued immediately after `type`/`key` satisfied
its quiet interval against the last fully painted *pre-input* frame: the
daemon's stability source (`last_revision_at`) only advanced on PTY reads, so
an idle session looked quiet the moment the wait was admitted. The next
`snapshot --json` showed the pre-input grid — typed text absent, cursor at its
old position. Reproduced 3/3 against the Josh REPL.

**Fix:** the session now stamps `last_input_at` after every successful
`type`/`key` write, and the stability window is measured from
`max(last_revision_at, last_input_at)`. A wait admitted immediately after
input cannot satisfy until at least one quiet interval has passed with no new
input *or* output. Regression: `stable_waits_account_for_recent_input` in
`tests/cli_smoke.rs` (`type`, `wait --stable 150ms`, assert ≥150 ms elapsed
and the typed marker visible in the post-wait snapshot), verified live against
the Josh REPL. On current builds, wait-after-input is safe; scripts may still
prefer the settle budgets in entry 2 when polling without waits.

## 2. Settle budgets: measured latencies and recommended waits

Guidance entry; still applies when scripts poll snapshots without `wait`.

**Observed:** the Josh REPL renders a line submission in 4–8 ms of paint time,
but the gap between CLI acknowledgment and the *last* painted revision can
exceed 100 ms. Consequently a 100 ms revision quiet bound or a 100 ms sleep
is **not** a safe settle budget, and tight bounds yield intermittent pre-input
snapshots under load.

**Rule:** after each input burst, sleep 300–500 ms before asserting on
`snapshot --json`, or poll for expected row text with bounded retries
(10 × 100 ms was robust in the investigation harness). Deterministic PNG
capture additionally requires the revision-stable, fixed-grid/fixed-palette
conditions in [screenshots.md](screenshots.md); byte-identical screenshots
were reproducible under those conditions in the Josh e2e.

**Evidence:** `/tmp/josh-repl-test/scen_g.py` and `scen_g1.py`…`scen_g4.py`
(driver-settle and polling variants, including the 3/3 wait-stale
reproduction), plus the investigation report linked above.
