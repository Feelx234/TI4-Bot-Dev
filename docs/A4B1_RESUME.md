# A4b-1 (objectives price): state and resume

Gate deferred until objectives area complete (policy: the heavy gate runs only when a whole area is done;
A4b-2, the `ScoringWindow` bookkeeping and Plots Within Plots, is still open).

## Done (branch `core-a4-objectives`)

* `kernel/fx_pay.rs` (public): `pay_cost`, `pay_first_plan`, `pay_all_three`, `pay_tokens`, `pay_trade_goods`,
  `pay_secret`, `pay_facts`, `PayEnv { scope, blanked }`. B8c-2 (Inheritance Systems) reuses `pay_first_plan`.
* `query/objectives.rs`: `plans`, `PlanC`, `useful_planets`, `first_plan`, `all_three_plan`, `token_pool` public;
  `can_afford` AllThree shares `all_three_from`.
* `rules/objective_windows.rs`: `ScoreOne::with_price(PayEnv)`; the price is paid on the answer, before the score;
  no `PayEnv` means a priced offer is refused.
* `objectives_write::score_one`: priced offers followed (production kernel, priced footprint with Board, Marks,
  Agenda); bails kept for a Bastion/Crimson seat at the table and for a payer holding ocean cards; an answer the
  core refuses is reported (window not finished), not bailed.
* Tests: `objectives_write_tests::price::*` (equivalence with `objectives::pay_for` / `secrets::pay_for` on ~2000
  positions incl. Jol-Nar, Mentak, Keleres; lockstep for all ten bought objectives and both secrets; bails).
* Known-issues lines `A4b-1-*` are in the core-integration worktree (`docs/ENGINE_KNOWN_ISSUES.md`, uncommitted
  there, next to other agents' hunks).

## Checks run

Mutation: 16 mutants, all killed (one survivor, M11, fixed with a test). Cheap: check_layers, lint_core, clippy
(0 warnings in core/bridge/dual/core_sync/render). The verify_package gate was started before the policy arrived
and stopped in the bench stage: tests 41 suites ok, tests-wasm, dual-suite (objectives=dual) ok, golden dual
objectives=dual 20 games 0 differences (scoring::step 385 compared, 27 bailed), bench / v1 / v4 / v5 200/200.
Not completed: bench-v2/v3, string bench, state-size, `driver=dual` golden, board_equivalence.

## Resume (when objectives is complete)

`CARGO_TARGET_DIR=/root/deploytmp/core-a4b1 CARGO_BUILD_JOBS=2 SKIP_AB=1 flock /root/.ti4-heavy.lock
tools/engine_core/verify_package.sh objectives /root/deploytmp/a4b1-verify none 4`, then
`dual_run tools/engine_profile/profile-20.json 20 objectives=dual,driver=dual`.
