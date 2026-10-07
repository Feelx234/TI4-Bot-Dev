# BF-deepwrought (The Deepwrought Scholarate, alias `deepwrought`)

Code: `factions/deepwrought.rs` (ocean model, Research Team, Oceanbound, Hydrothermal Mining, Radical Advancement, Luminous hook, Eanautic, Aello unlock), `factions/deepwrought_cards.rs` (Share Knowledge, Visionaria Select, Ta Zern). Ledger: deepwrought 11/11 implemented; every other faction unchanged.

## Items

| Kind | Id | Status | Tests |
|---|---|---|---|
| ability | researchteam | done | `research_team_lets_committed_ground_forces_coexist_instead_of_fighting`, `research_team_is_optional_and_declining_fights`, `research_team_is_not_offered_without_a_rival_force_or_to_other_factions` (real `invasion::resolve` commit route) |
| ability | oceanbound | done | `an_ocean_is_gained_readied_for_each_planet_where_units_begin_coexisting`, `only_the_deepwrought_gain_oceans_by_coexisting`, `oceanbound_discards_oceans_beyond_the_planets_with_coexisting_units`, `there_are_only_five_oceans`, `ocean_cards_pay_for_things_like_planets_and_exhaust`, `crash_landing_into_coexistence_is_the_beginning_oceanbound_waits_for` (real card effect) |
| technology | hydrothermal | done | `hydrothermal_mining_gains_a_trade_good_per_ocean_in_play_at_the_start_of_the_status_phase`, `..._counts_every_ocean_in_play_whoever_holds_the_technology` (real `STATUS_PHASE_BEGAN` window) |
| technology | radical | done | `radical_advancement_replaces_a_technology_with_one_that_has_exactly_one_more_prerequisite`, `radical_advancement_is_optional`, `radical_advancement_never_replaces_a_unit_upgrade_or_offers_a_purged_card` |
| unit | deepwrought_flagship (D.W.S. Luminous) | done | `the_luminous_moves_through_systems_holding_its_owners_units_past_blockades_and_gains_a_step`, `the_luminous_earns_one_step_for_each_system_it_moves_through_that_holds_its_units`, `the_tactical_action_offers_the_luminous_the_blockaded_move_and_no_other_ship` (real `tactical::movable`), `a_nekro_flagship_carrying_the_luminous_text_passes_too` |
| unit | deepwrought_mech (Eanautic) | done | `the_eanautic_moves_itself_and_chosen_infantry_home_when_another_player_activates_its_system`, `the_eanautic_taking_every_unit_ends_the_coexistence`, `the_eanautic_is_optional_and_needs_coexistence_and_another_players_activation` (real `SYSTEM_ACTIVATED` window) |
| promissory | shareknowledge | done | `share_knowledge_lends_a_non_faction_non_unit_upgrade_technology_until_the_status_phase_ends`, `share_knowledge_offers_nothing_a_holder_already_owns` |
| leader | deepwroughtagent (Doctor Carrina) | done | `doctor_carrina_lets_a_researcher_ignore_a_prerequisite_and_places_infantry_into_coexistence` (real Technology primary), `..._is_a_may_and_only_one_waiver_is_given`, `..._is_readied_again_when_the_research_gains_nothing` (real secondary), `..._does_not_ask_for_its_owners_own_research` |
| leader | deepwroughtcommander (Aello) | done | `aello_unlocks_once_an_ocean_card_is_in_play`, `aello_takes_one_off_another_players_paid_research_and_pays_its_holder` (real Technology secondary), `aello_is_reached_through_a_faceup_alliance`, pre-existing `the_deepwrought_commander_takes_one_off_another_seats_research_and_pays_its_holder` |
| leader | deepwroughthero (Ta Zern) | done | `ta_zern_purges_a_technology_everywhere_and_each_loser_researches_another`, `ta_zern_from_the_deck_only_makes_the_owners_of_it_research`, `ta_zern_is_a_may_and_needs_to_be_unlocked`, `ta_zerns_unlock_is_three_scored_objectives` (real `leaders::use_leader` / `component_actions`) |
| breakthrough | deepwroughtbt (Visionaria Select) | done | `visionaria_select_sells_research_for_trade_goods_and_a_promissory_note`, `..._is_a_may_for_each_player_and_researches_only_plain_technologies`, `..._needs_a_player_who_can_pay` (real component action) |
| neutrality | no Deepwrought | done | `a_game_without_the_deepwrought_is_untouched_by_every_window` (status begin/end, activation, spendable planets, researchable, Technology primary) |

