# BF-00h-cards: hidden-hand and card-window routes

Implementer: Sonnet subagent. Files written: `factions/hooks_cards.rs`, `choice.rs` (one block in
`impl SeatObservation` + one test), `action_cards.rs` (choosing wrappers + test module
`hidden_hands`), `reactions.rs` (`announce_staged_card_events` + test),
`tests/decision_delivery_inventory.rs` (two registrations). Nothing else touched. No faction card
implemented; no existing call site changed (nothing in shared game flow calls the new routes yet).

## Items

| # | Item | Status | What was added |
|---|---|---|---|
| 1 | Hidden-hand reveal via the typed view | done | `hooks_cards::{reveal, reveal_hand, revealed_to, clear_reveals, clear_reveals_from, clear_reveal_between, is_revealed_action_card}`, scopes `RevealScope::{Choice, Action, Standing}`, kinds `RevealKind::{ActionCards, PromissoryNotes, SecretObjectives}`. Read-back only on `SeatObservation::{revealed_cards, revealed_action_cards, revealed_promissory_notes, revealed_secret_objectives}`: bound to the acting seat, no seat argument; `Observed` has no accessor. Storage: rows `cards:reveal:<scope>\|<source>\|<viewer>\|<owner>\|<kind>` in `GameState::faction_marks` (empty map is skipped on serialise/hash, so neutral). A card the owner no longer holds is not reported; a faceup note is not a hidden card. |
| 2 | Take a chosen card, with the taker's choice | done | `hooks_cards::take_revealed_action_card` (atomic; only a card currently shown to the taker; first copy goes; stages `ACTION_CARD_TAKEN`). Choosing wrappers in `action_cards.rs` (the scanned module): `choose_from_own_hand`, `show_action_card` (owner picks the card to show: Kyver), `take_from_revealed_hand`, `look_at_hand_and_take` (reveal, choose, end the reveal on every path, enforce 2.4 hand limit: Mageon Implants, Spy Net). Registered in `decision_delivery_inventory`: `choose_from_own_hand`, `take_from_revealed_hand`. |
| 3 | Free-action window: discard 1 action card (Stall Tactics) | done, no new route | Existing `Hooks::component_actions` (offer, ids `faction\|yssaril\|...`) + `perform_component` (resolve, gets a `TimingContext`) already allow it. Resolution recipe: `action_cards::choose_from_own_hand(...)` then `hooks_cards::discard_chosen(...)`. Proven by `stall_tactics_is_reachable_through_the_existing_component_action_hooks` (uses the same calls a module's `perform_component` would). The mech DEPLOY ("after you use Stall Tactics, place 1 mech on a planet you control") is the module's own code after the discard: `action_cards::place_units_choosing`. |
| 4 | Copy another card's text (Ssruu, `yssarilagent`) | partial: query only | Text read: "This card has the text ability of each other player's agent, even if that agent is exhausted." (TF: genomes). `hooks_cards::borrowable_agents(state, content, player)` lists other seats' readied or exhausted agents when `player` holds a readied `yssarilagent`. Resolving the borrowed ability needs `leaders.rs`: see Requests R2. |
| 5 | Transaction limit hook (`yssarilbt`) | hook + query done, call site requested | `CardHooks::transaction_limit_exempt: Option<fn(&GameState, &ContentStore, active:&PlayerId, other:&PlayerId) -> bool>`, dispatch `hooks_cards::transaction_exempt_from_limit` (any module's `true`). Test hook proves dispatch and scope. Call site in `transactions.rs`: Requests R3. |
| 6 | Typed events | done (announcement call site requested) | New: `ACTION_CARD_TAKEN` (payload `player` = taker, `from`, `card`). Existing `ACTION_CARD_DISCARDED` reused for a staged discard. Hooks hold a `TimingContext` with no resolver, so they **stage** (`faction_marks` rows `cards:staged:NNNNNNNN`); `reactions::announce_staged_card_events(context, resolver)` drains and emits oldest first (discard goes through `announce_discard`, so pile and `last_action_discarded` are set exactly as for a played card). No staged event, no emission: neutral. Call site: R1. |

## Requests (files not owned)

- **R1 (game.rs / whoever drives timing).** After every `perform_component`, `use_leader` and
  timing-ability effect that can run a faction hook, call
  `crate::reactions::announce_staged_card_events(&mut context, &mut resolver)` (cheap no-op when
  nothing is staged). Until it is wired a staged discard stays in `faction_marks`, the card is out
  of the hand and not in the pile, and no event fires. Also, when an action completes
  (`ACTION_COMPLETED`), call `hooks_cards::clear_reveals(state, RevealScope::Action)`; until then
  only `Choice` and `Standing` scopes are safe to use.
- **R2 (leaders.rs).** For Ssruu: expose `leaders::use_leader_text(context, as_player, source_agent: &LeaderId) -> bool` that runs the
  existing per-leader dispatch (including `crate::factions::use_leader`) **without** the status
  gate on the source agent and **without** exhausting it, and have the Yssaril module exhaust
  `yssarilagent` itself on success. Also `component_actions` would need to offer the borrowed
  agents' "ACTION:" windows (`borrowable_agents` gives the list). Agents with non-ACTION windows
  ("At the start of..." timing abilities) are built per owner in `timing_abilities` at
  construction, so a module would have to register a copy for the Ssruu owner. Rules question Q1.
- **R3 (transactions.rs).** Wherever the once-per-player-per-turn count is checked, skip the
  check when `hooks_cards::transaction_exempt_from_limit(state, content, active, other)`, and do
  not record the transaction against the limit in that case. The Yssaril module keeps its own
  "allowed" flag in `faction_marks`.
- **R4 (any serialiser that hands state to a non-owner).** Reveal rows (`cards:reveal:*`) name
  another player's hidden cards; the engine's typed views never expose them, but a raw
  `GameState` dump does, as it does for every hidden hand. Nothing in the repo reads
  `faction_marks` outside `ti4-model::state` and `hooks_cards.rs` (grep).

## Rules questions

- **Q1** Does using Ssruu's borrowed text exhaust Ssruu (the card the text is on) and leave the
  other agent readied? Assumed yes; the engine does not implement it yet.
- **Q2** Kyver: "force that player to discard 3 random action cards": random choice needs a dice
  reason label; not built here (needs only `context.dice`; the module can do it with
  `hooks_cards::discard_chosen` per card). The show step is `show_action_card`.
- **Q3** Hand limit after Mageon Implants/Spy Net: `look_at_hand_and_take` enforces 2.4 at once
  (as `draw` does); Crafty is honoured through the existing `action_card_limit` hook.

## Commands and exact results

| Command | Result |
|---|---|
| `cargo test -p ti4-engine --lib choice::` | 52 passed, 0 failed |
| `cargo test -p ti4-engine --lib reactions::` | 15 passed, 0 failed |
| `cargo test -p ti4-engine --lib action_cards::` | 124 passed, 0 failed (9 are `hidden_hands`) |
| `cargo test -p ti4-engine --lib factions::hooks_cards` | 9 passed, 0 failed |
| `cargo test -p ti4-engine --lib factions::` | 55 passed, 1 failed, 1 ignored: `factions::yin::tests::greyfire_needs_two_ground_forces_and_a_non_yin_opponent`, in another agent's in-progress file |
| `cargo test -p ti4-engine --test decision_delivery_inventory` | first run 4/4 passed (after my two registrations); at the last run 3 passed, 1 failed: `every_producer_and_delivery_site_matches_the_reviewed_registry`. The scan-versus-registry diff is exactly `sardakk.rs::exotrireme` (x2), `sardakk.rs::supremacy`, `yin.rs::ask_one`, new sites added by other agents and not registered; none of mine are in the diff |
| `cargo test -p ti4-engine -q --no-fail-fast` (last run) | lib 1475 passed, 58 failed, 1 ignored. Sampled failure (`game::tests::a_turn_with_nothing_left_to_do_ends_without_asking`): `ScriptDiverged`, offered `faction:generic:sardakk_tactical_turn:TURN_BEGAN:after`, i.e. another agent's newly registered Sardakk timing ability. My code is not called from any game path, so it cannot change these; I did not bisect all 58 |
| `cargo clippy -p ti4-engine --all-targets` | no warning in `hooks_cards.rs`, `choice.rs`, `reactions.rs` or in my lines of `action_cards.rs` (one pre-existing `redundant_closure` at `action_cards.rs:3349`, not mine) |
| `rustfmt --edition 2024` | applied to my four source files |

The decision-delivery scanner walks `factions/` as well as `src/`, so a module that asks a choice
registers its own site; `hooks_cards.rs` asks none.
