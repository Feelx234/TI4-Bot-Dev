# BF-SSRUU-ACTION-CENSUS continuation

Supplemental acceptance census by luna, October 5. This records source inspection and added test cases; no Cargo checks or independent tier-C review were run here. The base-factions coordinator owns Cargo and review for the shared worktree.

Printed source is `crates/ti4-content/content/leaders.json` at `DEFAULT`/PoK. Relevant text:

- Arborec `arborecagent` (Letani Ospha): `ACTION:` choose a player's non-fighter ship; that player may replace it with a ship from their reinforcements costing up to 2 more.
- Hacan `hacanagent` (Carth): `During the action phase:` gain 2 commodities or replenish another player's commodities.
- Xxcha `xxchaagent` (Ggrocuto Rinn): `ACTION:` ready any planet; if it is in a system adjacent to a planet you control, you may remove one infantry from that planet and return it to reinforcements.
- Yssaril `yssarilagent` (Ssruu): `At any time:` it has the text ability of each other player's agent, even if exhausted.

## ACTION route audit

`leaders::component_actions` chains the native options with `borrowed_component_actions`. Borrowed candidates come from `hooks_cards::borrowable_agents`, which requires the borrower's Ssruu Readied and accepts another seated agent that is Readied or Exhausted. The borrowed option must have an action-phase window and a resolving handler. `use_leader` parses `yssarilagent|<source>` and calls `use_leader_text`; that revalidates the source against the live candidate set and dispatches the source effect as the Yssaril borrower, with no source-readiness gate. Only a successful effect exhausts Yssaril's Ssruu; source status is not changed.

- Arborec: `factions::leader_action` derives availability from `ship_targets`. The list is bounded by the selected ship's owner's unit catalogue, the printed +2 cost ceiling, and that owner's reinforcement supply. `agent_use` asks the borrower to choose when there are multiple valid ships, then gives the replacement choice to the selected ship owner. The added `ssruu_copies_arborec_text_as_user_and_routes_replacement_to_ship_owner` test uses a distinct Arborec source and Hacan ship owner, captures the owner on the replacement choice, checks the carrier replacement, checks the source agent stays exhausted, and proves an invalid replacement answer leaves state unchanged. Existing Arborec tests `the_agent_lets_the_ships_owner_trade_up_by_at_most_two` and `the_agent_is_not_offered_for_fighters_or_a_box_that_cannot_supply` cover the exact +2 boundary and supply gating for the native effect.
- Hacan: the shared dispatch runs with the borrower as `player`. The self branch adds 2 capped at the borrower's faction commodity limit; the other-player branch fills the selected player's own faction limit. `can_resolve_action` suppresses the option only when every seat is at its cap. Existing `ssruu_uses_an_exhausted_agent_without_exhausting_or_readying_it` exercises actual borrowed dispatch against the source-owner Hacan seat at zero, checks it fills to Hacan capacity, and checks source status stays exhausted while Ssruu exhausts. `ssruu_offers_other_seats_action_agents_with_distinct_ids` checks no offer when every seat is full.
- Xxcha: the shared dispatch uses the borrower's choice and state. The added `ssruu_copies_xxcha_to_ready_the_selected_exhausted_controlled_planet` exercises a borrowed copy with two exhausted planets controlled by different seats, chooses the source owner's planet, and checks only that planet readies. An invalid target answer leaves state unchanged. The source agent remains exhausted and Ssruu exhausts only after success.

The new Arborec and Xxcha tests are in `crates/ti4-engine/src/leaders.rs`. They have not been run in this turn; the coordinator must include them in the shared focused suite.

## Recursive Ssruu boundary

The ordinary six-seat faction assignment cycles the distinct entries in `seating::IN_SCOPE_FACTIONS`, so a supported game cannot give the Yssaril borrower another player's `yssarilagent`. As a defensive case, `borrowing_another_ssruu_does_not_reenter_the_copy_dispatch` constructs duplicate Yssaril seats and checks both that the other Ssruu is in the candidate set, no ACTION option is offered (its printed `At any time:` window is not an action window), and a direct borrowed-source attempt returns false without state changes. `use_leader_text` calls the per-source `dispatch_leader` directly; it does not call itself recursively. The Yssaril module has no native Ssruu dispatch, so even the artificial direct attempt fails closed. No recursive helper is needed for the currently supported unique-faction roster. If duplicate Yssaril seating is introduced later, the printed “each other player's agent” closure should be resolved as a flat set of those other agents with the borrower excluded, not by recursively invoking Ssruu.

## Remaining scope and status

This closes only the three audited ACTION-copy effects and the recursion boundary. It does not close all Ssruu copying. The seated-agent census still needs the timing/placement/research/delayed routes recorded in `BF-SSRUU-SEATED-AGENT-CENSUS.md`; the printed Muaat foreign-borrower limitation and any other source whose handler still binds native ownership remain unaccepted. No faction ledger, whole-faction completion, or tier-C acceptance is claimed.
