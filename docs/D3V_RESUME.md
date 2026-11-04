# D3 view side (lanes V0-V3): state

Branch `core-d3-view`, cut from `core-integration-20` @ `b8e43ef8d`. Not pushed. Plan: `docs/todo/D3-typed-bot.md` (branch `core-todo-d3-17`), `docs/D3_RESUME.md`. Rules kept: pure refactor, RULES_VERSION 35, serde/fixtures/fingerprints/wire TS untouched, `BOT_VERSION` untouched, no change in `ti4-engine-next/src/game/*`, `ti4-server`, `ti4-host`, `ti4-wasm`.

## Design (differs from the plan in one point)

* `crates/ti4-rule-bot/src/view.rs`: `trait View` (public facts, 37 accessors, signatures of the string `Observed`) and `trait SeatView: View` (the bound seat's hand, 6 accessors). **Object safe; the bot takes `&dyn View` / `&dyn SeatView`**, not `V: View` generics. Reasons: one copy of 21 000 lines (no monomorphisation; plan risk R1, wasm budget), `Box<dyn ObjectivePlanner>` registries and `fn plan(&self, obs: &dyn SeatView, ..)` keep working, closures taking a view stay nameable. Cost: a vtable call per accessor (the accessors are the expensive part of a decision; not measured, no bench run).
* Implemented for the string `Observed` / `SeatObservation` in `view.rs` (pure forwarding: the inherent method wins over the trait method of the same name, so the string bot runs the code it ran).
* `crates/ti4-rule-bot-typed/src/seen_view.rs`: `SeenView` = core `Seen` + `Owners` (seat -> player id table) + the game's `ContentStore`/`SourceSet`/`Galaxy`. No `GameState` is held or read. Borrowed answers are built once into `OnceCell`s. Typed bails (`Seen` returns `None`: ocean-card objective rows, `scoreable_public`) are counted in `SeenView::bails()` (0 in every run so far).
* Two accessors of the string `Observed` were NOT put in the trait because the bot does not need them as they are: `ship_routes_into` (only named in a comment; `Seen` has no routes yet) and `agenda_prediction_of` (the bot only asks whether the seat predicted: trait method `has_agenda_prediction`; no tag text rebuild). `invasion()` returns `InvasionFacts {system, invader, defender}` (what the bot reads), not the whole `ActiveInvasion` (its `phase` text would need the interned text table).
* `Seen::pos_for` gap (plan section 2.2): in this tree it is `Seen::terms`, now `pub fn terms(&self, seat, force_capacity) -> Terms` in `ti4-engine-core/src/seen/objectives.rs`; the thin crate's `ObjView::terms` (lane B2's copy) is deleted and delegates (`Seen::new(core, kernel, seat).with_blanked(..).terms(..)`).
* Public bot entry points keep their concrete signatures (`choose`, `choose_explained` take `Option<&SeatObservation>`); new `choose_explained_view(.., Option<&dyn SeatView>, ..)` is what the typed side calls.

## Files generic over the view

All of `ti4-rule-bot/src` except `random.rs` (no view): `lib.rs`, `answers.rs`, `agents.rs`, `agenda_vote.rs`, `objectives.rs`, `planner.rs`, `faction_plans.rs`, `rift.rs` and all 18 files of `backward/`. (The plan split V1/V2 by file; the call graph does not: `backward/*` calls `lib.rs` helpers (`unit_type`, `home_system`, `faction`, ...), so the closure is the crate.) `decider.rs` keeps the engine's `Decider::choose_seeing(&SeatObservation)` signature and coerces. `Decider` impls in tests keep theirs.

## Equivalence results (all with `bot_shadow --mode view-diff`)

`cargo build -p ti4-engine-core-bridge --bin bot_shadow`, then `bot_shadow --config tools/engine_profile/profile-20.json --games G --mode view-diff [--seen-parity N] --strict [--bot k=v]...`. The string half answers the game; the typed half runs the same bot code on `SeenView` over `Seen` loaded from the same position (`ti4_engine_legacy::seen_load::SeenOwned`). Compared per call: pick, smart flag, rng word position and next word. The bot's cross-call memory (`backward::memory()`: gather/pile; the only state it carries) is saved before the string half, restored for the typed half and compared after (also found a harness contamination: a mismatch in the typed half changed the next string pick through it; fixed in `shadow.rs`).

