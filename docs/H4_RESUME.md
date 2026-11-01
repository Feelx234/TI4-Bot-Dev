# D2 lane H4 (`core-h4-leftovers`): Doctor Sucaban, the round-end guard, the native display facts, resume note (2026-11-01)

Branch `core-h4-leftovers`, cut from `core-h3-leftovers` (`2e2f2410d`). Never pushed; the live branch `nightly-fixes-2026-10-26` is untouched. Read `docs/H3_RESUME.md` first (tasks 1-5 and the merge findings are there).
Pure refactor: `RULES_VERSION`, serde, fixtures and fingerprints are unchanged (the new `Ask`/`Opt`/`Subtype` variants are never stored; the one new state field, `CombatFields::round_end_produced`, is a core-only payload field the string window does not keep and `fields_back` drops).

## 1. Naalu agent Z'eu (`AbilityGap(41004)`): NOT ported here, it is lane G's and already done there

`core-g4-abilities` (which contains `core-g3-abilities`) ported Z'eu natively in its G3 commit `e5ca79081` (`silent/g3/zeu.rs`, gap rows deleted, lockstep `combat_live_g3_tests`, core test `zeu_removes_the_activation_token_like_the_string`).
The commit depends on the G3 module scaffolding (`silent/g3/mod.rs`, `SilentCodes::g3`), which this branch does not have, so a cherry-pick would be a half merge; per the directive nothing was ported twice.
The four sweeps stay red on THIS branch and nothing was loosened (`combat_live::{an_undefended_system_with_a_space_dock_goes_on_to_the_production_step_in_lockstep, a_space_fight_of_each_faction_runs_native_without_a_bail_and_as_the_string_does}`,
`tactical_live::{neuroglaive_takes_a_fleet_token_from_the_activator_in_lockstep, a_tactical_action_of_each_faction_runs_native_without_a_bail_and_as_the_string_does}`): every failure of every run of this lane is exactly `AbilityGap(Slot(41004))` and nothing else.
They go green when `core-g4-abilities` is merged (G4's own resume note records the sweeps as closed after the G2 merge). Merge note: the gap files (`silent/gaps.rs`, `ability_gaps.rs`, `ability_audit.rs`) are G's and M6d's rows; take theirs, delete by id with `tools/engine_core/delete_gap_rows.py`.
Also for the merge: F2c's `Render::sync_codes` (`8f53669d` on `core-d2-c3-dispatch2`) overwrites blindly; the correct body is H3's checked one (only when one table is a prefix of the other), as H3 recorded.

## 2. Task 4: Doctor Sucaban (and Ssruu's copy of it) in the core Research window, DONE

The string asks BEFORE the technology and before the affordability gate (`paid_research` -> `doctor_sucaban`): for each source (`sucaban_sources`: the seat holding a readied Jol-Nar agent, then the researcher's own readied Ssruu when another seat has the agent, readied or exhausted;
sorted by initiative order from the active seat, an agent before a copy of the same holder) the HOLDER answers `yes`/decline; an accepted offer exhausts the card and the RESEARCHER removes infantry one at a time (`trade_infantry_for_research`), the bill falling by one per unit;
a declined offer does not close the next one; a Ssruu copy that ends with no technology is rolled back as a whole.
* Core (`rules/production/strategy_cards.rs`, `Research`): two stages `Offer` and `Trade` before `Choose`; `begin_paid` computes the sources (`sucaban_sources`, only with a bill and infantry on the board), `next_offer`/`skip_offer`/`after_offers` walk them, `end_paid(landed)` restores the position the copy was offered from
  (`checkpoint: Option<Box<CoreState>>`, the `EmpyreanHero` pattern) when no technology landed. `infantry_sites` lists the places in the string's order (systems by id text, the space area before the planets, planets by id text; the generated enums are in text order).
  An infantry is a piece whose BASE TYPE is infantry; the string reads the unit type id for "infantry": the two agree for every unit (`a_unit_is_an_infantry_for_the_string_exactly_when_its_base_type_is`).
* Types (`choice/strategy.rs`): `Ask::{SucabanOffer { researcher, source, copied }, SucabanTrade}`, `Opt::{Exhaust, Infantry { system, planet: Option<Planet> }}`, `Subtype::{SucabanExhaust, SucabanBorrowedExhaust, SucabanRemoveInfantry}`, `leader_ctx` (the origin is `Content(Leader(DoctorSucaban | Ssruu))`; the trade question's origin is the agent in both cases).
* Renderer (`ti4-engine/src/render/strategy.rs`): the prompts name the researcher (`Doctor Sucaban: exhaust to let {researcher} trade infantry for research`) or the source owner (`Ssruu: copy {owner}'s Doctor Sucaban ...`), option `yes` / `system:planet` or `system:` (labels `an infantry on {planet}` / `an infantry in space at {system}`), parse back.
* `facts::research_asks_more` (core) is gone: `Carried.research` is `true`; the string's `research_asks_more` keeps Specialist Compounds (Thunder's Edge) and the Deepwrought commander (dropped), so those two stay string.
* Not carried: the diplomacy promise the string evaluates for an agent exhausted for another seat (diplomacy is dropped) and the `borrowed_agent_used_state` hook after a landed copy (the hook table has no implementer in either engine).
* Tests: core `research_sucaban` (10: the offer and its context, the trade and the cost, the end of the trade with the bill, a decline, no offer without a readied agent / infantry / a bill, the order from the active seat and the next source after a decline, the copy undone / kept, the places in the string's order, only the researcher's infantry, a bill of one, a Ssruu with nothing to copy);
  engine lockstep `strategy_research_tests::{doctor_sucaban_trades_infantry_for_the_second_research_in_lockstep, doctor_sucaban_of_another_seat_is_answered_by_its_holder, a_ssruu_copy_of_doctor_sucaban_is_undone_without_a_technology_and_kept_with_one}` (string vs core, question by question and position by position),
  `strategy_cards_core::technology_declines_what_the_core_window_does_not_carry` (the Jol-Nar table is carried now: `(1, 0)`), `strategic_live_tests` (the native sweep no longer excuses `TurnGap::StrategyAbility` for a Jol-Nar table). The two old `strategic_arms` tests of `research_asks_more` are replaced by the above.
* Mutation (single file, `D4_COVERS_FILE=/dev/null`, core tests `research research2 research_sucaban strategy_cards_prod2 strategic_arms`): `strategy_cards.rs` lines 755-905 (the new functions) 33 of 37 killed, the 4 survivors are older code (`pay_step`, `exhaust_specialties`) and one `0 -> 1` found by the first run (`begin_paid` with a bill of one: test added);
  the first run found `infantry_of` `&&` -> `||` (test `only_the_infantry_of_the_researcher_are_places_to_take_from`) and a redundant reset (deleted). The remaining survivors of the file are older windows (Leadership, Construction, Titan build, the paid-research answer).

## 3. Task 6: `round_end_produced` is a field of the combat window, DONE (Spinner is lane G's)

`Machine::round_end_produced` (Devotion's hits are produced once per round) lived in the machine object, rebuilt at every call of the live loop; it is `CombatFields::round_end_produced` now (`state/aftermath.rs`), written by `finish_produce`, read by `transition`.
`dual_window::fields_of` fills `None` (the string keeps no such field) and `fields_back` drops it, exactly as `scoring`, so the D1 round trip compares what it compared. Test `combat_machine::the_round_whose_end_hits_were_produced_is_kept_in_the_window_fields` (it cannot fail on old code, which had no field: it pins the contract; mutation of `machine.rs` 400-440: the guard line is killed).
* Spinner (Yin Spinner Omega, `yso`): ported by `core-g4-abilities` (`g4/yin_spinner.rs`, `g4/place.rs`, lockstep `combat_live::yin_spinner_*`); not done here.
* Not started: the Yin hero's scoring hook. "Duranium Armor repair queues": the machine repairs one unit per side per round natively (`Machine::duranium`); the string and the core agree in every lockstep that has a Duranium holder; no further queue was found (H3 listed it as `CombatUnported`, the machine does not raise it for Duranium).

## 4. Task 8 rest

* **Reaction-card previews on the native path: DONE.** `live/reaction_preview.rs` ports `reaction_preview::preview_for` for all 39 cards of `PREVIEWED` over the core position and the typed event of the window (`NestedFacts.event`), attached where the string attaches them: the window's single-card slot option (`payload.card`) and the options of the
  "play an action card" question (`reaction_preview::complete`, called from `live::render_asking` for nested questions). Harness Energy and Infiltrate (the old special cases of `live/details.rs`) are arms of it now. Guard: `every_previewed_card_reads_the_core_position_as_the_string_reads_its_own` runs the string's `preview_for` and the native arm side by side for every previewed card in a full position and in an empty one (the "nothing there" fizzles),
  and the native lockstep tests that already compared the Harness and Infiltrate previews (`tactical_live::harness_energy_*`, `invasion_live::infiltrate_*`) still pass.
* **The exploration's display facts on the native path: DONE** (found by comparing `details` in the invasion lockstep): `live/explore.rs` gives the four commodity cards their `commodities` standing and the infantry that pays its offer card (`exploration::pay_offer`); the native question had neither (`invasion_live::a_first_claim_explores_natively_...` failed on `details` as soon as `assert_same_questions` compared them; it compares them now).
* **The invasion walk question's dice and current die: already native** (the Jol-Nar commander's rerolls in a bombardment carry `dice`/`current` through `combat_live::complete`); with `details` compared in `assert_same_questions` the test `the_jolnar_commander_rerolls_the_dice_of_a_bombardment_natively` passes unchanged.
* **The Jol-Nar flag of the combat environment: DONE.** `driver/windows/combat/env.rs` read only the own unlocked commander; the string reads `promissory::has_commander_ability` (a durable grant, the own unlocked commander, Mahact's Imperia, a faceup Alliance). It calls `query::leaders::has_commander_ability_with` with the Imperia hook now.
  Test `aftermath_chain::the_jolnar_commander_flag_of_the_fight_follows_every_route_of_the_commander_ability` (fails on the old body: the faceup Alliance case).
* **Cause codes of the other staged ground forces: DONE.** `driver/agenda/ground.rs` announced only the rows of an agenda's effect; the rows Plague, Unstable Planet, Nova Seed and the Nekro hero stage stayed staged for ever on the native path (Infantry II and the Saar Trade Convoy never heard of those losses). The flush announces every row that has a text, in the order staged
  (`AgendaCodes::ground_other`, `GROUND_OTHER`, filled by `live/agenda.rs` with `render_destroy`'s words). A row of another cause (a ground combat's) stays staged for its own lane. Tests `agenda_flow::the_staged_ground_forces_of_other_effects_...` (fails on old code), `live::agenda::ground_tests`.
  There is no native lockstep of a Plague/Unstable Planet play (the component-action dispatch of lane F2b is not in this tree); the string flush announces them at the same points the turn prelude does.
* **The cannon offense with several routes: DONE** as a test: the Argent commander's cannon lockstep also moves a cruiser from the ring (several routes: the string lists `details.routes`, the native path produces the same list) and the Sai Seravus test no longer filters `detailsroutes`.

## 5. Not done

* Task 6: the Yin hero's scoring hook. Task 7: ChaosMapping (Saar), Imperial Arbiter, Quantum Datahub swap. The event-window question inside an invasion for non-action-card questions (H3 task 8).
* `cargo test -p ti4-engine --lib` still fails exactly the four Z'eu sweeps (section 1); `ti4-engine-core-bridge` still has the open task-2 test of H3 (F2b's dispatch).
* The engine half of Sucaban's renderer (`render/strategy.rs`) is killed by the lockstep tests (they fail without it), it was not mutated.
* No native playthrough game: no zero-bail game exists on this branch while the G rows (`silent/gaps.rs`: Foresight, Z'eu, Pillage, Tekklar, Scepter, ...) are loud.

## 6. Checks (cheap), private target `/root/deploytmp/core-h4` (`CARGO_BUILD_JOBS=2`, `CARGO_PROFILE_DEV_DEBUG=0`, no incremental; deleted at the end)

See the final report of the lane for the run lists; `check_layers.py core`: 0 violations, `lint_core.sh`: 0 hits.
