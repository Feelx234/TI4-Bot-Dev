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

## Counted bails (`Unknown::Unported(reason)`)

* THE BLOCKER: the exploration of a first claim (`previous.is_none()`: nearly every early invasion). There is no native planet exploration window
  (`exploration::explore_with`, ~40 card arms, attachments, fragments); the core has only the reward windows of `rules::explore_windows` and the Exploration
  Probe / Empyrean-hero (lane 9) pieces. Until a composer exists no zero-bail game is possible through this island.
* Space cannon defense: entropic scar, L4 Disruptors, attachment cannons. Round: X-89, Dacxive Animators, The Thundarian watching, hits added to a roll
  (lane 10/13 carry X-89 / Dacxive / Thundarian in the leaf facts: lift after the merge). Bombardment: the Jol-Nar commander's reroll question.
* Control: the Minister of Exploration (trade good staged for the capture), the Crown of Emphidia changing hands (needs `fx::steal_throne_card`),
  Integrated Economy (nested production), an Assimilate seat (the unit roster is not in the live `Env`: `AdapterFacts::plays` is None).
* Landing a ship-form unit that flips (Naaz Eidolon). Slumberstate Computing (TE breakthrough, out of scope).
* Not ported by decision: Research Team / Viper EX-23 (Deepwrought, Firmament), Keleres toll, Crimson deploy, Alarum, Last Bastion, Titans Slumberstate.
* Neutral sides (lane 11 owns them), Proxima Targeting VI (Last Bastion).

## Open

1. Exploration of a first claim (above).
2. `live::details::complete` has no invasion family yet: the string commit question carries no `details` the core render lacks in the tested positions;
   re-check on the first bot corpus.
3. The aftermath arm (M5) must call `advance_invasion` for the top sub-window and own `INVASION_RESOLVED`.
4. Mutation checks of the new files were not run (see the final report).
