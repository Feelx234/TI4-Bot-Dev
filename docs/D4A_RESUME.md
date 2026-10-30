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

## Status (updated at the first full green run)

Commit `8b7c10d0` is green: 517 tests of `ti4-engine-core-tests` pass (profile `lockstep` with debug assertions and overflow checks on for the
three core crates), and the 6 tests of `--features wrapper --test wrapper_props` pass. Later commits add `kernel_small`, `trigger`, accessor
tests in `reactions` and have not been run yet.

Run the tests: `HAVE_LOCK=1 flock /root/.ti4-heavy.lock tools/engine_core/d4/run_core_tests.sh <commit>` (or the session script that does the
same through the scratch worktree `/root/deploytmp/core-d4a-wt` and the one target dir `/root/deploytmp/core-d4a`). The wrapper tests need
`cargo test -p ti4-engine-core-tests --features wrapper --test wrapper_props`.

Mutation "after" numbers: lane B's `tools/engine_core/mutation_ratchet.sh` with `EXTRA_TEST_CRATES=ti4-engine-core-tests MODE=baseline`.
Results of this lane in `/root/deploytmp/core-d4a-mutres` (summary json and survivors per module) and `/root/deploytmp/core-d4a-mutation.out`.

Test files in `crates/ti4-engine-core-tests/tests/`:

* D4-1 low layers: `containers`, `keys`, `layout` (bits, hex, galaxy, board), `state_basics` (rng, seats, slices, meta, piece),
  `choice_layer`, `movement`, `query_tech`, `query_basic` (seats, economy, laws, board), `query_fleet`, `query_cards` (relic, promissory,
  leaders), `query_objectives`, `query_explore`, `query_trade`, `reach`; the kit itself: `kit`; properties P1-P6: `properties`,
  `properties_windows`.
* D4-2a: `agenda_small` (CovertDraw, DeadlyPlot, OfferDiscard, Committee), `agenda_close`, `agenda_resolve`, `agenda_reveal`, `agenda_vote`,
  `production_window`, `production_triggered` (Sling Relay, Integrated Economy, Hero offer), `construction`, `production_rules`, `pay_bill`,
  `strategy_primaries`, `strategy_secondary`, `strategy_rules`, `leadership`, `research`, `objective_windows` (ScoreOne, HandLimit, TradePrimary),
  `trade_window`, `scoring_book`.
* D4-2b: `leader_windows`, `leader_heroes` (Rin, Xxekir Grom), `explore_windows`, `relic_windows`, `tactical`, `fleet_enforce`, `driver_session`,
  `driver_flow` (menu, aftermath, turn flow, children), `leaders`.
* D4-2c: `action_cards` (reads, an every-card smoke in a rich position, five cards against their text, P10 report-only), `kernel_hooks`,
  `kernel_fx`, `kernel_small`, `seams`, `reactions`, `reaction_table`, `trigger`, `resolver_basics`, `abilities`.
* D4-3: `source_hygiene`, `wrapper_props` (feature `wrapper`: P7 on template openings / fixtures / golden positions, determinism, generic refused,
  P9 report-only).
* D4-0: `tools/engine_core/d4/test_inventory.py` + `test_disposition.tsv` + `docs/D4_TEST_DISPOSITION.md`, `tools/engine_core/d4_gate.sh`.

## Not done

* Window tests of: diplomacy windows and Keleres/Firmament/Obsidian windows (dropped by decision: `AgentOffer`, `PlaceCard`, `DiplomacyWindow`),
  the combat and invasion windows (D4-2e, needs int-5, not this lane), action card `Announce` beyond the every-card smoke, kernel `children`,
  `ability` beyond `abilities`/`resolver_basics`.
* P8 (checkpoint = replay): needs the D2 core checkpoint or a string checkpoint API; P10 on the rendered `Choice` (the typed twin runs on
  the action cards, report-only, 0 findings).
* D4-2d, D4-2e-combat, D4-7, D4-4b, D4-8 (not in this lane).
