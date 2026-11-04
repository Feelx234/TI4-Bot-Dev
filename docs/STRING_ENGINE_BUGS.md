# String-engine bugs the core does NOT copy (found by the whole-game shadow)

`tools/engine_core/native_play.sh --shadow` plays the core and the string side by side. A difference whose cause is the string's is recorded here, with the
reason the core is right (or cannot copy it), the games it shows in, and what a decision would have to do. The string is not changed (user rule). The list
of bugs the core copies ON PURPOSE is `docs/ENGINE_KNOWN_ISSUES.md` section B.

## S1. The reaction windows of the action phase are offered in the SEATING order (rotated from the active seat), not "active player first, then initiative order"

**User ruling (2026-11-01): the ACTIVE player first, then the other players in INITIATIVE order.** That is the rule the core implements
(`Resolver::player_order`: for the strategy and agenda phases still the speaker and the seating order). The string stays untouched and is wrong whenever
the seating order rotated from the active seat differs from that order, i.e. whenever two seats react to one event and the active seat is not the one
with the lowest initiative or the others' clockwise order is not their initiative order.

* What the string does: `Game::new` builds its timing `Resolver` once, with `state.initiative_order()` (`game.rs` `Resolver::new(state.initiative_order(), ..)`). At
  that moment nobody holds a strategy card, so every seat has initiative 99 and the order is the seating order. The resolver is never refreshed
  (`Resolver::initiative_order` is documented as "fixed at `Game::new`, never refreshed"; `sync_timing_context` sets the seating order, the active seat and the
  speaker only). `Resolver::player_order` of an action-phase window rotates that list to start at the active seat. So every "after SYSTEM_ACTIVATED"
  style window asks the seats in seat order from the active one, whatever the strategy cards are.
