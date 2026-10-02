# BF-muaat: The Embers of Muaat

Branch `wp/base-factions`, uncommitted. Code: `crates/ti4-engine/src/factions/muaat.rs` (only file edited besides this one).
Card text: `crates/ti4-content/content/*.json` at `DEFAULT` (latest printing).

## Items

| Kind | Id | Status | Tests |
|---|---|---|---|
| ability | `star_forge` | done (component action; 2 fighters or 1 destroyer at a war sun; `max_fit` + box checked before the token is spent) | `star_forge_places_two_fighters_at_a_war_sun_for_a_strategy_token`, `star_forge_can_place_a_destroyer_instead`, `star_forge_needs_a_token_a_war_sun_and_a_choice_and_changes_nothing_otherwise` |
| ability | `gashlai_physiology` | blocked, not claimed (hook request 1) | none |
| tech | `pws2` | done (data-driven: war sun unit swaps to `muaat_warsun2`, cost 10, move 3; shield stripping through `invasion::bombardable`) | `prototype_war_sun_ii_upgrades_the_war_sun_and_keeps_its_shield_breaking` |
| tech | `mr` | partial, not claimed: movement half works (`may_enter_supernova` when `mr` is held); PRODUCTION 5 in supernovas blocked (hook request 2) | `magmus_reactor_lets_ships_move_into_a_supernova_and_only_with_the_reactor` |
| unit | `muaat_warsun`, `muaat_warsun2` | done (seating deploys a war sun, verified; stats data-driven) | `muaat_starts_with_a_prototype_war_sun_in_its_home_system`, `prototype_war_sun_ii_...` |
| unit | `muaat_flagship` The Inferno | done (component action, cruiser placed in the flagship's system for a strategy token) | `the_inferno_places_a_cruiser_in_its_system_for_a_strategy_token`, `the_inferno_needs_the_flagship_and_a_token` |
| unit | `muaat_mech` Ember Colossus | done (asked after each Star Forge use for every mech in the forge's system or an adjacent one by the player's adjacency; optional; infantry goes on the mech's planet, or in space) | `ember_colossus_adds_an_infantry_with_the_mech_when_star_forge_is_used`, `ember_colossus_is_optional_and_needs_the_forge_nearby` |
| promissory | `fires` | done (holder's component action; Muaat fleet token returned to reinforcements, holder gains `ws`, unit upgrades applied, note returned) | `fires_of_the_gashlai_gives_the_holder_the_war_sun_and_returns_the_note`, `fires_of_the_gashlai_is_not_offered_without_the_note_a_token_or_a_missing_card` |
| leader | `muaathero` Adjudicator Ba'al | written and tested, NOT claimed, registered only under `cfg!(test)` (Review fixes 1) (timing ability `After SHIP_MOVED`, optional; replacement via `movement::apply_map_edit`, done on a copy and swapped in) | `nova_seed_replaces_the_tile_destroys_other_units_and_purges_the_hero`, `nova_seed_is_not_offered_when_its_conditions_fail_and_declining_changes_nothing` |
| leader | `muaatcommander` Magmus | partial, not claimed: unlock ("produce a war sun", `UNITS_PRODUCED`) done; effect blocked (hook request 3) | `magmus_unlocks_when_a_war_sun_is_produced_and_not_for_other_units` |
| leader | `muaatagent` Umbat | blocked, not claimed (hook request 4) | none |
| breakthrough | `muaatbt` Stellar Genesis | blocked, not claimed (hook request 5) | none |

Regression: `a_game_without_a_muaat_seat_is_unchanged_and_supernovas_still_block` (no `faction|muaat|` component action for
non-Muaat seats, `may_enter_supernova` false, a supernova hub still gives 0 reachable moves, `SHIP_MOVED` and `UNITS_PRODUCED`
windows ask nothing and leave the state equal).

## Hook requests

1. **Gashlai Physiology vs Magmus Reactor.** `MovementHooks::may_enter_supernova` lifts 86.1 for both through and into
   (`MovementRules.supernovae_open`; `can_enter` serves both). Gashlai is "move *through*", Reactor is "move *into*". Request: a
   second flag (`supernovae_pass_only`, or split the hook into `may_pass_through_supernova` / `may_end_in_supernova`) honoured by
   `can_enter` (a step that ends the move) versus `can_pass_through`. Until then the module returns true only for a holder of `mr`,
   so a Muaat player without the reactor cannot use supernovas at all (under-permissive, never illegal). If the coordinator prefers
   "Muaat always passes", changing `may_enter_supernova` to also return true for a Muaat seat is a one-line edit, but then Muaat could
   also end a move in a supernova before researching the reactor.
2. **Magmus Reactor PRODUCTION 5.** "Each supernova that contains 1 or more of your units gains the PRODUCTION 5 ability as if it
   were 1 of your units." `hooks_economy` has no extra-production hook. Request: `EconomyHooks::extra_production:
   fn(&GameState, &ContentStore, SourceSet, &PlayerId, &SystemId) -> i64`, summed into `production::capacity` and treated as a
   producer for `placements` (ships placed in the space area). The supernova tile has no planet, so ships only.
3. **Magmus effect** ("After you spend a token from your strategy pool: you may gain 1 trade good"). No event is emitted when a
   strategy-pool token is spent; the spends are scattered (`faction_abilities` Orbital Drop and Production Biomes, `faction_techs`,
   `entropic_scars`, strategy-card secondaries, Naalu Foresight, `Political Favor`, plus this module's own). Request: a typed
   event `STRATEGY_TOKEN_SPENT` (payload `player`, `source`) emitted through a single helper that every spend site calls
   (for example `PlayerState::spend_token(TokenPool::Strategic)` callers moving to a `supply`/`tokens` function with a resolver).
   The module would then add the optional trade good on that event.
4. **Umbat** ("choose a player: that player may produce up to 2 units that each have a cost of 4 or less in a system that contains
   one of their war suns or their flagship"). `production::produce_by_ability`/`ProductionWindow::for_ability` take only a unit-count
   limit. Request: an optional per-unit maximum cost on `for_ability` (e.g. `max_unit_cost: Option<i64>`) applied in
   `build_options`, so units costing more than 4 are not offered. The rest (agent exhaust, player choice, system choice among war
   sun / flagship systems) is straightforward once that exists. Also decide whether the chosen player's use is "production" for
   `UNITS_PRODUCED` (it reports `source: "ability"`, which also satisfies Magmus's unlock for a Muaat player).
5. **Stellar Genesis.** Needs (a) a typed event when a player gains a breakthrough (no `BREAKTHROUGH_GAINED`), (b) a way to place
   the Avernus planet token on a tile (`planets::place` exists and gives control and a readied card, but there is no move), and
   (c) the "ready the planet card" and Avernus's legendary ability ("ACTION: exhaust to use Star Forge without spending a command
   token"), which this module could add once (a) exists. The token follow-a-war-sun movement needs a `SHIP_MOVED` payload with the
   systems the ship passed through, not only `origin`/`system`.

## Rules questions

- Star Forge "2 fighters": if only one fighter fits (capacity, box, fleet pool) one is placed. Placement respects capacity and the
  fleet pool (`PlacementLimits::Respect` semantics via `max_fit`), as the helpers document for new cards.
- Ember Colossus: "this system or an adjacent system" is read as the system the Star Forge units are placed in, and adjacency is
  the player's own (wormholes, Quantum Entanglement-style links). Each mech may place its own infantry once per Star Forge use. A mech
  in a space area gets the infantry in space (as capacity allows).
- The Inferno: "this system" is the flagship's system; the action is withheld when no cruiser fits there.
- Fires of the Gashlai: the holder's "war sun unit upgrade technology card" is the generic `ws` card; it is not offered when the
  holder already has it (nothing to gain) or when the Muaat fleet pool is empty. The Muaat owner is the first seat playing `muaat`.
- Nova Seed: "destroy all other players' units" is performed by removing them from the board (no `SHIP_DESTROYED` /
  ground-destroyed events, no Spec Ops/Letnev-style destruction reactions). The Muaat player's own ground forces on the old tile's
  planets leave with the planets (returned to reinforcements). Frontier and command tokens stay (card notes). Attachments on the
  purged planets are dropped from `planet_attachments`. Not usable in the Fracture (card notes); home systems of any faction and
  Mecatol Rex (`18`) are excluded. Anything later in the same step as the hero reads the old tile on the game's map; the recorded
  edit is replayed at the end of the step (the existing Ghost hero pattern), and `active_system` is moved to the Nova Seed
  tile immediately.
- Magmus unlock counts any production that reports a war sun (`UNITS_PRODUCED`), including ability production.

## Decision sites to register

`crates/ti4-engine/src/factions/muaat.rs`, function `ask` (the single helper every Muaat question goes through: Star Forge
choice, Ember Colossus placement): `Choice` 1, `AskObserved` 1. No other function in the file builds or asks a `Choice`. (The Nova
Seed "use it?" is the resolver's own optional-ability question.)

## Commands and exact results

- `cargo test -p ti4-engine --lib -- factions::muaat` -> `16 passed; 0 failed`.
- `cargo test -p ti4-engine --lib -- factions::` -> `273 passed; 0 failed; 1 ignored`.
- `cargo clippy -p ti4-engine --all-targets`: nothing reported in `muaat.rs`.
- `rustfmt --edition 2024 crates/ti4-engine/src/factions/muaat.rs`: run.
- `cargo test -p ti4-engine -q --no-fail-fast` -> lib `1782 passed; 0 failed; 1 ignored`; integration binaries ok except
  `decision_delivery_inventory`: 3 passed, 1 failed (`every_producer_and_delivery_site_matches_the_reviewed_registry`: unregistered
  decision sites of this and other factions; registry not edited by me).
- Ledger (`print_faction_ledger`): `muaat 8/13 implemented`; gaps: ability `gashlai_physiology`, tech `mr`, leaders `muaatagent`,
  `muaatcommander`, breakthrough `muaatbt`.

## Review fixes

1. **`muaathero` unclaimed (blocker).** Ships moving without cargo emit no typed `SHIP_MOVED` in `game.rs`, so the ability is not
   reachable live; fixing that changes in-scope play and awaits operator approval. The ability is registered only under
   `cfg!(test)` (as `naalu.rs` does); code and tests kept. Also blocked on a "movement finished" window: a per-ship `SHIP_MOVED`
   fires Nova Seed mid-movement and the swapped-in supernova locks the rest of the fleet out (S2). The purge must also go
   through the shared destroy helpers, mark planets purged (`SystemState::purge_planet`), and clear planet-keyed laws and ingress
   tokens (S3); the current direct removal does none of that.
2. **Nit 5.** `may_enter_supernova` now requires a Muaat seat holding `mr` (own technologies or `assimilated_technologies`), so a
   non-Muaat holder gets nothing.
3. **Nit 6.** Hooks without a sources argument (`component_actions`, `commander_unlocked`) assume `DEFAULT`; `perform_component`
   and abilities use the context's sources. Documented, as in `winnu.rs`.
4. **Nits 7/8 (rules decisions, recorded).** Destroying other players' units by removal (no destruction events) and dropping
   attachments with the purged planets are decisions pending the shared-helper route in item 1; Star Forge placing one fighter
   when only one fits is a decision.
