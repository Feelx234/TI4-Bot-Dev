# BF-nekro-units: Nekro flagship, mech, leaders and the two further technologies

Part B of the Nekro Virus. Part A (abilities, Valefar Assimilators, Z, Antivirus) is `factions/nekro.rs`
and `plans/evidence/BF-nekro.md`. Code: `factions/nekro_units.rs`, plus `combat.rs`, `invasion.rs`,
`production.rs`.

## Items

| Kind | Id | Status | Tests (`factions::nekro_units::tests::`) |
|---|---|---|---|
| unit | `nekro_flagship` (The Alastor) | done | `nekro_alastor_makes_the_chosen_ground_forces_ships`, `nekro_alastor_chooses_any_number_including_none`, `nekro_alastor_is_asked_per_planet_and_unit_type`, `nekro_alastor_participants_roll_in_a_real_combat_and_the_marks_end_with_it`, `nekro_a_participant_can_sustain_damage_and_is_then_destroyed`, `nekro_a_destroyed_participant_leaves_the_others_choice_intact`, `nekro_a_retreat_ends_the_participation_but_leaves_the_forces_on_their_planet`, `nekro_alastor_is_neutral_without_the_flagship_or_a_combat_seat` |
| unit | `nekro_mech` (Mordred) | done | `nekro_mordred_gets_plus_two_against_an_opponent_with_a_token`, `nekro_mordred_bonus_reaches_the_space_and_ground_thresholds` |
| leader | `nekroagent` (Nekro Malleon) | done, needs wiring | `nekro_malleon_*` (5) |
| leader | `nekrocommander` (unlock) | done, needs wiring | `nekro_commander_needs_three_technologies_and_valefar_counts_only_with_a_token` (the draw effect was already in `borrowed_commanders.rs`) |
| leader | `nekrohero` (UNIT.DSGN.FLAYESH) | done, needs wiring | `nekro_hero_devours_a_specialty_planet_for_goods_and_a_technology`, `nekro_hero_needs_a_specialty_planet_in_a_system_with_its_units` |
| technology | `nekroc4y` | done (no obtain route, see below) | `nekro_null_reference_produces_the_destroyed_ship_type_at_the_home_dock`, `nekro_null_reference_is_neutral_without_the_card_the_means_or_a_ship` |
| technology | `nekroc4r` | done, needs wiring | `nekro_error_error_*` (4) |
| neutrality | all | done | `nekro_a_game_without_nekro_is_unchanged`, the neutral halves of the tests above |

All claimed in `LEADERS`, `UNITS`, `TECHNOLOGIES` of `nekro_units.rs`; `nekro.rs` already points its `MODULE` at them.
The ledger line is `nekro 11/14 implemented`; the three open ones are part A's (`vax`, `vay`, `nekrobt`).

## Design

* **Alastor.** A mandatory `SPACE_COMBAT_STARTED` window (after) for a seat in the combat that has the flagship in
  the system's space area and ground forces on planets there. One question per planet and unit type: how many (1..n or
  none). The answer is a mark `nekro:alastor|<activation_seq>|<system>|<player>|<planet>|<unit type>` holding the count.
  Ground forces stay on their planets. `combat::planet_combatants` now returns the Naaz Eidolon Maximum **and** the
  first n marked ground forces of that type on that planet, and every place that carried the Maximum
  (`ships_of`, SUSTAIN DAMAGE offer, pending sustain, apply sustain, casualty removal) goes through it.
  Sustain options for a non-ship are always `planet:<p>:variant:<index>`; `planet_unit_sustains` gives a ground force
  its own printed SUSTAIN DAMAGE (law and `may_sustain` hooks still apply). `remove_combat_ship` removes a participant
  from its planet and shrinks the mark. The marks end at `SPACE_COMBAT_ENDED` (`announce_end`) and when the owner
  retreats (`retreat_to`); a mark of an earlier activation is ignored by its key.
* **Mordred.** `nekro_units::mech_roll_bonus`, read by `combat::effective_from` (space) and
  `invasion::ground_combat_value` (ground). "Opponent has an X or Y token on a technology": for each technology of
  an opponent in the combat, `nekro::assimilated_card(state, mech_owner, tech)`.
* **Malleon.** `leader_action` / `use_leader`. Offered when some player holds an action card or a command token. The
  owner chooses among the players who could pay; the chosen player decides (card, token pool, or neither). Atomic.
* **Commander unlock.** `commander_unlocked`: technologies owned minus the owned Valefar cards plus
  `nekro::assimilators_with_tokens`, at least 3.
* **Flayesh.** `leader_action` / `use_leader`. Targets: planets with a specialty (attachments included,
  `planets::tech_specialties_now`) in a system holding any of the owner's units. Planet and technology are chosen before
  anything changes; then the other players' units on the planet are removed (ground forces staged as destroyed with
  cause `nekrohero`, structures removed), trade goods = resources + influence (`planet_value_now`), and the technology
  is granted with `technology::grant` (gaining, not researching: Propagation is not involved). The technology must be a
  coloured, non-upgrade card of the current deck (`technology::active_aliases`) not owned, belonging to no faction or to
  the owner's.
* **`nekroc4y`.** `SHIP_DESTROYED` (after) for the owner's own ship (`is_ship`), optional, only when the owner has a
  space dock on a planet of the home system and `production::can_produce_unit_by_ability` says the same type can be
  bought and placed. Production is `production::produce_unit_by_ability` (new): the ordinary ability window with limit
  1 and `ProductionWindow::with_only_unit`, so payment, placement and fleet limits are the usual ones.