* What the core did before the ruling (integration 15): the position's initiative order (`query::seats::initiative_order`) ROTATED to start at the active seat
  (the string's rotation over a live list). That agreed with the ruling only when the active seat's successors in the list are the others in order. Now
  (lane P): `[active] ++ (initiative order without the active seat)`. Test: `resolver_basics::an_action_phase_window_offers_the_active_player_first_and_the_others_in_initiative_order`.
* Where it shows: 6-player `saar` seed 1 of the shadow sweep, step 38: seat `c` activates a system, seat `a` (Saar agent) and seat `e` (L1Z1X agent) both
  react. The string asks `e` first (seating order from `c`: c, d, e, f, a, b); the core asks `a` first (card initiative). The script of the core's answers then
  meets the wrong question in the string ("script wanted `leader:l1z1x:...` but a was offered `leader:saar:...`"). The Nomad/Saar agent differences of
  `docs/KM4_RESUME.md` section 2 are the same class.
* Lane R (2026-11-01): the same list is used by EVERY window outside the strategy and agenda phases, not only the action phase's. In the status phase nobody is
  active, so the string's list is the plain seating order of `Game::new` (`player_order`: `first` is `None`, `sequence.clone()`). The cases found: a card played in
  the status phase (`when STRATEGY_CARDS_WOULD_RETURN` -> `ACTION_CARD_PLAYED:when`: mahact 6p seed 1) and a card played by the active seat inside its own window
  (`ACTION_CARD_PLAYED:when` after `reaction:mahact:SYSTEM_ACTIVATED:after`: mahact 4p seed 1, the id of the offer being `technology:xxcha:it:...`).
* **User ruling (2026-11-03) for a window with NO active player in the status phase: clockwise from the seat AFTER the speaker, the speaker LAST** (the speaker gets the
  biggest advantage). This replaces the initiative order the core took there before (LRR 1.19 reading dropped). Implemented in `Resolver::player_order` (integration 18):
  `phase == Status && active_player == None` -> the seating order starting after the speaker, the speaker last; test
  `resolver_basics::a_status_phase_window_without_an_active_player_goes_clockwise_after_the_speaker_and_the_speaker_last` (fails on the old code: the initiative order).
  The strategy and agenda phases (speaker first) and the action phase (active player first, then initiative order) are unchanged. **The string still differs**: its
  list for such a window is the seating order from the FIRST seat of `Game::new`, whoever the speaker is, so the two agree only when the speaker is the last seat
  clockwise. It stays a known string bug (S1); the dual adapters (`reactions_core::resolver_of`, `kernel_sync::shim::replay`) hand the core the string's list as the
  seating order with its last seat as speaker, which is the string's order, so the lockstep still compares the rest of the window.
* Lane S (2026-11-03): the same list also orders two seats whose abilities answer ONE event when the second seat's ability is the only optional one of its
  window: the string then asks the ability's own prompt (the Mentak seat's "Suffi An: exhaust to draw 1 action card", options `exhaust` / `decline`), not the
  window's offer. yssaril 6p seed 2 (step 1294): active seat `e` (Arborec), `PILLAGE_USED:after`; the string asks `f` (Mentak) first (seating order from `e`: e,
  f, a, ...), the core asks `a` (Yssaril, holding Ssruu's copy of the Mentak agent) first (initiative). The harness recognises it
  (`s1_other_seat_first`: the core's question is an ability offer of seat X, the string's question at the same place is of seat Y, and the string's order puts Y
  before X where the ruling's puts X before Y).
* The shadow harness (`native_play --shadow`) recognises it: a difference at a reaction question (same prompt asked of another seat, the same options in
  another order, or the script's reaction meeting another seat's reaction) while the string's order differs from the ruling's (in the status phase without an active seat: the ruling of 2026-11-03) is reported as
  `KNOWN STRING BUG ... (known string bug S1)`, counted on its own and left out of the exit code (the game cannot go on past it).
* Why the core does not copy it: the string's order is not a function of the position. It is the initiative order at the moment the `Game` object was
  BUILT. A game replayed from its first decision (the server) has the seating order; a `Game` built around a position that already holds the cards (every
  lockstep test, a snapshot) has the card order, and the string and the core agree there. The core cannot know which one the string had.
* What a fix of the string would need (not done, user rule): `Resolver::set_initiative_order` from `sync_timing_context` and the ruling's order in
  `player_order`; this changes the question order of recorded games, so behind a `RULES_VERSION`.

## S2. The windows of the flip into a strategy phase (round 2 and later) are offered in the order of the phase BEFORE, not from the speaker

Found by lane R in the 288-game set (about 8 games: letnev 6p seed 4, l1z1x 6p, yin 6p, yssaril 6p, titans 6p, arborec 5p, muaat 6p, xxcha 5p): the first
difference sits in the `phase` step, at `STRATEGY_PHASE_BEGAN:after` or at an `ACTION_CARD_PLAYED:when` window opened by a card played in it.

* What the string does: `Game::step` (the round flip, `game.rs` near `self.emit_typed("STRATEGY_PHASE_BEGAN", ..)` of the later rounds) emits the typed event
  without `sync_timing_context` first (the first round's branch does call it). The resolver's phase is still the phase before (status), its active player
  is nobody, so `player_order` returns the initiative list of `Game::new` as it is: the seating order from the first seat.
* What the core does: `Resolver::player_order` for the strategy phase: the speaker, clockwise (LRR 1.20: the strategy phase orders by the speaker).
* Who is right: the core. The two agree only when the speaker sits first.
* The shadow harness recognises it in a `phase` group (`s2_orders_differ`: the seating list against the order from the speaker) and reports
  `KNOWN STRING BUG ... (known string bug S2)`.
* What a fix of the string would need (not done, user rule): `sync_timing_context` before the emission of `STRATEGY_PHASE_BEGAN` at the round flip; it changes
  the question order of recorded games, so behind a `RULES_VERSION`.

## S3. A free tactical action takes its "notes at the start of the tactical action" AFTER the activation returned a Support for the Throne

Found by integration 19 (empyrean 6 players seed 5, step 1063; the `--seeds 8` Z-gate command). Betray a Friend (secret objective): "Win a combat against a player whose
promissory note you had in your play area at the start of your tactical action."

* What the string does: a tactical action from the menu snapshots `note_holdings` when `TACTICAL_ACTION_BEGAN` is announced, i.e. BEFORE the activation (and before
  `spend_support_on_activation` returns a Support for the Throne). A FREE tactical action (`begin_free`, game.rs near `SUPPORT_FOR_THE_THRONE_RETURNED`) spends the Support
  first and snapshots afterwards. In the same turn the same objective is scored or not depending on how the action began.
* What the core does: the snapshot is taken when the action opens, before the activation, for both kinds (`open_tactical`, `open_free`, `begin_free`). The action starts
  with the activation; the Support is returned BY the activation, so the seat held the note at the start of its action: the feat is earned.
* Who is right: the core (the card says "at the start of your tactical action"; a free tactical action is a tactical action; one reading for both).
* The harness recognises it (`s3_feat_differs`: a state difference at `players[N].event_feats[..]` where only the core holds `won_against_a_note_holder`) and reports
  `KNOWN STRING BUG ... (known string bug S3)`.
* What a fix of the string would need (not done, user rule): snapshot before `spend_support_on_activation` in the free path; it changes a recorded game's scoring, so behind a
  `RULES_VERSION`.

## S4. The Yin hero's list of planets leaves out the OPEN Wormhole Nexus' planet

Found by integration 19 (yin 4 players seed 8, step 927); rewritten after the user ruling of 2026-11-04. Dannel of the Tenth (Quantum Dissemination): "Commit up to 3 infantry
from your reinforcements to any non-home planets and resolve ground combats on those planets."

* Ruling: the LOCKED Mallice (`lockedmallice`, tile `82a`) may not be targeted; the unlocked Mallice (`mallice`, once the Wormhole Nexus is open) may.
* What the string does: `yin::hero_spots` walks `Galaxy::system_ids()`, the systems on the hex grid. The Nexus sits beside the board: it has no hex, so no planet of it is
  ever offered. Right for the locked face; **wrong for the open one** (the string bug that remains).
* What the core does (`driver/turn/hero.rs::yin_spots`, since the fix of 2026-11-04): the planets of the systems on the hex grid in the order of the hexes (as the string),
  never `lockedmallice`, and `mallice` last once the Nexus is open (`NEXUS_UNLOCKED`) and in play. (The first version of the core listed the systems in the order of the
  system table, which differs from the string's hex order: found by the same game once S4 stopped hiding it, fixed in the same change.)
* The harness still recognises the open case (`s4_nexus_in_error`: the script wants `82a|mallice` and the string's offer has no `82a|..` entry) and reports
  `(known string bug S4)`. No shadow game of the Z-gate reaches it.
* What a fix of the string would need (not done): `hero_spots` over `system_ids()` plus the open Nexus' planet; it changes the offered options of recorded games, so behind a
  `RULES_VERSION`.

## S5. A secret objective scored for the first time does not run the leader unlock check at once

Found by integration 20 (merging `core-silent-fix-18`, whose B-X01 fix made the core unlock at the score). Rule 51.7: a leader unlocks the moment its condition is met; a hero
unlocks with the third scored objective, whatever the kind of the objective.

* What the string does: `objectives::ScoringWindow::select` sends a secret in the scorer's hand to `secrets::award`, which scores it and runs NO unlock check; only a public
  (or already revealed) objective goes through `objectives::award` and its `award_tail` (`check_unlocks`). The leader is unlocked at the next check: the next turn start
  (`refresh_commander_unlocks`) or the status bookkeeping, where `LEADER_UNLOCKED:<leader>` is announced. (The silent-gap note SILENT_GAPS_18 B-X01 said the string's
  `award_tail` always runs; it does not for this one path.)
* What the core does: `windows/scoring.rs` unlocks for the scorer right after any score (no map, no event, as `award_tail`), a secret scored for the first time included.
  The `LEADER_UNLOCKED` event is announced only by the turn start and the bookkeeping, so with the core it is not announced at all for that leader (it is unlocked already).
* Who is right: the core (51.7; the comment of `award_tail` says it: a hero unlocked by the third objective "must not wait for a status phase the game may never reach").
* The harness recognises it (`s5_unlock_differs`: the string announces `LEADER_UNLOCKED:..` where the core does not, or the leader's state is `unlocked` in the core and `locked`
  in the string) and reports `(known string bug S5)`. The shadow cannot play on past it.
* A fix of the string (one line after `secrets::award` in `select`, `leaders::check_unlocks(state, content, sources, None, &player)`) was written and tested
  (`a_secret_scored_for_the_first_time...` passed) and **not kept**: it moves the recorded legacy games (`golden::tests::the_smoke_subset_reproduces`, g020 step 795:
  the state at the start of the step differs from the recording). Behind a `RULES_VERSION` it would be one line. The legacy test
  `status_live_tests::a_secret_scored_for_the_first_time_leaves_the_hero_locked_in_the_string_s5` pins the string's behaviour (remove it with the fix).

## S6. Ixthian Artifact, For: the string destroys every ship without Sustain Damage everywhere and defers the research

Alpha report fb_1793193205 ("the artifact which was resolved in the agenda phase did nothing"; rules version 29 game, a roll of 6-10: the effect was announced as deferred and
changed nothing). Printed card: "The speaker rolls 1 die. If the result is 6-10, each player may research 2 technologies. If the result is 1-5, destroy all units in Mecatol Rex's
system, and each player with units in systems adjacent to Mecatol Rex's system destroys 3 of their own units in each of those systems."

* What the string does (`agenda_effects::effect_artifact`): a 6-10 returns `Effect::Deferred` ("the windfall researches two technologies each, which needs a payer") and nothing
  happens; a 1-5 removes every ship that cannot sustain damage from EVERY system of the board and touches no ground force, no structure and no ship that sustains.
* What the core does (rules version 36, `rules/agenda/effects/crater.rs`): 6-10, each player in speaker order may research up to two technologies for free (a child `Research::agenda`
  window; prerequisites apply; the question carries the agenda as its source and can be declined); 1-5, every unit in the Mecatol Rex system is destroyed (ships, ground forces,
  structures, whoever owns them) and, system by system (system order) and player by player (speaker order), a player with more than 3 units in a system beside Mecatol Rex chooses
  the 3 that are destroyed (`Ask::Crater`, one question each; a choice among units that are all alike is not asked), a player with 3 or fewer loses them all. The effect is
  always `Resolved`.
* Who is right: the core (the card text above).
* The harness: the dual window (`dual_window::agenda_effect`) bails on Ixthian Artifact, For and counts it as a known string bug S6, so the lockstep and the dual runs do not compare
  it; the whole-game shadow reports a difference in the step that announced `AGENDA_EFFECT_RESOLVED:artifact` / `AGENDA_EFFECT_DEFERRED:artifact` as `(known string bug S6)`
  (`native_play::effect_bug`) and cannot play on past it.
* What a fix of the string would need (not done, user rule): the research as a child question, the crater question, and the Mecatol Rex system; it changes the questions of
  recorded games, so it is behind a `RULES_VERSION` (36, `RULES_CRATER_PARLEY`).

## S7. Parley returns the last committed unit only; the invader lands again

Alpha report fb_1792786598 ("Why did I lose Lodor even though I played Parley"; rules version 26 game). Printed card: "After another player commits units to land on a planet you
control: Return the committed units to the space area."

* What the string does (`action_cards::parley`, `invasion.rs::commit_one`): the engine commits ground forces ONE per decision and announces `UNITS_COMMITTED` after each; Parley
  reads `GameState::last_committed_unit` and returns that single unit. The invader is asked to commit again and lands the rest, so the card cancels nothing when more than one unit
  is committed (and a dual-form unit, recorded in its ship form and landed in its ground form, is not found on the planet at all).
* What the core does (rules version 36, `rules::invasion::commit::parley` and the invasion machine's `cancel_landings`): the card returns the unit of the landing it answers (found
  in either of its forms) and flags the landing as cancelled; the machine then returns every unit it landed on that planet in this commit step (`Invasion::step_commits`), drops the
  planet from the committed ones and ENDS the commit step (the units are committed together, so a cancelled landing cannot be made again; what was landed on other planets stays and
  fights). The holder plays it at the landing of his choice, a decline of one window leaves the next landing's window open as before.
* Who is right: the core (the units are committed simultaneously; "the committed units" are all of them).
* The harness: the whole-game shadow reports a difference in a step that answered `parley` as `(known string bug S7)` (`native_play::effect_bug`).
* What a fix of the string would need (not done, user rule): the per-step list of landed units and the end of the commit step in `InvasionWindow`; behind a `RULES_VERSION`
  (36, `RULES_CRATER_PARLEY`).

## Harness limits found on the way (the shadow's, not the engines')

* H1. A game the string has not stepped yet carries its galaxy with `hyperlane_lanes` off (`laws::apply_to_galaxy` runs at the start of every step). The core
  was loaded from that galaxy and found no route through a printed lane of a hyperlane map (`done_moving` alone). `NativeGame::from_game` applies the laws to
  its copy of the galaxy first. A product loader (D2-8) that builds a core from a never-stepped `Game` needs the same call.
* H2. The string sometimes takes a step that asks and says nothing (the scoring window of an event nobody scores, ahead of the phase flip) where the native
  loop has a step of its own with no question: the shadow steps the string again with the same answers (up to four times) instead of comparing the native
  group against the empty step.
