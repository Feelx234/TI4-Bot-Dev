# D2 lane A (`core-d2-f-live`): the live loop, resume note (2026-10-31)

> UPDATE (lane A2, `core-d2-f-live2`): sections 4-5 below were refreshed; what lane A2 did, how to run the cheap playthrough and the open list for lane F2 are in `docs/D2_LF2_RESUME.md`.

Branch `core-d2-f-live` = `core-integration-8` (67994b33) merged with `core-d2-l1` (714b0074; no conflicts). Lane A is the SINGLE OWNER of the shared
live-loop contract: KF1/KF2 of `docs/todo/D2-islands-composition.md` (branch `core-todo-d2d`) and M0 of `docs/todo/D2-islands-machines.md` (branch
`core-todo-d2c`) are ONE design, written down here. Every other lane (M1 trade, M2 production, M3 invasion, M4 combat, M5 aftermath, M6 tactical, composition
lanes B-F) starts from the `INTERFACE:` commit of this branch. Never push; never touch `nightly-fixes-2026-10-26`.

## 0. User decisions that bind every lane (2026-10-31)

* **The Thunder's Edge STRATEGY CARD SET (`card_set` "te": `te4_construction`, `te6_warfare`, `TeWarfare`, the TE primaries and secondaries) is IN scope** and
  must run natively in the core (all 14 alpha games use it). Other TE content (breakthroughs, Heart of Ixth, Fracture first claim, Saar Deorbit Barrage,
  Arborec Psychospore, the Yin breakthrough) stays OUT. So the KX strategic-action package (lane E) ports the te cards' primaries/secondaries, the `Warfare`
  `StrategySecondaryAfterTactical` flow (`Window::StrategySecondaryAfterTactical` stays a live kind), and the zero-bail corpus plays the te set.
* **The `playthrough` bin plays what the server plays**: `card_set = te`, `starting_tech = true`, `thunders_edge = false` (closes the corpus blind spot of the
  in-game starting-technology question, `AskArm::StartingTech`).
* The guiding rule: an ENTIRE game in the new engine without any bail on the agreed content; the core owns the loop; an embedded legacy `Game` only as a
  bridge-only test harness, never in production builds.

## 1. The merged design (what is decided)

