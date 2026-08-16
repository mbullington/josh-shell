# agent-terminal errata

<div class="status-coverage">

**Status coverage:** [AT-WAIT-001](../status/matrix.md#AT-WAIT-001) — **Implemented** (qualified by erratum 1 below); [AT-INPUT-001](../status/matrix.md#AT-INPUT-001) — **Implemented**. See [status conventions](../welcome/status-conventions.md).

</div>

Known behavioral defects and empirical guidance, recorded from real automation
runs against the Josh REPL. Each entry states the observed behavior, the rule
to follow until it is fixed, and the evidence. Full investigation writeup lives
outside the book at `docs/repl-integration-investigation.md` (repo root
relative), with scenarios and harness under `/tmp/josh-repl-test/`.

## 1. A stable wait can satisfy against a pre-input frame

**Affected:** agent-terminal 0.1.0, `agent-terminal wait`.

**Observed:** `wait --stable` issued immediately after `type`/`key` can succeed
while the next `snapshot --json` still shows the *pre-input* grid: the typed
text is absent, the cursor sits at its old position, and the revision has not
advanced. Reproduced 3/3 against the Josh REPL with 100–200 ms stable
intervals.

**Why it matters:** [input-wait.md](input-wait.md) says PTY reads continue
while clients wait, but reads are not the same as paints. A wait admitted
before the submitted input has been painted can satisfy its quiet interval
against the last fully painted frame, and a snapshot taken right after the
wait returns reflects that lag. Scripts that treat "wait succeeded" as "all
submitted input is rendered" read a torn, pre-input state.

**Rule until fixed:** do not use `wait --stable` as the first operation after
submitting input. Either sleep past observed paint latency (see entry 2) and
then snapshot, or poll `snapshot --json` rows for the expected text with
bounded retries. `wait --text` is less exposed because its final check matches
post-input text, but follow it with a short sleep before asserting on the full
grid.

**Fix direction:** serialize input submit → paint → wait-admission ordering,
or record the input revision and reject wait admission until it has painted.
Remove this entry and update [input-wait.md](input-wait.md) when fixed.

## 2. Settle budgets: measured latencies and recommended waits

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
