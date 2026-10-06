# BF-F5: leaders, transactions, votes, cards routes

Files written: `leaders.rs`, `transactions.rs`, `vote.rs`, `factions/hooks_cards.rs`, this file.
`promissory.rs` and `factions/mod.rs` were not changed (all new hooks live in `CardHooks`).

## Items

| # | Item | Status | Signatures / call sites | Tests |
|---|---|---|---|---|
| 1 | Ssruu borrowed agent text | done (engine side); Yssaril module claim still the module owner's | `leaders::use_leader_text(context, as_player, source_agent) -> bool`; `leaders::split_borrowed`; `leaders::BORROWED_SEPARATOR`; `leaders::component_actions` now chains `borrowed_component_actions`; option id `component\|leader\|yssarilagent\|<agent>`; `use_leader` and `uses_the_action` accept that id, so `game.rs perform_leader_action` and `is_free_option` work unchanged; hook `CardHooks::borrowed_agent_used(context, borrower, source_agent)` via `hooks_cards::borrowed_agent_used` | `ssruu_uses_an_exhausted_agent_without_exhausting_or_readying_it`, `use_leader_text_refuses_without_ssruu_or_for_a_non_agent_and_changes_nothing`, `ssruu_offers_other_seats_action_agents_with_distinct_ids`, `the_borrowed_agent_hook_runs_after_a_successful_borrow_only` |
| 2 | Transaction limit exemption + resolved parties | done engine side; game.rs wiring requested | `transactions::may_open_again(state, content, proposer, partner)` (used by `available_actions`); `transactions::record_unless_exempt`; `TradeWindow::open_with_content(state, content, proposer, partner)` (`open` delegates with the embedded corpus); `TradeWindow::parties()`; `transactions::resolved_payload(proposer, partner)` -> `{proposer, partner}` | `an_exempt_pair_may_open_again_and_the_opening_is_not_recorded`, `the_resolved_payload_names_both_parties` |
| 3 | Voting order + content in vote bonus | done | `CardHooks::votes_first(&GameState,&PlayerId)->bool`, `hooks_cards::votes_first`, read in `VoteWindow::new` (first-voters take the opening seats in clockwise order; hack-last seats still last); `CardHooks::vote_bonus_with_content(&GameState,&ContentStore,&PlayerId)->i64`, `hooks_cards::vote_bonus_with_content`, summed in `VoteWindow::record` (`record` now takes `content`, private) | `a_seat_a_module_puts_first_votes_first_and_others_keep_their_order`, `a_module_vote_bonus_with_content_is_banked_with_the_votes`, `card_hooks_are_neutral_when_empty` |
| 4 | `COMMAND_TOKEN_PLACED` | helper done; no placement site is in my files | `hooks_cards::COMMAND_TOKEN_PLACED`, `TokenPool {Tactic, Fleet, Strategy, Reinforcements}`, `command_token_placed_payload(player, system, pool)` -> `{player, system, pool}`, `announce_command_token_placed(context, resolver, player, system, pool) -> Result<(), TimingError>` | `command_token_placed_payload_and_window_carry_player_system_and_pool` |
| 5 | Leader effects announce events | documented; coordinator already flushes staged destructions and cards after leader actions | see below | none (no code) |

## game.rs requests (coordinator)

