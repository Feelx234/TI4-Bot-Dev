# D4A resume (lane A of D4: D4-0, D4-1, D4-2a/2b/2c, D4-3)

Status file, rewritten at each stop. Branch `core-d4a` (from `core-integration-4` @ `42e69e3c`), worktree `.claude/worktrees/core-d4a`.

## What exists

* New test-only crate `crates/ti4-engine-core-tests` (kit in `src/`, tests in `tests/`), `wrapper` feature for `tests/wrapper_props.rs`.
* Kit: `Scenario` (typed builder, real factions only, permutable construction order), `Rig` (driver wrapper, transcript, replay, option probe),
  `smoke` (four answer policies + contract + conservation + determinism + snapshot), `delta`, `snap` (`BLESS=<text>`), `inv`, `generators`,
  `props` (P1-P6), `covers`.
* `tools/engine_core/d4/run_core_tests.sh <commit> [BLESS=<text>] [-- filters]`: the runner script (one build pass that reports every compile
  error, then the tests; waits while `/root/.ti4-heavy.lock` is held).

## How to resume

1. `git log --oneline core-integration-4..core-d4a` shows the WIP commits; the newest is the resume point.
2. Run the tests of the newest commit through a runner: `tools/engine_core/d4/run_core_tests.sh $(git rev-parse HEAD)`; fix compile errors and
   failing expectations (a failure is a wrong expectation or a core bug: decide with the rules text; for a core bug write the regression test,
   fix only when local, add a `D4-found-NN` row to `docs/ENGINE_KNOWN_ISSUES.md` of the `core-integration` worktree).
3. Bless the snapshots a new test names: `... BLESS=<area>` (e.g. `BLESS=tactical`), review `git diff` of `tests/snapshots/` against the rules.
4. Delete `/root/deploytmp/core-d4a`, `core-d4a-wt`, `core-d4a-logs` when done.

## Status (updated while the first test run waits for /root/.ti4-heavy.lock)

Written but NOT YET COMPILED OR RUN (no runner has built the crate since the first run at `3c1b36b8`, which compiled and passed ~190
tests; 14 failures there were wrong expectations, fixed since): everything committed after `3c1b36b8`. The first thing to do is one run of
`HAVE_LOCK=1 flock /root/.ti4-heavy.lock tools/engine_core/d4/run_core_tests.sh core-d4a BLESS=<areas>` (a runner queues on the lock).

Test files in `crates/ti4-engine-core-tests/tests/`:

* D4-1 low layers: `containers`, `keys`, `layout` (bits, hex, galaxy, board), `state_basics` (rng, seats, slices, meta, piece),
  `choice_layer`, `movement`, `query_tech`, `query_basic` (seats, economy, laws, board), `query_fleet`, `query_cards` (relic, promissory,
  leaders), `query_objectives`, `query_explore`, `query_trade`, `reach`; the kit itself: `kit`; properties P1-P6: `properties`,
  `properties_windows`.
* D4-2a: `strategy_primaries`, `objective_windows` (ScoreOne, HandLimit, TradePrimary), `trade_window`, `production_rules`, `pay_bill`,
  `leadership`, `strategy_rules`, `scoring_book`, `agenda_vote`, `agenda_reveal`.
* D4-2b: `leader_windows`, `explore_windows`, `relic_windows`, `tactical`, `driver_session`, `leaders`.
* D4-2c: `action_cards` (reads, an every-card smoke in a rich position, five cards against their text), `kernel_hooks`, `kernel_fx`, `reactions`.
* D4-3: `source_hygiene`, `wrapper_props` (feature `wrapper`: P7, determinism, generic refused, P9 report-only).
* D4-0: `tools/engine_core/d4/test_inventory.py` + `test_disposition.tsv` + `docs/D4_TEST_DISPOSITION.md`, `tools/engine_core/d4_gate.sh`.

## Not done

* Rule tests of: agenda `Close`, `Resolve`, `Committee`, `CovertDraw`, `DeadlyPlot`, `OfferDiscard`; production `ProductionWindow`, `SlingRelay`,
  `EconomyWindow`, `HeroOffer`, `Construction`, `Research`; strategy `SecondaryStep`; diplomacy windows (dropped by decision); action card
  `Announce`; driver `menu`, `turn_flow`, `aftermath`, `children`; kernel `resolver`, `seam/*`, `reaction`; `rules/abilities`.
* P8 (checkpoint = replay), P10 (view perturbation); the mutation passes and floors (`mutants.sh`), stage-coverage numbers.
* D4-2d, D4-2e-combat, D4-7, D4-4b, D4-8 (not in this lane).
