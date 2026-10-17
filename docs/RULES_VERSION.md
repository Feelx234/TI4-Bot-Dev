# Rules version (gating changed engine decisions)

A saved game is a log of decisions that the engine replays: after a restart, in the recovery
preflight of a deploy, on an offline resume. If a later build changes **what the engine offers or
does** at some point (an option that is no longer offered, a different order of events), a log that
answered the old offer no longer replays. `rules_version` is how such a change keeps old games
playable.

## What it is

`GameState::rules_version: u32` (crates/ti4-model/src/state.rs). Constants next to it:

- `RULES_VERSION` (now 2): what a **new** game plays. `GameState::new` sets it, so every way of
  creating a game (server, lobby, wasm host, selfplay, tests) gets the latest.
- `RULES_<CHANGE>` (`RULES_INFILTRATE_FIX = 1`, `RULES_BRIBERY_PER_PLAYER = 2`): the first version with one gated change.

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

Version 2 = Bribery asked once per voter (`fb-bribery-per-player`; numbered 2 on this branch, a
merge agent renumbers it against the other branches):

1. `Game::step_vote`: `VOTES_CAST` ("after you cast votes", "after the speaker votes": Bribery,
   Distinguished Councilor) fires once per voter, when they have named their outcome or abstained.
   Version 0/1: after every answer of the vote (the outcome, each planet exhausted, the trade
   goods), so a Bribery holder was asked again after every planet.

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