| run | positions | pick/smart/rng differences |
|---|---:|---:|
| default tuning (v6), games 0,5,11,14,17 | 9 926 | 0 |
| bot_version 1, 3, 5 (game 3 each) | 5 615 | 0 |
| all default-off knobs on (`bw_fleet_shape mecatol_guard mecatol_imperial_priority agenda_value agenda_budget agenda_riders land_all home_guard rift_caution empty_fight_rule ground_rule sftt_rule end_turn_rule secondary_rule winnu_mecatol letnev_armada empyrean_rule home_last_ship r1_trade r1_leadership r1_diplomacy r1_tech2`), games 0,5,11,14 | 9 916 | 0 |

With `--seen-parity N` (N-th call) the harness also compares, through the two views, `probe::forecast_bits` (`crates/ti4-rule-bot-typed/src/probe.rs`): the f64 bit patterns of `agenda_vote::evaluate` for every agenda x {for, against, each player}, `activation_value` of every occupied system, the objective `Goal` points/progress/threshold, and about 350 accessor rows (seat sheets, laws, notes, hand, movable_into, rift exits, can_reach, loadable, spend, ...) hashed to 64 bit. Games 0,5 stride 20: 324 probes, 234 004 rows, 0 differences, 0 bails; games 0,5,11 stride 20: 489 probes, 379 821 rows, 0 differences. **Floating-point forecasts: bit-identical.**

Known and declared equivalent: the string `board()` keeps an entry for a system that has emptied (and `planet_units` entries with an empty list); the typed board has none. The probe leaves empty entries out; `SystemState`'s own equality already treats them alike. `SeenView::model_system` fills units, tokens, planet control and planet units only (as `SeenOwned::to_model_system`); `coexisting` and `purged_planets` stay empty (no corpus game has them).

### Mutation check of `seen_view.rs` (single-file mutants, core tests not needed; `bot_shadow` games 0,5,11 stride 20)

| mutant of `seen_view.rs` | picks/smart/rng | probe rows differing |
|---|---:|---:|
| `controlled_planets` skips the first | 37 (games 0,5) | - |
| `available_spend` +1 | 11 | - |
| `movable_into` capacity +1 | 45 | - |
| seat sheet victory points +1 | 5 | - |
| `loadable` skips the first | 30 | - |
| `planet_is_ready` negated | 88 | - |
| `have` of a progress row +0.5 | 5 | - |
| `can_reach` negated (game 0) | 3 | - |
| `scored_by` drops the secret objectives | 0 | 138 |
| `systems_with_token` of the model system dropped | 0 | 210 |
| `rift_exits_into` = 0 | 2 | 30 |
| `phase` Status -> Agenda | 29 | 13 |
| speaker = seat 0 | 42 | 188 |
| promissory notes skip the first | 0 | 71 |
| strategy card goods +1 | 40 | 0 |
| `objectives_implicating_system` skips the first | 0 | 118 |
| `scoreable_public` +1 | 3 | 245 |
| held action cards skip the first | 2 | 183 |
| held secrets skip the first | 1 | 239 |
| leaders skip the first | 0 | 243 |
| exhausted technologies emptied | 0 | 47 |
| invasion defender dropped | 3 | 4 |

Survivors: laws (`law_outcome`/`laws()` skipping the first agenda of the table) and the unknown-card default of `strategy_card_goods`: no corpus game has that law in play / asks an unknown card; both mutants are equivalent on the corpus, not on the code. A game with every law in play would close the first.


## Size and speed (estimates, wasm not built)

