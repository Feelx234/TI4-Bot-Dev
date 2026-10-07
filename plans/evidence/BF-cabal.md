# BF-cabal: Vuil'raith Cabal work-package evidence

Package plan: `plans/HANDOVER_2026-10-06_CABAL_PI.md`. One section per package, appended in order.
Card text is authoritative from `crates/ti4-content/content/*.json`; the full quoted set for every Cabal
card lives in the `crates/ti4-engine/src/factions/cabal.rs` module doc comment, so each section below
quotes only the text that package implements.

## CB-00: register the Cabal module (skeleton, no assets claimed)

Card text: none claimed. CB-00 registers the module only, so no card clause is implemented here; the
13 Cabal card texts (abilities `amalgamation`/`riftmeld`, techs `dt2`/`vtx`, units
`cabal_flagship`/`cabal_mech`/`cabal_spacedock`/`cabal_spacedock2`, promissory `crucible`, agents
`cabalagent`/`cabalagent_tf`, commander `cabalcommander`, hero `cabalhero`, breakthrough `cabalbt`) are
quoted verbatim in the `cabal.rs` doc comment for the later packages to implement against.

Route: `crates/ti4-engine/src/factions/cabal.rs` — `FACTION = "cabal"`, `MODULE: FactionModule` with
empty claim lists (`abilities/technologies/units/promissory/leaders/breakthroughs` all `&[]`) and
`hooks: Hooks { timing_abilities: Some(timing_abilities), ..Hooks::NONE }`;
`fn timing_abilities(_state: &GameState, _owner_name: &str, _seat: &PlayerId) -> Vec<Ability>` returns
an empty `Vec`. Registered in `crates/ti4-engine/src/factions/mod.rs`: `pub mod cabal;` (between
`borrowed_round_agents` and `ghost`) and `&cabal::MODULE,` inserted in `MODULES` after `&argent::MODULE`
with the array length raised to `[&FactionModule; 15]`.

Tests added: none (CB-00 adds no behavior). Existing roster tests (`every_planned_faction_*`,
`mod.rs:856-1016`) contain no hardcoded module count, so no expectation was edited.

Rule decisions: an empty `timing_abilities` keeps the Cabal seat inert, so a game without a Cabal seat is
bit-for-bit unchanged; `MODULES` order is alphabetical and Cabal implements no hook, so the existing dice
stream and hook order are untouched. Counted the `MODULES` entries instead of assuming: HEAD had 13, the
uncommitted Nomad working-tree edit had already raised it to 14, so the registered total is 15.

