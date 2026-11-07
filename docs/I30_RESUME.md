# I30 resume

Branch `core-integration-30` = `core-integration-29` (`437ca1bd9`) + `core-multiask-1-fix` (4 conflicts: native_play `--twice`, replay_export
count, wasm budget and its comment) + `core-search-api-1` + `core-determinise-1` + `core-mcts-core-1` (no conflicts; a stack) + `core-mcts-prep-2`
(3 conflicts: workspace members, the lab's `lib.rs` header, the production-graph dev-tool list) + own commits (check registration, the net
evaluator, bench, wasm budget, pin reasons, replay_export comment, docs). Not pushed, not deployed, live checkout untouched. `RULES_VERSION` 36,
`BOT_VERSION` unchanged. Details: `docs/INTEGRATION30.md`.

## Done

* Five merges, each its own `--no-ff` commit; conflict table in `INTEGRATION30.md` section 2.
* One re-baseline: quick game and suite hash **unchanged** (the hit threshold is draw-neutral), pins kept with reasons; goldens reproduce (no
  bless); bench 229 entries re-recorded, all 1,200 victories, verify 200 identical x6; bridge pins pass; shadow sweep 0 first differences.
* Wasm 4,317 B raw over -> raw budget raised by exactly that (approved, at most 5 KB).
* Net evaluator wired into `ti4-mcts` (`NetEvaluator`, `--eval net:PATH`), seat order handled (seating order -> seat index); greedy `mcts_eval`
  runs on the merged line (240 games, 0 errors).
* prep-2's P3 `TurnGap(StrategyAbility)`: the Titan Prototype / Minister of Industry gap, fixed by multi-ask-fix's `ed35c20a7` (confirmed by a
  revert probe); its regression test is in core-tests.
* `check_layers.py` modes setup / chance / resolver / search / mcts in `end_of_d1.sh` and `verify_package.sh`; `ti4-mcts-net` in `clippy_core.sh`
  and the production graph.

## Remaining

1. `mcts_eval` takes no `MctsAgent` yet (`--agent mcts:net=PATH,sims=32/8`, L6 resume); the lab's `Pos` / `Decide` still to move onto C6's
   `Position` / `SeatAgent` (C6 join note).
2. Open from multi-ask (unchanged): server hang of a human seat's reaction inside a bot's step (`MULTIASK1_RESUME.md` 8.2), live vs replay of
   the export at 64 / 65 answers (7.6.1).
3. Flaky timing test `session_deterministic_replay::deterministic_replay_produces_identical_decision_logs` under load (passes alone).
4. Not run: heavy gates (by the brief), web `tsc` / `vitest`.

## Join notes

* `EvalSpec` is `Clone`, not `Copy`; `play_game(.., &EvalSpec, ..)`.
* A new evaluator returning a net value must map the net's seating-order value to seat-index order (`NetEvaluator` does).
* int-29's join notes stand (`Pick::Chance` arms; `search` copies of the step functions; new live callers through `ti4_engine_chance`).

## Final fingerprints

| | value |
|---|---|
| quick game `native_play --quick --twice` | `fd54dc6c956758bc`, 1,110 steps, deterministic (shadowed `7824cabdfbb528b6`, ZERO GAP) |
| `perf_kernel suite 1` | `31fe4e2bdd62069f` (NOSTATE `6881f4a9caf01d02`), 13,751 steps, 12,706 decisions, 297 chance requests / 384 faces |
| golden | 200 games, 146,690 decisions, 0 failed |

## Final wasm sizes

| | raw | gzip | brotli |
|---|---:|---:|---:|
| int-30 real build | 6,665,523 | 2,122,498 | 1,525,693 |
| budget | 6,665,523 | 2,126,000 | 1,532,000 |

Disk: target dirs deleted at the end (`/root/deploytmp/i30-{dev,gate,lock,wasm,probe}`); kept: `i30-logs` (with `ti4_wasm.merged.wasm`),
`i30-bench`, `i30-bench2` (bench outputs, the selfplay copy).
