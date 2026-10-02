# BF-00b-economy: production, placement, payment and action-card routes

Branch `wp/base-factions`, uncommitted (wave B; the coordinator commits). No faction card is
implemented here. Every hook has a neutral default, so a game with empty faction modules plays
identically. New events are emitted unconditionally (see Review fixes S6: event ids and timing-log
lines shift once).

Files touched: `production.rs`, `action_cards.rs`, `laws.rs`, `supply.rs`, `payment.rs`,
`tokens.rs`, `factions/hooks_economy.rs`, this file.

## Hooks (`factions::hooks_economy::EconomyHooks`, set per module via `economy: EconomyHooks { .. , ..EconomyHooks::NONE }`)

| Field | Signature | Card | Call point / timing |
|---|---|---|---|
| `planet_spend_value` | `fn(&GameState,&ContentStore,&PlayerId,&PlanetId,Spend,i64)->i64` (fold, registration order) | Winnu `htp` | `production::capacity` (resources a dock reads) and `production::payment_faces` (every face, incl. Archon's Gift / Freelancers alternate). Live whenever the module says so; module owns "which planet, which use". |
| `trade_good_worth` | `fn(&GameState,&PlayerId,i64)->i64` (fold) | Mentak `mc` | `production::trade_good_worth`: `available`, `payment_options`, `apply_payment_option`, and now `payment::plans` (votes, objective costs). |
| `cannot_produce` | `fn(&GameState,&ContentStore,&PlayerId,unit_base:&str,producer_base:&str)->bool` (any) | Arborec `mitosis` ("space docks cannot produce infantry") | `production::placements`: barred producers are dropped before spots are computed. |
| `action_card_draw_bonus` | `fn(&GameState,&ContentStore,&PlayerId,requested:usize)->usize` (sum) | Yssaril `scheming` (draw 1 additional) | `action_cards::draw`, before the first card leaves the deck; not applied when `count == 0`. |
| `action_cards_drawn` | `fn(&mut GameState,&ContentStore,&mut Table,&PlayerId,&[ActionCardId])->Result<(),IllegalChoice>` | Yssaril `scheming` ("then choose and discard 1") | `action_cards::draw`, after drawing, before the hand limit. Only when `count > 0`. |
| `action_card_limit` | `fn(&GameState,&ContentStore,&PlayerId,usize)->usize` (fold) | Yssaril `crafty` | `action_cards::enforce_hand_limit`, applied after `laws::action_card_limit` (Sanctions), so `usize::MAX` beats the law. |
| `action_cards_forbidden` | `fn(&GameState,&PlayerId)->bool` (any) | Yssaril `tp` | `laws::action_cards_forbidden` (so `action_cards::is_playable`, the component-action gate). Also `pub(crate) hooks_economy::action_cards_forbidden` for the reaction gate (Request 2). |

Test-only: `hooks_economy::with_test_hooks(extra, || ..)` adds hooks after the real modules on the
current thread, so call-site tests run while modules are empty.

## Events (new names; payload keys)

| Event | Emitted by | Payload |
|---|---|---|
| `UNITS_PRODUCED` | `production::ProductionWindow` when a use of PRODUCTION ends having produced 1+ unit (after `breakthroughs::on_production_finished` and Prophecy of Ixth) | `player`, `system`, `source` ("production" / "ability"), `count`, `units` = array of `{unit_type, place}` (`place` = `"space"` or planet id) |
| `ACTION_CARDS_DRAWN` | `action_cards::draw_announced` | `player`, `count` (drawn incl. extra), `source` |
| `TRADE_GOODS_GAINED` | `supply::gain_trade_goods_announced` / `gain_trade_goods_via` | `player`, `amount` (gained), `source` |

All three go through `hooks_economy::emit` (or the `_via` variant), unconditionally.

## Items

| # | Item | Status | What was added | Tests |
|---|---|---|---|---|
| 1 | After-production event (Yin `yso`) | done | `UNITS_PRODUCED` (above). `PRODUCTION_USED` (game.rs) fires *before* the first choice with `player`,`system` only, so a new after-the-fact event was needed. Fires from the tactical-action path (game.rs `Aftermath::Producing` passes the game `ctx` with timing) and from `produce_by_ability`. Not from `production::resolve` (Warfare secondary / Construction): it builds `Resolving { timing: None }` (Request 6). | `units_produced_event_reports_what_was_made_and_where`, `units_produced_is_not_emitted_when_nothing_listens` |
| 2 | Ability-driven production API | done (reuse is clean) | `ProductionWindow::for_ability(state,content,sources,player,system,limit:Option<i64>)` and `production::produce_by_ability(state, ctx:&mut Resolving, galaxy, player, system, limit)->Result<ProductionReport,IllegalChoice>`. Same window: offers, affordability, payment faces/credit, plastic cap, fleet limits, blockade (68.10), one bill per use. `limit=Some(n)` replaces the system PRODUCTION total (flagship "up to 5", commander "1"); `None` reads capacity. Ground forces may go on any controlled planet in the system (see Rules questions). Does NOT fire `PRODUCTION_USED`, Sarween/AIDA/War Machine/Harrugh: whether ability production "uses PRODUCTION" is a rules question. Muaat `star_forge` is placement, not production: use item 3. Hero "any number of units in any number of systems": call once per system with a large `limit`; the module picks systems (ground-force systems). | `ability_production_has_its_own_limit_and_places_ground_forces_on_controlled_planets`, `units_produced_event_reports_what_was_made_and_where` |
| 3 | Placement from reinforcements | done | `place_units` as is: places exactly where told, returns nothing, supply-capped, no validation, ignores unit upgrades (places the base/faction unit id, never `*_infantry2`). New in `action_cards.rs`: `PlacementTarget {ControlledPlanet, ShipSpace, ControlledPlanetOrShipSpace}`; `placement_spots(state,content,sources,player,target,within:Option<&SystemId>)->Vec<(SystemId,Option<PlanetId>)>`; `place_units_counted(context,player,system,planet,base_type,count)->usize`; `place_units_choosing(context,player,base_type,count,target,within,optional,ability)->Result<usize,IllegalChoice>` (player picks the spot, one spot is not asked, `optional` adds decline, nothing asked/changed if no spot or box empty, all units to one spot); `replace_unit(context,player,system,planet,remove_base,place_base)->bool` (atomic: needs the unit and the plastic). These resolve the unit through the player's upgrades (as production does). Consumers: Arborec `mitosis` (`ControlledPlanet`, count 1), Yin `yso` (`ControlledPlanetOrShipSpace`, 2, optional), Muaat `star_forge` (`ShipSpace`, `within` = a war-sun system, "2 fighters or 1 destroyer" is two calls/one choice by the module), Sardakk agent (`ControlledPlanet` within active system, 2 infantry), Saar `ffac2`, mech replacement via `replace_unit`. `place_units` is unchanged. | `placement_spots_list_controlled_planets_and_ship_spaces`, `the_player_chooses_where_reinforcements_go_and_decline_changes_nothing`, `a_single_spot_is_not_a_question_and_no_spot_changes_nothing`, `replacing_a_unit_needs_the_unit_and_the_plastic_and_is_atomic` |
| 4 | Production cost / payment adjustment (Winnu `htp`, `winnuagent`) | done (hook) + design note | `planet_spend_value` hook (above). `winnuagent` needs **no hook**: "reduce the combined cost of the produced units by 2" is a timing ability on `PRODUCTION_USED` (any player's use) that adds 2 to `state.production_discount_remaining` (read by `ProductionWindow::refresh` right after the event, same path as Sarween/AIDA). `htp` needs module state for "which planet is swapped during this use" (Request 5). | `a_planet_spend_value_hook_reaches_capacity_and_payment_faces` |
| 5 | Trade-good spend value (Mentak `mc`) | done | `trade_good_worth` hook; also made `payment::plans` (votes, objective costs) honour it (goods needed = ceil(shortfall / worth)); `Plan::worth_for(state,content,sources,player,kind)` is the worth-aware twin of `Plan::worth`. | `a_trade_good_worth_hook_reaches_available_and_payment`, `a_trade_good_worth_hook_changes_what_goods_cost_in_a_plan` |
| 6 | Action-card draw event + hooks (Yssaril `scheming`) | done | Central function is `action_cards::draw` (callers: `exploration.rs` x2, `strategy_cards.rs` x2, `legendary.rs`). Hooks sit inside it, so every `draw` caller gets Scheming. `draw_announced(state, ctx:&mut Resolving, player, count, source)` emits `ACTION_CARDS_DRAWN`. Not covered (direct deck pops, no table): `status.rs` (status-phase draw), `agenda_effects.rs` x2 (Politics Rider, Unconventional Measures), `action_cards.rs` Unconventional arm. (Request 4.) | `draw_hooks_add_a_card_and_then_let_a_module_discard`, `draw_announced_emits_the_event_with_its_payload` |
| 7 | Hand-limit hook (Yssaril `crafty`) | done | `action_card_limit` hook in `enforce_hand_limit` (after Sanctions). Only `action_cards::enforce_hand_limit` enforces the limit. | `a_hand_limit_hook_runs_after_the_law_cap` |
| 8 | Play legality (Yssaril `tp`) | partial | Hook `action_cards_forbidden`, wired into `laws::action_cards_forbidden` (read by `is_playable`, the component-action gate). The reaction-window gate `reactions::playable_now` does not read `laws::action_cards_forbidden` today (Political Censure is not enforced there either); it needs one call to `crate::factions::hooks_economy::action_cards_forbidden(state, player)` (Request 2). | `a_play_forbidden_hook_closes_the_component_action_gate` |
| 9 | Place a command token from reinforcements (Arborec `stymie`) | done | `tokens::can_place_command_token(state, owner, system)` and `tokens::place_command_token_from_reinforcements(state, owner, system)->bool`. `owner` is the player whose token it is (Stymie: the player who moved, not the note holder). Atomic; false if no reinforcements (`tokens_in_reinforcements`) or already a token there. The non-home-system rule stays with the caller (`ti4_content::galaxy::is_home_system`). | `a_command_token_comes_from_the_owners_reinforcements`, `no_reinforcements_means_no_token_and_no_change` |
| 10 | Trade-goods-gained event (Mentak `pillage`, agent) | partial (route built, call sites not converted) | Survey: 31 `trade_goods +=` sites outside tests in 15 files (action_cards 7, agenda_effects 4, exploration 3, faction_abilities 2, faction_techs 2, game 2, legendary 2, relics 2, combat, draft, laws, promissory, strategy_cards, technology, transactions 1 each), plus `=`/saturating variants; most hold only `&mut GameState` or a `TimingContext` without a resolver. Narrowest route: `supply::gain_trade_goods(state,player,n)->i32` (plain, greppable), `gain_trade_goods_announced(state, ctx:&mut Resolving, player, n, source)->i32` and `gain_trade_goods_via(context, resolver, player, n, source)->Result<i32,TimingError>` (the `reactions::announce` shape) emitting `TRADE_GOODS_GAINED`. Converting a site = replacing its `+=` with a call; the amount gained is unchanged. The sites worth converting first for Pillage: Trade primary (`strategy_cards.rs`), commodity conversion / transactions (`transactions.rs:476` is already covered by `TRANSACTION_RESOLVED`, which Pillage also names), `game.rs` income. All outside this package's files. | `gaining_trade_goods_through_the_helper_emits_the_event_with_its_payload`, `a_zero_gain_or_a_silent_game_emits_nothing` |

## Requests (files outside this package)

1. `crates/ti4-engine/tests/decision_delivery_inventory.rs` (red until done, only failure in
   `cargo test -p ti4-engine --no-fail-fast`): add to `PRODUCERS`
   `Producer { module: "action_cards.rs", function: "place_units_choosing", count: 1, delivery: Delivery::ObservedHere }`
   and to `OBSERVED_ASKS` `("action_cards.rs", "place_units_choosing", 1)`. The new function builds
   one `Choice` and asks it with `TimingContext::ask_seeing`.
2. `reactions.rs::playable_now`: filter with `!crate::factions::hooks_economy::action_cards_forbidden(state, player)`
   (return empty when true) so Transparasteel Plating also closes reaction windows. Do not use
   `laws::action_cards_forbidden` there unless Political Censure should start applying to reactions
   (behaviour change).
3. `preview.rs:454`: `plan.worth(..)` -> `plan.worth_for(state, content, sources, player, kind)` so a
   Mirror Computing holder's preview shows the doubled goods value.
4. Draw sites that bypass `action_cards::draw` and so miss Scheming and the event: `status.rs:99`
   (status-phase draw; also Neural Motivator), `agenda_effects.rs` (Politics Rider, Unconventional
   Measures), `action_cards.rs` Unconventional arm (mine, left alone: no table in reach). Sites that
   call `draw` and hold a `Resolving` (`exploration.rs`, `strategy_cards.rs`) may switch to
   `draw_announced` for the event.
5. `htp` state: a `GameState` field naming the swapped planet and the `production_seq` it is live for
   (e.g. `value_swap: BTreeMap<PlayerId, (PlanetId, u32)>`), set by a timing ability on
   `PRODUCTION_USED`. The `planet_spend_value` hook then returns the other value while
   `production_seq` matches. `ProductionWindow::refresh` re-derives capacity after the event, so a swap
   chosen in that window is seen.
6. `production::resolve` (Warfare secondary, Construction, hero) builds `Resolving { timing: None }`;
   `UNITS_PRODUCED` and any production reaction cannot fire there. A caller with a timing handle
   should use `ProductionWindow` + its own `Resolving`.
7. `reactions::EMITTED_EVENTS`: add `UNITS_PRODUCED`, `ACTION_CARDS_DRAWN`, `TRADE_GOODS_GAINED`
   only if a registry window should map to them; `timing_abilities` do not need it.
8. `place_units` (existing callers) ignores unit upgrades; the new helpers do not. Decide whether to
   make `place_units` upgrade-aware (behaviour change for owners of upgrades; ti4-sim would notice).

## Rules questions

- Does ability production (Arborec flagship/commander/hero) count as "a use of PRODUCTION"
  (Sarween Tools, AI Development Algorithm, War Machine, Harrugh Gefhara, Yin Spinner "after you
  produce units")? Implemented: `UNITS_PRODUCED` fires (it says "produce"), `PRODUCTION_USED` and
  the tactical-action extras do not.
- Where may ability-produced ground forces go? Implemented: any planet in the system the player
  controls, structures still capped by 79.2. The printed PRODUCTION rule places ground forces on a
  planet containing a producer.
- Combined PRODUCTION limit with `cannot_produce` (Mitosis): the barred dock's value still counts
  toward the limit (it cannot be split per producer in the combined-capacity model); the spot rule is
  exact, the budget is an over-approximation.
- Hegemonic Trade Policy swap: whether the swap also applies to a planet's value when paying for a
  non-production bill in the same use (implemented as "every payment face while live").

## Commands and exact results

- `cargo test -p ti4-engine --lib production::` : 48 passed, 0 failed
- `cargo test -p ti4-engine --lib action_cards::` : 112 passed, 0 failed
- `cargo test -p ti4-engine --lib laws::` : 25 passed, 0 failed
- `cargo test -p ti4-engine --lib supply::` : 9 passed, 0 failed
- `cargo test -p ti4-engine --lib payment::` : 10 passed, 0 failed
- `cargo test -p ti4-engine --lib tokens::` : 20 passed, 0 failed
- `cargo test -p ti4-engine --lib factions::` : 11 passed, 0 failed, 1 ignored (includes 3 in `factions::hooks_economy`)
- `cargo test -p ti4-engine -q --no-fail-fast` : lib 1444 passed, 0 failed, 1 ignored; integration
  binaries ok except `decision_delivery_inventory`: 3 passed, 1 failed
  (`every_producer_and_delivery_site_matches_the_reviewed_registry`, Request 1).
- `cargo clippy -p ti4-engine --all-targets` : no warning in this package's files except one
  pre-existing `redundant_closure_for_method_calls` at `action_cards.rs:3155` (not mine, present at HEAD).
- ti4-sim was not run (forbidden); neutrality rests on: every hook folds/sums/any to the identity
  with no module hook, new events are skipped without a listener, and the existing 1444 lib tests
  (including game-level production tests) pass unchanged.

## Review fixes (second pass)

- **S1** `payment::plans` / `Plan::worth_for` now read only the module hook (`hooks_economy::trade_good_worth(state, player, 1)`),
  not `production::trade_good_worth`, so they behave exactly as at HEAD with no module acting (the hard-coded `mc`
  technology is NOT applied there). `mc` is hard-coded in `production::trade_good_worth` only; the Mentak package
  must move it onto the hook and delete the hard-code in one step (doing only one of the two doubles or loses it).
  Hook docs no longer name `mc` as the consumer. `preview.rs` now calls `worth_for` (neutral). Test
  `plans_ignore_the_hard_coded_mc_technology_as_before`.
- **S3** `replace_unit(context, placer, owner, system, planet, remove_base, place_base)`: `owner` is whose unit is removed
  (opponent for Indoctrination/Greyfire), supply checked for `placer`'s unit; atomic, no board entry created on failure.
  Test `replacing_an_opponents_unit_checks_the_placers_supply_and_is_atomic`.
- **S4** `PlacementLimits {Respect, Ignore}` is now the last parameter of `place_units_choosing`; `max_fit(...)` uses
  `fleet::standing` (no new fleet excess, no new capacity excess). `Respect` drops spots that take none and places only
  what fits; `Ignore` is the explicit opt-out. `place_units_counted` stays the unchecked primitive. Test
  `placement_respects_capacity_and_the_fleet_pool_unless_told_not_to`.
- **S5** `production::barred_capacity`; `build_options` caps a unit at `min(remaining, limit - barred)` (ability windows
  exempt). Tests `a_barred_producers_production_does_not_count_for_the_barred_unit`,
  `infantry_is_not_offered_when_every_producer_is_barred`. Assumes earlier purchases drew on the barred share first
  (most permissive reading); ship placement is not filtered by `cannot_produce`.
- **S6** `emit_if_listened` / `listened` removed; events go through `hooks_economy::emit` unconditionally. Event ids and
  timing-log lines in games that produce, draw via `draw_announced` or gain goods via the helpers shift once
  (only `UNITS_PRODUCED` is live in play today: tactical-action production emits it, so traces of games with production change by one event per use that produced units).
  Tests renamed `units_produced_is_emitted_even_when_nothing_listens`, `a_zero_gain_emits_nothing_and_a_real_one_always_does`.
- **Request 2** done: `reactions::playable_now` returns nothing when `hooks_economy::action_cards_forbidden`; Political Censure
  not applied there. Test `a_play_forbidden_hook_closes_reaction_windows`.
- **Request 4** not done, recorded: `status.rs::resolve_before_token_gain(state)` and `agenda_effects.rs::{rider_payoff, resolve unconventional}`
  hold only `&mut GameState` (no content store, no `Table`), so they cannot call `action_cards::draw` (needs content + table for
  Scheming's discard and the hand limit) without a signature change in their callers (`game.rs`). Needed: pass `&ContentStore`
  and `&mut Table` (and a timing handle for `draw_announced`) to those functions. Their draws are unchanged.
- **Request 6** done without touching game.rs: `production::resolve_timed(.., table, timing: Option<TimingHandle>, ..)`; `resolve`
  delegates with `None`. Callers with a resolver (Warfare secondary, Construction, hero in `strategy_cards.rs`/`leaders.rs`)
  must pass their handle. Test `resolve_timed_announces_units_produced_through_the_callers_handle`.
- **Nit** `with_test_hooks<T>(hooks, run) -> T` restores via a drop guard on panic (already was; parameter renamed in docs only). Hook docs state that a choice asked from `action_cards_drawn` must be registered in `tests/decision_delivery_inventory.rs`.
- `tests/decision_delivery_inventory.rs`: registered `action_cards.rs::place_units_choosing` (PRODUCERS + OBSERVED_ASKS) and
  renamed `production.rs::resolve` to `resolve_timed` in OBSERVED_ASKS (the ask moved there).

Results: `production::` 51, `action_cards::` 115, `laws::` 25, `supply::` 9, `payment::` 11, `tokens::` 20,
`factions::` 11 (+1 ignored) passed. `cargo test -p ti4-engine -q --no-fail-fast`: lib 1455 passed, 1 ignored; all
integration binaries ok (decision_delivery_inventory 4 passed). Clippy: nothing in these files except the pre-existing
`action_cards.rs:3154` redundant closure.

## Coordinator follow-up (2026-10-02)

The agent was cut off by a rate limit before the verification fixes. Applied by the coordinator:
- Mitosis cap: `build_options` subtracts units of the same base type already produced this use
  from `limit - barred` (only when `barred > 0`, so neutral). Test
  `infantry_already_produced_spends_the_unbarred_share`.
- Removed the duplicate `place_units_choosing` entries in `tests/decision_delivery_inventory.rs`.
