# core-integration-22: the lanes of the D2-9 / D3 side join, and the red tests of int. 21 (2026-11-04)

Branch `core-integration-22`, cut from `core-integration-21` @ `4a6cc5200`. Not pushed; the live branch `nightly-fixes-2026-10-26` was not touched (a detached read-only worktree of it was used for one trace and removed). `RULES_VERSION` 35, serde, generated wire TS types, `host_create.golden` unchanged. Quick game: 1,165 steps, fingerprint `74d5675ac754ac81` (seen after the merges and at the end).

Read first: `docs/INTEGRATION21.md`, `I21_RESUME.md`, `CORE_GAPS_19.md`, `NANOFORGE_RESUME.md`, `D3V_RESUME.md`, `MUTDEBT_RESUME.md`, `D29A_RESUME.md`, `D29D_RESUME.md`.

## 1. Merges (part 1)

| # | Branch | Tip | Conflicts and joins |
|---|---|---|---|
| a | `core-nanoforge` | `aea0284cd` | none. `cargo check --workspace --all-targets` (no torch crates) green; tests `nanoforge` 5, `action_dispatch` 78, `query_cards` 14 pass |
| b | `core-d3-view` | `9dcd089e0` | **no textual conflict** (git merged `ti4-engine-next/src/queries/objectives.rs`, `ti4-engine-core/src/seen/objectives.rs`, `Cargo.lock` by itself; int. 21 had touched 14 files of `ti4-rule-bot`, merged without a new `&Observed` function left over). Three compile joins (commit "join core-d3-view"): (1) `ti4-rule-bot-typed/src/seen_view.rs` named `ti4_engine::objectives_core::{family, cost_progress}`, which after the swap are the LEGACY helpers returning legacy `CountFamily`/`CostFamily`; it uses the thin crate's `objectives::{family, cost_progress_of}` (two names added to the `pub use` line `objectives` of the thin `lib.rs`, so the surface stays native: `surface_paths.py --check` 0 legacy; the baseline got the four native paths `generated::IdStr`, `objectives::cost_family_token`, `transit::Cargo`, `transit::CargoSource` of d3-view); (2) `ti4-engine-core-bridge/src/shadow.rs::view_answer` called `ti4_engine_legacy::seen_load::SeenOwned::of(obs)` with the thin `Observed`: it is the thin crate's `ti4_engine::seen_load::SeenOwned` (as `check_view` three screens above already did); (3) test `bot_shadow::counted_calls` built `spec.build()` (the wrapper game, rules version 35 only: "rules version 23 is not supported") and installed a thin `Decider` on it: it builds `legacy_game(spec)` and wraps the counter in `legacy_adapter::ThinDecider` (an int. 21 leftover, not a d3-view bug). Tests: rule-bot 190, rule-bot-typed 1, core `seen::` 3, `surface_objectives` pass; `bot_shadow` 10 of 10 (1,602 s in the dev profile under load) |
| c | `core-mutdebt-20` | `357c9842d` | none (tests and `// mutants:skip` comments only); the legacy lib run below covers its tests |
| d | `core-d29a-graph` | `8f36fe44d` | none (scripts). `check_production_graph.sh --self-test` ok; `--report` 21 violations, 0 failed controls (all the facade edge `ti4-engine -> ti4-engine-legacy` and the `typed-core` default feature: red until lane 9b strips them, as D29A says); `check_layers.py thin` 5 violations (same reason) |
| e | `core-d29d-aliases` | `dd8fc1243` | none. `cargo metadata --no-deps` ok; every `-p` package of every alias exists; the named test `refusals` is a target of `ti4-host` (not of ti4-server: the dry check printed `[]` because its regexp kept the quote; `cargo test -p ti4-host --test refusals` 3 pass). `cargo c-web` green, `cargo test -p ti4-host --test refusals` green. NOT run (heavy or needs 9b): `t-web`, `t-web-slow`, `b-web`, `t-legacy`, `t-dev` as aliases (the same packages ran as named suites below) |

Lane 9b (`core-d29b-strip`) was not touched. My edits to files it also edits: one line in `ti4-engine-next/src/lib.rs` (the `pub use crate::queries::objectives::{...}` of `mod objectives`: `family` and `cost_progress_of` added) and nothing in `Cargo.toml`.

## 2. Red tests of int. 21 (part 2)