Commands and results (all with test debug=0, incremental=false, codegen-units=16, LIBTORCH unset,
`-j1`, `--test-threads=8`, output tee'd to ignored `out/`):
- `rustfmt --edition 2024 crates/ti4-engine/src/factions/cabal.rs crates/ti4-engine/src/factions/mod.rs`
  → OK, no further diff.
- `cargo test -p ti4-engine --lib -j1 factions::cabal -- --test-threads=8` → `test result: ok. 0 passed;
  0 failed; 0 ignored; 0 measured; 2241 filtered out`, exit 0 (`out/cb00-focused-3.log`). Zero matching
  tests is expected: CB-00 adds no behavior.
- `cargo test -p ti4-engine --lib -j1 print_faction_ledger -- --ignored --nocapture` → `test result: ok.
  1 passed; 0 failed`, exit 0, and the ledger prints `cabal      0/14 implemented` followed by the 14
  unclaimed ids (`amalgamation`, `devour`, `riftmeld`, `dt2`, `vtx`, `cabal_flagship`, `cabal_mech`,
  `cabal_spacedock`, `cabal_spacedock2`, `crucible`, `cabalagent`, `cabalcommander`, `cabalhero`,
  `cabalbt`). Nomad shows `9/14`. Log: `out/cb00-ledger.log`.
- `cargo test -p ti4-engine -j1 -- --test-threads=8` → lib `2240 passed; 0 failed; 1 ignored`; one
  integration binary `1 passed`; `tests/decision_delivery_inventory.rs` `3 passed; 1 failed`, exit 101
  (`out/cb00-full.log`).

Blocked (outside CB-00's Writable paths): the only failing test is
`every_producer_and_delivery_site_matches_the_reviewed_registry`, panic at
`crates/ti4-engine/tests/decision_delivery_inventory.rs:1478`, `unreviewed decision sites:`
`nomad.rs ask (Choice)`, `nomad.rs ask (AskObserved)`, `nomad.rs temporal_command_suite (AskObserved)`,
`nomad_agents.rs ask (Choice)`, `nomad_agents.rs ask (AskObserved)`. Every site is in the other session's
uncommitted Nomad files; `cabal.rs` adds no `Choice::new`/`ask`/`ask_seeing` site. Registering them needs
an edit to `tests/decision_delivery_inventory.rs`, which CB-00 may not write, so the full-workspace gate
is reported rather than fixed.

Environment note: the first two focused-test attempts failed with `memory allocation of 2097152 bytes
failed` / rustc exit `0xc0000409` while compiling `ti4-engine` (commit charge exhausted by unrelated
processes, `strata` PID 21840 holding ~66 GB private). The third attempt after the operator confirmed
memory was available compiled in 27.78s, so the failure was environmental and not caused by CB-00.

## CB-02: Reanimator (capture own infantry destroyed beside a Cabal mech)

Card text (`units.json` `cabal_mech`, "Reanimator"): "After this unit is destroyed, capture it.
ACTION: Recover this unit." plus the faction ability text quoted in the module doc comment: "If one of
your infantry is destroyed while in an area that contains one of your mechs, capture it."

Route: `crates/ti4-engine/src/factions/cabal.rs` — `MODULE.units = &["cabal_mech"]`;
`hooks.timing_abilities` → `fn timing_abilities(state, owner_name, seat)` (returns nothing unless
`is_cabal(state, seat)`) → `fn reanimator(owner_name, seat)`: `Ability::stateful("unit:{owner}:
cabal_mech:GROUND_FORCE_DESTROYED:after", seat, "GROUND_FORCE_DESTROYED", Relation::After, …)` with
`.with_stateful_condition(…)` requiring the event player to be the Cabal seat, `is_cabal`, the destroyed
unit's `supply::base_type_of` to be `infantry`, and `reanimator_present` (a Cabal unit whose base type
is `mech` on that planet). Effect: `capture_own_unit` pushes `(captor, unit)` onto
`Player::captured_units`. Helpers: `is_cabal`, `event_player`, `base_type`, `reanimator_present`.

Tests added (`cabal.rs mod tests`, driven through `fixtures::armed_resolver` + `fixtures::with_context`
+ `invasion::assign_ground_hits_in_timing`, which is the engine path that emits `GROUND_FORCE_*`):
`infantry_destroyed_beside_a_cabal_mech_is_captured` (mech + 2 infantry, 3 hits → mech damaged, both
infantry on the Cabal sheet, none left on the planet), `infantry_destroyed_without_a_mech_are_lost`,
`a_mech_in_another_system_does_not_reach_the_hit`, `another_players_infantry_is_not_taken`,
`a_destroyed_cabal_mech_is_not_captured`, `a_mech_taking_a_hit_is_not_captured`, and the §1.2
neutrality test `a_game_without_a_cabal_seat_is_unchanged` (both seats sol, mech + 2 infantry, 3 hits →
nothing captured).

Rule decisions: (1) Reanimator is a unit trigger on the mech card, so it is a stateful timing ability,
not a ground hook (`hooks_ground.rs` has no unit-destroyed hook). (2) The capture row is written
directly because the hit is applied before any window is announced and `supply::capture_from_board`
refuses a captor that is also the owner; `cabal.rs` may not edit `supply.rs`. (3) The condition is
gated on `is_cabal` because `reactions::arm` (`reactions.rs:908-934`) arms every module's
`timing_abilities` for every seat (`factions/mod.rs:577` chains all modules): without the gate the
neutrality test failed with a sol seat capturing its own infantry. (4) The mech is deliberately included
in the fixture: `invasion.rs:1247 apply_ground_hit_with_origin` makes a sustain-damage unit absorb the
first hit, so the mech is damaged but still present when the infantry die.

Commands and results (test debug=0, incremental=false, codegen-units=16, LIBTORCH unset, `-j1`,
`--test-threads=8`, tee'd to ignored `out/`):
- `rustfmt --edition 2024 crates/ti4-engine/src/factions/cabal.rs` → OK.
- `cargo test -p ti4-engine --lib -j1 factions::cabal -- --test-threads=8` → `test result: ok. 7 passed;
  0 failed; 0 ignored; 0 measured; 2241 filtered out; finished in 0.03s`, exit 0 (`out/cb02_focused3.log`).
- `cargo test -p ti4-engine --lib -j1 print_faction_ledger -- --ignored --nocapture` → `ok. 1 passed`,
  exit 0, ledger now `cabal      1/14 implemented` (`out/cb02_ledger.log`).
- `cargo test -p ti4-engine -j1 -- --test-threads=8` → lib `2247 passed; 0 failed; 1 ignored`;
  `tests/content_ids_resolve.rs` `1 passed`; `tests/decision_delivery_inventory.rs` `3 passed; 1 failed`,
  exit 101 (`out/cb02_full2.log`).

Blocked: unchanged from CB-00 — the only failing test is
`every_producer_and_delivery_site_matches_the_reviewed_registry`
(`crates/ti4-engine/tests/decision_delivery_inventory.rs:1478`) on the five uncommitted Nomad sites
(`nomad.rs ask`/`ask`/`temporal_command_suite`, `nomad_agents.rs ask`/`ask`). The panic lists no Cabal
site; CB-02 added no `Choice::new`/`ask`/`ask_seeing` site, and this package may not edit that test file.

Task limits used: timeoutSeconds 1800, noEditSeconds 180, maxToolErrors 2. No build retried for memory.

## CB-03: Amalgamation (return a captured unit to produce that type for free)

Card text (faction ability, quoted verbatim in `cabal.rs` doc comment): "When you produce a unit: You
may return 1 captured unit of that type to produce that unit without spending resources."

Route: new opt-in hook pair in `crates/ti4-engine/src/factions/hooks_economy.rs` —
`EconomyHooks::production_unit_exchange` :181 and `production_unit_exchange_performed` :188, `NONE`
entries :213-214, dispatchers `production_unit_exchange` :383 and `perform_production_unit_exchange`
:398 (modelled on the existing `pds_placement_alternative` / `_performed` pair :159/:172, registration
precedent `factions/titans.rs:91-92`). `production_cost_reduction` :144 was rejected: it carries no unit
type, no `Table`, and is not opt-in. Cabal side: `amalgamation_offer` (cabal.rs:245) and
`amalgamation_perform` (cabal.rs:265), registered at cabal.rs:91-92.

`crates/ti4-engine/src/production.rs` call sites: the exchange is probed before affordability (:2357),
`if !affordable && !exchange { continue; }` (:2372) so a unit that is unaffordable is still offered
through the exchange; the paid option is emitted only `if affordable`; a zero-cost option
`exchange|{id}` (kind `PRODUCE_KIND`, `cost/credit/owed = 0`, `exchange = true`) is added with the same
placement previews (:2495-2532); the resolve arm (:2729-2742) re-checks the exchange (it can have gone
away between the offer and the answer) and ends in `Stage::Placing { made: 1 }` with no credit and no
discount, so `place` (:2478-2544) still charges production capacity and cargo exactly as before.
"Return" is the removal of the row from the captor's `Player::captured_units`
(`crates/ti4-model/src/state.rs:566`) via `supply::return_captured` (:640); types are matched by
`supply::base_type_of`, not by unit alias.

Tests added (cabal.rs:927, 960, 985, 1004, 1034, 1069, 1094): `a_captured_cruiser_is_returned_to_produce_a_cruiser`,
`an_exchanged_unit_costs_no_resources`, `the_exchange_is_not_offered_without_a_matching_capture`,
`the_exchange_is_offered_only_for_the_matching_type`, `an_exchanged_unit_still_spends_the_productions_capacity`,
`a_returned_unit_goes_back_to_its_owners_reinforcements`, `a_non_cabal_is_never_offered_an_exchange`.
Driven through the real `ProductionWindow` (`pending_choice` + `resolve` with `crate::choice::Resolving`);
its fields are private, so the fixture funds the player with `trade_goods` (the only field production
consumes, production.rs:560) and reads capacity from `Quantity::ProductionRemaining` preview deltas.

Gate failure and fix: first focused run `12 passed; 2 failed` (`out/cb03_focused.log`) —
`a_captured_cruiser_is_returned_to_produce_a_cruiser` (`left: 0 / right: 1`) and
`an_exchanged_unit_still_spends_the_productions_capacity`. The test was wrong, not the engine: produced
units enter SPACE (`production.rs:81 pub const SPACE: &str = "space"`, destinations derived at :2788 from
`option.id.strip_prefix("place|")`). Added helper `in_space` (cabal.rs:650) reading
`state.system_state(system).units_of(owner)`; no engine change.

New decision sites: none. The exchange is an option inside the existing production window, so CB-03 adds
no `Choice::new`/`ask`/`ask_seeing` call.

## CB-06: Vortex (exhaust to capture a reinforcement unit beside a Cabal space dock)

Card text (`technologies.json` alias `vtx`, quoted verbatim in the `cabal.rs` doc comment): "ACTION:
Exhaust this card to capture 1 unit of another player's type from his reinforcements that is not a
structure. This unit must be in a system adjacent to one of your space docks."

Route: `Hooks.mapped_component_actions` (`crates/ti4-engine/src/factions/mod.rs:171-180`, signature
carries `&ti4_content::galaxy::Galaxy`) + `Hooks.perform_component` (:182), `Hooks::NONE` entries
:299-301. Chosen over plain `component_actions` :169 because the target test needs map adjacency, and
because generic technology components are hardcoded in `technology.rs:92` (it offers only
`component|tech|sr`), so a faction technology action can only come from the module. The hooks are offered
only when a Galaxy is present (`game.rs:1530-1541`); both dispatchers (`mod.rs:433/444/457`) iterate
every module, so `vortex_component_actions` is gated on `is_cabal` and on `technology_ready`.

Cabal side (cabal.rs): `VORTEX`/`VORTEX_ACTION` :384-385, `technology_ready` :388 (owned and not
exhausted), `decision` :395, `is_own_dock` :411, `own_dock_systems` :424, `vortex_targets` :447 (dock
systems → `galaxy.adjacent()` → board; skips own units, `is_structure()`, and types with
`supply::remaining == 0`; sorted and deduped), `vortex_component_actions` :494, `vortex_perform` :516
(recomputes the targets, asks only when there is more than one, then `supply::capture_from_reinforcements`
and only on success inserts `TechnologyId::new("vtx")` into `exhausted_technologies`, so a refused action
is atomic). `MODULE.technologies = &["vtx"]`.

Tests added (cabal.rs:1287, 1336, 1358, 1376, 1403, 1424, 1456):
`a_vortex_action_captures_a_reinforcement_unit`, `a_vortex_question_takes_the_selected_type`,
`a_refused_vortex_changes_nothing`, `vortex_ignores_structures_and_its_own_players_units`,
`a_vortex_target_must_still_be_in_the_reinforcements`, `a_vortex_needs_the_card_and_the_cabal`,
`an_exhausted_vortex_is_not_offered`. Pattern taken from `factions/saar.rs:1800-1920`.

Fixture defect found and fixed: `vortex_ignores_structures_and_its_own_players_units` failed with
`the dock is what the action reads adjacency from / left: {"06", "54"} / right: {"06"}`.
`fixtures::seated_game` runs `setup::start_game_seeded` + `seating::deploy`, so the Cabal's home
`cabal_spacedock` survived the fixture's clearing and added latent adjacency to every CB-06 test. Fixed by
making `is_own_dock` (:411) the single source of truth for the dock predicate and clearing every own dock
over `state.board.keys()` in `vortex_scene` (:1234).

New decision site (for the delivery registry): `vortex_perform` → `Choice::new(...).contextualized(
decision(state, player, "vtx", "vortex_target"))` with option ids `"{owner}|{unit}"`, asked through
`context.ask_seeing`.

## CB-04: Riftmeld (return a captured unit to ignore a unit-upgrade technology's prerequisites)

Card text (faction ability, `cabal.rs` doc comment): "When you research a unit upgrade technology: You
may return 1 captured unit of that type to ignore all of the technology's prerequisites."

Route: no new engine seam. `StrategyHooks::research_waiver_offer` / `research_waiver_paid`
(`crates/ti4-engine/src/factions/hooks_strategy.rs:122` and :127, `ResearchWaiver` :53,
`ResearchWaiverPayment` :40) already feed `technology::can_research` (:932, which ends
`prerequisites_met(..) || inheritance_systems_ready(..) || research_waiver_offer(..).is_some()`) and
`technology::research_with_waiver` (:1156, which refuses unless `can_research && !prerequisites_met`,
validates the payment id against `waiver.payments`, and restores a state clone when the payment fails).
The ask is built by `strategy_cards.rs:393-471` (`waiver|{index}` then the payment options). Precedent:
`factions/yin.rs:44-45`. `Hooks::waived_prerequisites` (`factions/mod.rs:155`) was rejected: it is a
silent automatic counter, not the opt-in card the text describes.

Cabal side (cabal.rs): `unit_upgrade_base` :290 (finds the unit kind whose `required_technology()` is the
technology and which has `upgrades_from()`, returns its base type — unit-upgrade prerequisites hang off the
unit record, not `technologies.json`), `riftmeld_payments` :307 (ids `captured|{owner}|{unit}`),
`riftmeld_offer` :338 (gated on `is_cabal`, id `riftmeld`), `riftmeld_paid` :357
(`supply::return_captured(..).is_some()`). Registered as `hooks.strategy`.

Shared-engine change required by this package: `research_waiver_offers` in
`crates/ti4-engine/src/factions/hooks_strategy.rs` indexed modules that merely REGISTER the hook, and
`MODULES` is alphabetical (`factions/mod.rs:74-87`), so registering Cabal renumbered Yin's waiver from 0
to 1 and broke `factions::yin::tests::brother_omar_supplies_green_and_waives_prerequisites_for_an_infantry`
(yin.rs:3108) and `strategy_cards::tests::yin_commander_chooses_an_infantry_payment_and_decline_is_atomic`
(strategy_cards.rs:2126, `ScriptDiverged { wanted: "waiver|0", offered: ["waiver|1", "decline"] }`). Fixed
in `hooks_strategy.rs` so the index is the position among waivers ACTUALLY offered for this state and
technology (`.filter_map(|hook| hook(state, content, player, tech)).enumerate()`), and `research_waiver_paid`
selects the module whose offer is `Some` at the same index. Editing Yin's tests or reordering `MODULES` was
rejected: both hardcode a position that any future module renumbers. `technology.rs:2315` already derived the
index this way. `hooks_strategy.rs` is not on the do-not-edit list.

Tests added (cabal.rs:1131, 1176, 1199, 1219): `a_returned_cruiser_pays_for_cruiser_ii_prerequisites`
(`cr2` = `crates/ti4-content/content/technologies.json:344`, requirements `GYR`),
`riftmeld_is_not_offered_without_a_matching_capture`,
`riftmeld_is_not_offered_for_a_technology_that_upgrades_no_unit` (uses `gd`; asserts `unit_upgrade_base`
is `None` and no offer), `a_non_cabal_cannot_return_a_captured_unit_for_research`.

New decision sites: none in Cabal code; the waiver ask is the existing `strategy_cards.rs` site.

## CB-01: Devour + The Terror Between (design notes, pre-implementation)

Status (corrected): implemented and tested; the design notes below are the pre-implementation reconnaissance (see the CB-01 results section at the end). Package scope: claim `devour` (ability) and
`cabal_flagship` (unit) in `crates/ti4-engine/src/factions/cabal.rs` `MODULE` (:80, currently
`abilities: &["amalgamation","riftmeld"]`, `technologies: &["vtx"]`, `units: &["cabal_mech"]`) and
extend `fn timing_abilities(state: &GameState, owner_name: &str, seat: &PlayerId) -> Vec<Ability>`
(cabal.rs:110, body `if !is_cabal(state, seat) { return Vec::new(); } vec![reanimator(owner_name, seat)]`).

Normative text. `crates/ti4-content/content/abilities.json:345-350` devour:
`"permanentEffect":"Capture your opponent's non-structure units that are destroyed during combat."` (no
window field; claimed at `crates/ti4-content/content/fabilities.json` → `crates/ti4-content/content/factions.json:1266`).
`crates/ti4-content/content/units.json:533-547` cabal_flagship: baseType flagship, sustainDamage true,
cost 8, combat 5+/2d, bombard 5+/1d, move 1, capacity 3/used 1, `"ability":"Capture all other
non-structure units that are destroyed in this system, including your own."` — a permanent effect, no
space-dock effect (an earlier assumption of a flagship space dock was dropped). No
`crates/ti4-content/content/franken_errata.json` entry for cabal_flagship. Devour errata
(franken_errata.json:402-427): `{"itemCategory":"ABILITY","itemId":"devour","undraftable":true,"source":"pok"}`;
`amalgamation` gains `additionalComponents:["ABILITY:devour"]` and `optionalSwaps:["HERO:cabalhero"]`;
`riftmeld` gains `additionalComponents:["ABILITY:devour"]` plus an alternateText adding "If you do not have
the Amalgamation ability, you may only have one captured unit of each non-fighter ship type in your play
area." `undraftable` is a drafting restriction, not a play behaviour → no engine effect.

Capture accounting. `supply::held` (crates/ti4-engine/src/supply.rs:64) counts board units (space area AND
planet_units) of the matching base type owned by the player PLUS every `captured_units` row over ALL captors
whose `owner == player`, so writing a captured row is exactly what removes the victim's model from their
available reinforcements. `Player::captured_units: Vec<(PlayerId, UnitTypeId)>`
(crates/ti4-model/src/state.rs:564-566) — first field is the VICTIM. `remove_combat_ship`
(crates/ti4-engine/src/combat.rs:152-162) only removes from the board and returns nothing to any pool, so
capture = push `(victim_owner, unit.type_id)` onto the Cabal seat (same shape as cabal.rs `capture_own_unit`
:196). `grep UNIT_CAPTURED|CAPTURED_UNIT_RETURNED` over crates/ and crates/ti4-content/content → no
consumers, so writing the row directly (no event) is safe.

Participation test (the hard part). `destroy_units` (crates/ti4-engine/src/combat.rs:2965-3018) stages the
whole round's losses into `state.pending_destructions` and `announce_staged_destructions`
(combat.rs:3074-3090) drains that list fully before ANY announcement, so board presence cannot answer "is
the Cabal in this combat" — a Cabal fleet wiped that round is already gone. Use `GameState::faction_marks`
(crates/ti4-model/src/state.rs:1461) marks written on `SPACE_COMBAT_STARTED`/`GROUND_COMBAT_STARTED`
`Relation::After` and cleared on the matching `*_ENDED:after`, following `factions/mentak.rs` ipswitch
(`fn hero_mark(system: &SystemId) -> String { format!("mentak:hero:{system}") }` :1117, `hero_start` :1127,
`hero_destroyed` :1163, `hero_end` :1216) and `factions/sardakk.rs` Tekklar (mark write :745-760, cleanup
:795-825). `GameState::last_combat_sides` (crates/ti4-model/src/state.rs:1323) is NOT reusable: written
only at crates/ti4-engine/src/game.rs:582 (post-combat handoff, sides minus the winner) and read only at
crates/ti4-engine/src/action_cards.rs:1190.

Event payloads. `SHIP_DESTROYED` (combat.rs:2771, via `try_announce_ship_destroyed_type` :2751-2773 which
first sets `state.last_ship_destroyed`) keys: `system`, `player` (victim), `unit`, `last`, `cause`,
`during_space_combat` (`ship_destroyed_payload` :2685-2699). `GROUND_FORCE_DESTROYED` (invasion.rs:1367,
:1541, :7589, :7748) keys: `system`, `planet`, `player` (victim), `unit`, `cause`, `damaged`.
`SPACE_COMBAT_STARTED` (combat.rs:4179, before any round) / `SPACE_COMBAT_ROUND_ENDED` (:4021 then flush
:4022) / `SPACE_COMBAT_ENDED` (:4117 then flush :4126) carry `system`, `round`, `attacker`, `defender`
(`factions/hooks_combat.rs:19-22`) → participant test is exact:
`event.text("attacker") == Some(player.as_str()) || event.text("defender") == Some(player.as_str())`
(mentak.rs:741-744). `GROUND_COMBAT_STARTED` (invasion.rs:3002-3007) carries `system`, `planet`,
`attacker`, `defender`; `GROUND_COMBAT_ENDED` (invasion.rs:2964-2978) adds `winner` (absent when both sides
are wiped) and `control_changed`; both through `fn emit_ground_event(&mut self, state: &mut GameState,
ctx: &mut Resolving<'_>, name: &str, payload: std::collections::BTreeMap<String, serde_json::Value>)`
(invasion.rs:2611-2623). `window("SPACE_COMBAT_STARTED", After)` is registered
(crates/ti4-engine/src/reactions.rs:293).

"During combat" tests. Space: `pub fn destroyed_during_combat(event: &crate::event::Event) -> bool {
event.boolean("during_space_combat") == Some(true) }` (crates/ti4-engine/src/combat.rs:2676);
`pub const SHIP_CAUSE_COMBAT: &str = "space_combat"` (combat.rs:2671). Ground:
`event.text("cause") == Some("ground_combat")` (precedent crates/ti4-engine/src/factions/sardakk.rs:740).
Ground causes in invasion.rs: "ground_combat" (:2807, :2816, :2827, :2838), "harrow" (:2867, :2877);
`matches!(cause, "bombardment" | "space_cannon_defense" | "harrow")` (invasion.rs:1325) → bombardment,
space cannon defense and harrow are NOT combat, so Devour must not fire for them. The flagship clause has no
cause restriction ("destroyed in this system").

Structures and unknown ids. Canonical test is the content flag
`ti4_content::units::UnitType::is_structure()` = `self.record.flag("isStructure")`
(crates/ti4-content/src/units.rs:100; also `is_space_only_structure()` :108, `is_ground_force()` :121,
`is_ship()` :97) via `ti4_content::units::unit_type(content, id, sources) -> Option<UnitType>`; engine
precedents `factions/argent.rs:180-194`, `factions/cabal.rs:473`, `objectives.rs:473`,
`invasion.rs:3205`. Ad-hoc baseType string tests exist (action_cards.rs:1034, production.rs:784,
laws.rs:101) but are not canonical. Decision: a unit id with no catalogue entry is NOT captured (precedent
cabal.rs:467-471 `let Some(kind) = ti4_content::units::unit_type(..) else { continue; }`). Devour skips
neutral owners (`crate::neutral_units::is_neutral`, crates/ti4-engine/src/neutral_units.rs:66) and the
Cabal's own units.

Ability shape and ids. `Ability::stateful(format!("unit:{owner_name}:cabal_mech:GROUND_FORCE_DESTROYED:after"),
seat.clone(), "GROUND_FORCE_DESTROYED", Relation::After, Arc::new(move |event, _resolver, context| { ... Ok(()) }))`
`.with_stateful_condition(Arc::new(move |event, _resolver, context| bool))` (cabal.rs:132-172); closures get
`context.content`, `context.sources` (a `SourceSet` value), `context.state`. `.with_optional(true)` only for
"you may" abilities — Devour and the flagship are mandatory. `Ability` has exactly ONE `event_type`, so each
trigger needs its own Ability (precedent `factions/yin.rs:830-895` `yinagent` + `brother_milor_ground`).
Naming convention from existing base factions: `ability:{owner_name}:{card_id}:{EVENT}:after` (e.g.
`ability:{owner_name}:indoctrination:GROUND_COMBAT_STARTED:after`, yin.rs:488),
`unit:{owner_name}:{unit_id}:{EVENT}:after`, `leader:{owner_name}:{id}:{EVENT}:after`. Synthetic
(non-card) helper ids are allowed: `reclamation_mark`, `reclamation_clear`, `argentbt_mark`, `winnubt_clear`;
`crates/ti4-engine/tests/` contains only `content_ids_resolve.rs` (validates ids CLAIMED in a MODULE) and
`decision_delivery_inventory.rs`, so participation-tracker ids need no claim.

Test harness to mirror. `crates/ti4-engine/src/factions/mentak.rs`: `arena()` :1262,
`scripted(answers)` :1268 (`Table::with_default(Box::new(Scripted::new(answers.iter().coped())))`),
`count(state, system, kind, owner)` :1278, and `emit(state, answers, event_type, payload)` :1295-1317,
which builds `armed_resolver` + `plain_hub().galaxy` and drives
`ctx.event_sequence.next(event_type, payload)` through `resolver.emit_with_context`. Payload builders return
`Vec<(&'static str, serde_json::Value)>`: mentak `started(system)` :2310 (system/attacker="a"/defender="b"),
`destroyed(system, owner, unit)` :2318 (system/player/unit/`last`=false), `ended(system, winner)` :2070.
cabal.rs already has `game()` :601, `in_space`, `on_planet`, `separate_planets()`, `strike(state, system,
planet, victim, hits)` :673 (drives `invasion::assign_ground_hits_in_timing(..., "ground_combat")`, which
emits GROUND_FORCE_DESTROYED but NOT GROUND_COMBAT_STARTED/ENDED — a ground-half test must emit the start
event explicitly or drive the full invasion entry point).

Decisions for the implementation.
1. Partition, not overlap. The flagship clause applies whenever a Cabal flagship is on the board in that
   system at announcement time; Devour's condition additionally requires that NO Cabal flagship is in that
   system. The flagship clause is strictly broader (own + opponent units, any cause), so excluding that case
   from Devour loses nothing and guarantees at most one capture row per destroyed unit.
2. Double-capture hazard. Reanimator (CB-02) and the flagship ground clause could both capture the same
   destroyed Cabal infantry, which would write two rows and over-remove Cabal reinforcements because
   `supply::held` counts captured rows. Mitigations considered: (a) add a "no Cabal flagship in this system"
   exclusion to Reanimator's condition (edits already-reviewed CB-02 code), or (b) dedup per event using
   `Event.id` (`crates/ti4-engine/src/event.rs:16-27`, unique per trace via `EventSequence::next`,
   event.rs:84-105) recorded in `GameState::faction_marks`. Decision was still open at the end of this
   reconnaissance; the explicit partition (1) is preferred because it keeps the behaviour testable.
3. A flagship cannot witness its own destruction: it is already off the board when its own SHIP_DESTROYED is
   announced, so its own destruction captures nothing. A second Cabal flagship in the system would capture the
   first (literal reading of "all other").
4. Devour skips neutral owners (`crate::neutral_units::is_neutral`, `crates/ti4-engine/src/neutral_units.rs:66`)
   and the Cabal's own units; structures are excluded with `ti4_content::units::UnitType::is_structure()`
   (`crates/ti4-content/src/units.rs:100`, engine precedent `factions/argent.rs:180-194`); an unknown unit id
   (no catalogue entry) is not captured (precedent cabal.rs:467-471).
5. Ground cause restriction: Devour's ground half requires `event.text("cause") == Some("ground_combat")`
   only — not "harrow", "bombardment" or "space_cannon_defense" (exclusion precedent
   `crates/ti4-engine/src/invasion.rs:1325`; Yin doc line "bombardment and space cannon defense are not
   combat"). The flagship clause has no cause restriction ("destroyed in this system").
6. Participation marks. `format!("cabal:devour:space:{system}")` and
   `format!("cabal:devour:ground:{system}|{planet}")`, value = owner id string, written on
   SPACE_COMBAT_STARTED:after / GROUND_COMBAT_STARTED:after and cleared on the matching *_ENDED:after only
   when the stored value equals this owner (Mentak idiom mentak.rs:1117-1239; Sardakk Tekklar
   sardakk.rs:745-825). Board presence cannot be used instead: `destroy_units` removes the whole round's
   losses before any announcement (`combat.rs:2965-3018`, `combat.rs:3074-3090`).
7. Planned helper `fn terror_between_present(state: &GameState, owner: &PlayerId, system: &SystemId) -> bool`
   scanning `state.system_state(system).units` for `unit.owner == *owner && unit.type_id.as_str() == FLAGSHIP`
   with `const FLAGSHIP: &str = "cabal_flagship"` (cabal.rs has no FLAGSHIP/MECH/SPACE_DOCK constants today).
8. Non-optional stateful abilities create no decision sites (Reanimator precedent), so CB-01 needs no
   `decision_delivery_inventory.rs` entries as long as it never asks.

Known limitation to carry forward: for NON-combat destructions (action card / agenda) in the flagship's
system, the flagship cannot capture destructions announced BEFORE its own event in the same flush, because it
is already off the board. Record this in the scope ledger.

Errata note: `crates/ti4-content/content/franken_errata.json:402-427` marks `devour` `undraftable` and adds
`additionalComponents: ["ABILITY:devour"]` to `amalgamation` and `riftmeld`. `undraftable` is a drafting
restriction, not a play behaviour, so CB-01 does not implement it. `cabal_flagship` has no errata entry.

## CB-01 results (Devour + The Terror Between): claimed `devour`, `cabal_flagship`

Implemented as designed above: `devour_ships`/`devour_ground_forces` (partition: Devour yields wherever a
flagship is in the system), `terror_between_ships`/`terror_between_ground_forces`, participation marks via
`combat_participation`. Reanimator (CB-02) also yields to the flagship so one destroyed model is captured once.
Fixes this session: three ground tests used a mech victim (sustain damage absorbs the first hit), one test emitted
the flagship's own destruction with the flagship still on the board. Real-route tests (combat window and invasion
window armed as the game arms them, every die a hit): `a_real_space_combat_feeds_devour`,
`a_real_space_combat_the_cabal_is_not_in_feeds_nothing`, `a_real_space_combat_with_the_flagship_captures_each_loss_once`,
`a_real_ground_combat_feeds_devour`, `a_real_ground_combat_with_the_flagship_captures_each_loss_once`, plus the
no-Cabal variants. Structures, neutrals, own units (Devour), non-combat destructions (Devour) are excluded by the event tests.
Known limitation: a non-combat destruction announced before the flagship's own event in the same flush cannot be
captured by a flagship that is itself destroyed in that flush (it is already off the board).

Pi waiver neutrality (CB-04 follow-up): `hooks_strategy::research_waiver_offers` indexes the waivers actually offered, so
a Cabal waiver is never offered to non-Cabal seats and Yin's `waiver|0` is unchanged (both Yin tests pass unchanged).
CB-03 test fixes: produced units land in space (`in_space`), and the fixture no longer loops on an uncapped id.

## CB-08: commander unlock (claimed `cabalcommander`)

`commander_unlocked` (cabal.rs): units in 3 systems that are gravity rifts (printed `isGravityRift` or a dimensional
tear). Real route: `leaders::check_unlocks` flips the status and the existing production exemption then offers
`build|infantry|2` at a limit of 1. Tests: `the_commander_unlocks_with_units_in_three_gravity_rifts`,
`another_players_units_do_not_unlock_the_commander_and_it_names_only_its_own_card`,
`the_shared_unlock_check_delivers_the_commander_and_its_exemption_is_then_live`, neutrality
`a_game_without_a_cabal_seat_never_makes_a_dimensional_tear`.

## CB-05: Dimensional Tear I/II + dt2 (claimed `cabal_spacedock`, `cabal_spacedock2`, `dt2`)

New movement hooks (`hooks_movement.rs`): `gravity_rifts(state)` (systems made rifts for everyone) and
`rift_roll_exempt(state, mover, system)`; applied by `hooks_movement::apply_rift_effects`, called from
`action_cards::apply_movement_effects` (the single door every real move's rules pass through; `game.rs::sail` builds
`MovementRules::new` without state, so a with_laws hook could not reach it, and game.rs may not be edited). That is the
one-line edit outside the listed files. `MovementRules::rift_roll_exempt` and `transit::rifts_exited` honour it: the Cabal
does not roll for its own tear, others do, printed rifts still roll. The tear is derived from the board (the dock unit), so a
destroyed dock removes it. Fighter support (6/12) and PRODUCTION 5/7 are printed on the unit data and verified by test.
`dt2` is the generic unit upgrade (90.8). Tests: `a_dimensional_tear_is_a_gravity_rift_that_only_its_owner_does_not_roll_for`,
`a_printed_rift_still_rolls_for_the_cabal`, `a_destroyed_dock_stops_being_a_rift`,
`dimensional_tear_i_and_ii_carry_six_and_twelve_fighters_free`, `the_tears_print_production_five_and_seven`,
`researching_dimensional_tear_ii_upgrades_the_dock_on_the_board`, neutrality `a_game_without_a_cabal_seat_has_no_extra_rift`.

## CB-09: It Feeds on Carrion (claimed `cabalhero`)

`leader_action`/`use_leader` hooks; shared `leaders::use_leader` purges the hero. Dice via `context.dice.roll_by` (game RNG),
ships in or adjacent to a tear (adjacency needs the map; without one only the tear's system), captured ships' cargo that no
longer fits is found by diffing the owner's space cargo around `fleet::enforce_seeing` and captured; atomic (state, dice, rng,
log restored on a refused answer). `leader_action` is offered whenever a tear exists (it has no map). Tests: six
`the_hero_*`/`a_captured_carrier_*`/`a_ship_that_keeps_*`/`a_game_without_a_cabal_seat_cannot_play_the_hero`.

## CB-07: The Stillness of Stars (claimed `cabalagent`)

Replenish announcement: `promissory::trade_agreement_on_replenish` (called by every replenish site) calls
`cabal::note_replenished`, which stages `COMMODITIES_REPLENISHED` only while a Cabal is seated; the driver's staged flush
announces it. Optional timing ability; converts commodities to trade goods (staged `TRADE_GOODS_GAINED`) and captures a unit
(structures excluded: no printed cost). New decision site `stillness_of_stars` (type choice, asked before any mutation).
Tests: seven `the_agent_*`/`another_players_trade_replenish_*`/`declining_*`/`the_cabals_own_*`/`an_exhausted_agent_*`/
`a_game_without_a_cabal_seat_stages_no_replenish_event`.

## CB-10: Crucible (claimed `crucible`)

Holder's optional ability `SYSTEM_ACTIVATED:after` (armed for every seat only while a Cabal is seated). Playing it records the
activation in `faction_marks` and returns the note; `MovementHooks::rift_crucible` sets `rifts_ignored` and
`MovementRules::rift_extra_steps = 1` (path_from_ship adds the extra step once per route; corrected in Review fixes). Tests: four `crucible`-route tests incl.
decline, own-note and no-Cabal neutrality. Multi-rift and extra-step path tests added in Review fixes.

## CB-11: Al'Raith Ix Ianovar (claimed `cabalbt`)

`fracture::after_breakthrough_gained` skips the roll for `cabalbt` (game's expedition path and this ability both call it; it is a no-op
once the Fracture is in play). Ability on `BREAKTHROUGH_GAINED` moves up to 2 ingress tokens from non-rift systems into rift
systems without an ingress (decision site `alraith`); `move_bonus` hook gives +1 to the holder's ships starting in a Fracture
system. Tests: five in `cabal.rs` (entry without a roll + moves, decline, already in play, +1 move, neutrality).
`FRACTURE_ENTERED_PLAY` is not emitted by the ability when it enters the Fracture (game.rs owns that emit).

## Review fixes (2026-10-06)

1. Vortex skips neutral owners (`vortex_targets`); test `vortex_does_not_offer_neutral_units` (fails without the fix).
2. Crucible: the note says "+1 movement total, regardless of how many gravity rifts you pass through". `movement.rs`
   `path_from_ship` now carries a per-route "bonus used" flag (and keys `best` by system and flag), so the extra step is
   granted once, at the first rift. Earlier "per rift" wording in this file and in comments is wrong and corrected. Test
   `movement::tests::the_crucible_bonus_is_one_extra_step_however_many_rifts_the_route_passes` (three-ring galaxy, unique
   six-system line): three rifts on move 1 do not arrive with the bonus; two rifts on move 3 arrive only with it.
3. Merchant Station replenish (`exploration.rs`) now calls `promissory::trade_agreement_on_replenish`, so Trade Agreement
   and Stillness of Stars see it. Test `a_merchant_station_replenish_is_announced_and_offers_the_agent`.
4. `is_gravity_rift_system` uses the point lookup `ti4_content::galaxy::system`.
5. `tear_systems` returns early when no seat is Cabal (a tear needs a Cabal dock, so behaviour is identical).
6. `use_leader` refuses (`Some(false)`) without a galaxy, so adjacency is never skipped; test
   `the_hero_is_refused_without_a_map_rather_than_skipping_adjacency` (state unchanged, no dice rolled). `leader_action`
   still has no map and stays offered whenever a tear exists.
Nits: stale module/MODULE comments and the CB-06 banner fixed; Reanimator doc quotes the current `units.json` text;
research waiver payment prompt is now "<label>: choose the payment". Stale-mark guard test
`a_combat_the_cabal_retreated_from_leaves_no_mark_for_a_later_combat_of_others`.

Recorded readings and limits:
- Reanimator simultaneity: the capture condition is evaluated after the hit, so a mech destroyed in the same volley as its
  infantry still counts as present (reads "destroyed together" as the mech being there when the infantry die). The
  implementation reaches infantry in an area with a Cabal mech; `units.json` text says "on this planet" (same area).
- Cabal captures write `captured_units` rows directly and emit no `UNIT_CAPTURED`; there is no listener today.
- `riftmeld` and `cabalbt` ability helpers use the DEFAULT source set and the embedded content store; no per-game source
  or custom content scope is threaded through them.

Results (test debug=0, incremental=false, codegen-units=16, `-j1`): `cargo test -p ti4-engine --lib -j1 cabal` ->
84 passed; `cargo test -p ti4-engine -j1 -- --test-threads=12` -> lib 2331 passed, 0 failed, 1 ignored; integration
1, 4, 5 passed (registry test passes: no new decision sites). Ledger: `cabal 14/14 implemented`. Log `out/cabal-review-fixes-full.log`.