* `view.rs` is about 270 lines: 43 forwarding bodies for `Observed` plus 43 for `SeatObservation` (macro) plus the vtables. Estimate +4-8 KB unzipped, +1.5-3 KB gzip for the wasm module (thunks that mostly inline into the call), against 1.74 of 1.80 MB. Replacing 600 `&Observed` by `&dyn View` adds no monomorphised copy, so no generic bloat. `SeenView`, `probe` and the bridge change are not linked into wasm (only `ti4-rule-bot-typed` links them; wasm does not depend on it). Measure with `docs/BUILD_SIZES_AND_TIMES.md` before the merge; the NO-GO bound is +20 KB gzip.
* Speed: not measured. Added: one vtable call per accessor; the bot makes about 10 000 accessor calls per decision (view_calls.json) of which `system`, `seat`, `movable_into`, `loadable` allocate: expect noise-level change. `SeenView::board()` scans `System::ALL` once per view (about 140 systems) - fine for a differential, to be replaced by the core's touched-systems list when `SeenView` becomes the production view.

## BOT_VERSION

`crates/ti4-rule-bot/src/tuning.rs:168` says `BOT_VERSION: u32 = 6` and `MAX_BOT_VERSION = 6` (v6 = v5 + `muaat_keeps_note`, `rift_risk`, `agenda_second_vote`, "user decision 2026-10-21"); `Tuning::default()` is version 4 (the test `for_version(4) == Tuning::default()`), while `current()` uses `for_version(BOT_VERSION)`. The memory note says "BOT_VERSION stays 4 until the user says ship". Not changed; the code on this line is 6. The view-diff runs above use the shipped tuning (6), and 1/3/5 as listed.

## Not done / remaining

* **V3 pieces left:** replay of stored games from `/root/rulebot-results/*` through `view-diff` (needs a loader for the stored `GameSpec`s; the harness takes `--spec JSON`, so `bot_shadow --spec` per stored game works but was not run in bulk); Thunder's Edge games (`SeenOwned` is `Unmapped` on 9 positions: they show as `seen-unavailable` skips, none occurred in profile-20); the `Unmapped` closure; perturbation test of the bot (hidden fields scrambled, picks unchanged) - `SeenOwned::scrambled` exists, not wired to the bot run.
* Tests added: `ti4-engine-core-bridge/tests/bot_shadow.rs::{the_bot_on_the_typed_view_answers_as_on_the_string_view, the_view_diff_catches_a_mutant}` (1 game, 1500 steps, probe stride 25; 10 tests of that file pass, 95 s). The 190 rule-bot unit tests and `surface_objectives` (7) pass; `cargo test -p ti4-engine-core --lib seen::` passes.
* `answers.rs`/`agents.rs` choice-side touchpoints (lane C) untouched.

## Join notes (merge after core-integration-21)

* Conflicts to expect: `crates/ti4-engine-next/src/queries/objectives.rs` (ObjView::terms replaced by a delegation; two unused imports dropped) and `ti4-engine-core/src/seen/objectives.rs` (`terms` public + `force_capacity`). `Cargo.lock`: `ti4-rule-bot-typed` gained `ti4-content` and `ti4-engine-core`.
* `view.rs` and `seen_view.rs` import by path from `ti4_engine::`: `choice::{Observed, SeatObservation, PublicSeat}`, `objectives::{CardProgress, Imagined, family_token, cost_family_token}`, `objectives_core::{family, cost_progress}`, `production::Spend`, `tactical::Movable`, `transit::{Cargo, CargoSource}`, `generated::IdStr`. If int-21 swaps `ti4_engine::choice::Observed` to the thin crate's own `Observed`, only these `use` lines (and `bot_shadow`'s `ti4_engine_legacy::seen_load::SeenOwned`, which takes the legacy `Observed`) change; the trait methods are the same on both.
* If int-21 touches `ti4-rule-bot` (live-bot mirror fixes), the sed was `&Observed<'_>` -> `&dyn View`, `&SeatObservation<'_>` -> `&dyn SeatView` (plus `'a` forms by hand in `agenda_vote.rs`, `lib.rs::unit_type`, `fleet_shape::cheapest_ship`); re-apply the same on any new function.
