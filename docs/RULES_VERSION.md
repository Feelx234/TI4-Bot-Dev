# Rules version (gating changed engine decisions)

A saved game is a log of decisions that the engine replays: after a restart, in the recovery
preflight of a deploy, on an offline resume. If a later build changes **what the engine offers or
does** at some point (an option that is no longer offered, a different order of events), a log that
answered the old offer no longer replays. `rules_version` is how such a change keeps old games
playable.

## What it is

`GameState::rules_version: u32` (crates/ti4-model/src/state.rs). Constants next to it:

- `RULES_VERSION` (now 4): what a **new** game plays. `GameState::new` sets it, so every way of
  creating a game (server, lobby, wasm host, selfplay, tests) gets the latest.
- `RULES_<CHANGE>` (now `RULES_INFILTRATE_FIX = 1`, `RULES_MENDOSA_SHIP = 2`, `RULES_SABOTAGE_TIMING = 3`, `RULES_LETNEV_FLEET_POOL = 4`): the first version with one gated change.

A state without the field is **version 0**, the legacy rules (what the alpha ran until 2026-10-16).

## How it is stored (the checksum rule)

`#[serde(default, skip_serializing_if = "is_zero_u32")]`: it is **not written while zero**. A legacy
`init.json` therefore serializes back to the exact bytes its envelope checksum covers, and the
`state_checksum` of its snapshots is unchanged. Never write a new field unconditionally into the
state, the init record or the envelope: that was the cause of the 2026-10-15 recovery regressions
(`action_turn`, `reaction_modes.json`; see commits f345235f, d284909c). A new game's `init.json`
records `"rules_version": 1` inside `initial_state`, and recovery reads it back from there (the
server replays from `init.json`'s `initial_state`, never from a fresh setup).

Wasm host (offline): `HostSave` (format 2, not bumped) and `Restart` carry `rules_version`
(left out for 0), and `Resume.rules_version`. `host_api::prepare` sets the state's version from the
resume (`None` = 0, legacy) and uses `RULES_VERSION` only for a start with no resume. So an
offline format 2 save written before the field resumes as legacy, and a new game's saves keep
theirs.

## Gated so far

Version 1 = the Infiltrate fix (`fix-infiltrate-offer`):

1. `action_cards::reaction_can_matter`: Infiltrate is offered only to the gaining player and only
   if a rival PDS or space dock can be replaced (`infiltrate_swaps`). Version 0: offered after
   every capture.
2. `InvasionWindow::finish_control_gain`: the previous holder's structures are destroyed after
   `PLANET_CONTROL_GAINED` (version 0: before it).
3. The Infiltrate effect and preview replace the rival's structures (version 0: the invader's own).

Version 2 = the Captain Mendosa fix (`fix-nightly-f2-mendosa`), `factions::saar`:

1. The boost mark is `seq|origin|type|damaged|count|bonus` and `agent_move_bonus` gives it to the
   first ship of that kind at the origin while no ship of the kind has left (version 0/1:
   `seq|origin|index|bonus`, matched to the list position, which shifts when an earlier ship leaves).
2. `agent_ships` offers equal ships (same type, damage, origin) once.

A game created at version 0 or 1 keeps playing the old behaviour.

Version 3 = Sabotage timing (`fix-sabotage-timing`, `RULES_SABOTAGE_TIMING`). Version 0/1/2: the
moment a player picked a card in a reaction window (e.g. "after you activate a system"), the card
was announced and every Sabotage holder was asked at once, before the other players of the same
window had declared anything. Version 3: **declaration round first**.

- In every reaction window except Sabotage's own (`ACTION_CARD_PLAYED`), choosing a card only
  *declares* it: the card is spent (hand to discard) and queued in `GameState::declared_card_plays`
  (not serialized while empty, so legacy checksums are byte-identical). The window keeps asking
  the seats in order, as before, until all have passed.
- Then, still inside the window and before the event resolves, the declared plays are announced
  in declaration order; each opens its own `ACTION_CARD_PLAYED` Sabotage window, offered to the
  holders in table order, never to the player themself and not for a Sabotage.
- Interpretation chosen (flag): "the declaration round" is one reaction window of one event; the
  Sabotage holder sees every declared card only as it is announced (it is not shown the whole
  list up front); a card declared earlier is not recalled if a later declaration makes it moot
  (e.g. two Public Disgraces); a card played in `ACTION_CARD_PLAYED` (Sabotage, Instinct Training)
  and component-action plays stay immediate. A Sabotage cancels only the one play it answers.

Version 4 = Letnev fleet pool (`fix-letnev-fleet-pool`, `RULES_LETNEV_FLEET_POOL`):

1. `tokens::TokenRedistribution::fleet_floor`: Letnev's redistribution (status phase, Warfare)
   also offers arrangements with fewer than two fleet tokens, down to none (the floor of two is a
   policy guard, not a rule; Armada allows pool + 2 ships per system). Version < 4: floor of two.

## Adding the next gate

1. Raise `RULES_VERSION` in state.rs and add `pub const RULES_<NAME>: u32 = <new>;`.
2. Branch the changed decision on `state.rules_version >= RULES_<NAME>`; keep the old code path
   verbatim for lower versions (see `infiltrate_legacy`, `destroy_rival_structures` call order).
3. Test both: the new behaviour at `RULES_VERSION`, the old one with `state.rules_version = 0` (or
   the previous version), and a replay of a log recorded under the old version
   (`a_legacy_log_with_an_infiltrate_offer_replays_identically` in game.rs).
4. Run the recovery preflight (`deploy/public/deploy.sh --preflight-only --ref <head>`): every game
   that recovered before must still recover. Games in progress keep their stored version; only new
   games get the latest.

Do not use it for bug fixes that a replay cannot observe (a view, a label, a preview that is not
hashed), only for changes of offers, results or event order.