* **Persistence = typed payload on the `Pending` stack + `Pending.nested`** (the machines doc's design; checkpoint and fork = plain `CoreState::clone`). The
  machine OBJECT (with its children) is rebuilt from `(payload, position)` at every call by `Held::from_pending` and is run by a fresh `Driver`; it is written back
  by `Held::to_pending` when it asks and removed from the stack when it ends. No wrapper-held window, no `LiveSlot`. I found the journal sound: the driver already
  replays a step from its start (`Driver::settle` snapshot), so a suspended nested question is exactly `(operation, answers so far)`; the payload on the stack is the
  window as the driver rolled it back. Proven by `driver::live::tests::a_nested_question_survives_a_dropped_driver_and_continues_to_the_same_position`.
* **A blocking table never needs the journal** (the string's `Table::ask_seeing` is synchronous, so the engine half answers a nested question inside the same
  `live_step`). The journal exists so a caller that returns between questions (D3 suspend/resume, the oracle's pull style, a clone mid-window) continues identically.
* **One `live_step` = one string `step()`**: ask the arm's pending question once, apply the answer, put the nested questions of the answer to the same asker,
  stop at the arm's next stage question (the next step's `legal_options`) or when its window ended. A failed step restores the position it started from.
* **Facts and ability tables**: the live `Env` carries the kernel, the ability registry of every seat, the `AbilityTable` and the game's seeds. A live driver is ALWAYS
  seeded. Building the registry from the core alone (instead of `reactions_core::Follow::armed/arm`, which read the string `GameState` and the `Render` codes) is OPEN
  (section 4, item 1): lanes whose windows have no nested abilities do not need it.

## 2. Stable interfaces (do not change signatures; add, do not rename)

| Where | What |
|---|---|
| `ti4-engine-core/src/state/pending.rs` | `Window::Invasion(Box<state::Invasion>)`, `Window::Production(Box<state::ProductionUse>)` (STUB payloads in `state/invasion.rs`, `state/production.rs`: `{actor, data:[u32;3]}`; lanes M3/M2 REPLACE the body, keep the names); `Pending.nested: Option<Nested{op: NestedOp, answers}>` (written only by `driver::live`) |
| `kernel/stage.rs` | `trait Held: Window { const KIND; fn from_pending(&CoreState,&Kernel)->Option<Self>; fn to_pending(&self,&mut CoreState); fn closed(&self,&mut CoreState,&Kernel) {} }` (L3: rule areas implement it) |
| `driver/live.rs` | `Env{kernel,registry,table,seeds}`, `advance::<M>(core, env, Option<Pick>) -> Result<(Live, Out), LiveError>`, `ask::<M>(core, env) -> Result<Option<Asking>, LiveError>`, `open::<M>`, `is_open::<M>`, `Live::{Asks(Box<Asking>), Done}`, `Asking{choice, nested: Option<(NestedFacts, Box<CoreState>)>}`, `Out{emitted, opened}`, `LiveError::{NotOpen, NotAsking, Illegal, Driver(..), Unknown(..)}`; the ARM REGISTRY `live_arm(Arm) -> Option<LiveFn>` and `ask_arm_fn(Arm) -> Option<AskFn>` (one line each per arm: THE only edit a lane makes in this file) |
| `driver/mod.rs` | `Driver::{restore_stage, restore_nested, reask, nested_journal}`; `Tape::{answers, set_answers}` |
| `driver/step.rs` | `step(core, &Facts, Needs, NativeArms) -> Step`; `NativeArms::{NONE, ALL, of, has, registered, bits}`; `Step::Native(Arm)` (the arm is in the set AND has a live function; otherwise `Step::Lend` exactly as before, so `NativeArms::NONE` leaves the dual untouched) |
| `driver/session.rs` | `Facts::of_core(core, blocked)` (prepared turn, closing menu, starting picks from the position), `Arm::ALL`, `AskArm::Trade` + `live_ask_arm(facts)` (the dual's `ask_arm` is unchanged: the two merge when the trade island is native) |
| `ti4-engine/src/live/` | `live_step(core, arm, &Cx, &mut dyn Asker) -> Result<StepOut, Stop>`, `Asker`, `Question`, `Scripted`, `Cx::new(&Render, &Env)`, `Stop::{Illegal, Exhausted, Blocked, Live(..), NoLiveFn(..)}`, `render_asking`, `pick_of`, `details::complete` (core-only display facts; add one `match` arm per family), `arms::{name, parse, names, from_env}` (`TI4_NATIVE_ARMS`) |
| `ti4-engine/src/dual.rs`, `product.rs` | `TI4_BAIL_IS_FAILURE=1` / `dual::set_bail_is_failure` / `with_bail_is_failure`: every counted bail, gate and lend is ALSO a `Difference` (area `Driver`, category `Value`, label `bail::<label> [<why>]`); `product::{note_lent, lent_count, take_lent}` |

### How a lane turns an arm native (the recipe)

1. State: the payload type in `ti4-engine-core/src/state/` (plain, text-free, `Clone + PartialEq + Eq`); a field exists only for what the position cannot re-derive.
2. Machine: `impl Window for X` + `impl Held for X` (`KIND`, `from_pending`, `to_pending`) in your rule file. `ask_of` must be a function of the payload alone.
3. Register: add `Arm::X => Some(advance::<X>)` to `live_arm` and `Arm::X => Some(ask::<X>)` to `ask_arm_fn` (`driver/live.rs`).
4. Display facts: if the string question carries `details`, add a family arm to `live/details.rs::complete` computed from `CoreState` only.
5. Test: an engine test like `live::tests::the_token_gain_arm_run_by_the_core_...` (string window and live arm side by side: questions, answers, positions,
   plain events) and a core test with `Rig`/`advance` incl. `clone mid-window continues identically`.
6. Remove the arm's `Lent` use only when its close (the part the string still does) is core code too; until then the arm returns `finished` and the caller
   runs the lent remainder (see `Arm::Tokens`: the close, steps 81.6-81.8, is KS5's).

## 3. What is proven (vertical slice)

`Arm::Tokens` (the status phase token gain, `rules/token_gain.rs`, lane C extends it as KS4) runs natively end to end through `live_step` and is compared with the
string's own `tokens::TokenGain` (`live::tests::the_token_gain_arm_run_by_the_core_asks_what_the_string_asks_and_leaves_the_same_position`): prompt, option ids, kinds,
labels, `DecisionContext`, display `details` (pools, reinforcements, Letnev's hint, tokens to place), answers, token pools after, plain events `COMMAND_TOKEN_GAINED:<Pool>`,
`finished` exactly when the string `is_complete`. Failure paths restore the position (`a_failed_live_step_restores_the_position_it_started_from`). The journal
round trip, the illegal-answer rollback and the clone-mid-window check are core tests (`driver::live::tests`, `rules::token_gain::tests`).

## 4. Also in the branch (beyond the interface commit)

* **KF3** `driver/kernel_env.rs`: ONE `KernelEnv{core, kernel}` implements `leaders::Env`, `strategy::Env` and `abilities::Env` from the hook table and the core
  queries. Agreement with the string views is `ti4-engine-core-bridge/src/check_kernel_env.rs` (+ `tests/kernel_env.rs`): every method, every seat, every leader/technology/
  card kind, over the template openings and the `check_leaders` variants. Departures (agreed scope): `has_specialty_research_planet` = false (Jol-Nar breakthrough, TE),
  `other_action_resolves(XxekirGromTe)` = false, no Keleres agent term.
* **`live/kernel.rs`**: `kernel_for(sources)` is the hook table with EVERY faction module (`register_all`). `core_adapter::kernel_for` is the movement table only; the live
  loop must use this one.
* **`live/env.rs`**: `env_for(&Render, &CoreState, kernel, seeds) -> Env` builds the registry/table from the core position (no string `GameState`), sharing
  `reactions_core::{table, set_table_codes, id_text}` with the replay path. The `plays` facts of an action card played inside a window are `live::plays` (A2); build the `Env` with `env_with_plays` (the bare `env_for` leaves them out).
* **KF4**: `Pending::failed_component_actions` (+ `note_failed_component`, cleared by `turn_flow::forget_failed_components` (the live turn-end composer calls it; `end_turn_scratch` is lent to the dual without the Pending slice)). It is not in any codec: the stack is not serialized.
* **KF6**: `window_driver::driver_live`; the 99 `driver_for` sites stay (replay path); a site is converted when its live opener goes through `live::advance`.
* **Nested rendering from the core**: `dual_window::NestedPos` (+ `of_core`), `DecisionContext::{about_battle_in, about_invasion_in}`; `live::render_asking` completes nested questions.
* **`playthrough` bin** (`ti4-engine-core-bridge/src/bin/playthrough.rs`): whole games per lobby faction (seat a), 4/6 players, `card_set te`, `starting_tech`, all areas
  `Dual` by default, `TI4_BAIL_IS_FAILURE` on; prints per-game lines, bails by cause (the `zero_bail.py` shape), unmapped, lent, live coverage; exit 1 unless ZERO BAIL.

## 5. Lane A: what was open and where it stands (refreshed by A2)

1. `plays` facts for `Env`: DONE in A2 (`live::plays`, `AdapterFacts::plays_provider`, `driver::live::table_at`, `live::env::env_with_plays`; the bridge check `check_play_facts` compares them with the string facts field by field; merged with `core-w-l11`, whose `SeatPlayMore`/`SharedPlay` are filled from the core too).
2. `NativeSubject`: DONE in A2, built as a SHADOW run behind a legacy game rather than the plan above (the legacy game steps for real, no cascade): `ti4-engine-core-bridge/src/native_subject.rs`, `oracle --native ARMS`; see `docs/D2_LF2_RESUME.md`.
3. Final KF2 validation (`drive_live` against the replay `drive` over the `dual_window!` corpus): still open; the shadow oracle covers the arm level (one arm exists: tokens). Start with `strategy::secondary` after lane E ports its follower facts.
4. The two empty hook slots (`commander_unlocked`, `leader_action`): open, lane F2, exact list in `docs/D2_LF2_RESUME.md`.
5. `playthrough` for the island lanes: DONE in A2 (`--quick`, per-island table, gate-profile recipe).

## 6. Checks run (cheap) and results

* `cargo test -p ti4-engine-core --lib`: 464 passed. `check_layers.py core`: 0 violations; `lint_core.sh`: 0 hits.
* `cargo test -p ti4-engine --lib` (default areas): 4894 passed after the fix below (the first run failed 7 tests: clearing `failed_component_actions` inside `end_turn_scratch` needs the `Pending` slice, which the dual does not lend; it is its own step now).
* `TI4_AREAS=all=dual TI4_DUAL_FAIL_FAST=1 cargo test -p ti4-engine --lib -- dual:: dual_window window_driver live:: decision_context d1_pending d1_action reactions_core product::`: 99 passed. `dual::tests::detects_an_injected_difference_in_each_category` fails ALONE under `TI4_DUAL_FAIL_FAST=1` (the env var is read lazily by `init_default` after `serial()` turned fail-fast off): pre-existing, not touched.
* `cargo test -p ti4-engine-core-bridge --test kernel_env`: passes; reports the two empty hook slots (`commander_unlocked` 935, `leader_action` 1265 answers the string gives that no faction module registers yet).
* Single-file mutation checks (Haiku runner, committed snapshot): `driver/live.rs` 12 mutants, 7 killed, 5 survived; `rules/token_gain.rs` 14 mutants, 10 killed, 4 survived. Tests added for 6 of the 9 survivors; 3 are equivalent mutants (`restore_stage` negation is caught by the driver's own `NotAsking`; `left()`'s unreachable default `0`).
* FINDING by the first `playthrough` run: `unsupported::find` (called by `Driver::run` under the gate policy `Panic`) read the Trade slice of a position that a dual window had not lent and panicked in the first technology window of EVERY game of a gate binary. Fixed (`find` reads loaded slices only) with a regression test; `dual_run`/`end_of_d1.sh` had never reached it because the heavy run has not happened yet.
* `playthrough --factions sol --players 4 --seeds 1 --maxsteps 300` (debug build, all=dual, te set, starting_tech): 300 steps in 2 rounds, 575 counted bails; the biggest causes: `d1::step [bail: tactical]` 110, `aftermath` 64, `turn_prelude` 35, `turn::hooks [bail: legendary::end_turn / minister_of_war / technology::end_turn]` 34 each, `ask.closing` 28, `invasion::control_gain_exploration [gate]` 21, `ask.draft` 15, `tokens` 9, `trade` 6, `status` 1, `strategy_announce` 1. A whole game in a debug build under all=dual takes more than 17 CPU minutes for 2500 steps: use a release build for whole games.

Not run: any heavy gate (golden/wide dual, benches, soak, mutation ratchet, census, workspace clippy), the armed dual. (A2's checks: `docs/D2_LF2_RESUME.md`.)
