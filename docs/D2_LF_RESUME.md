# D2 lane A (`core-d2-f-live`): the live loop, resume note (2026-10-31)

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
  `reactions_core::{table, set_table_codes, id_text}` with the replay path. Missing: the `plays` facts (see its header).
* **KF4**: `Pending::failed_component_actions` (+ `note_failed_component`, cleared by `turn_flow::forget_failed_components` (the live turn-end composer calls it; `end_turn_scratch` is lent to the dual without the Pending slice)). It is not in any codec: the stack is not serialized.
* **KF6**: `window_driver::driver_live`; the 99 `driver_for` sites stay (replay path); a site is converted when its live opener goes through `live::advance`.
* **Nested rendering from the core**: `dual_window::NestedPos` (+ `of_core`), `DecisionContext::{about_battle_in, about_invasion_in}`; `live::render_asking` completes nested questions.
* **`playthrough` bin** (`ti4-engine-core-bridge/src/bin/playthrough.rs`): whole games per lobby faction (seat a), 4/6 players, `card_set te`, `starting_tech`, all areas
  `Dual` by default, `TI4_BAIL_IS_FAILURE` on; prints per-game lines, bails by cause (the `zero_bail.py` shape), unmapped, lent, live coverage; exit 1 unless ZERO BAIL.

## 5. Open in lane A (in this order)

1. **`plays` facts for `Env`** (action cards played inside a live window): `reactions_core::play_facts` reads the string position. Until then such a play stops the step
   with `LiveError::Unknown(PlayFacts)` (a counted bail).
2. **NativeSubject** (oracle, bridge): NOT built. Plan: a `Subject` that holds a legacy `Game` as the lender (bridge only) and, for each step whose `step_arm` is in
   `NativeArms` and has a live function, runs the arm on a `CoreState` materialized from the pre-step `Game::fork()` with `Scripted` answers taken from the legacy step's
   decisions, then compares questions, plain events, `GameState` slices and the closing position with the legacy post-step position (`oracle::<arm>` labels), counting
   `product::note_lent` for every step it leaves to the legacy. The window payload of the pre-step position comes from `d1_pending::observe` (`tokens_of`, ...).
   It needs a small `Game` API (`fork`, observed windows) and a write-back path; that is why it is lane A's next item and NOT done.
3. Final KF2 validation named in the doc: `drive_live` against the replay `drive` over the existing `dual_window!` corpus (the Core backend of ONE area run through
   `live::advance`); needs the `plays` facts for the areas with card plays, so start with `strategy::secondary` after lane E ports its follower facts.

## 6. Checks run (cheap) and not run

See the final report of the lane. Not run: any heavy gate (golden/wide dual, benches, soak, mutation ratchet, census, workspace clippy).
