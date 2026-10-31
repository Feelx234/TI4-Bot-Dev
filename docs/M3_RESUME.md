# D2 lane M3 (`core-m3-invasion`): the invasion island, resume note (2026-10-31)

Branch `core-m3-invasion`, cut from `core-d2-f-live` (a253ba70). Never pushed; the live branch is untouched. Lanes 9-13 (`core-w-l9..l13`) were NOT merged:
`core-w-l13` (which contains l9 and l10) conflicts in `combat_barrage.rs`, `hero_dual.rs`, `strategy_cards_core.rs` with this base, and the lanes are still WIP;
the coordinator's merge of them brings `rules::invasion::fight::CommittedFight`, X-89 / Dacxive / `pay_env` in the ground facts and `fx::steal_throne_card`,
after which the gates listed below for those items can be lifted (one owner: this lane's composer calls the leaf windows, the lanes own their bodies).

## What is native

`ti4-engine-core/src/driver/windows/invasion/` (`mod.rs`, `ops.rs`, `fight.rs`, `live.rs`) = `InvasionMachine`, the composer. The payload is
`state::Invasion` (`state/invasion.rs`, replaces the stub; the 16 fields of `InvasionWindow` map as documented there). `Held` impl, arm `Arm::Invasion`
(registered in `driver/live.rs`: `advance_invasion` / `ask_invasion`; `step_arm` does not choose it: the aftermath arm (M5) hands its steps over while the
invasion is the top sub-window).

Stages: `Begin` (public boundary, `INVASION_BEGAN`, the rolled bombardment with its staged dice) -> `Bombarding` (leaf `Bombardment`, the coexistence
`BombardTarget` question, the reroll announcement `UNIT_ABILITY_ROLLED`) -> `Custodians` (offer, `PayBill` payment resumable from the payload) -> `Committing`
(`LandOne` over the core commit pool: space troops, temporary space commits, faction candidates) -> `finish_committing` (`SpaceCannonDefense`, forced
combats) -> `Fighting` (`FightRound` question, round start with the hooks and the Dunlain Reaper question/payment, `GroundRoll`, `GroundRollTail`,
`GroundRoundHits` with Harrow, the end of the combat with feats, temporary commits returned) -> `Advancing` / `ChoosingNextCombat` (coexistence) ->
`FinalizingControl` -> control gain (`ControlGained`, exhausted planet, lost-home feat) -> `Done`.

Scoring pauses: the machine stops after the chunk that queued a feat occurrence (`pause`); `advance_invasion` leaves the invasion on the stack, pushes the
`Window::EventScoring` of `scoring_window(..)` above it and returns `Live::Done` (the STEP is over, the invasion is still open: the caller reads the stack).

Openers: `open_invasion` (aftermath), `open_at_commit` (Sardakk hero). `closed()` clears `transient.invasion.active`.

Engine half: `ti4-engine/src/invasion_live.rs` (`fill_codes`, lockstep tests against the string `InvasionWindow` through `live_step`).

Shared-file edits (all additive): `kernel/ability.rs` (`Unknown::Unported(&'static str)`, `Codes::invasion: InvasionCodes`), `driver/session.rs` (`Arm::Invasion`, `ALL`
20), `driver/step.rs` (`NativeArms::ALL` 20 bits), `driver/live.rs` (the two registry lines), `dual_window.rs` (one match arm for the new `Unknown`),
`d1_action.rs` (`sub_of` builds `Invasion::observed`), `d1_pending.rs` / `live/arms.rs` (arm name), `rules/invasion/{begin,stages}.rs` (pub helpers
`reaper_offered`, `reaper_replace`, `deploy_question`, `Custodians::at_question`), `rules/production/pay.rs` (`PayBill::resume`, `paid`).

## Merges and what they changed

`core-w-l12` (729c7184) is merged (one conflict, `combat_barrage.rs`: both sides kept). `rules::invasion::forced::ForcedCombats` is the tail's forced-combat step (its
Slumberstate question is Thunder's Edge: a counted bail). Lane 10's `x89` flags are in the roll/tail facts, so the X-89 gate is gone. `core-w-l9/l13` are NOT
merged (they conflict with this base in `hero_dual.rs`, `strategy_cards_core.rs`); `core-w-l11` (neutral sides) is still running.

The live kernel (`live/kernel.rs`) was missing the seams the combat kernel sets up: it now registers `providers::register_core`, `adapters::bridge_legacy` (hook slots on
their seams, no string), the faction modules, `providers::register_ground_abilities` (NEW: Fragile/Unrelenting + the Shield Paling exemption on ground rolls,
which the string answered through its oracle) and the round-agent die hook. Found by the first lockstep (Sardakk rolled at 8 instead of 7).

Version branches: the machine reads `rules_at_least(16)` (numbered round, copied round agent) and `rules_at_least(1)` (rival structures destroyed after the capture is announced)
from the position; `a_version_zero_game_runs_natively_like_the_string_window` covers versions 0, 1, 15, 16 (the 14 alpha saves are version 0). Nothing is bailed for a version.

## Counted bails (`Unknown::Unported(Unported::..)`, words in `invasion_live::unported_text`)

* THE BLOCKER (coordinator item (a)): the exploration of a first claim (`ExplorationOfAFirstClaim`, only for a planet that has a trait to explore into; Mecatol Rex and
  trait-less planets are fine). Nearly every early invasion hits it. What is needed: a planet-aware exploration window hosted by `finish_control_gain` right after
  `ControlGained` (seam: `fight.rs::finish_control_gain`, the `previous.is_none()` branch; stage `IStage::Exploring` to add): `ChooseDeck` for two traits (exists),
  `DistantSuns` (exists), the draw (`fx_explore::draw`), the log row (`fx_explore::open_record`), the arms of `exploration::explore_drawn`/`resolve_instant`
  (Fragment, Attach with DMZ demilitarize, the instants), `Reward`/`PayMechOrInfantry`/`DominusOrb`/`IonStorm` children (exist), the closing of the log row with the holdings
  diff. Lane 9's `FrontierExplore` (`hero_windows.rs` on core-w-l13, ~400 lines) is the frontier-deck (no planet) version of exactly this and should be generalised
  (planet: Option, Attach arm) rather than rewritten; it is not on this branch because l9/l13 do not merge cleanly yet.
* Space cannon defense: entropic scar, L4 Disruptors, attachment cannons. Round: Dacxive Animators, The Thundarian watching, hits added to a roll.
  Bombardment: the Jol-Nar commander's reroll question.
* Control: the Minister of Exploration (trade good staged for the capture), the Crown of Emphidia changing hands (needs `fx::steal_throne_card`, l13).
  Integrated Economy (dropped by the coordinator: kept as a bail so it is never silently skipped).
* Landing a ship-form unit that flips (Naaz Eidolon). Slumberstate Computing (TE).
* Not ported by decision: Research Team / Viper EX-23 (Deepwrought, Firmament), Keleres toll, Crimson deploy, Alarum, Last Bastion.
* Neutral sides (lane 11), Proxima Targeting VI (Last Bastion). The Naalu Matriarch's `temporary_ground_force` and the generic may-sustain prohibitions were answered by the string oracle in
  the dual: the live kernel has no provider for them (a position with those cards in play differs silently: check on the first bot corpus).

## Tests (all cheap)

* `ti4-engine` `invasion_live::tests` (7): lockstep of the string `InvasionWindow` and `Arm::Invasion` through `live_step`, same questions/answers/units/control/VP/exhausted planets:
  conquest x6 invader factions, bombardment + PDS defense x6 defender factions, custodians declined/paid/free (hero opening), the Dunlain Reaper x18 seeds, home capture
  with scoring pauses x4, version 0/1/15/16 with Assimilate, and the first-claim bail with `TI4_BAIL_IS_FAILURE` semantics (`with_bail_is_failure`).
* `ti4-engine-core-tests` `invasion_composer` (5): conquest, a clone mid-window continues identically, payload identity after every question, first-claim bail, hero opening.
* `M3_DEBUG=1` prints the string and native dice and the questions of each lockstep case.

## Mutation checks (single files of `driver/windows/invasion/`, `tools/engine_core/mutants.sh`, 363 live points)

Run against the engine lockstep tests (`invasion_live`): 205 killed, 158 survived; against the core tests (`invasion_composer`): 102 killed. 145 survive both. The
first round found a real bug (`publish` ran after the question, not before: fixed + `the_public_boundary_follows_the_stage_the_question_is_asked_in`); comparing events
found another (the bombardment's `UNIT_ABILITY_ROLLED` named a state-side code). Survivors by function (the positions the tests do not reach): `begin` 13 (bombardment
dice of a hitting bombardment, held counts), `commit_pool` 12 (temporary space commits, faction candidates: Matriarch, Sardakk commander), `step`/`settle` 15 (the idle guard,
branches of the chunk loop that need a coexistence or a forced combat), `round_unported` 8 and `copied_round` 5 (the gates: each needs a position with Dacxive, a Thundarian,
a Ssruu copy), `finish_ground_round` 7, `answer_reaper` 6, `answer_next_combat` 6 and `bomb_window`/`answer_bombard` 7 (coexistence: three seats), `announce_rerolls` 7 (a reroll
reaction on the bombardment), `return_temp_commits` 5, `publish` 6. Next tests to write: a three-seat coexistence invasion, a Naalu Matriarch commit, a Scramble Frequency
reaction after a hitting bombardment, one position per gate (each must bail with its `Unported`).

## Open

1. Exploration of a first claim (above).
2. The aftermath arm (M5) must call `advance_invasion` for the top sub-window, close the `EventScoring` window the pause leaves above the invasion, and own `INVASION_RESOLVED`.
3. `live::details::complete` has no invasion family: the tested positions need none (the rendered commit/fight/custodians questions equal the string's).
