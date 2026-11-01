# String-engine bugs the core does NOT copy (found by the whole-game shadow)

`tools/engine_core/native_play.sh --shadow` plays the core and the string side by side. A difference whose cause is the string's is recorded here, with the
reason the core is right (or cannot copy it), the games it shows in, and what a decision would have to do. The string is not changed (user rule). The list
of bugs the core copies ON PURPOSE is `docs/ENGINE_KNOWN_ISSUES.md` section B.

## S1. The reaction windows of the action phase are offered in the SEATING order (rotated from the active seat), not in the live initiative order

* What the string does: `Game::new` builds its timing `Resolver` once, with `state.initiative_order()` (`game.rs` `Resolver::new(state.initiative_order(), ..)`). At
  that moment nobody holds a strategy card, so every seat has initiative 99 and the order is the seating order. The resolver is never refreshed
  (`Resolver::initiative_order` is documented as "fixed at `Game::new`, never refreshed"; `sync_timing_context` sets the seating order, the active seat and the
  speaker only). `Resolver::player_order` of an action-phase window rotates that list to start at the active seat. So every "after SYSTEM_ACTIVATED"
  style window asks the seats in seat order from the active one, whatever the strategy cards are.
* What the core does: `sync_clock` / `chain::sync_initiative` / `live_common::driver_of` hand the resolver the position's initiative order
  (`query::seats::initiative_order`: the lowest initiative among the seat's cards, ties by seat), rotated from the active seat the same way. This is what the
  string's own doc comments say the windows do ("initiative-ordered timing windows", `Resolver::new`'s "action-phase resolver with the supplied initiative
  order").
* Where it shows: 6-player `saar` seed 1 of the shadow sweep, step 38: seat `c` activates a system, seat `a` (Saar agent) and seat `e` (L1Z1X agent) both
  react. The string asks `e` first (seating order from `c`: c, d, e, f, a, b); the core asks `a` first (card initiative). The script of the core's answers then
  meets the wrong question in the string ("script wanted `leader:l1z1x:...` but a was offered `leader:saar:...`"). Any table where two seats react to one
  event and the card-initiative order differs from the seating order rotated from the active seat shows it; the Nomad/Saar agent differences of
  `docs/KM4_RESUME.md` section 2 are the same class.
* Why the core does not copy it: the string's order is not a function of the position. It is the initiative order at the moment the `Game` object was
  BUILT. A game replayed from its first decision (the server) has the seating order; a `Game` built around a position that already holds the cards (every
  lockstep test, a snapshot) has the card order, and the string and the core agree there. The core cannot know which one the string had.
* What a decision needs: either fix the string (`Resolver::set_initiative_order` from `sync_timing_context`; this changes the question order of recorded
  games, so behind a `RULES_VERSION`), or give the core the seating order for replays of old versions (a `rules_version` gate in `sync_clock`).
  Owner: the user (rules ruling: LRR "when multiple players resolve abilities at the same time" -- active player first, then the other seats in the order the
  rule names).

## Harness limits found on the way (the shadow's, not the engines')

* H1. A game the string has not stepped yet carries its galaxy with `hyperlane_lanes` off (`laws::apply_to_galaxy` runs at the start of every step). The core
  was loaded from that galaxy and found no route through a printed lane of a hyperlane map (`done_moving` alone). `NativeGame::from_game` applies the laws to
  its copy of the galaxy first. A product loader (D2-8) that builds a core from a never-stepped `Game` needs the same call.
* H2. The string sometimes takes a step that asks and says nothing (the scoring window of an event nobody scores, ahead of the phase flip) where the native
  loop has a step of its own with no question: the shadow steps the string again with the same answers (up to four times) instead of comparing the native
  group against the empty step.
