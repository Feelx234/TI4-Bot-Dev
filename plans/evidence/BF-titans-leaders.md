# BF-titans-leaders: Titans leaders, Terraform, Slumberstate Computing

Files: `factions/titans_leaders.rs` (new logic + tests), `planets.rs` (`traits_now`,
`attachment_cannons`), `combat.rs` (Geoform cannon in `space_cannon_offense`; post-HITS_TO_ASSIGN
cancellation spend; one route test). No other file touched.

## Items

| Kind | Id | Status | Tests |
|---|---|---|---|
| Commander | `titanscommander` | **claimed** | `the_commander_unlocks_on_five_structures`, `only_the_owners_structures_count`, `the_unlocked_commander_gains_a_trade_good_when_production_is_used` |
| Agent | `titansagent` | **claimed** (space, ground, bombardment, barrage windows) | `tellurian_*` (2, titans_leaders), `combat::space_routes_tests::tellurian_cancels_the_hit_that_would_destroy_the_only_ship` |
| Hero | `titanshero` | **claimed** | `the_hero_is_offered_as_an_action_only_unlocked`, `geoform_readies_elysium_and_adds_three_and_three`, `geoform_space_cannon_fires_at_ships_activating_the_system` |
| Promissory | `terraform` | **claimed** | `terraform_attaches_to_a_controlled_non_home_planet`, `terraform_refuses_home_planets_mecatol_and_the_owner`, `terraform_cannot_be_offered_twice_and_a_wrong_option_is_refused` |
| Breakthrough | `titansbt` | **claimed** | `slumberstate_*` in `titans.rs` (3) |
| Neutrality | no Titans seat | done | `games_without_the_titans_see_nothing_of_it` |

## Card text and route

* Tellurian: "Before a hit would be assigned: You may exhaust this card to cancel that hit."
  Timing ability on `HITS_TO_ASSIGN` (space combat round hits; `combat.rs` now spends the granted
  cancellation straight after the emit, before the sustain offer) and on `SPACE_CANNON_HITS`
  (absorbed immediately after). Effect = exhaust + `combat::grant_hit_cancellation(1)`. Optional;
  condition: the event's `player` is the owner, hits > 0, agent readied.
* Tungstantus unlock: "Have 5 structures on the game board." `Hooks::commander_unlocked`, counting
  the owner's `isStructure` units in space and on planets. Effect: existing `titanscommander`
  window in `borrowed_commanders.rs` (reused, not duplicated).
* Ul the Progenitor: "ACTION: Ready Elysium and attach this card to it. Its resource and influence
  values are each increased by 3, and it gains the SPACE CANNON 5(x3) ability as if it were a
  unit." `leader_action` (Elysium controlled by someone and not already wearing Geoform) +
  `use_leader`: remove Elysium from `exhausted_planets`, push attachment `titanshero` onto
  `planet_attachments`. Value +3/+3 flows through `production::attachment_bonus`. Cannon:
  `planets::attachment_cannons` (reads `spaceCannonHitsOn`/`spaceCannonDieCount` from the
  attachment record, owner = current controller) rolled in `space_cannon_offense`, subject to the
  same bars as unit guns. Shared code purges the hero card; the attachment stays on the planet.
* Terraform: "ACTION: Attach this card to a non-home planet you control other than Mecatol Rex. Its
  resource and influence values are each increased by 1, and it is treated as having all 3 planet
  traits (cultural, hazardous, and industrial)." Component options
  `faction|titans|terraform|<planet>` for a holder who is not the owner, one per legal planet
  (controlled, not any seat's home or printed homeworld, not Mecatol either tile, none attached
  yet). Pushes attachment `titanspn`, marks the note faceup (stays with the holder). Traits:
  `planets::traits_now` (printed + attachment `planetTypes`).

## Rule decisions

* Tellurian cancels hits on the **owner's** units only (card text names no owner; an agent's
  benefit goes to its owner).
* Geoform may be used while another player controls Elysium (text requires no control); the
  planet's controller owns the gun and the value. Offered whenever Elysium is controlled by anyone.
* Hero attachment survives the hero's purge (the attachment record, not the leader card, carries
  the effect). Plasma Scoring does not add a die to the Geoform gun (not a unit).
* Terraform is one card: while attached to any planet it is not offered again. Attaching does not
  return the note to the Titans.

## Delivery sites (all wired; the original request list is closed)

Each former "needed outside my scope" item has landed in the shared files:

1. Geoform: `invasion.rs::space_cannon_defense` rolls `planets::attachment_cannons` for owners other
   than the invader; `combat.rs::space_cannon_offense` rolls it for the active system. `titanshero`
   claimed.
2. Terraform: `exploration::choose_deck` and every state-aware trait reader (`action_cards.rs`
   Archaeological Expedition, `faction_abilities.rs` Peace Accords explore, `faction_techs.rs`,
   `legendary.rs` Maxis explore, `factions/naaz.rs::explorable`) read `planets::traits_now`.
   `terraform` claimed in `PROMISSORY`.
3. Tellurian: `GROUND_HITS_TO_ASSIGN` (ground combat, bombardment, space cannon defense, Harrow) and
   `ANTI_FIGHTER_BARRAGE_HITS` windows exist (`combat::open_hits_window`, `spend_cancellations` is
   `pub(crate)`); both are in `titans_leaders::timing_abilities`. `titansagent` claimed.
4. `titansbt` Slumberstate Computing: (a) `invasion.rs::coexist_instead` (asks fight/coexist, calls
   `coexistence::begin`), (b) `slumberstate_draw_bonus` via `EconomyHooks::action_card_draw_bonus`,
   (c) `sleeper_allowance` at the Titans' `TURN_BEGAN`. Claimed in `BREAKTHROUGHS`.
5. `titans.rs` keeps `..super::titans_leaders::HOOKS` (it carries `commander_unlocked`,
   `leader_action`, `use_leader`, `component_actions`, `perform_component`). If it ever sets
   `component_actions`/`perform_component` itself it must chain `titans_leaders`' (module-private;
   make them `pub(crate)` then).

## Decision sites to register

None added by this work: Tellurian uses the resolver's optional-ability prompt, Terraform and
Geoform use component options and `leader_action`. The only registry failure in the engine suite
is `titans.rs::ask` (`Choice`, `AskObserved`), the other agent's file.

## Commands and results

* `cargo test -p ti4-engine --lib -j4 titans`: 18 passed, 0 failed.
* `cargo test -p ti4-engine --lib -j4 tellurian_cancels_the_hit`: 1 passed.
* `cargo test -p ti4-engine -j4 --no-fail-fast -- --test-threads=16`: lib 2144 passed, 1 ignored;
  content_ids_resolve 1 passed; decision_delivery_inventory 3 passed, 1 FAILED
  (`every_producer_and_delivery_site_matches_the_reviewed_registry`: unreviewed
  `titans.rs::ask` Choice and AskObserved, one each; not from this work); remaining binary 5 passed.
* `print_faction_ledger`: `titans 10/16 implemented`; unimplemented listed: `titans_mech`,
  `titans_pds` (other agent), Leader `titansagent`, Leader `titanshero`, Breakthrough `titansbt`.
  The ledger does not list the unclaimed `terraform` note.
