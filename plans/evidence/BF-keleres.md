# BF-keleres (part A): Council Keleres

Branch wp/base-factions, uncommitted. Part B (flagship, mech, heroes, Rider) is `keleres_units.rs`.

## Items

| Kind | Id | Status | Tests (`factions::keleres::tests::`) |
|---|---|---|---|
| ability | the_tribuni | done | the_tribuni_needs_an_unplayed_base_faction_whatever_the_seating_order |
| ability | council_patronage | done | council_patronage_replenishes_then_gains_a_trade_good_for_every_variant |
| ability | laws_order | done (action-phase turns only) | laws_order_blanks_every_law_until_the_turn_ends, laws_order_is_not_offered_... |
| tech | asn | done | asn_resolves_a_second_systems_production_once_per_action |
| tech | iihq | done, with a dependency (below) | iihq_makes_neighbours_..., custodia_vigilia_arrives_stays_..., custodian_favour_pays_two_command_tokens_... |
| breakthrough | keleresbt | done, with a dependency (below) | same helper (`has_iihq`) as iihq |
| tech | executiveorder | done (agenda machinery, 2026-10-06) | executive_order_* (5) |
| leader | keleresagent | done (every payment window, 2026-10-06) | the_agent_* (6) |
| leader | kelerescommander | unlock done | the_commander_unlocks_by_spending_a_trade_good_... |

## Design decisions

- **One module carries the hooks.** All modules' hooks run for every seat, so three modules over one hook table tripled every timing ability and component action. MODULE_M carries the table (plus part B's combat hooks, and `leader_action`/`use_leader` for Harka); X and A claim only. Hooks decide from the seat's faction.
- **The Tribuni** (`seating::validate_tribuni`, `tribuni_base`, `tribuni_variants_available`): checked in `deploy` (order independent, before any mutation) and `build_board`; also one Keleres seat only. Home system, units and hero are the variant record's; command tokens are pool counts and control is per seat, so nothing else is "taken".
- **Law's Order**: `laws::blanked` (mark = `turn_seq`, action phase only) makes `laws::in_play/active/elected` blank; the two raw readers in game.rs (`minister_of_war`, `imperial_arbiter`) now use `laws::elected`. Not blanked: raw `state.laws` readers elsewhere (agenda_effects enactment, secrets count, `Observed::laws()` feature view). Window is TURN_BEGAN, which fires for action-phase turns only.
- **ASN**: `production::agency_supply_network`, called in `game.rs::enter_production` (tactical) and `production::resolve_timed` (Warfare). The Keleres picks one other system with producers; it is resolved before the primary system's production. Once per action via a `faction_marks` key cleared on ACTION_COMPLETED. No Sarween/AIDA discount on the second use (rules question: is it one "use"?).
- **IIHQ / keleresbt**: neighbours via `transactions::are_neighbours` (mutual). Custodia Vigilia is placed in the Keleres home system by `keleres::reconcile`, called at the top of `Game::step`; it holds no units. Standing effects: SPACE CANNON 5 via `planets::attachment_cannons`, PRODUCTION 3 via `extra_production_planet` (production now also asks hooks about controlled planets with no units), 2 command tokens via `strategy_cards::imperial_primary`. "Cannot lose": reconcile restores control (backstop only).
- **Executive Order**: component actions `faction|keleres|executiveorder|top|bottom`. The vote runs synchronously inside the action (`executive_order`), speaker = owner, spending planets for influence or (owner) resources, trade goods 1 each for owner / 2 each for Gila. Not offered: reveal-window cards (the card is drawn, not revealed), diplomacy talks, `VOTES_CAST`/`AGENDA_RESOLVED` events. Rolled back whole on an illegal answer.
- **Agent**: permission lasts the producing player's turn and covers `production::available/payment_options/apply_payment_option` (option id `commodity`). Other spending windows (research, token purchase, votes) have no pre-payment window.
- **Commander unlock**: ACTION_CARD_PLAYED after, card window "Action", pay 1 trade good.

## Hook requests (files not mine)

1. `thunders_edge::breakthrough_for` and `breakthroughs::for_faction` match `record.faction == faction`; use `keleres::tag_belongs_to(record_faction, faction, false)` or a Keleres variant never receives `keleresbt` from an expedition (my tests grant it directly).
2. `invasion.rs::landable_planets`: filter out `custodiavigilia` (`.filter(|p| p.as_str() != "custodiavigilia")`) so it cannot be invaded; reconcile restores control but not removed/foreign units.
3. `vote.rs`: none needed (Executive Order does not use `VoteWindow`).
4. Coordinator: add `keleresagent`, `kelerescommander` to `keleres_units::LEADERS_M/X/A`.

