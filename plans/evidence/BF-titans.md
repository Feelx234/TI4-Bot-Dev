# BF-titans: abilities, units and unit-upgrade technologies (leaders: `BF-titans-leaders.md`)

Files: `crates/ti4-engine/src/factions/titans.rs` (module claims, hooks, 31 tests). Shared hunks:
`factions/hooks_ground.rs` (new `GroundHooks::forced_combat_planets`), `invasion.rs` (`add_forced_combats`,
called from `finish_committing` and the settle path), `game.rs` (Scanlink exploration now carries the timing
handle). Permission class P1; no folders, no external state.

## Items

| Kind | Id | Status | Tests |
|---|---|---|---|
| ability | `terragenesis` | done | `terragenesis_*` (6), real routes `terragenesis_fires_from_a_real_exploration` (`explore_with` + resolver), `terragenesis_follows_a_scanlink_exploration_in_a_real_tactical_action` (Game) |
| ability | `awaken` | done | `awaken_*`, `the_mech_alternative_*`, `a_planet_with_two_pds_takes_no_third`, real route `awaken_in_a_real_tactical_action_forces_the_new_pds_into_the_ground_combat` |
| ability | `coalescence` | done | `awaken_beside_another_players_units_marks_the_planet_for_combat`; Game routes: ground (Awaken PDS, nothing committed, fights), space (`ouranos_in_a_real_tactical_action_joins_the_space_combat`); negative `without_coalescence_a_planet_with_no_rival_forces_starts_no_combat` |
| unit | `titans_flagship` | done | `ouranos_*` (3) and the Game route above |
| unit | `titans_pds2` / tech `ht2` | done, with a caveat (below) | `hel_titan_ii_prints_the_card_through_the_data`, `..._defends_a_planet_as_a_ground_force`, `..._cannot_be_transported`, `..._gives_its_planet_production_1`, `..._fires_space_cannon_at_its_own_system_and_the_next` |
| unit | `titans_cruiser`, `titans_cruiser2` / tech `se2` | done (data-driven, verified) | `saturn_engine_prints_both_cards_through_the_data`, `saturn_engine_ii_replaces_the_cruiser_on_research_build_and_move` (research, board swap, `buildable_for`, move 3) |
| unit | `titans_pds` (Hel-Titan I) | done (content flags it a ground force; completion pass) | `hel_titan_i_prints_the_card_and_fights_on_its_planet` |
| unit | `titans_mech` (Hecatoncheires) | done (Awaken and Construction) | `hecatoncheires_may_stand_in_for_the_awakened_pds`, Construction tests via `with_test_hooks` (Shared fixes) |
| neutrality | all | done | `a_game_without_the_titans_is_offered_nothing_and_writes_no_marks`; every ability gates on `faction == titans`, the `titans:` mark namespace is only written by Titans effects |

## Card text

- Terragenesis: "After you explore a planet that does not have a sleeper token: You may place or move 1 sleeper token onto that planet."
- Awaken: "After you activate a system that contains 1 or more of your sleeper tokens: You may replace each of those tokens with 1 PDS from your reinforcements."
- Coalescence: "If your flagship or your AWAKEN faction ability places your units into the same space area or onto the same planet as another player's units, your units must participate in combat during 'Space Combat' or 'Ground Combat' steps."
- Ouranos: "DEPLOY: After you activate a system that contains 1 or more of your PDS, you may replace 1 of those PDS with this unit."
- Hecatoncheires: "DEPLOY: When you would place a PDS on a planet, you may place 1 mech and 1 infantry on that planet instead."
- Hel-Titan I/II: "This unit is treated as both a structure and a ground force. It cannot be transported." II adds "You may use this unit's SPACE CANNON against ships that are adjacent to this unit's systems." (II: Combat 6, PLANETARY SHIELD, SPACE CANNON 5, SUSTAIN DAMAGE, PRODUCTION 1.)
- Saturn Engine I: Cost 2, Combat 7, Move 2, Capacity 1. II (se2): Cost 2, Combat 6, Move 3, Capacity 2, SUSTAIN DAMAGE.