| # | Test | Cause (root) | Fix | Guard |
|---|---|---|---|---|
| 1 | `wrapper_props` (4 of 6 red) | **Not the swap**: the same test on `core-integration-20` fails the same way (second private target, removed). Cause 1: the reference `CoreState` of `round_trip` was built slice by slice WITHOUT the owner table (`Ctx::attach`/`adopt` that `Codec::load` does), so `load(store(x))` (which has the owners) differed from it in `owners` only (diagnosed with the new field-by-field message `differing_fields`). Cause 2: the leak scan built `spec.build()` (wrapper game, version 35 only) from the golden config of rules versions 1..35 | the reference core attaches/adopts like `Codec::load`; the scan plays `legacy_game` through `ThinDecider`; the failure message names the differing fields | the six tests of `wrapper_props` (155th binary) all pass (347 s); they failed before |
| 2 | `ti4-selfplay` `same_seeds_give_identical_games` (`Unported(Thundarian)`) | a readied The Thundarian with a Nomad seated stopped every ground round (`round_unported`) | the composer's `round_body` now loops like the string's `roll_ground_step`: roll, tail, emit `GROUND_COMBAT_ROLL_STEP_ENDED` when `nomad_agents_watches_roll_step`, replay (round number reset, at most `MAX_ROLL_REPLAYS`) when the agent set the replay mark. The Nomad ability row `THUNDARIAN_GROUND` existed already. `Unported::Thundarian` removed | `invasion_composer::a_readied_thundarian_in_a_ground_combat_is_offered_at_the_end_of_the_roll_step_and_replays_it_in_the_core` (fails on the pre-fix code with the counted bail); selfplay test passes (27 s). The whole-game comparison with the string is the Z-gate below |
| 3 | `lobby_all_factions::slow_tricky_factions_play_a_longer_smart_game` (ghost) | **Two causes, then a third gap.** (a) `MAX_TYPES`: fights of more than ten kinds named none. Roll groups are one seat's units, so past ten kinds the groups are the combinations of each seat's own kinds (at most `MAX_SIDE_TYPES` 12 per seat); up to ten kinds the texts are minted exactly as before (fingerprint `74d5675ac754ac81`, 1,165 steps unchanged). (b) The actual Ghost stop was different: a Naaz Eidolon a carrier holds in the space area takes the Z-Grav form at the round start ("flip to ship forms") and the texts did not name it. (c) the game then stopped at `TurnGap(StrategyAbility)`: Minister of Industry in play made `structure_facts` None, so a Construction primary was the string's | (a)+(b) in both copies of `combat_live` (thin `ties/` and legacy), tests `the_texts_of_a_fight_of_more_than_ten_kinds_name_each_seats_combinations`, `..._name_the_z_grav_form_of_a_naaz_mech_the_carrier_holds`, and `..._name_ten_kinds_and_every_combination_of_them_and_none_beyond` rewritten (it asserted "none beyond ten": the rule changed on purpose; now it asserts the groups of the seat's kinds) | (c) Minister of Industry is native in the strategic `Construction` window (rules version 25 and later, the ordinary production window): after a space dock its owner places, a `HomeProduction::at` child runs (the Thunder's Edge choice machinery, now with a `resume` step), `Spawn::Construction` reports what was made (`Report::Produced`) so the strategic action announces it; `structure_facts_with(.., industry)` keeps the Titan Prototype and the action cards on the old `None` (their parents do not announce a production). Test `construction::a_space_dock_placed_by_the_minister_of_industry_opens_the_production_of_its_system`. **The lobby test passes (431 s, gate profile)**. Not compared with the string by a lockstep (the string adapter still declines under the law); no Z-gate game reached the law |
| 4 | `fixtures_verification` | the hand-built sample game used invented ids | `create_sample_game` uses real ids (action cards `dh1 mb1 fs1 sh1`, secrets `dtgs btv uf`, strategy cards `pok1leadership pok5trade pok6warfare`, public objectives `corner develop`); `projection_redaction` looks for the quoted new ids. Regenerated with `TI4_WRITE_WIRE=1`: **`fixtures/protocol/actor_snapshot.json`, `opponent_snapshot.json`, `spectator_snapshot.json`** changed (ids; and the table now has `objective_progress`/`home_system_lost` of the native query); `stale_submission_rejected`, `terminal_game_over`, `event_visibility`, `update-*` and the generated wire TS types are byte-identical | `fixtures_verification` passes |
| 5 | `bluff_reaction` chained holds | see 3 below | none (identical behaviour, documented) | the tests keep `seat_round_holds: 1` |

### Bluff holds (item 5)

Traced the same test (`pass_ends_the_hold_early..`, default policy, declared `space_combat`) with a `HOLDTRACE` line at the top of `maybe_hold` on the live branch (string engine) and here. The string engine's `Resolver::run_window` and the core resolver call `window_skipped` under the same rule (a seat with nothing eligible, once per seat and window), so the set and the order of windows are the same; the group `space_combat` has 13 windows and the live string run reaches `SPACE_COMBAT_STARTED`, `COMBAT_ROUND_STARTED`, `ANTI_FIGHTER_BARRAGE_STARTED`, `UNIT_ABILITY_ROLLED`, `RETREAT_STEP_STARTED`, `HITS_TO_ASSIGN` in a row, each a hold of a declared seat (a hold per window, capped by `seat_round_holds` 8 and the 60 s round budget). The only difference: the native step tells them after the step (with the position the step ended in) instead of while it runs (the status message of the hold is built from `state.round` and the waiting seats, both the same within a step), and the string engine's declared-before-launch timing in that test put the first declaration after the first windows. **Not a behaviour change for a player: two holds in a row existed with the string engine whenever a combat opened several declared windows; the gap between them was the engine work between windows (microseconds).** Not restored; the tests that exercise ONE hold keep `seat_round_holds: 1`. (Honest limit: the live trace could not show the chain because the string game's first question comes after the early windows; the claim rests on the identical resolver rule plus the order of windows in the string trace.)

## 3. Matrices (final binary, `gate` profile)

### 3.1 Plain (the core alone; final binary, `gate` profile)

| run | games | finished, zero gaps | native steps |
|---|---|---|---|
| `--quick --shadow` | 1 | 1 | 1,165, fingerprint `74d5675ac754ac81`, shadow compared 808 |
| 96-set | 96 | **96** | 102,992 (= int. 21) |
| 288-set | 288 | **288** | 298,270 (= int. 21) |

### 3.2 Z-gate (`--factions all --players 3,4,5,6 --seeds 8 --shadow`, four processes by `--players`, two at a time): **all four exit 0**, run twice (once before the Minister of Industry port, once on the final binary, identical)

| `--players` | games | finished, zero gaps | known string bugs | difference of the core |
|---|---|---|---|---|
| 3 | 192 | 192 | 20 | 0 |
| 4 | 192 | 192 | 39 | 0 |
| 5 | 192 | 192 | 34 | 0 |
| 6 | 192 | 192 | 59 | 0 |
| total | 768 | 768 | 152 (= int. 21: 20 / 39 / 34 / 59) | 0 |

The Thundarian port is covered by these shadows wherever a Nomad with a readied agent fights a ground combat (the string and the core agree; no difference).

### 3.3 Suites

| Check | Result |
|---|---|
| `ti4-server` `tests/it`, 75 modules one by one, gate profile, `--test-threads=2` | **75 of 75 green**, 357 tests (incl. `lobby_all_factions` 431 s for the tricky table, `fixtures_verification`, `projection_redaction`, `bluff_reaction` 11, `undo`). int. 21 had 73 of 75 |
| unit suites (dev profile) | host 310 (+3 +2 +2 +3 in its other binaries), rule-bot 190, policy 287 (618 s), wasm `--lib` 9, thin `ti4-engine` 365, bridge `--lib` 141 (534 s), core 479, factions 154, legacy `--lib` 5,321 (+3: two new `combat_live` texts tests and the rewritten one counted as before; + int. 20's mutdebt tests), selfplay 5 of 5 (was 4 of 5), rule-bot-typed 1 |
| D4 crate, one binary at a time (binaries deleted after each) | **155 of 155 binaries, 2,001 tests** (int. 21: 154 / 1,990; new: `nanoforge` + 6 tests of this lane), 0 failures; `wrapper_props` (the 156th file, feature `wrapper`) 6 of 6 |
| `cargo check --workspace --all-targets` (no torch crates), `cargo check -p ti4-wasm --target wasm32-unknown-unknown` | ok / ok |
| `check_layers.py core` / `lint_core.sh` / `clippy_core.sh` / `surface_paths.py --check` | 574 files 0 violations / 0 hits, 4 allowed / ok (engine 41 = baseline 41) / ok (199 paths, 0 legacy; baseline +4 native paths) |
| `bot_shadow` (bridge integration test) | 10 of 10 (1,602 s, dev profile, under load) |
| mutation | one single-file mutant of the new Thundarian loop (replay disabled) was run with the composer tests: killed by `the_thundarians_replay_rolls_the_ground_step_again_without_assigning_its_hits` (the first test I wrote did not kill it; hence the second). No mutation batch of the other new code (budget) |


## 4. String bugs

None new.

## 5. Remaining

* Questions for the user (unchanged, not decided here): Q1 Wormhole Nexus planet as Dannel of the Tenth target (core offers it, string S4), Q2 fixing `secrets::award` in the string (S5).
* No red test left in the lists of this lane. Open items, in order of weight:
  1. The two copies of `combat_live` (thin `ties/` and legacy) are changed alike, but the new texts tests exist only in the legacy file (the thin copy has no tests of its own; it is what the lobby test and the Z-gate exercise). A per-seat cap of `MAX_SIDE_TYPES` 12 kinds still names nothing when a seat alone fields more (not reachable by a normal faction: a seat has at most 7 ship kinds and their twins), and the Airo Shir Aur pre-fight texts (no seat split) still name nothing past ten kinds.
  2. Minister of Industry: the string adapter (`strategy_cards_core::structure_facts`) still declines under the law, so the native play and the string differ there by construction (the string would perform the primary); the native side has one unit test and the lobby test, no lockstep game. A Minister-of-Industry shadow game (law elected, Construction played) would be the check.
  3. The bluff holds claim rests on the resolver rule and the order of windows, see 2.
  4. `t-web`, `t-web-slow`, `b-web`, `t-legacy`, `t-dev` were not run as aliases; their packages ran as named suites. `check_production_graph.sh` and `check_layers.py thin` are red until lane 9b (expected).
  5. Mutation batches of the other new code (texts split, Minister port, fixtures) were not run.
* Disk: free 32 GB at the end (no hourly alarm); target dir removed.

