# Heavy fix 1 (core-heavyfix-1, 2026-11-05)

Branch `core-heavyfix-1` from `core-integration-26` (`d7adf5fe8`, tag `heavy-base-int26`). Not pushed, not deployed; the live branch
`nightly-fixes-2026-10-26` was not touched. Rules version stays 36. Private target `/root/deploytmp/hf1`.

The full heavy rerun on int-26 (`/root/end-of-d1-20261105-r2`, finished 19:49 Berlin) FAILED in 6 of 23 steps: `tests`, `bench-off`,
`bench-resolver`, `bench-state`, `bench-resolver+state`, `zero-bail`. Three causes; one commit each.

## 1. tests: `leaders::tests::the_questioning_agents_bail_where_the_core_holds_less` (stale test, not a regression)

* What: one of 5,344 tests; `leaders.rs:3290` asserted `leaders_core::jolnar_hero(..).is_none()` for "Rin with units not in the form the
  owned upgrades demand".
* Root cause: lane `core-lent-gaps` (`dc009b69e`) ported Rin's re-form on purpose (the core window re-forms the units after each swap with the
  faction roster, as the string's `apply_unit_upgrades` does; the old guard compared `GameState::identical`, which skips the units on planets)
  and added `rin_re_forms_a_plain_mech_on_the_home_planet_like_the_string_and_the_core_follows`. The older bail test was not updated.
* Fix (`4a69524b1`): the case now asserts `is_some()` (the former bail stays native), with the reason in a comment. The parity is guarded by
  the lent-gaps test above.

## 2. bench x4: four games differ, all from Rin's re-form (a deliberate rules change, baselines re-recorded for those four games)

The four churn modes differ in exactly the same four games (every other game of the 6 x 200 identical; all 1,200 still `victory`):

| bench | seed / table | old hash (cb9daeb52) | new hash | what changed |
|---|---|---|---|---|
| bench-v2 | 69, 4p 4pHyperlanes arborec,yin,jolnar,xxcha | `5c4e914c3525d3e0` | `ffc9db427f69bbc9` | decisions equal; 2 Transit Diodes offers name `jolnar_mech` instead of `mech` (vp `[7,5,6,10]` both) |
| bench-v4 | 7, 6p 6pStandard winnu,titans,mentak,ghost,arborec,jolnar | `80edc1db03617d4b` | `15c76d5e934491bd` | diverges after Rin (2,921 -> 3,008 decisions), vp `[7,7,7,8,10,9]` -> `[7,6,7,8,10,9]`, same winner |
| bench-v5 | 99, 4p 4pHyperlanes nomad,winnu,jolnar,yssaril | `0f5a4356b536f6a8` | `a317aabe9380c701` | diverges after Rin (2,070 -> 2,557 decisions, 6 -> 8 rounds), winner seat 0 -> seat 2 |
| bench-v5 | 10, 6p 6pOuterRim empyrean,winnu,naaz,mahact,sardakk,jolnar | `d3dac39dbcd83498` | `19fbc2265c2ddc21` | decisions equal; 2 load offers (`load\|8`) differ (vp equal) |

How it was found (single-game repros, no heavy run): the int-25 selfplay (`/root/end-of-d1-20261105/selfplay-new`, `3e7a099ab`) and the int-26
one, each with a throw-away dump of the decision records, on the four seeds. In every game the first differing record follows a Rin use
(`leader_jolnarhero_swap`). Proof of the cause: the int-26 build with ONLY the re-form line of `RinHero` disabled
(`rules/leader_windows.rs`, `apply_unit_upgrades` after a swap) reproduces the int-25 hash of all four games exactly. So no perf-lane change
(lazy board indexes, Env cache, `begin_paid`, `AbilityTable` Arc, lazy `Plays`) and nothing of setup-native / zerobail is involved; the
quick fingerprint, the perf suite hash and the 24 perf games never use Rin, which is why they were equal.

Why it is correct: 90.8, a technology that arrives re-forms the seat's units to what it is now entitled to; the string runs
`apply_unit_upgrades` after each Rin swap, the native core did not before `dc009b69e` (a plain mech on a Jol-Nar planet stayed `mech`; a unit
upgrade gained by a swap left the units in their old form).

Fix (`6bd911642`): in `tools/rulebot/baseline-v2.json`, `-v4.json`, `-v5.json` only these four game entries (termination, winner, vp, rounds,
decisions, hash) are taken from the int-26 bench output; every other game, the meta and the `combined` statistics stay as recorded at
`cb9daeb52` (the statistics are informational, the verify compares hash, termination and vp per game). Regression test (core, native):
`leader_heroes::rin_re_forms_the_seats_units_after_a_swap_when_the_roster_is_held` (red with the re-form disabled).

Observation, not followed up: seed 69 has a plain `mech` on the Jol-Nar home planet before Rin; where it comes from (probably an effect that
places the generic mech, as the lent-gaps resume notes for the agenda "a mech onto a home planet") was not investigated.

## 3. zero-bail: one unclassified cause, `game::close_vote [bail: nested (a fact the adapter did not compute for the window)]`

* Where: dual-lent shard 1, golden-lent game 16 (`--seed 3 --players 4 --map 4pHyperlanes --factions nomad,naaz,letnev,mentak
  --rules-version 23`), the close of Political Censure (the Nomad elected; the Nomad voted for it, the Letnev voted for itself).