## Decision sites to register (decision_delivery_inventory)

`keleres.rs`: `cast_votes`, `laws_order`, `resolve_vote`, `spend_planets`, `spend_trade_goods` (Choice + AskObserved each). `production.rs`: `agency_supply_network` (1 Choice, 2 AskObserved).

## Results

- `cargo test -p ti4-engine --lib -j1 keleres`: 24 passed, 1 ignored (part B's Harka test, awaiting the hook now wired) before my tests; `factions::keleres::tests`: 14 passed.
- `cargo test -p ti4-engine -j1 -- --test-threads=12`: lib 2424 passed, 0 failed, 2 ignored; `decision_delivery_inventory` 1 failed (expected: the sites above, plus part B's `keleres_units.rs::ask`).
- Soak: `base_faction_soak keleresm|keleresx|keleresa 0 12 6`: 12 games each, 0 failures (the soak puts a variant in the seat of its base faction when that is at the table).
- Ledger: each variant 11/13; missing `keleresagent`, `kelerescommander` (coordinator's claims).

## Update 2026-10-06: Executive Order and the agent completed

Both partial assets are done and claimed (`executiveorder` in `keleres.rs` TECHNOLOGIES; `keleresagent` in `keleres_units.rs` LEADERS_M/X/A). This section supersedes the Executive Order and Agent lines above and the "Decision sites" list.

### Executive Order (through the agenda machinery)

- `keleres::perform_component` now only pays the cost (exhaust) and draws the top or bottom agenda, leaving `faction_marks["keleres|executive_order_pending"] = "<owner>|<agenda>"`. The synchronous helpers (`executive_order`, `cast_votes`, `planet_face`, `planet_vote_options`, `spend_planets`, `spend_trade_goods`, `resolve_vote`) are gone.
- `Game::apply_choice` (faction-action arm) takes the pending agenda (`keleres::take_executive_order`) and calls the new `Game::begin_executive_order`: the owner becomes `state.speaker` (the real speaker is held in the new `Game.executive_order` field), then `open_next_vote(vec![agenda])` runs the ordinary flow: `reveal_agenda` (AGENDA_REVEALED windows: Veto, Quash, Political Favor, Hack Election, Keleres Rider, Odlynn, Political Secret), Committee Formation, diplomacy talks if on, `start_vote`/`VoteWindow` (VOTES_CAST per voter), `close_vote` (AGENDA_RESOLVED windows, predictions, law enactment, agenda effect, occurrence scoring). When the queue runs out `open_next_vote` calls `finish_executive_order`: speaker restored, `COMPONENT_ACTION_RESOLVED` emitted (so it comes after the vote and `ACTION_COMPLETED`'s `component` flag still reads it), then `finish_action` (turn-closing / next turn as for any component action). No `AGENDA_PHASE_RESOLVED`, and planets exhausted for votes are not readied (step 8.4 is the agenda phase's; they ready at the status phase).
- `vote.rs`: `VoteWindow::with_spender(owner)`. For the spender, `planet_offers` adds a `"<planet>|resources"` option per planet with resources (influence option id unchanged), and the trade-goods stage (previously Gila only) is offered with `trade_goods_rate` = 1 per good (2 with Gila once a vote is cast). Elder Qanoj's +1 applies to the resource face too. Non-spenders are unchanged. `Game::start_vote` attaches the spender when an Executive Order is running.
- Order of voting: clockwise from the owner's left, owner last, owner breaks ties (`state.speaker` is the owner for the vote).
- Atomicity: an illegal vote answer is refused by `table.ask_seeing`/`VoteWindow::resolve` before any mutation and the same question stays owed (test). An illegal answer inside a reveal-window card has the agenda phase's existing behaviour (that window's own rules), not a whole-action rollback: the draw and exhaustion stand. This differs from the old synchronous version, which cloned and restored the state.
- Tests (`factions::keleres::tests::`): `executive_order_runs_through_the_agenda_flow_and_enacts_the_law` (real `Game`, AGENDA_REVEALED/VOTES_CAST/AGENDA_RESOLVED/LAW_ENACTED, Hack Election offered and played in the reveal window, order c,a,b, 2 goods = 2 votes, speaker restored, deck/laws), `executive_order_votes_in_speaker_order_and_the_owner_breaks_the_tie`, `executive_order_may_exhaust_planets_for_resources_only_for_its_owner`, `an_illegal_executive_order_vote_answer_changes_nothing_and_can_be_retried`, `executive_order_is_not_offered_without_the_card_or_an_agenda`.

### Agent (every payment window)

- `keleres::offer_agent` (one `Choice::new` + one `ask_seeing`, registered) asks each Keleres holder of a readied agent, once per payment window, when the paying player holds commodities. "use" exhausts the agent and sets `faction_marks["keleres|agent_goods|<payer>"]`; a decline sets the same mark to `declined`, so nothing more is offered inside that window. `spendable_commodities` is non-zero only while the mark is a grant. The opener closes the window (`close_agent_window`). Safety net: `agent_clear` on ACTION_COMPLETED removes marks an aborted window left. The old PRODUCTION_USED ability and the per-turn grant are gone.
- Seams: `production::pay_offering_agent` (wraps `pay_with_observation_credit`; every `pay`, `pay_seeing`, `pay_seeing_with_credit` caller: unit production by ability, relics, exploration, Custodians, Mentak/Saar/Yin/Empyrean/Thunder's Edge/faction-ability costs), production windows (`Game::enter_production` for the tactical action, closed at PRODUCTION_RESOLVED; `production::resolve_timed` for Warfare/Construction; `produce_by_ability_capped`), `strategy_cards::paid_research` (Technology primary's second research and the secondary; offered before the affordability gate and only when the commodities are what makes it affordable, since plans are auto-picked), Leadership's token purchase (`buy_tokens_with_influence`, `buy_tokens_first_yes_assumed`: one offer for the whole purchase; the follower gates `leadership_influence_eligible` and `strategy::secondary_can_do_something` see what the agent would add via `keleres::with_agent_granted`). `payment::plans` counts granted commodities as goods and `payment::apply` spends commodities first; `technology::research` (Inheritance Systems) now pays through `payment::apply`.
- Offered only when the commodities would make the bill payable (`pay`: `available` with the grant >= owed).
- Fixed trade-good spends (no payment window) go through one shared helper in `supply.rs`: `potential_goods` (trade goods + commodities a readied agent could still grant: the up-front legality gate, payer only), `open_goods_window` (offers the agent via `keleres::offer_agent` when the payer has commodities and no window is open; a decline suppresses re-offers), `spendable_goods` (trade goods + commodities of an open, accepted window: what options are generated against), `spend_goods` (atomic, commodities first), `close_goods_window`, plus `with_goods_window` (TimingContext sites) and `pay_goods_seeing` (one-shot). Pattern at every site: gate on `potential_goods`, open the window before the decision, re-gate on `spendable_goods`, decide, `spend_goods`, close. Spent goods and commodities just leave the count. No seated Keleres: `potential_goods == spendable_goods == trade_goods`, no offer, identical sequences.
- Sites converted: Bribery, Mercenary Contract, Focused Research (`action_cards.rs`); Wrath of Kenara (`combat.rs`, window opened before the question; asked even when no die is a near miss, only for a Hacan flagship owner with goods or commodities); Munitions Reserves (`faction_abilities.rs`); Saar Scavenger Zeta DEPLOY; Suffi An's unlock (`keleres.rs`); Thunder's Edge expedition "3 trade goods" slice (gate and payment); the trade-goods-for-votes stage (`vote.rs`: stage entered when `potential_goods > 0`, options generated from `spendable_goods`; `Game::step_vote` offers the agent to the spending seat before asking, auto-declines when a declined agent leaves nothing to spend, closes the window after resolving, and restores the pre-offer position on an illegal answer). Covers Gila the Silvertongue and Executive Order.
- Not converted, by rule: Quantum Datahub Node GIVES 3 trade goods to another player (only the strategy token is spent; the card says "give", and a commodity changing hands is a trade good anyway, 21.5); transactions and Pillage take are not spends. Exploration Functioning Base / Local Fabricators already offer "spend 1 trade good" and "spend 1 commodity" as separate options (the cards allow either), so the agent adds nothing. Law's Order already accepts a trade good or a commodity.
- Remaining table-less gaps: `objectives::pay_for` `Cost::TradeGoods` / `Cost::AllThree` (no table, and it has no production caller at all: only tests; `can_afford` / `bought_progress` feed policy features and stay on plain trade goods). Hacan commander's unlock reads `trade_goods >= 10` as a holding, not a spend.
- Tests: `the_agent_is_offered_at_a_payment_and_its_commodities_pay_for_that_payment_only`, `declining_the_agent_keeps_commodities_out_of_the_payment`, `the_agent_is_offered_when_a_production_window_opens`, `the_agent_opens_a_research_that_only_its_commodities_can_pay_for`, `the_agent_opens_a_leadership_token_purchase_for_its_commodities`, `the_agent_is_neutral_without_a_ready_agent_or_commodities_or_keleres`; fixed spends: `bribery_is_paid_with_commodities_through_the_agent`, `declining_the_agent_leaves_a_fixed_spend_unchanged`, `a_fixed_spend_without_keleres_is_never_offered_the_agent`, `a_fixed_spend_only_reaches_what_the_window_allows_and_is_atomic`, `a_vote_with_trade_goods_is_paid_by_commodities_through_the_agent` (real `Game`, Executive Order vote), `declining_the_agent_leaves_the_vote_without_a_trade_good_stage`.
- Fixed-spend registry change (`decision_delivery_inventory`): `combat.rs::wrath_of_kenara` -> `wrath_of_kenara_buy`, `faction_abilities.rs::space_combat_round_started` -> `munitions_offer` (the question moved into the helper inside the goods window; counts unchanged, 1 each, ObservedHere).

### Registry (decision_delivery_inventory)

Removed `keleres.rs`: `cast_votes`, `resolve_vote`, `spend_planets`, `spend_trade_goods` (Producer and OBSERVED_ASKS). Added `keleres.rs::offer_agent` (1 Choice, 1 AskObserved, ObservedHere). `vote.rs::pending_choice` stays 4, `production.rs::pay_with_observation_credit` unchanged.

### Results

- Fixed-spend extension: `cargo test -p ti4-engine --lib -j1 keleres`: 52 passed, 0 failed. `cargo test -p ti4-engine -j1 -- --test-threads=12`: lib 2438 passed, 0 failed, 1 ignored; `content_ids_resolve` 1; `decision_delivery_inventory` 4; other 5; exit 0 (log `out/keleres_fixed_spend_full_test2.log`; the first run, `..._test.log`, failed the two registry tests until the two renames above). Formatting: rustfmt on supply.rs, combat.rs, faction_abilities.rs, saar.rs, keleres.rs; combat.rs and supply.rs were then rebuilt to only my hunks because rustfmt also reflowed unformatted base code. game.rs, vote.rs, action_cards.rs, thunders_edge.rs not run through rustfmt (other hunks).
- (Earlier, before fixed spends) `cargo test -p ti4-engine --lib -j1 keleres`: 46 passed, 0 failed.
- `cargo test -p ti4-engine -j1 -- --test-threads=12`: lib 2432 passed, 0 failed, 1 ignored; `content_ids_resolve` 1 passed; `decision_delivery_inventory` 4 passed; remaining doc/other binary 5 passed; exit 0. Log: `out/keleres_full_test.log` (new file, ignored area).
- Ledger (`print_faction_ledger`): keleresm, keleresx, keleresa 13/13 implemented.
- Soak `base_faction_soak keleresm|keleresx|keleresa 0 12 6`: 12 games each, 0 failures.
- Formatting: `rustfmt --edition 2024` on keleres.rs, keleres_units.rs, vote.rs, payment.rs, strategy.rs, technology.rs (clean at base). game.rs, production.rs and strategy_cards.rs were not run through rustfmt (other hunks); my hunks in them were formatted by hand.
- Hook request 3 (vote.rs) is now fulfilled by me. Review: Tier C (votes, payments, hidden information untouched); frontier review of the `begin_executive_order` driver path and the payment seam is still owed, not done by this author.

## Coordinator review (Claude Opus, Tier C: payments, votes, agenda flow), 2026-10-07

- Payment seam (`payment::plans/apply`, `supply::spendable_goods/potential_goods/spend_goods`): commodities count as trade goods only inside an accepted agent window, for the payer only; affordability is rechecked before mutation; commodities spent first; spent goods return to the supply (operator, 2026-10-07). Sound.
- Executive Order: runs through the real agenda flow with the Keleres player as speaker. Fixed: `finish_executive_order` restored the prior speaker unconditionally, undoing an agenda/rider that made someone speaker during the vote; it now restores only if the Keleres player is still speaker.
- `vote.rs` `planet_offers` keeps the old influence > 0 filter, so voting is unchanged without Keleres.
- Known gaps: `supply::with_goods_window` swallows an illegal answer to the agent offer (returns None) instead of propagating it; `objectives::pay_for` (bought objectives) is table-less and unreached by production code; Quantum Datahub is treated as a transfer, not a spend.
- Checks: engine lib 2438 passed / 1 ignored, registry 4/4; sim 51; routine soak 19 factions 475/475 and Keleres variants 25/25 each after the fixed-spend work.
