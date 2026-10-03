# BF-yin: The Yin Brotherhood (`crates/ti4-engine/src/factions/yin.rs`)

Branch `wp/base-factions`, uncommitted. Texts from `abilities.json`, `technologies.json`, `units.json`,
`promissory_notes.json`, `leaders.json`, `breakthroughs.json` at DEFAULT. Python not used.

## Items

| Kind | Id | Status | Tests |
|---|---|---|---|
| ability | `devotion` | done (hook `combat.produced_hits`, `RoundEnded`, producer-assigned hit) | `devotion_destroys_a_cruiser_for_a_producer_assigned_hit`, `devotion_declined_or_unavailable_changes_nothing` |
| ability | `indoctrination` | done (timing `GROUND_COMBAT_STARTED` After, pays influence via `production::pay_seeing`, `action_cards::replace_unit`) | `indoctrination_pays_two_influence_and_swaps_one_infantry`, `..._declined_or_unaffordable_changes_nothing`, `..._belongs_to_the_yin_player_only` |
| technology | `ic` Impulse Core | done (`produced_hits`, `CombatStart`, `non_fighter`) | `impulse_core_forces_the_hit_onto_a_non_fighter`, `impulse_core_needs_the_technology_and_a_target`, `impulse_core_and_devotion_run_inside_a_real_combat` (real `CombatWindow`) |
| technology | `yso` Yin Spinner (Omega) | done (timing `UNITS_PRODUCED` After, `place_units_choosing`) | `yin_spinner_places_two_infantry_where_the_owner_chooses`, `yin_spinner_is_not_offered_without_the_technology_or_for_another_producer` |
| unit | `yin_flagship` Van Hauge | done (timing `SHIP_DESTROYED` After; `combat::destroy_units` for every ship in the system; the emitter drains the staged destructions) | `van_hauge_takes_every_ship_in_the_system_with_it`, `van_hauge_does_not_fire_for_other_ships_or_other_systems` |
| unit | `yin_mech` Moyin's Ashes | done (Indoctrination's "mech" form, 3 influence) | `indoctrination_with_a_mech_costs_one_more_and_places_a_mech` |
| promissory | `greyfire` | done (timing `GROUND_COMBAT_STARTED` After for the holder; `give_back` to Yin) | `greyfire_replaces_an_infantry_and_returns_to_the_yin_player`, `greyfire_needs_two_ground_forces_and_a_non_yin_opponent` |
| leader | `yinagent` Brother Milor | done (timing `SHIP_DESTROYED` / `GROUND_FORCE_DESTROYED` After; exhausts; destroyed unit's owner places) | `brother_milor_gives_two_fighters_for_a_destroyed_ship`, `..._declined_leaves_the_agent_ready`, `..._two_infantry_for_a_ground_force_lost_in_combat_only` |
| leader | `yincommander` Brother Omar | **partial**, not claimed: unlock done (`commander_unlocked`, mark `yin:ability_used:<player>` set by Indoctrination and Devotion); effect blocked | `brother_omar_unlocks_once_a_faction_ability_was_used` |
| leader | `yinhero` Dannel of the Tenth | **blocked** (no invasion-as-effect route) | none |
| breakthrough | `yinbt` Yin Ascendant | **blocked** (no alliance-grant route) | none |

Ledger: `yin 8/11 implemented` (open: `yincommander`, `yinhero`, `yinbt`).

## Hook requests

1. **`yincommander` effect** (`technology.rs` + a hook in `factions/mod.rs` or `faction_abilities.rs`). "This card
   satisfies a green technology prerequisite. When you research a tech owned by another player, you may return 1
   of your infantry to reinforcements to ignore its prerequisites." `Hooks::waived_prerequisites` is a pure
   budget that waives *any* slot, so it cannot express "adds one green" and has nowhere to charge the infantry.
   Needed: (a) a hook `extra_prerequisite_colours(state, content, player) -> &[colour]` (or `Vec<(&str, usize)>`)
   read by `technology::prerequisites_met` into `holdings`; (b) a per-research optional cost hook
   `research_waiver_offer(state, content, player, tech) -> Option<Offer>` plus
   `research_waiver_paid(&mut state, player, tech)` so the engine asks "return 1 infantry to ignore all
   prerequisites?" and the module performs the return (like Inheritance Systems, `technology::INHERITANCE_SYSTEMS_COST`).
   Whether the tech is "owned by another player" is a fact the module can read from `state`.
2. **`yinhero`**: an invasion as an effect (`game.rs` `Aftermath` entry fed by `use_leader`), commitment from
   reinforcements to any non-home planets in any system, no SPACE CANNON against these units (flag on the window),
   per `BF-00c-ground.md` item 9. Nothing in `yin.rs` is possible without it.
3. **`yinbt`**: "gain the alliance ability of a random, unused faction" when gaining the card or scoring a public
   objective: a grant record on `GameState` plus an alliance-ability dispatch for the granted faction.
   Also needs an "on score public objective" event. Not buildable here.
4. Optional: a typed `FACTION_ABILITY_USED` record would replace the private `faction_marks` key for the
   commander unlock; not required.

## Decision sites to register (`tests/decision_delivery_inventory.rs`, coordinator)

| Function | Count |
|---|---|
| `yin.rs::ask_one` | 1 `Choice` (`Producer`, `Delivery::ObservedHere`) and 1 `AskObserved` (`OBSERVED_ASKS`) |

Currently `every_producer_and_delivery_site_matches_the_reviewed_registry` fails on exactly this site
(`yin.rs::ask_one`, plus other agents' sites). `ask_one` serves Devotion, Impulse Core and Indoctrination.
Other choices go through registered helpers (`place_units_choosing`, `production::pay_seeing`) or the
resolver's own optional-ability ask.

## Rules questions

- Yin Spinner "place up to 2 infantry on any planet you control or in any space area": implemented as all units
  to one spot (the helper's contract). If the two may be split across spots, call the helper twice with 1.
- Devotion: after retreats and before `SPACE_COMBAT_ROUND_ENDED` (as BF-00c-space); not offered when the opponent
  has no ship in the system.
- Greyfire "2 or more ground forces that are not controlled by the Yin player": read as the holder's opponent being
  a non-Yin player with 2+ ground forces on the planet, including 1+ infantry to replace.
- Brother Milor "during combat": `SHIP_DESTROYED` is emitted only from space combat and the Direct Hit played into
  it, so all such events count; ground forces count for `ground_combat` and `harrow`, not bombardment or space
  cannon defense. Fighters are placed without a capacity check (excess goes at end of turn, 16.3); the
  destroyed unit's owner places.
- Van Hauge fires for any `SHIP_DESTROYED` of a `yin_flagship` (chain destructions are announced by combat.rs).
- Does ability production (not relevant to Yin) / does Indoctrination's replacement count as "participating"
  infantry: any of the opponent's infantry on the planet at the start of the combat.
- Indoctrination uses influence only through the standard payment loop (planets, trade goods).

## Commands and exact results

| Command | Result |
|---|---|
| `cargo test -p ti4-engine --lib factions::yin` | 20 passed, 0 failed |
| `cargo test -p ti4-engine --lib factions::` | 68 passed, 0 failed, 1 ignored (ledger) |
| `cargo clippy -p ti4-engine --all-targets` | 0 warnings in `yin.rs` (others' warnings remain) |
| `rustfmt --edition 2024 --check crates/ti4-engine/src/factions/yin.rs` | clean |
| `cargo test -p ti4-engine -q --no-fail-fast` | lib 1557 passed, 0 failed, 1 ignored; `decision_delivery_inventory` 3 passed, 1 failed (`every_producer_and_delivery_site_matches_the_reviewed_registry`: unregistered `yin.rs::ask_one`; not edited); other targets ok |

Earlier mid-session full runs showed many `game::tests` failures and a `movement.rs` compile error from other
agents' in-progress edits; a rerun was green.

## Ledger line

`cargo test -p ti4-engine --lib print_faction_ledger -- --ignored --nocapture`:
`yin        8/11 implemented` (missing: Leader yincommander, Leader yinhero, Breakthrough yinbt).

## Update 2026-10-03 (new shared routes)

| Item | Status | Tests |
|---|---|---|
| `yincommander` Brother Omar effect | done, claimed: `StrategyHooks::extra_prerequisite_colours` (green x1), `research_waiver_offer` (tech owned by another seat, 1+ infantry on the board) and `research_waiver_paid` (first infantry in board order returns; no table in the hook) | `brother_omar_supplies_green_and_waives_prerequisites_for_an_infantry`, `brother_omar_green_pays_a_single_green_prerequisite` (real `technology::can_research`) |
| `yinhero` Dannel | done, claimed: `leader_action` + `use_leader`. Asks up to 3 planets first (non-home, from the map in `context.galaxy`; refuses with no map), lands infantry, runs `invasion::ground_combat` per planet to its end, then `establish_control`; space cannon is never run; state restored on an illegal answer | `dannel_lands_up_to_three_infantry_and_takes_an_empty_planet`, `dannel_fights_the_defenders_to_the_end_and_declining_changes_nothing` |
| `yinbt` | out of scope: needs the alliance abilities of other factions' commanders (not built); `BREAKTHROUGH_GAINED` now exists but the grant itself has no route | none |

Limits of the hero: the synchronous resolver emits no ground-combat events, so Indoctrination, Greyfire, Brother Milor and
`GROUND_FORCE_DESTROYED` listeners do not see these combats; one rival side is fought per planet; `leader_action` cannot see
the map, so it is offered whenever infantry remain in the box (a no-map use changes nothing and does not purge).
Decision sites to register: unchanged (`yin.rs::ask_one` x1 Choice, x1 AskObserved; the hero reuses it).

Results: `cargo test -p ti4-engine --lib factions::yin` 24 passed; clippy 0 warnings in yin.rs; rustfmt clean.
Ledger: `yin 10/11 implemented` (missing: Breakthrough yinbt).

## Correction 2026-10-03 (Codex completion)

Brother Omar is now claimed. The research-waiver contract exposes every legal infantry location as a
stable payment target; the Technology-card resolver asks first for the waiver and then for that exact
infantry. `research_with_waiver` revalidates the selected offer and payment, snapshots state before
charging it, and grants the technology only after the payment hook succeeds. Declining either choice
returns to the technology choice with no state mutation. Table-less `technology::research` never
auto-spends the optional waiver. Normal prerequisite research and the existing Inheritance Systems
route remain unchanged.

Dannel repeats the existing synchronous ground-combat call until no rival ground force remains or
Yin has none, correcting the former one-rival-only behavior. It remains **unclaimed** because that
resolver does not emit the required ground-combat / destruction event windows; therefore Yin
listeners such as Indoctrination, Greyfire, and Brother Milor cannot participate. The shared route
must expose eventful ground-combat resolution usable by a leader effect, while preserving Dannel's
no-space-cannon condition.

Focused validation before concurrent combat edits blocked rebuilding: `cargo test -p ti4-engine --lib
factions::yin --no-fail-fast` — 25 passed, 0 failed. New focused waiver tests are present in
`technology::bf_f3_tests` and `strategy_cards::tests`; their run is pending the unrelated combat.rs
compile repair. The ledger is now `yin 9/11 implemented` (missing: `yinhero`, `yinbt`); `yinbt`
remains unclaimed until the full alliance text has a grant and dispatch route.