1. Opening a transaction (`game.rs` ~3087 and ~3117): replace `!self.state.transacted_with(actor).contains(&partner)` with `crate::transactions::may_open_again(&self.state, self.content, actor, &partner)` and `self.state.record_transaction(actor, &partner)` with `crate::transactions::record_unless_exempt(&mut self.state, self.content, actor, &partner)`. Use `TradeWindow::open_with_content(&mut self.state, self.content, ..)` wherever `TradeWindow::open` is called.
2. `TRANSACTION_RESOLVED` emitters (~3208 diplomacy path, ~3257 `step_trade`): pass `crate::transactions::resolved_payload(proposer, partner)` instead of an empty map (`step_trade`: `self.trade.as_ref().unwrap().parties()`; the diplomacy path has its window's two seats). Existing listeners (Lie in Wait, Mentak Pillage) read no payload keys, so this is additive.
3. Diplomacy candidates (`diplomacy/candidates.rs:553`) also filter on `transacted_with`; it needs the same `may_open_again` for an exempt pair to be offered under structured diplomacy.
4. Nothing needed for Ssruu: the option id is parsed by the existing `component|leader|` branch and `use_leader` handles `yssarilagent|<agent>`. `takes_turn` already follows `leaders::uses_the_action`, which now looks through the prefix.

## Command-token placement sites (not mine; call `announce_command_token_placed` after the token is placed)

`tokens.rs::place_command_token_from_reinforcements` (pool Reinforcements; returns bool, has no resolver, so the caller announces), `tactical.rs:185` (activation, Tactic), `action_cards.rs:2157`, `strategy_cards.rs:833`, `agenda_effects.rs:756` and `:1107`, `combat.rs:2621`, `status.rs` redistribution (Strategy/Tactic/Fleet), `factions/naalu.rs:462` (Z'eu / naaluagent-te). In `leaders.rs` there is none (Jace X only removes tokens). The event is not in `reactions::EMITTED_EVENTS`/window table; listeners register with `timing_abilities`.

## Item 5: leader effects and events

`leaders::use_leader(context, ..)` runs inside a `TimingContext`, which has no resolver, so an effect cannot open a window itself. The route that exists and that `game.rs` already runs after `perform_leader_action`: effects stage (`combat::destroy_units` staged destructions, `hooks_cards::discard_chosen` / `take_revealed_action_card` staged cards) and the coordinator flushes (`announce_staged_destructions`, `announce_staged_cards`). Not yet covered: leader production emits no `UNITS_PRODUCED` (Arborec hero). Suggested shape: stage a `StagedProduction` row the same way (needs `reactions.rs`/`production.rs`), flush with the others. No `&mut Resolver` path was added to `use_leader`, because the flush already gives every staged effect its window and a second path would double-announce.

## Limits left

* Borrowed module agents: `use_leader_text` dispatches as the borrower. A module agent whose `leader_action`/`use_leader` hook requires the caller to hold that leader declines, so it is not offered. Only agents the shared code delivers (`xxchaagent`, `hacanagent`) and module agents accepting a foreign caller are borrowable.
* Non-ACTION agents ("At any time" / reaction windows): timing abilities are built per owner at construction and check the owner holds the leader. Not delivered through Ssruu; each such module would register a copy conditioned on `borrowable_agents` (module work).
* The Ssruu window text says "exhaust this card"; Ssruu is exhausted by shared code after a successful borrow (see rules question).
* Yssaril's "allow another player to use Stall Tactics/Scheming" (yssarilbt part b) is not part of this package.

## Rules question

Does Ssruu exhaust when the borrowed text says "exhaust this card"? Implemented: yes (the card holding the text is Ssruu, and without it Ssruu would be an unlimited source of agent effects).

## Decision sites to register

None new: `use_leader_text` reuses the existing leader dispatch arms (their Choice sites are already registered); no new `Choice::new` or ask sites were added in my files.

## Commands and results

See the bottom section, filled after the runs.

### Run results (final)

| Command | Result |
|---|---|
| `cargo test -p ti4-engine --lib -- leaders:: transactions:: vote:: promissory:: factions::` | 422 passed, 0 failed, 1 ignored |
| `cargo test -p ti4-engine -q --no-fail-fast` | lib 1845 passed, 1 ignored; all integration binaries pass except `decision_delivery_inventory` (2 of 4 fail). I added no `Choice::new`/ask site; the failure comes from other agents' unregistered sites (faction modules), not verified site by site |
| `cargo clippy -p ti4-engine --all-targets` | no warning at any line in `leaders.rs`, `vote.rs`, `hooks_cards.rs`; one new `redundant_iter_cloned` in `transactions::available_actions` was fixed; remaining `transactions.rs`/`promissory.rs` warnings are the pre-existing ones |