* Root cause (instrumented single-game `dual_run --game 16`): the Letnev's reaction slot plays Deadly Plot (when an outcome it did not vote for
  would be resolved). The core `Close` window's first run finds the tape dry at that question, goes on with the default pick (the decline),
  and in that speculative rest of the run pays Future Sight (the agenda not discarded) and evaluates the neighbouring Mentak's Pillage on the
  trade good, which reads the neighbours: the lent window holds no map and the string never asked (in the string the agenda is discarded, so
  there is no gain). `Unknown::MissingFact`. The run is rolled back, but `Tape::unknown` survives the rewind, so the driver kept the unknown
  and the window bailed, although the answered re-run (Deadly Plot played) discards the agenda and pays nothing.
* So it was neither a dual-only cause to classify nor a native rule gap: a counting bug of the core driver (an unknown from a path that is never
  taken). In native play the same mechanism could have turned a speculative unknown into a gap; the native evidence had none.
* Fix (`6e061ef4c`): `Tape` notes what `unknown` held when the run went dry; `take_unknown` reports that value for a dry run (what the rolled
  back rest noted is dropped), the full value otherwise. Regression tests: `agenda_close_tests::deadly_plot_before_future_sight_and_a_neighbouring_pillage_is_followed_in_lockstep`
  (thin crate, lockstep: `(compared, bailed) = (0, 1)` without the fix, `(1, 0)` and no difference with it; Future Sight pays nothing,
  Deadly Plot spent, agenda discarded) and `kernel::ability::tape_tests::the_unknown_of_a_dry_runs_speculative_rest_is_dropped_and_one_noted_before_the_question_is_kept`.
* `dual_run golden-lent.json 20 all=dual --game 16`: `game::close_vote: 6 compared, 0 bailed`, 0 differences, hashes equal.
* The other causes of the step (adapter: `action_cards::announce`, `combat::settle`, `te4_construction`, the Naaz hero, `bombard_step`, ...,
  and the census rows) were all matched by rows of `zero_bail_classes.tsv` with native evidence (`zero-bail.txt` of r2: unclassified 1, gap 0,
  silent 0); this one was the only unclassified cause and is gone by the fix, not by a new row. No row was added or changed.

## 4. Cheap checks on the tip (`6e061ef4c`)

| check | result |
|---|---|
| `native_play --quick --shadow --twice` | fingerprint `538dc68f23dddcc9`, deterministic, ZERO GAP (unchanged) |
| `perf_kernel suite 1` | `suite_hash 775f4136e60a99d5` (unchanged) |
| `cargo test -p ti4-engine-core -p ti4-engine-core-factions --lib` (lockstep) | 484 + 160 passed |
| `leader_heroes` (core tests) | 8 passed |
| `ti4-engine-legacy --lib leaders::tests`, `agenda_close_tests` (`TI4_AREAS=all=dual`) | 88 + 38 passed |
| `clippy_core.sh` | ok, engine warnings 41 = baseline |
| `check_layers.py core / thin / setup` | 576 / 192 / 14 files, 0 violations |
| `check_production_graph.sh` (strict) | 0 violations, 0 failed controls |
| wasm (build-wasm.mjs with binaryen's wasm-opt, size gate) | 6,597,047 B (+511 B on int-26): 6.60 / 2.10 / 1.51 MB against 6.63 / 2.11 / 1.52: ok; no legacy literal |

Note for whoever builds the wasm outside the int-26 worktree: `build-wasm.mjs` finds `wasm-opt` only in `web/node_modules` or the PATH; without it
the raw file is 8.03 MB and the gate says OVER. `TI4_WASM_OPT=<worktree>/web/node_modules/binaryen/bin/wasm-opt` fixes that.

## 5. Rerun of the failed steps

`ZERO_BAIL=enforce SHARDS=3 WORKERS=6 GAMES=20 tools/engine_core/end_of_d1.sh --only build tests dual-all dual-lent bench-off bench-resolver bench-state bench-resolver+state zero-bail --out /root/end-of-d1-20261105-r3`
(detached, target `/root/deploytmp/hf1`). `build` makes the `selfplay-new` the benches verify; `dual-all` and `dual-lent` are the inputs the
zero-bail step reads (and the fix changes what the dual's windows do), so they are rerun too; no other heavy step.

Result (`/root/end-of-d1-20261105-r3/summary.txt`, 22:34 to 01:10 Berlin): **END OF D1 RUN OK (9 steps)**.

| step | result |
|---|---|
| build | ok (319 s) |
| tests | ok (2,712 s), every suite 0 failed |
| dual-all | ok (1,082 s): 34,626,766 calls, 0 differences; `game::close_vote` 141 compared, 0 bailed |
| dual-lent | ok (1,319 s): 38,371,495 calls, 0 differences; `game::close_vote` 161 compared, 0 bailed (r2: 160 / 1) |
| bench-off / -resolver / -state / -resolver+state | ok (about 980 s each): 200 identical, 0 different x 6 in each mode |
| zero-bail (enforce) | ok: 83,546 bails/gates in 52 causes, census 80,186, adapter 3,360, gap 0, unclassified 0, silent 0; native 96 + 288 games ZERO GAP; 24 bench files all victories |

Not rerun (green in r2 and not affected beyond what the dual steps above cover): layers, prod-graph, lint, clippy (run by hand, ok), dual-drv,
dual-wide, board-t/g, sync, census, strict, soak, d4gate. The fix of item 3 changes when the core window driver reports an unknown; strict and
soak run the same driver and were not rerun.
