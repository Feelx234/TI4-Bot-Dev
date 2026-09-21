# Operator bug batch, 2026-09-21 — what was reported, what was actually wrong

Authority: the operator's message of 2026-09-20 — "work through as many bugs as possible
independently … you have freedom to do as necessary in the repo to fix the bugs described in
`plans/BUG_PLAN_2026-09-20.md` as well as the following less formal list". Independent-review
requirement waived by the operator in the same session ("the requirement for independent reviews is
waived until further notice"); each fix below is therefore self-reviewed, and the finding trail is
this file rather than a second reviewer's sign-off.

Normative sources used: the card text in `crates/ti4-content/content/` (the corpus is the accepted
source for this project — `plans/MASTER_PLAN.md`), the LRR numbers already cited in the files
touched, and the package specs in `plans/BUG_PLAN_2026-09-20.md`. Historical Python reference:
**not used** (AGENTS.md — parity is not an acceptance criterion since the 2026-08-21 decision).

Reproduction discipline: every fix below was written test-first, run red against the unmodified
engine, then green. Where a fix is one line, the red run is recorded by editing the line back out
and re-running, which is what the "red before" lines below are.

---

## F-01 — Hacan's agent is never offered (operator: "hacan agent is broken, never offered")

**Symptom.** A Hacan seat holding a readied Carth of Golden Sands is never offered it as a component
action, so the leader cannot be used in play.

**Root cause.** `leaders::is_action_window` (crates/ti4-engine/src/leaders.rs:389) decided whether a
leader may be offered on its owner's turn by reading the corpus `abilityWindow`:

```rust
window.starts_with("ACTION")
    || window.eq_ignore_ascii_case("during the action phase")
```

The corpus prints the Prophecy of Kings agents with the colon the card frame has on it:

```json
{ "id": "hacanagent", "abilityWindow": "During the action phase:" }
```

`"During the action phase:"` does not start with `ACTION` and is not equal to
`"during the action phase"` — the trailing colon is the whole difference. `component_actions` filters
on this predicate before it looks at anything else, so the leader was filtered out of the offer list
for every PoK agent, for every faction, always. The four heroes that work print `"ACTION:"`, which
is why the action-phase leaders looked delivered.

**Why the "leader fix" commit did not fix it.** `16f389a` (LEADER-FIX-001, "Deliver action-phase
leaders…") put `hacanagent` into `action_leader_delivered` and reported "44/44 leader tests". Its
Hacan test is `hacan_agent_can_replenish_another_player`, which calls `use_scripted` — that is
`use_leader` directly, with the leader named by id. `use_leader` is the *use* path; the *offer* path
(`component_actions` → `is_action_window`) is upstream of it and was never exercised by that test, or
by any test in the file: no test asserted that a leader the engine claims to deliver actually reaches
an offer list. So the commit was true of the code it tested and useless for the symptom, because the
symptom was in the half it did not test. There are two independent conditions for "the player is
asked": the engine can resolve it, and the corpus says its window is the action phase. One of them
was asserted, and the assertion was about the wrong half.

**Fix.**
- `is_action_window` compares the printed window with the frame's trailing colon removed (and
  surrounding whitespace), which is a property of how the corpus spells the text rather than of any
  one card.
- `action_leader_delivered` becomes membership of a `DELIVERED_ACTION_LEADERS` array, so the set is
  enumerable and can be held against the corpus.
- New `can_resolve_action` arm for `hacanagent`: both branches are commodity-shaped (gain 2, or
  replenish another seat), so with every hand at its cap the use burns the card for nothing and is
  not offered. Previously `_ => true`.

**Tests** (`leaders::tests`):
- `the_hacan_agent_is_offered_on_its_own_turn` — the offer path, on the real corpus. Red before:
  `Carth of Golden Sands was not on the offer list: []`.
- `every_delivered_action_leader_prints_an_action_window` — walks `DELIVERED_ACTION_LEADERS` and
  asserts each id's printed window is an action window. This is the guard that could not be written
  while the delivered set was a `matches!`, and it is what stops the class, not the instance. Red
  before.
- `an_agent_with_no_commodities_to_gain_is_not_offered` — the new precondition, both directions (all
  seats capped: not offered; one seat below cap: offered).

**Blast radius.** Of the six in-scope factions' leaders, `hacanagent` is the only one whose window is
newly recognised, and only if it is also in the delivered set — so the offer set grows by exactly one
option, for one faction, when Hacan holds the agent below its commodity cap. Other PoK agents with
`"During the action phase:"` (`nekroagent` and friends) stay out: they are not in the delivered set,
which is a separate, older decision.

---

## F-02 — Maxis Central Control offers almost nothing (operator: "maxis central control does not offer all eligible planets")

**Symptom.** Taking Faunus and passing offers a near-empty planet list.

**Root cause.** `legendary::maxis_candidates` enumerated `for (system, board) in &state.board`.
`GameState::board` is a `BTreeMap` that is written the first time anything touches a system — a unit
placed, a planet taken, a token put down — while `GameState::system()` hands out a fresh empty
`SystemState` for a system the map holds and the board does not. So `state.board` is a record of what
has *changed*, and the card is about planets that contain no units: the ones least likely to have
earned an entry. On a table where a handful of systems had been visited, that is what was offered;
every untouched planet on the map — the whole point of the ability — was invisible.

The same shape of mistake is worth naming once: `for … in &state.board` reads as "for every system",
and is not. Eighteen places in `ti4-engine` iterate `&state.board` this way, this one included (see
F-02-notes below); this package fixes the one the operator reported and records the rest.

**Fix.** `maxis_candidates` takes the map (`Option<&Galaxy>`) and enumerates the union of the map's
systems and the board's, as a `BTreeSet` so the offer order does not depend on which of the two
happened to be consulted first. The board still contributes, for systems a map does not hold — a
Fracture system brought into play — and for a game with no map at all, which the unit tests are. The
four exclusions (homeworld, legendary, space station, Mecatol) and the units/attachments tests are
unchanged; they now run against `state.board.get(system)` being `None`, which means "nothing has
happened here", not "no such place".

**Test** — `maxis_central_control_offers_the_planets_nobody_has_touched`:
a one-ring hub of neutral tiles, Faunus in hand, and no board entry for the ring. Asserts (a) the
ring really is absent from `state.board` — so the test cannot drift into testing something else;
(b) the map produces candidates the board alone hid, strictly more than before; (c) one of them,
taken through the real ask/settle path (`pass` with a scripted table), changes `planet_control`.
Red before the fix (`[]`).

Existing Maxis tests pass unchanged by giving the new parameter `None` — they pin the board-only
cases deliberately, and this package does not silently re-scope them.

**F-02-notes — the same shape elsewhere, recorded not fixed.** `action_cards.rs` (8 sites),
`agenda_effects.rs:144`, `laws.rs:377`, `movement.rs:59`, `production.rs:661`, `relics.rs:556/735`,
`strategy_cards.rs:644`, `technology.rs:637`, `transactions.rs:170`, `choice.rs:1063`,
`objectives.rs:285`, `fracture.rs:308`. Each needs its own rule text before it can be called wrong:
most are asking "which systems has something happened in" and are correct — a relic pinned to a
system, a law that purges a system, production from controlled planets. `transactions::presence`
(:170) is the one to look at next: it is the reach test for 94.x neighbourhood, and a system holding
a controlled planet always has a board entry, so it is probably fine — but "probably" about reach is
how BUG-02 happened. A package of its own, with a failing test, not a drive-by.

---

## Commands and results (batch so far)

```
cargo fmt -p ti4-engine                                  # clean
cargo test  -p ti4-engine                                # 1347 passed / 0 failed (lib)
                                                         # + all integration targets green
cargo clippy -p ti4-engine --all-targets                 # no warning points at leaders.rs or
                                                         # legendary.rs; the pre-existing set
                                                         # (game.rs, combat.rs, invasion.rs …) is
                                                         # unchanged in count
```

Red-before evidence: recorded per test above, produced by editing the fixed line back out (not by
deleting the test), then restoring.

## Not touched

The four unrelated paths that were already dirty when this batch started
(`crates/ti4-mlp/examples/capture_offline_pilot.rs`, `crates/ti4-mlp/examples/offline_bc.rs`,
`plans/INDEX.md`, `scripts/publish_and_train_stopped_corpus.ps1`) belong to earlier in-flight work
and are left uncommitted. `target-cuda-repack/` is untracked build output and stays that way.

---

## F-01a — what F-01 broke, and the lesson about which suite to run

F-01 ran the engine suite (1347 green) and stopped. The workspace suite says more:

- `ti4-sim::behavior::tests::the_suite_reproduces_and_stays_within_the_recorded_bounds` — four
  action-mix shares left the recorded bounds, because a Hacan seat now uses a component action and
  every use lengthens the event stream the shares divide by. Re-baselined to **v43** through the
  versioned process, with the bisection that attributes it to F-01 alone (Maxis and the Nexus
  measured inert), the old/new table, and one uncomfortable finding: the suite was *already* outside
  its recorded `faction_differentiation` floor on the tree this batch started from. Details and
  numbers: `plans/evidence/M08-021.md`.
- `ti4-review`'s semantic golden — regenerated, with the diff inspected: the first divergence is at
  frame 18, where a Hacan seat is offered `component|leader|hacanagent` for the first time (14
  occurrences across 241 frames, none in the recorded file). The same golden is byte-identical with
  and without the Wormhole Nexus change, which is how F-03 is proven innocent of it.
- The three `ti4-bridge` golden binaries and `ti4-mlp`'s `smoke_refusals` fail in this checkout
  before any of this work and for a reason unrelated to it: `crates/ti4-bridge/tests/golden/` is not
  in Git, and `out/vocabulary/current.json` is a machine-local artifact. Recorded so a later run
  does not read them as damage from this batch.

Running only the crate you edited is how a delivered leader stays undelivered for eight days: F-01
was tested exactly as far as the file it changed. The workspace suite is the cheapest thing that
knows about the offer set from the outside.

---

## F-03 — the Wormhole Nexus was not on the board (operator: "malice / wormhole nexus is not on the board")

**Symptom.** Mallice and its wormhole nexus do not appear on the map in either viewer, and no gamma
wormhole leads anywhere.

**Two separate causes, both real.**

1. *It was not in the game.* The Nexus is placed off the hex grid — `Galaxy::place_off_map`, because
   it has no hex and is reached only through the wormholes printed on it. That call lived inline at
   the end of `seating::build_board`, which is the Rust spiral board. The reviewer and the replayer
   do not use the spiral: they build from a captured Python map pool
   (`OpeningMap::PythonPool` → `MapPool::galaxy` → `Galaxy::placed`), and so does `Save54Captured`.
   Neither ever placed the Nexus. It is not in the pools either — all 1000 arrangements of
   `out/pools/full_np8_12_final.json` were checked: no `82` in any of them, and tile `39` appears in
   597 of them as the ordinary alpha wormhole. What was captured is the ring of hexes, and the Nexus
   is not one, so a pool cannot contain it; a map family that builds from a pool silently drops a
   tile that is always in play under Prophecy of Kings.
2. *Even when it was in the game, it was invisible.* `view::board_view` showed a nexus tile only if
   `state.board` held that system id — and `GameState::board` is written the first time a unit, a
   capture or a token touches a system (the same trap as F-02). The tile was therefore hidden until a
   player had already flown into the one place they could only find by looking at it. The HTML export
   carried the same rule in its own JavaScript, and `board_metadata` listed both faces for every
   session whatever the sources were, so a base-scope game would have shown a tile it never had.

**Fix.**
- `seating::place_wormhole_nexus(galaxy, content, sources)`: the rule, in one place, idempotent and
  quiet when the tile is already on the grid. `build_board` calls it (as before) and
  `ti4-training::rollout::seated` calls it for every family after the match that chooses the map, so
  the pool and Save-54 boards gain it. Gated on `Source::Pok`, as the rule is.
- `board_metadata` lists the two faces only when the game's map knows the tile
  (`wormhole_kinds`, whose own documentation says it is how a caller sees that an off-grid system is
  in play).
- `board_view` draws exactly one face — `82b` when `state.nexus_unlocked`, `82a` otherwise — because
  the face is a fact about the frame, and drawing both would stack two hexes in the lower-left
  corner. The export's JavaScript makes the same choice from `state.nexus_unlocked`.

**Tests.**
- `rollout::tests::every_map_family_has_the_wormhole_nexus_in_play` — three families, and the
  assertion that fails first is `save-54 captured: the Wormhole Nexus is not in play at all ({})`.
  Red before the fix, and it also pins the face: gamma alone while locked.
- `rollout::tests::the_nexus_is_a_prophecy_of_kings_rule_and_is_placed_once` — no Nexus for `BASE`,
  one for `POK`, and the second call changes nothing (so the family that already placed it does not
  turn setup into a `DuplicateSystem` error).
- `board_view::the_wormhole_nexus_is_drawn_before_anybody_has_been_there` — the tile is drawn with no
  `state.board` entry for it, one face only, and the face follows the latch.
- `board_view::a_game_whose_map_has_no_nexus_draws_none` — the other half of the rule.
- `lib::tests::metadata_invents_no_wormhole_nexus_for_a_map_that_has_none`, and
  `replay_metadata_carries_detached_special_areas_before_they_enter_play` updated to place the tile
  off the map rather than rely on metadata inventing it.

**Fixture the change had to move.** `board_layout::a_wider_window_moves_the_board_not_the_meaning`
asserted `tiles.len() == 37` — the example arrangement's hexes, with no Nexus because no pool board
had ever had one. It is 38 now, and the same test's special-area expectation turned out to be wrong
in a way nobody had ever seen: it expected the band offset unscaled (`- 61.0`) where the painter
scales it (`- 61.0 * scale`) for both special areas. Dead code, because no frame in that test had
ever had a nexus tile. The assertion is now the one the renderer implements.

**Not claimed.** That a player can *reach* the Nexus on any particular board. The locked face prints
gamma only, and the map filler deliberately excludes wormhole tiles (`neutral_systems`), so whether
a given generated map has a gamma partner at all is a property of that arrangement. Reachability on
a map that does have one is covered by `galaxy::an_off_map_system_reaches_its_wormhole_partners_both_ways`.
