# BF-naaz: The Naaz-Rokha Alliance

Implementer: Sonnet subagent. Branch `wp/base-factions`, nothing staged or committed. Only `factions/naaz.rs` and this file written.
Texts: `crates/ti4-content/content/*.json` at DEFAULT. Historical Python: not used.

## Items

| Kind | Id | Status | Tests |
|---|---|---|---|
| ability | `fabrication` | done (claimed). ACTION; relic option (2 same-type fragments, frontier fragments stand in, spent last) and token option (1 fragment, pool chosen by the player, capped by reinforcements, 20.4) | `fabrication_turns_two_matching_fragments_into_a_relic`, `fabrication_frontier_fragments_stand_in_and_are_spent_last`, `fabrication_turns_one_fragment_into_a_token_of_the_chosen_pool` |
| promissory | `bmf` | done (claimed). Offered to a holder who is not Naaz; purges, gains relic, `give_back` | `black_market_forgery_is_the_holders_action_and_returns_home`, `..._needs_two_matching_fragments` |
| unit | `naaz_flagship` | done (claimed). `unit_dice`: +1 die for each Naaz mech (`naaz_mech`, `naaz_mech_space`) of the owner in the flagship's system, space and ground | `the_flagship_gives_mechs_in_its_system_a_die` |
| leader | `naazagent` | done (claimed). `TURN_PASSED` after; the player whose turn ended picks one explorable planet they control, then the agent exhausts and the planet is explored | `the_agent_lets_a_player_explore_one_of_their_planets`, `the_agent_may_be_declined_and_must_be_ready` |
| leader | `naazcommander` | done (claimed). Unlock (mechs in 3 systems, space or planet, any mech base type) via `commander_unlocked`; effect on `PLANET_CONTROL_GAINED` after, only with `previous_owner` | `the_commander_unlocks_with_mechs_in_three_systems`, `the_commander_explores_a_planet_taken_from_another_player` |
| leader | `naazhero` | done (claimed, with a rules question below). Gains 1 relic, up to 2 secondaries of readied (held, not exhausted) or unchosen cards (not `te6warfare`), each once; rolled back if a secondary errors | `the_hero_gains_a_relic_and_performs_up_to_two_secondaries` (relic count and deck; the secondary outcome itself is not asserted), `the_hero_is_not_offered_without_a_relic_to_gain`, `the_hero_does_not_offer_exhausted_cards` |
| tech | `sc` Supercharge | **partial, not claimed**. Space combat rounds only (`space_combat_round_started` asks, exhausts, marks `combat_round_seq`; `unit_roll_modifier` +1 for that round) | `supercharge_adds_one_for_the_round_it_was_exhausted_in` |
| ability | `distant_suns` | blocked, Hook request 1 | none |
| tech | `pfa` | blocked, Hook request 1 | none |
| unit | `naaz_mech`, `naaz_mech_space` | not claimed, Hook request 3 | none (flip API tested in `fleet.rs`) |
| unit / breakthrough | `naaz_voltron`, `naazbt` | not started, Hook request 4 | none |
| tech (starting) | `pa`, `aida` | not touched (not on the module's lists; not in the ledger gaps) | |

Regression test for no Naaz seat: `a_game_without_naaz_is_offered_nothing` (no component action, no exploration, board and marks unchanged after `TURN_PASSED` and `PLANET_CONTROL_GAINED`).

## Hook requests

1. **Explore hook** (`exploration.rs`, `explore_drawn`/`explore_with`; all callers go through it: invasion.rs:2602, game.rs:2336, action_cards.rs:3679, faction_abilities.rs:732, legendary.rs:521, relics.rs:668).
   Distant Suns needs a draw-time hook, e.g. in `faction hooks_cards` or `Hooks`:
   `explore_extra_draw: Option<fn(&GameState, &ContentStore, SourceSet, &PlayerId, &PlanetId) -> bool>` (true when the player has a mech on that planet), consulted in `explore_drawn` when `planet` is `Some`: draw a second card from the deck, ask the explorer which to resolve, discard the other (a discarded fragment is not gained).
   Pre-Fab Arcologies needs `explored: Option<fn(&mut GameState, &ContentStore, SourceSet, &PlayerId, &PlanetId, &Explored)>` called at the end of `explore_with` when `planet` is `Some` ("after you explore a planet, ready that planet"; the module would remove the planet from `exhausted_planets`). No typed `PLANET_EXPLORED` event exists; `FRONTIER_EXPLORED` is a log label only.
2. **Ground combat round start**: Supercharge says "start of a combat round"; only space rounds have `space_combat_round_started`. Request `ground_combat_round_started` (same signature) from `invasion.rs`, and a `unit_roll_modifier` read for it (the modifier uses `combat_round_seq`; confirm ground rounds advance it or give a ground sequence).
3. **Mech flip call sites** (BF-00d request 4, still unwired): `fleet::flip_form` at start of a space combat (before `combatants`, since a ground-form mech in space is not a ship), at end of the space battle in the active system, and when a space form is committed to land. I did not flip from `space_combat_round_started`: no hook flips back, and the space form is not a ground force, so it could not land afterwards.
4. **Eidolon Maximum / Absolute Synergy**: needs a breakthrough trigger (4 mechs in one system, return 3, flip card), replacement of the mech's unit type by `naaz_voltron`, "cannot be assigned hits from unit abilities", "repair at the start of every combat round" (`space_combat_round_started` can do the repair for space rounds), "game effects cannot place or produce your mechs" (production/placement gate, `cannot_produce` exists in `hooks_economy` but placement via effects does not), and "when destroyed or removed, flip this card and return it". Not started: too many shared sites.

## Decision sites to register (functions in `naaz.rs` that build or ask a `Choice`)

| Function | Count | Question |
|---|---|---|
| `ask` (helper) | 1 | used by the five below |
| `purge_for_relic` | 1 via `ask` | which fragment type (only when more than one qualifies) |
| `fabricate_token` | 2 via `ask` | which fragment type (only if >1); which pool |
| `agent` | 1 via `ask` | planet to explore (asked of the player whose turn ended) |
| `use_leader` | 1 via `ask`, in a loop | which strategy card, or decline |
| `space_combat_round_started` | 1 direct `table.ask_seeing` | exhaust Supercharge or decline |

The optional-ability yes/no prompts (agent, commander) come from the resolver. The commander and agent also reach `exploration::choose_deck` (shared site).

## Rules questions

* Fabrication / BMF: whether a frontier fragment may be the sole "type" (counted as any of the three decks, as 35.9 does for relics).
* Fabrication token: no pool is printed; the player chooses (Leadership-style). Capped by reinforcements, so no option is offered at 0.
* Hero: "spend command tokens from your reinforcements instead of your strategy pool". A spent token returns to reinforcements, so I only require one to be there and move nothing. The Leadership secondary (influence) is unchanged. A secondary that is "not performed" for lack of effect is not skipped, only an `Err` rolls the action back (relic included).
* Hero: the secondary of a held card is performed by the Naaz player although its holder is someone else; `strategy_cards::secondary` does not read the holder.
* Agent: "explore 1 of their planets" taken as any planet the player controls whose trait deck has a card; space stations and home planets (no trait) are not offered.
* Flip: per unit or all Eidolons (BF-00d note). Z-Grav Eidolon "also a ship" and fleet supply: unchanged from BF-00d.

## Commands run (LIBTORCH=D:/Projects/ti4-engine-rs/out/libtorch-2.9.1-cpu)

| Command | Result |
|---|---|
| `cargo test -p ti4-engine --lib -- factions::naaz` | 16 passed, 0 failed |
| `cargo test -p ti4-engine --lib -- factions::` | 254 passed, 0 failed, 1 ignored |
| `cargo clippy -p ti4-engine --all-targets` | no warning in `naaz.rs` (other files warn: not mine) |
| `rustfmt --edition 2024 crates/ti4-engine/src/factions/naaz.rs` | clean |
| `cargo test -p ti4-engine -q --no-fail-fast` | lib 1763 passed, 1 ignored; all binaries ok except `decision_delivery_inventory::every_producer_and_delivery_site_matches_the_reviewed_registry` (unregistered sites from several faction files, including mine, listed above) |

Ledger (after this package): `naaz 6/13 implemented`; missing: `distant_suns`, `pfa`, `sc`, `naaz_mech`, `naaz_mech_space`, `naaz_voltron`, `naazbt`.

Note: an earlier run failed to compile because of another agent's `saar.rs` (`across_outer`); retried until it built.

## Review fixes

* S6: Supercharge stays **live** (space combat rounds only, a legal subset) and unclaimed until the ground half exists (Hook request 2).
* N3: the hero checks a reinforcement token per card; Leadership (influence secondary) needs none (`costs_token` in `use_leader`).
* N5: Fabrication/BMF with only frontier fragments offers one type, not three equivalent ones (`purge_for_relic`).
* N4 (Hook request 5, shared): the agent's `TURN_PASSED` window never fires after the last turn of the action phase because `TURN_PASSED` is skipped there; `game.rs` should emit it for the final pass too.
* Decision sites: unchanged in number and names (`ask` callers: `purge_for_relic`, `fabricate_token`, `agent`, `use_leader`; `space_combat_round_started`).

## Wave F routes (BF-F1/F2/F3)

| Item | Status | Tests |
|---|---|---|
| `distant_suns` | done, claimed. `EconomyHooks::explore_extra_draw`: Naaz seat with a mech (any `mech` base type) on the explored planet; the shared route draws the second card and asks which to resolve (decision site is in `exploration.rs`) | `distant_suns_draws_an_extra_card_for_a_planet_with_a_mech` (no mech, other player's mech: one card) |
| `pfa` | done, claimed. `EconomyHooks::explored`: owner of `pfa` removes the planet from `exhausted_planets` | `pre_fab_arcologies_readies_the_planet_after_exploring` |
| `sc` | done, claimed. `GroundHooks::ground_combat_round_started` shares `offer_supercharge` with the space hook; the mark is `<space|ground>:<combat_round_seq>` | `supercharge_adds_one_for_the_round_it_was_exhausted_in`, `supercharge_also_starts_ground_combat_rounds` |
| `naaz_mech`, `naaz_mech_space` | claimed: flips are wired at space-combat start/end, landing and retreat (BF-F1/F2); stats and ship/ground status are data. My test calls `fleet::flip_to_ship_forms/_ground_forms`; the combat/landing call sites are tested in `combat::space_routes_tests` and `invasion::ground_routes`, not here | `the_eidolon_is_a_ship_in_the_active_space_area_and_flips_back` |
| `naazagent` | the final-pass `TURN_PASSED` route is the coordinator's (BF-F-N); no change in my file and no test of that emit here |  |
| `naaz_voltron`, `naazbt` | still not started: grep finds no breakthrough or Eidolon Maximum code outside `fleet.rs` (`other_form` is `None` for it). Same blockers as Hook request 4 | |

New decision sites in `naaz.rs`: none (`offer_supercharge` is the former `space_combat_round_started` ask, now also reached from the ground hook).
