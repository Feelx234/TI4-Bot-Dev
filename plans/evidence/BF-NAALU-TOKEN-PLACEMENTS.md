# BF-NAALU-TOKEN-PLACEMENTS — Naalu Z'eu command-token placements

## Scope

Implement Naalu Thunder's Edge agent Z'eu: “After any player's command token is placed in a
system: You may exhaust this card to return that token to that player's reinforcements.” The
ability must observe every real command-token placement, including placements other than tactical
activations, and only once for each actual placement.

## Producer audit

The repository-wide source search covered `command_tokens.insert` and `SystemState::place_token`.
Live placement paths are:

| Path | Source | Handling |
|---|---|---|
| Tactical activation | `game.rs` after `tactical::activate` | Already emits `COMMAND_TOKEN_PLACED` directly. Keep this single event; do not stage a duplicate. |
| Combat retreat token | `combat.rs` | Outside this package's write scope; parent agent owns this call site. |
| Reinforcement token helper, including Arborec Stymie | `tokens.rs`, called by `factions/arborec.rs` | `stage_command_token_placed` stages `COMMAND_TOKEN_PLACED` with pool `reinforcements`. |
| Diplomacy primary | `strategy_cards.rs` | Stage once for each newly inserted opponent token, pool `reinforcements`. |
| Diplo Rider | `action_cards.rs` | Stage for each newly inserted token after spending a fleet-pool token, pool `fleet`. |
| Shared Research agenda | `agenda_effects.rs` | Stage only newly inserted home-system tokens, pool `reinforcements`. |
| Wormhole Recon agenda | `agenda_effects.rs` | Stage only newly inserted tokens, pool `reinforcements`. |
| Naalu Foresight | `factions/naalu.rs` | Immediately opens the typed event through its live timing resolver, pool `strategy`. |

Direct insertions in `leaders.rs`, `movement.rs`, `status.rs`, `tactical.rs`, `factions/arborec.rs`,
`factions/naalu.rs`, `factions/argent.rs`, and the remaining `action_cards.rs` matches are test
fixtures or assertions, not additional game placement paths. This audit leaves the two explicitly
out-of-scope integration points above to the parent agent.

## Implementation and tests

`tokens::stage_command_token_placed` delegates to `supply::stage_event`, preserving the no-module
no-op. The reinforcement placement helper stages only after it successfully puts a token on the
board. Naalu's agent remains test-only until the parent confirms both out-of-scope sites and event
flush points are covered.

Tests added in `factions/naalu.rs` exercise a non-activation reinforcement placement through
`supply::flush_staged_events` and an armed resolver, declining, an exhausted agent, and two
placements where the agent can be used only once. A no-module test for the staging helper is in
`tokens.rs`. No Cargo test was run; the parent requested a single shared build.

## Status

Not yet claimed complete. The parent still needs to stage the combat-retreat placement, verify the
existing game activation is the only activation event, and ensure staged events flush after every
listed source. After that audit, enable `naaluagent-te` in live timing registration and run focused
tests.

## Root integration 2026-10-04

Retreat's reinforcements token now stages its placement in combat::retreat_to. Tactical activation already emits exactly once from Game; free actions without a token placement have no token event. Game::step always drains staged events before its next decision. With all audited live producers covered, root enabled the existing Naalu agent outside cfg(test), independently of the rejected earlier combined premature-enablement patch. Source tests still pending the next coordinated crate pass. No acceptance claim from unexecuted tests.