* **`nekroc4r`.** `component_actions` / `perform_component`, ids `faction|nekro|c4r|{pds,repair,draw}`, one option per
  ACTION that can resolve now; the card is exhausted after a performed action. PDS: `action_cards::place_units_counted`
  on a controlled planet that passes `structure_allowed` and has a PDS in reinforcements; repair clears
  `sustained_damage` on every unit of the owner; draw uses `choose_from_own_hand`, `hooks_cards::discard_chosen`
  (staged `ACTION_CARD_DISCARDED`, delivered by `Game::announce_staged_cards`) and `action_cards::draw`.

## Coordinator edits needed (nekro.rs is not mine)

`nekro.rs` `MODULE.hooks`, in addition to what part A sets:

```rust
        commander_unlocked: Some(super::nekro_units::commander_unlocked),
        leader_action: Some(super::nekro_units::leader_action),
        use_leader: Some(super::nekro_units::use_leader),
        component_actions: Some(super::nekro_units::component_actions),
        perform_component: Some(super::nekro_units::perform_component),
```

Until these are set, the rustc dead-code warnings for those five functions (and their helpers) stay, and the agent, hero,
commander unlock and `nekroc4r` are claimed but not reachable through the real leader/component route. Their tests call the
functions directly. After wiring, one real-route check is worth adding by the coordinator: `leaders::component_actions`
offers `nekroagent` (window "During the action phase:") and `nekrohero` (ACTION), and `leaders::check_unlocks` unlocks
`nekrohero` ("Have 3 scored objectives", generic) and `nekrocommander`.

`MODULE.technologies` is `nekro_units::TECHNOLOGIES` (`nekroc4y`, `nekroc4r`); part A's `vax`/`vay` must be added by the
coordinator (the ledger shows them open).

## Decision sites to register (`tests/decision_delivery_inventory.rs`)

| Module | Function | Choice | AskObserved |
|---|---|---|---|
| `nekro_units.rs` | `ask` | 1 | 1 |
| `production.rs` | `produce_unit_by_ability` | 0 | 1 |

All of Alastor, Malleon, Flayesh and the `nekroc4r` PDS question go through `nekro_units::ask`; the card choice goes through
the existing `action_cards::choose_from_own_hand`.

## Rules questions and limitations

1. **Alastor, ground forces in the space area.** "Your ground forces in this system" is read as ground forces on planets;
   those aboard ships are cargo and are not offered.
2. **Alastor, participants without a ship.** The participants count as ships for the combat, so a combat continues while
   any of them stands even after the last real ship is gone (unlike the Eidolon Maximum, which needs a real ship). The
   opposite reading would make `ships_of` depend on a real ship; say if the Maximum precedent should apply.
3. **Alastor, which units.** Participants are the first n units of the chosen type on that planet in list order; the
   card's "any number" is per planet and type, so damaged and undamaged mechs of one planet are not told apart.
4. **Alastor and retreat.** A retreating owner's participants leave the combat and stay on their planets (they have no move
   value). They do not travel with the fleet.
5. **Alastor and Assault Cannon / "3 non-fighter ships".** Participants are counted as ships there too (they are in
   `ships_of`).
6. **Malleon.** "Spend 1 command token from their command sheet": the token leaves the chosen pool for the supply; the
   player chooses the pool. No STRATEGY_TOKEN_SPENT is staged. The owner chooses among players who could pay; a chosen player
   who declines still uses the agent up.
7. **Flayesh technology.** "1 technology that matches the specialty": any coloured current-deck technology of the matching
   colour the owner does not own and may hold; unit upgrades are not coloured and are excluded. A planet with no such
   technology left still resolves (units destroyed, trade goods).
8. **`nekroc4y` / `nekroc4r` are not obtainable in play.** They are `source: pok` in the corpus and attached to the Nekro
   faction, but they appear in no deck (`techs_pok_c4` lists `vax` and `vay` only) and not on the faction sheet, so
   `technology::researchable` never offers them. They are implemented as printed and gated on owning the card; how a Nekro
   comes to own one is a content question.
9. **`nekroc4y` "produce".** Read as a production at that dock: the cost is paid and the usual placement and fleet checks
   apply; a dock in the home system is required. A fighter destroyed offers the two-for-one fighter production.
10. **Staged discards.** The agent's and `nekroc4r`'s discards stage `ACTION_CARD_DISCARDED`; `Game` delivers staged card
    events after the action, but a direct call of the functions (as the tests do) leaves them staged.

## Commands and results

* `cargo test -p ti4-engine --lib -j1 nekro_units`: 26 passed, 0 failed (env `CARGO_PROFILE_TEST_DEBUG=0 CARGO_PROFILE_TEST_INCREMENTAL=false CARGO_PROFILE_TEST_CODEGEN_UNITS=16`, `-j1`).
* `cargo test -p ti4-engine -j1 --no-fail-fast -- --test-threads=12`: lib 2508 passed, 1 failed, 1 ignored; `content_ids_resolve` 1 passed;
  `decision_delivery_inventory` 2 passed, 2 failed; doc-tests 5 passed. Failures:
  * `faction_abilities::tests::the_gap_is_reported_rather_than_implied`: asserts that some ability is still unimplemented;
    with every faction's abilities claimed the list is empty. Not related to these files.
  * `decision_delivery_inventory` (both): the new sites above, part A's `nekro.rs::take_from`, and other agents' unreviewed
    sites (arborec, ghost, mentak, naalu, naaz, winnu, yssaril).
* Ledger (`print_faction_ledger -- --ignored`): `nekro 11/14 implemented`; open: `vax`, `vay` (technologies), `nekrobt`.
* rustfmt: `combat.rs`, `nekro_units.rs` formatted (combat.rs was clean before the edits); `invasion.rs` clean;
  `production.rs` had unformatted hunks before and was left alone (the new code there is hand-formatted).