## Content findings

* **Sundered is not a Deepwrought asset.** `abilities.json` tags `sundered` with faction `crimson` (source Thunder's Edge); the Deepwrought sheet lists `researchteam` and `oceanbound` only (franken errata: `oceanbound` + `researchteam`). It is not in the ledger and not implemented here. No unit/upgrades beyond flagship and mech exist for this sheet.
* Ocean cards are the five tile-less planet records `ocean1`..`ocean5` (1 resource, 1 influence, types FAKE/FACTION). There is no ocean deck record.

## Model decisions

* **Ocean cards**: rows `deepwrought:ocean:<id>` = holder in `faction_marks`; exhaustion is `exhausted_planets`; not `placed_planets` (they are off the map and must not appear as planets of a system). They are spendable for resources/influence through `deepwrought::spendable_oceans`, called from `production::spendable_planets`.
* **Oceanbound's cap is derived**: `oceans()` is the stored list cut to the number of planets holding the holder's coexisting units (lowest id kept), so "any time" holds at every read; `gain_ocean` also deletes stale rows. Which ocean is gained (lowest free) or lost (highest) is not asked: the five are identical 1/1 cards with no text.
* **Beginning coexistence**: one door, `deepwrought::begin_coexisting` (wraps `coexistence::begin`, then Oceanbound when it was new). Called by Research Team and Carrina; Crash Landing (which writes `coexisting` itself) calls `note_coexistence_began`.
* **Research Team** (`invasion.rs::offer_research_team`, called at the start of `finish_committing`): asked per committed planet when another player controls it, a rival ground force stands there and the invader is not already coexisting. "Coexist" calls `begin_coexisting` (3.1: the controller keeps the planet). Two shared guards make a coexisting invader behave: `advance_fighting` starts no combat for it, `establish_control` gives it no control.
* **Luminous**: new `MovementHooks::own_unit_passage`; `MovementRules` keeps `own_passage_types` and `own_unit_systems` (systems holding any of the mover's units, read at build). In `path_from_ship` such a ship passes a system holding the mover's units despite rival ships, and each such intermediate system adds one step to that route's allowance (origin and active system do not count). A Nekro flagship with the Z token on the Deepwrought passes too (`flagship_has_text`; `deepwrought_flagship` added to `nekro::LENDABLE_FLAGSHIPS`).
* **Doctor Carrina**: the research window is the two strategy-card research routes (`strategy_cards::offer_research`, the Technology primary's free research, and `paid_research`, the secondary and the primary's second research). The holder is asked once as the research begins (before the researcher lists technologies, because the waiver widens the list) and only when it matters (the waiver opens a technology, or the holder has a placement). The waiver is the mark `deepwrought:carrina:<researcher>` (value: the holder), read by the module's `waived_prerequisites` hook, and is cleared when the research ends. After a research that gained a technology with at least one printed prerequisite, the holder may place 1 infantry from reinforcements on a non-home planet the researcher controls (not a space station), into coexistence (`place_into_coexistence`: infantry first, then `begin_coexisting`). A research that gained nothing undoes the use (agent readied).
* **Share Knowledge**: `promissory::is_action_placed` now includes `shareknowledge` so the note waits in hand for its ACTION; the module's component action offers it when the Deepwrought owns a non-faction, non-unit-upgrade technology the holder lacks. The loan is the mark `deepwrought:share:<note>` = `<holder>|<technology>`; at `STATUS_PHASE_ENDED` the technology leaves the holder (`technologies` and exhaustion) and the note goes home (`promissory::give_back`).
* **Visionaria Select**: component action; the card exhausts (`deepwrought:bt:exhausted:<player>`, readied at `STATUS_PHASE_ENDED`). Players in seating order after the holder who can afford 3 trade goods (`supply::potential_goods`) and hold a giveable note are each asked: accept/decline; which note; the 3 trade goods are spent through the goods window (return to supply); the note moves to the holder; the player may research a non-faction, non-unit-upgrade technology whose prerequisites they already meet; the holder gains it as well if they lack it. Atomic: any illegal answer restores the state.
* **Ta Zern**: `leader_action` / `use_leader` hooks (generic caller exhausts/purges). Candidates: non-unit-upgrade technologies the owner owns or has in their deck (`deepwrought::deck`: active cards, generic or own faction, not purged). The chosen card is purged from every seat that owns it (`technology::purge`) and recorded as `deepwrought:purged_tech:<id>`, which `technology::can_research` now refuses for everyone. Each seat that purged an owned copy (the owner first, then seating order from them) researches another technology (prerequisite-met cards only; mandatory when any exists).
* **Aello**: unlock `commander_unlocked` hook ("an ocean card in play" = the owner has one in play). The research discount is pre-existing (`strategy_cards::deepwrought_commander`, paid secondary research) and reaches Alliance/Yin/Mahact/Nekro through `promissory::has_commander_ability`.

## Open questions and limits

1. **"if they do" (Carrina)**: read as: the technology researched has at least one prerequisite to ignore. The note says the waiver may be used even when the prerequisite is already met, so the actual need is not required.
2. **Research routes**: Carrina is wired into the Technology strategy card routes only. Other research routes that do not go through `offer_research`/`paid_research` (Specialist Compounds, Focused Research and other action-card research, Sardakk, Cabal, Yin waivers, Ta Zern, Visionaria) do not open the window. Ssruu copying of Doctor Carrina is not built (the Ssruu census covers other agents separately).
3. **Which ocean** is gained or discarded is not a decision (see above).
4. **Exchange Program** places infantry on a planet without writing coexistence (a pre-existing engine simplification), so it never reads as "begin coexisting" for Oceanbound.
5. **Ocean cards and influence**: spendable for payments (`payment`), but not counted by voting influence or by effects that read controlled planets; they are not planets of a system.
6. **Research Team** is asked only where a rival ground force stands (structure-only or empty planets are simply taken, as for Titans' Slumberstate); coexistence is not re-reconciled when units die in later combats (`coexistence::reconcile` is wired only where this package moves units).
7. **Eanautic**: each coexisting Eanautic is its own use (the ability may resolve again); infantry moved are chosen by count, not by individual unit. Destination is asked only when the home system holds several controlled planets.
8. **Visionaria Select**: the Support for the Throne note cannot be given (its point travels through its own bookkeeping); a Keleres agent window can pay the 3 trade goods with commodities (existing goods-window rule).
9. **Luminous**: "systems that contain your units" is read when the movement rules are built; units that arrive in an intermediate system later in the same activation do not extend an already-built route.
10. Exact reading of "technology of the same color": the `types` color (BIOTIC etc.); "exactly 1 more prerequisite" is the printed total of requirement letters; faction technologies count as the player's deck only for their own faction.

## Shared files touched

`factions/mod.rs` (module registered, `MODULES` 23), `factions/hooks_movement.rs` (+`own_unit_passage`), `movement.rs`, `invasion.rs` (Research Team, two coexistence guards), `strategy_cards.rs` (Carrina window around `offer_research`/`paid_research`), `technology.rs` (`prerequisites_met` crate-visible, purged-card gate), `promissory.rs` (`shareknowledge` is ACTION-placed), `production.rs` (`spendable_planets` + oceans), `action_cards.rs` (Crash Landing calls Oceanbound), `factions/nekro.rs` (`deepwrought_flagship` lendable), `tests/decision_delivery_inventory.rs` (new sites; the registry row `strategy_cards.rs::offer_research` is now `offer_research_inner`), `ti4-sim/examples/base_faction_soak.rs`. `game.rs` untouched.

## Results

* `cargo test -p ti4-engine -j1 -- --test-threads=12`: lib 2616 passed / 1 ignored, then 1, 4, 5 passed, 0 failed (`out/dw_engine_full2.log`).
* `cargo test -p ti4-policy -p ti4-sim -j1 -- --test-threads=12`: 273 and 51 passed (1 ignored), 0 failed (`out/dw_policy_sim.log`).
* Ledger: deepwrought 11/11, all others unchanged (`out/dw_ledger2.log`).
* Soak `deepwrought 0 25 10`: 25 games, 0 failures (`out/dw_soak.log`).

## Operator rulings 2026-10-07 (supersede open questions 1, 2 and 5 above)

### 1. Research Team on defense (Dane's ruling)

`invasion.rs::offer_research_team_defense` runs right after the invader's own Research Team offer, in `finish_committing`. For each committed planet (the invader is not already coexisting there), every seat with Research Team, units on the planet and not already coexisting is asked (`research_team_defend_coexist`, the Deepwrought decides, not the invader). "Coexist" calls `begin_coexisting(defender, taker = invader)`, so Oceanbound triggers and the ordinary coexistence rules apply: a Deepwrought that CONTROLS the planet steps aside and the invader becomes the controller (3.2, planet exhausted); a seat that does not control it just coexists (3.1). The planet goes into `InvasionReport::coexisted`: `advance_fighting` starts no ground combat there and offers no rule-12 follow-up combat; nothing is captured (`report.captured` empty). Decision taken: control follows 3.2 literally for a controlling defender; if the operator wants the defender to keep control, that is a separate ruling (coexistence.rs has no such clause).

Tests (real `invasion::resolve` commit route): `a_defending_deepwrought_may_coexist_instead_of_fighting`, `the_defending_deepwrought_decides_and_declining_fights` (only the Deepwrought is asked; declining fights), `a_deepwrought_already_coexisting_is_not_asked_when_attacked_again`, `only_a_deepwrought_defender_with_units_on_the_planet_is_offered_research_team_on_defense`.

### 2. Doctor Carrina "if they do"

Read as: the researcher actually needed the ignored prerequisite. (The card note "you can ignore a prerequisite even if you can already meet it" is overridden by the ruling.) Implemented in `factions/deepwrought_research.rs`: the agent is offered only when arming the waiver opens a technology (relevant to that route) that was closed; the placement follows only when a technology gained was not takeable without the waiver (`Window::plain`). A research that gains nothing still undoes the use (agent readied); a research that needed no waiver leaves the agent spent with no placement. Tests: `doctor_carrina_places_nothing_when_the_researcher_already_met_the_prerequisite`, `doctor_carrina_is_not_offered_when_ignoring_a_prerequisite_opens_nothing`, plus the unchanged `doctor_carrina_lets_a_researcher_ignore_a_prerequisite_and_places_infantry_into_coexistence`.

### 3. Doctor Carrina on every research

Window text: "When another player researches a technology". A research must reach the one shared pair `deepwrought_research::open` (before the researcher lists technologies, because the waiver widens the list) and `settle`. `technology::complete_research` is table-less and runs after the choice, so it cannot ask; the shared entry is therefore this pair, wrapped around each route's list-choose-research span (replacing `strategy_cards::deepwrought_agent_offer/settle`, removed).

Routes covered (all through `open`/`settle`):

| Route | Site |
|---|---|
| Technology strategy card primary (free research) | `strategy_cards::offer_research` |
| Technology secondary and the primary's paid second research | `strategy_cards::paid_research` (incl. Yin/Cabal waiver resolution inside) |
| Jol-Nar Specialist Compounds / Technology via breakthrough | `strategy_cards::specialist_compounds` (opened after the colour is fixed) |
| Action card Focused Research | `action_cards::focused_research_spend` |
| Action card Reveal Prototype (unit upgrades) | `action_cards::reveal_prototype` |
| Action card Divert Funding ("then research another technology") | `action_cards::divert_funding` (after the technology is returned) |
| Sardakk N'orr Supremacy ("research a unit upgrade") | `sardakk::supremacy` (opens before the token-or-research choice) |
| Deepwrought Visionaria Select (each paying player) | `deepwrought_cards::visionaria_round` |
| Deepwrought Ta Zern (each loser) | `deepwrought_cards::hero_resolve` |

Not routes / not hooked, with reason: Radical Advancement (replaces an owned technology: a gain, not a research); Nekro Propagation (replaces the research with tokens: nothing researched, settle readies the agent); Share Knowledge, Maw of Worlds, Entropic Scars, Enigmatic Device and exploration cards `ed1`/`ed2` (engine grants a chosen technology ignoring all prerequisites, so no waiver could ever be needed and the agent would never be offered), Book of Latvinia (not implemented as a research; "no prerequisites" so a waiver is moot), Technology Rider (payoff not implemented in the engine), Research Agreement and other pure gains. Ssruu copying of Carrina remains unbuilt (census covers it separately). Yin Impulse Core and other waivers resolve inside the strategy-card routes and are covered there.

Real-route tests: strategy card `doctor_carrina_lets_a_researcher_ignore_..._coexistence` (primary), `doctor_carrina_is_readied_again_when_the_research_gains_nothing` (secondary); action card `action_cards::tests::doctor_carrina_reaches_focused_research`; faction route `doctor_carrina_also_reaches_a_research_made_through_visionaria_select`. Existing Visionaria and Ta Zern scripts gained one `decline` for the new Carrina prompt (the holder is now asked when the waiver opens something for the researcher).

Decision registry: `deepwrought_research.rs::open`/`settle` (Choice, delivered via `deepwrought_research.rs::put`), `invasion.rs::offer_research_team_defense`; the two `strategy_cards.rs::deepwrought_agent_*` rows removed.

### 4. Ocean cards count as influence for voting

`vote.rs::votable_planets` and `VoteWindow::planet_offers` now include the holder's readied ocean cards (`deepwrought::oceans`), so they are offered for influence (and for resources to an Executive Order spender) like any planet and exhaust into `exhausted_planets`. Test: `vote::tests::ocean_cards_count_as_influence_for_voting` (real `VoteWindow`).

### Results (2026-10-07)

* `cargo test -p ti4-engine -j1 -- --test-threads=12`: lib 2625 passed / 1 ignored, then 1, 4, 5 passed, 0 failed (`out/dw_engine_full3.log`).
* `cargo test -p ti4-policy -p ti4-sim -j1 -- --test-threads=12`: 273 and 51 passed (1 ignored), 0 failed (`out/dw_policy_sim3.log`).
* Ledger (`print_faction_ledger --ignored`): identical to `out/dw_ledger2.log`, deepwrought 11/11 (`out/dw_ledger3.log`).
* Soak `deepwrought 0 25 10` (release): 25 games, 0 failures (`out/dw_soak3.log`).
* Shared files touched additionally: `invasion.rs` (+`InvasionReport::coexisted`), `vote.rs`, `action_cards.rs`, `factions/sardakk.rs`, `factions/mod.rs` (+`deepwrought_research`). `game.rs` untouched. No policy/training/UI/server crates.
