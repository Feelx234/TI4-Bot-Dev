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
* A third form (multiask-1-fix, yssaril 5 players seed 1, step 606): when another leader unlocks at the same bookkeeping, both announce an unlock at the index and the
  string's extra one shifts the core's by one (`LEADER_UNLOCKED:xxchahero` before `LEADER_UNLOCKED:jolnarcommander`). `s5_extra_unlock` takes it: the string's unlock
  at the index is one the core never announces, and the core's event there is the string's next one.
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

## S8. Nano-Forge is a relic with an ACTION the string never offers or plays

Found by the probe of the 20 golden dual games (lane core-dual6, 2026-11-05; games 4, 9 and 19; `docs/DUAL_REST_RESUME.md`). Printed card (`relics.json`, alias `nanoforge`):
"ACTION: Attach this card to a non-legendary, non-home planet you control, its resource and influence values are increased by 2 and it is a legendary planet. This action cannot
be performed once attached."

* What the string does: the relic is drawn, held and counted, and nothing else. `relics::action_aliases()` does not list it, `use_relic` has no arm for it (`UseArm::Unresolved`),
  so `action_relics_text` filters it out of the list of relics with an action and the menu never shows `relic|nanoforge`.
* What the core does (`docs/NANOFORGE_RESUME.md`): `query::relic::action_relics` offers it while the holder controls a planet that is not a printed or seated home, not legendary
  (printed or by the attachment) and not destroyed; the window asks the planet (no decline once chosen), attaches the `nanoforge` attachment (+2 resources, +2 influence) and the
  planet counts as legendary (`query::board::is_legendary`, read by Stellar Converter's targets and the two legendary-planet objectives).
* Who is right: the core (the card text above; the user decision of 2026-11-03/04 is to port it).
* The harness recognises it on the DUAL run only (`dual::note_known_string_bug`, counted as a COMPARED window `<label> [known string bug S8: ...]`, never a bail and never a
  difference, so a zero-bail run does not fail on it): `relics::action_relics` drops `nanoforge` from the core's list before the comparison, and `menu_dual` drops
  `Opt::UseRelic(NanoForge)` from the core's relics source of the action menu. The core backend (the product) keeps the relic. Tests:
  `relics::tests::nano_forge_is_cut_from_the_cores_list_on_the_dual_run_only`.
* What a fix of the string would need (not done, user rule): an arm in `use_relic` and an entry in `action_aliases` (the attachment record and its +2/+2 exist); it adds an option
  to the menu of recorded games, so behind a `RULES_VERSION`.

## S9. Rearmament Agreement, For: the string places the PLAIN mech (and ignores the reinforcements) for every seat but the Naaz

Found by lane core-jolnar-mech-1 (2026-11-06, `docs/JOLNAR_MECH_RESUME.md`): bench-v2 seed 69 (4p 4pHyperlanes arborec,yin,jolnar,xxcha), round 5, the speaker
decides Rearmament Agreement For; a plain `mech` appears on Jol, Nestphar and Archon Ren (Jol-Nar, Arborec, Xxcha homes) and stays plain until a technology gain
or Rin re-forms it. Printed card (`agendas.json`, alias `rearmament`): "For: Each player places 1 mech from their reinforcements on a planet they control in their
home system."

* What the string does (`agenda_effects::effect_rearmament`): pushes `UnitTypeId("mech")` for every seat but the Naaz (`naaz_mech`), with no supply check for
  them; the next `apply_unit_upgrades` of that seat (any technology gained, Rin's swap) turns it into the faction's own form.
* What the core did until this lane: the same (`FactionlessMech`, copied on purpose by lane core-lent-gaps, `LENT_GAPS_RESUME.md` section 3).
* What the core does now (`rules/agenda/effects/placements.rs::mechs`): the seat's own form through `fx_strategy::place_for` (`placed_unit`: J.N.S. Hylarim
  for the Jol-Nar, the Letani Behemoth for the Arborec, ...), refused when the reinforcements hold none (31.4) or a faction effect forbids it; the Naaz path is
  unchanged. Without a unit roster in the facts the effect is `unsupported`. Test: `agenda_table_effects2::rearmament_for_places_each_seats_own_mech_from_its_reinforcements`.
* Who is right: the core (the reinforcements of a faction with its own mech hold that mech; there is no plain mech in them).
* The harness: the dual window (`dual_window::agenda_effect`) cuts the core's side to the string's form (a roster whose mechs are plain but the Naaz's,
  `technology_core::string_rearmament_roster_for`) and counts the window as `agenda_effects::resolve_with [known string bug S9: ...]` (compared, never a bail);
  a position where a non-Naaz seat's reinforcements hold no mech bails with `known string bug S9: ... the reinforcements do not hold` (the string would place a
  fifth one). The whole-game shadow reports a difference in a step whose string events hold `AGENDA_EFFECT_RESOLVED:rearmament` as `(known string bug S9)`
  (`native_play::effect_bug`) and cannot play on past it.
* What a fix of the string would need (not done, user rule): `placed_unit_id(.., "mech")` and `supply::allowed` in `effect_rearmament`.

## S10. Politics places the two agendas it looked at one at a time, in the order they were drawn

Found by lane core-multiask-1 (2026-11-06, `docs/MULTIASK1_RESUME.md`). Printed card (Politics primary, rule 66.2): "Look at the top 2 cards of the agenda
deck. Place each card on the top or bottom of the deck in any order." Six final orders.

* What the string does (`strategy_cards::politics`): asks "place {agenda} where" for the first card drawn, then for the second, each top or bottom. Both on top
  puts the second above the first; both on the bottom puts the second below the first. The other two orders (first above second on top, first below second
  at the bottom) cannot be reached.
* What the core does now (`rules/strategy_primaries.rs::Politics`): ONE question over the six orders (`Ask::PoliticsAgendas`, `Opt::Stack`). Test
  `strategy_primaries::politics_reaches_every_order_of_the_two_agendas_the_rules_allow` (4 of 6 before the conversion, 6 now).
* Who is right: the core.
* The harness: the shadow plays the four shared orders to the string as its two answers (`multiask::politics_answers`); a game whose answer is one of the
  other two stops there as `(known string bug S10)` (`native_play::s10_politics_order`). The rule bot only picks the four shared orders (same verdict per agenda
  as before); the shadow's random seat picks any.
* What a fix of the string would need (not done, user rule): one question over the six orders in `strategy_cards::politics`.

## Harness limits found on the way (the shadow's, not the engines')

* H1. A game the string has not stepped yet carries its galaxy with `hyperlane_lanes` off (`laws::apply_to_galaxy` runs at the start of every step). The core
  was loaded from that galaxy and found no route through a printed lane of a hyperlane map (`done_moving` alone). `NativeGame::from_game` applies the laws to
  its copy of the galaxy first. A product loader (D2-8) that builds a core from a never-stepped `Game` needs the same call.
* H2. The string sometimes takes a step that asks and says nothing (the scoring window of an event nobody scores, ahead of the phase flip) where the native
  loop has a step of its own with no question: the shadow steps the string again with the same answers (up to four times) instead of comparing the native
  group against the empty step.
* H3. (multiask-1-fix, naaz 4 players seed 1, step 258.) The string ASKS a question of one option that the native engine takes without asking (Public Disgrace's
  "choose a different strategy card" with one other card on the mat). The follower answered it with the next entry (the action menu's answer), the string refused
  it and restored the first choice, so the seat kept `te6warfare` where the core dealt it `te4construction` (the rule: "must choose a different card, if able").
  The follower takes a question of one option itself when the next entry does not answer it (`LONE_TAKEN` in `answered_by`: no question of the group).