## Routes and rule decisions

- **Sleeper tokens**: public, in `GameState::faction_marks`: key `titans:sleeper:<planet>`, value `<owner>|<system>`; one per planet, 5 per owner (`place_sleeper`, `remove_sleeper`, `has_sleeper`, `sleeper_planets`; `pub(crate)` for `titans_leaders.rs`). No model field added; the map is empty and unserialised without Titans.
- **Terragenesis**: timing ability on `PLANET_EXPLORED` (after). Options: place (while fewer than 5 are out) and move from each planet holding one; a single option is not asked again after the ability is chosen. Fires on every exploration that carries a timing handle: invasion captures (the main route) and, new in this package, Scanlink Drone Network (the Titans' starting tech; `game.rs` passed `timing: None`, so no window opened). Routes still without a handle are listed in Hook requests 3.
- **Awaken**: timing ability on `SYSTEM_ACTIVATED` (after), per-token decision (PDS / mech+infantry / leave asleep), all questions before the first placement. PDS id resolved as production does (upgrade, faction unit, generic), so Hel-Titan II once `ht2` is held. Gated on the 2-PDS planet cap, the box (6 PDS, counting tokens decided earlier in the same activation), Demilitarized Zone, space stations and `effect_placement_forbidden`. After Awaken the new PDS make Ouranos eligible in the same window (tests decline it explicitly).
- **Coalescence, ground**: Awaken records `titans:coalescence:<planet>` = `<activation_seq>|<system>` when it puts units onto a planet that holds another player's units. `GroundHooks::forced_combat_planets` returns those planets (same activation, Titans invader, Titans units still there); the invasion adds them to the planets it fights on and takes control of (`add_forced_combats`, after the commitment event and space cannon defense, which concern only committed planets). Ground combat itself still starts only where a rival has ground forces.
- **Coalescence, space**: needs no code. The active player's ships in the active system always take part in its space combat, and Ouranos is placed in that space area; the Game route proves it rolls.
- **Ouranos**: removes one PDS (a damaged one first) from the chosen planet and puts the flagship in the space area; offered only while the flagship is not on the board or captured. The fleet pool and capacity are not checked at placement (37.3 / 16.3 enforce at end of turn, as for other DEPLOYs).
- **Hel-Titan II**: everything is data plus shared code already: `is_ground_force` (content crate), `consumes_capacity` false for structures (so `transit::loadable` never offers it), `production::capacity` = 1 from the `productionValue`, adjacent SPACE CANNON via `combat::reaches_adjacent` (text-derived), `ground_force_owners` for combat, sustain.

## Open rules questions

1. The invasion (and with it the Ground Combat step) opens only when the active player still has ships in the system (`combatants`, `game.rs` aftermath). Awaken beside enemy ground forces therefore forces combat only when the Titans also have a ship there. Left as is (rule 49 reading); Coalescence's text does not say.
2. "You may replace each of those tokens": implemented as a per-token choice (including leaving a token). The other reading is all-or-nothing.
3. Awaken replacing a token on a planet the Titans do not control: the PDS is placed and, if no rival ground force is there, the invasion establishes control through it like a committed planet.

## Hook requests (historical; all closed by "Shared fixes" and "Completion pass" below, except the Terragenesis windows still without a handle)

1. **`titans_pds` (Hel-Titan I) is not a ground force.** `crates/ti4-content/src/units.rs` `UnitType::is_ground_force` reads `matches!(base_type, "infantry" | "mech") || id == "titans_pds2"`; the corpus marks both Hel-Titans `isGroundForce`. Fix: `|| matches!(self.id(), "titans_pds" | "titans_pds2")` (or read `record.flag("isGroundForce")`), plus the matching assertion in the content test `the_titans_pds_is_a_ground_force_despite_being_a_structure`. Until then Hel-Titan I never defends, fights or is forced into combat, so it is not claimed; tests use `ht2`.
2. **Hecatoncheires and Construction.** `strategy_cards.rs::place_structure` pushes the literal `Unit::new(UnitTypeId::new(kind), ..)` with `kind` `pds`/`spacedock`: (a) it ignores the faction unit and upgrades, so a Titans PDS from Construction (primary, secondary, TE variants, relics via `relics.rs`) is the generic `pds`, never Hel-Titan I/II (the same gap gives every faction PDS I plastic after PDS II; `apply_unit_upgrades` only corrects on research); (b) it is the second "when you would place a PDS" site, so Hecatoncheires cannot be offered there. Needed: resolve the id as `action_cards::placed_unit_id` does (make it `pub(crate)`), and after the player picks a PDS spot call a new economy hook (suggested: `EconomyHooks::structure_placement_alternative(&GameState, &ContentStore, SourceSet, &PlayerId, &PlanetId, base_type) -> Vec<ChoiceOption>` plus a performing hook taking a `TimingContext`) so Titans can offer "1 mech and 1 infantry instead". `titans_mech` is claimed when that exists; the Awaken half is ready (`Awakening::MechAndInfantry`).
3. **Terragenesis windows.** `exploration::explore_with` opens `PLANET_EXPLORED` only with a timing handle. Still `timing: None` and able to give the Titans an exploration: `faction_abilities.rs:736` (explore after a control-gaining faction effect), `legendary.rs:537` (explore after a legendary/attachment control gain), `relics.rs:668` (`exploration::explore`, Crown of Emphidia), `naaz.rs:223` (Garv and Gunn exploring a Titans planet). `action_cards.rs:3796` is a frontier draw (no planet), irrelevant. Each needs the same handle `game.rs` Scanlink now passes.

## Decision sites to register (`tests/decision_delivery_inventory.rs`)

`titans.rs` function `ask`: `Choice` x1 and `AskObserved` x1 (one helper; callers: Terragenesis `terragenesis_token`, Awaken `awaken_replace`, Ouranos `ouranos_pds`). The registry test currently fails naming exactly these two rows.

## Commands and results (CARGO_PROFILE_TEST_DEBUG=0, CARGO_PROFILE_TEST_INCREMENTAL=false, -j4)

- `cargo test -p ti4-engine --lib -j4 factions::titans`: 43 passed (31 here, 12 in `titans_leaders.rs`), 0 failed.
- `cargo test -p ti4-engine -j4 --no-fail-fast -- --test-threads=16`: lib 2175 passed / 0 failed / 1 ignored; `content_ids_resolve` 1 passed; doc-tests 5 passed; `decision_delivery_inventory` 3 passed / 1 failed (`every_producer_and_delivery_site_matches_the_reviewed_registry`: unreviewed sites `titans.rs` `ask` Choice and AskObserved, expected, see above).
- `cargo clippy -p ti4-engine --all-targets -j4`: no warning in `titans.rs`, `hooks_ground.rs` or my invasion/game hunks.
- `rustfmt --edition 2024` run on `titans.rs`; `--check` clean on `hooks_ground.rs`, `invasion.rs`, `game.rs`.
- Ledger (`print_faction_ledger`): `titans 10/16 implemented`; missing: Unit `titans_mech`, Unit `titans_pds` (this file), Promissory `terraform`, Leaders `titansagent`, `titanshero`, Breakthrough `titansbt` (leaders agent).
- Not run: workspace suite, ti4-sim, training. The Scanlink handle changes event numbering in any game where a Scanlink exploration happens (a `PLANET_EXPLORED` window now opens there, as it already does after a landing).

## Shared fixes (agent D)

- **Construction unit id** (`strategy_cards.rs`): `structure_options` and `place_structure` resolve the placed unit through `action_cards::placed_unit_id` (now `pub(crate)`): faction unit, upgrade, `unit_form_override`. Titans place `titans_pds` / `titans_pds2` (after `ht2`); factions with no faction PDS/space dock and no upgrade still place generic `pds`/`spacedock` (tests: `construction_places_the_factions_own_pds_and_its_upgrade`, `a_faction_without_its_own_pds_still_places_the_generic_one`). Side effect: any faction holding PDS II / Space Dock II now places the upgraded id (previously generic). Pre-existing, untouched: the Minister of Industry check `kind.contains("space_dock")` never matches kind `spacedock`.
- **Hecatoncheires hook** (`factions/hooks_economy.rs`): `EconomyHooks::pds_placement_alternative` (offer: pure, returns options, ids must not be `pds`) and `pds_placement_alternative_performed` (atomic perform by option id); dispatch `pds_placement_alternatives` / `perform_pds_placement_alternative`. `place_structure` consults them after a PDS spot is picked and asks "place a PDS or an alternative" (options: `pds` plus alternatives) before any mutation; only when alternatives exist (no new question otherwise). Only PDS site owned: Construction (and relics via `place_structure`); `action_cards.rs` has no PDS placement. No TimingContext variant: callers have none. Tests via `with_test_hooks`: taken instead / PDS chosen / mech box empty / non-Titans not asked.
- **Terragenesis windows**: `relics.rs` Crown: new `crown_of_emphidia_explore_with(state, &mut Resolving, galaxy, player)`; old fn delegates with `timing: None` (and now uses the real table for the exploration, was a throwaway table). `game.rs close_tactical` still calls the old fn: coordinator must switch it to build a Resolving like the Scanlink site. NOT fixable by me: `faction_abilities.rs:~734` (Peace Accords effect), `legendary.rs:~535` (Maxis effect), `naaz.rs:~218` (`explore_planet`): all run inside a `TimingContext` (an effect of an event the resolver is already emitting); the `Resolver` is mutably borrowed, so no `TimingHandle` exists. Needs a staged-event route (like `hooks_ground::stage_*` + announce) for `PLANET_EXPLORED`.
- Decision sites: `strategy_cards.rs place_structure` Choice 1->2; `relics.rs crown_of_emphidia_explore` Choice/AskObserved moved to `crown_of_emphidia_explore_with` (each 1).
- Results: lib 2182 passed/0 failed/1 ignored; `decision_delivery_inventory` 2 passed / 2 failed (the unreviewed sites above only).

## Completion pass (2026-10-05): remaining assets

Claimed now: `titanshero`, `terraform`, `titansagent`, `titansbt` (LEADERS/PROMISSORY/BREAKTHROUGHS in `titans_leaders.rs`), `titans_pds` (units in `titans.rs`). Only `titans_mech` stays blocked (Construction hook, other agent).

| Item | Change | Tests |
|---|---|---|
| Geoform | `invasion.rs::space_cannon_defense` rolls `planets::attachment_cannons` for owners other than the invader | `invasion::tests::a_geoform_attachment_fires_in_space_cannon_defense` |
| Terraform | `exploration::choose_deck` reads `planets::traits_now`; claim id `terraform` matches the corpus alias | `a_terraformed_planet_explores_into_any_deck` |
| Tellurian | new events `GROUND_HITS_TO_ASSIGN` (`invasion.rs`: ground combat, bombardment, space cannon defense, Harrow) and `ANTI_FIGHTER_BARRAGE_HITS` (`combat.rs::apply_barrage`), payload `system, player (victim), hits` plus `planet, cause` / `gunner`; `combat::open_hits_window` spends only cancellations granted during the window; `spend_cancellations` is `pub(crate)`; both registered in `titans_leaders::timing_abilities` | `tellurian_cancels_a_space_cannon_defense_hit_on_ground_forces` (+ existing space routes) |
| Hel-Titan I | content now flags it a ground force; fights, not loadable | `hel_titan_i_prints_the_card_and_fights_on_its_planet` |
| Coalescence gate | `game.rs` Fighting-stage `holds` also true when `forced_combat_planets` is non-empty, so Awaken-placed units fight with no Titans ship in the system | `coalescence_forces_the_ground_combat_when_the_titans_have_no_ship_there` |
| Slumberstate | (a) `invasion.rs::add_forced_combats` -> `coexist_instead`: asks fight/coexist (fight first) when the holder has `titansbt`, committed nothing else, another player controls the planet and has a ground force; `coexistence::begin`. (b) `titans_leaders::slumberstate_draw_bonus` via `EconomyHooks::action_card_draw_bonus`, status phase only, one per coexisting partner for the holder and one per holder for each partner. (c) `sleeper_allowance`: at the Titans' `TURN_BEGAN` (the "at any time" substitute) pick a planet another player controls, that player is asked to allow, token placed | `slumberstate_lets_coalescing_units_coexist_instead_of_fighting` (Game route, with and without the breakthrough), `slumberstate_adds_a_status_draw_per_coexisting_partner`, `slumberstate_lets_another_player_allow_a_sleeper_on_their_planet` |

Rule decisions: Tellurian cancellation window is per batch of hits (a round's, a bombardment unit's, one defense). The status bonus hook is generic, so it is gated on `Phase::Status`; any other draw during the status phase by a coexisting pair would also get it. "Commit no other units" = no planet committed before the forced ones are added. Sleeper allowance is an optional prompt every Titans turn while a legal planet exists. Ground-combat/bombardment/Harrow windows share the helper proven by the space cannon defense route; no dedicated Game-level route test for them.

New decision sites to register: `invasion.rs` `coexist_instead` (`Choice` x1, `AskObserved` x1); `titans_leaders.rs` `sleeper_allowance` (`Choice` x1, `AskObserved` x1; one closure asks both the Titans and the holder).

Results: `cargo test -p ti4-engine --lib -j4 titans`: 54 passed, 0 failed. Full `cargo test -p ti4-engine -j4 --no-fail-fast -- --test-threads=16`: lib 2188 passed / 0 failed / 1 ignored; content_ids_resolve 1 passed; doc 5 passed; `decision_delivery_inventory` 3 passed / 1 failed (`every_producer_and_delivery_site_matches_the_reviewed_registry`: exactly the four unreviewed rows above). Ledger: `titans 15/16 implemented`, missing `Unit titans_mech`. rustfmt --edition 2024 run on `titans.rs`, `titans_leaders.rs`; my hunks in `invasion.rs`/`combat.rs`/`exploration.rs`/`game.rs` hand-formatted (those files have other unformatted code I left alone).

## Review fixes (2026-10-05)

| # | Finding | Change | Test |
|---|---|---|---|
| 1 | Construction pushed Saar's space-only `saar_spacedock` onto a planet | `strategy_cards.rs::construction_unit`: resolves via `action_cards::placed_unit_id`, falls back to the generic id when the resolved unit `is_space_only_structure()`; used by `structure_options` and `place_structure` | `saar_construction_places_the_generic_dock_on_a_planet_not_the_floating_factory` |
| 2 | `ANTI_FIGHTER_BARRAGE_HITS` opened with no target | `combat.rs::apply_barrage`: window only when the target has fighters or a Waylay is in play; otherwise the hits pass through unchanged (Raid Formation's excess hook still sees them) | existing barrage tests (suite green) |
| 3 | Ordering change after `HITS_TO_ASSIGN` | see below; `open_hits_window` doc now says which cancellations are spent | `a_cancellation_spares_a_sustaining_ship_before_the_sustain_offer` |
| 4 | Sleeper allowance offered every turn, on any planet | `titans_leaders.rs`: candidates use `awakenings`' predicates (not demilitarized, not a space station); once per round via public mark `titans:allowance_asked:<player>:<round>` (set when offered, whatever the answer); skipped when the box holds neither a PDS nor a mech (`titans::awakening_plastic_available`) | `the_sleeper_allowance_is_offered_once_per_round` |
| 5 | Terraform traits read printed traits | `traits_now` now used by: `action_cards.rs` Archaeological Expedition, `faction_abilities.rs` Peace Accords explore, `faction_techs.rs` explore picker, `legendary.rs` Maxis explore, `factions/naaz.rs::explorable` (+ its test). Left on printed traits (no state at hand, or tests only): `exploration::trait_of`/`traits_of` themselves, and test helpers in `exploration.rs`, `invasion.rs`, `naaz.rs`, `faction_techs.rs:843`, `action_cards.rs` tests | `a_terraformed_planet_explores_into_any_deck` (existing) |
| 6 | `coexist_instead` swallowed the ask error | now `Result<bool, IllegalChoice>`; `add_forced_combats` / `finish_committing` return `Result` and `resolve` propagates; the `settle` site (returns `()`) stores it in `strict_timing_error`, read through new `InvasionWindow::take_settle_error`, checked in `drive` and in `game.rs` Aftermath::Invading after `window.settle` (one new line, not a round_income hunk). `coexistence::begin` fails only before any mutation (missing taker, which this path never has), so a failure leaves state untouched and the units fight | suite green; no dedicated error-injection test |
| 7 | Geoform gun comments | one-line comments at `combat.rs::space_cannon_offense` and `invasion.rs::space_cannon_defense`: Disable / Plasma Scoring do not apply, the attachment is not a PDS | - |
| 8 | Stale docs | `titans.rs` MODULE header, `titans_leaders.rs` LEADERS doc, `BF-titans-leaders.md` delivery-site section and table, `BF-titans.md` item rows and hook-request heading | - |
| 9 | Minister of Industry never fired on Construction | `kind.contains("space_dock")` -> `kind == "spacedock"` (law text: "When the owner of this card places a space dock in a system, their units in that system may use their PRODUCTION abilities.") | `minister_of_industry_produces_on_a_construction_space_dock` |

### Item 3: the ordering change

Space combat (`CombatWindow`): after emitting `HITS_TO_ASSIGN` the window now calls `spend_cancellations` for the whole round-scoped pool (granted earlier this round or in the window), before the sustain offer. Before, a cancellation granted inside the window (Tellurian) was spent only on the next pass, i.e. after the player had answered the sustain offer, so a sustain was burned on a hit that was then cancelled. Shields Holding played earlier in the round was already spent before the emit (unchanged). The test drives the real window with a start-of-combat hit on a lone dreadnought: without a cancellation its owner is offered a sustain; with a grant made before the window, and with Tellurian granting inside the window, the start hit's offer is spared on some seeds (a later round-1 hit can raise a fresh offer, so it is judged over 30 seeds). `open_hits_window` (ground, bombardment, barrage windows) spends only what the window itself granted.

### Known limits and notes

- `slumberstate_draw_bonus` is gated on `Phase::Status` only: any status-phase draw through `action_cards::draw` by a coexisting pair gets the bonus, not just the "draw action cards" step. Known limit.
- Awaken reads "replace each of those tokens" as a per-token optional choice (all-or-nothing is the other reading); unchanged.
- Scanlink Drone Network and Crown of Emphidia explorations now open `PLANET_EXPLORED` in every game (a genuine fix, not Titans-gated).
- `GROUND_HITS_TO_ASSIGN` and `ANTI_FIGHTER_BARRAGE_HITS` are emitted in every game, which shifts later event ids. Grep of `ti4-sim`, `ti4-policy`, `ti4-review`, `ti4-mlp`, `ti4-training`, `ti4-replayer` for those two names and `PLANET_EXPLORED`: no matches; grep for event-id arithmetic (`event_id`, `EventId`, `event_sequence`) in sim/policy/review: no matches. No consumer found that depends on event ids.
- No new decision site (the `coexist_instead` / `sleeper_allowance` rows were already counted); `decision_delivery_inventory` passes (4 passed).

### Results

- `cargo test -p ti4-engine -j2 --no-fail-fast -- --test-threads=16` (CARGO_PROFILE_TEST_DEBUG=0, CARGO_PROFILE_TEST_INCREMENTAL=false), exit 0: lib 2192 passed / 0 failed / 1 ignored; integration binaries 1 + 4 (`decision_delivery_inventory`) + doc 5 passed.
- `print_faction_ledger`: `titans 16/16 implemented`.
- rustfmt `--edition 2024 --check` clean on `invasion.rs`, `game.rs`, `faction_abilities.rs`, `factions/titans.rs`, `factions/titans_leaders.rs`, `strategy_cards.rs`, `faction_techs.rs`, `factions/naaz.rs`. `combat.rs`, `action_cards.rs`, `legendary.rs` are unclean at base (pre-existing hunks elsewhere); my hunks in them were formatted by hand and no diff reported inside them.
