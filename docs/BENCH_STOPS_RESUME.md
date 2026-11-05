# Bench stops (branch `core-bench-stops`, from `core-bench-rv36` d4af7b436 on core-integration-24, 2026-11-05)

Task: the native verify of the six bench configs at rules version 36 (6 x 200 games, `docs/BENCH_RV36_RESUME.md`) had 61 native games ending in `engine_error`
where the string plays to a victory. Rule: no bails except Thunder's Edge and the dropped-feature list, so each cause is fixed in the core, one regression test per
root cause, `Unported` items ported (not removed).

## Result

* **0 `engine_error` in all 1 200 games** (6 x 200, all `victory`), native verify with the final binary (`/root/deploytmp/rv36/ver-stops2/*.json`; 10 + 13 + 8 + 11 + 12 + 7 = 61 fixed).
* **Decision streams unchanged**: of the 1 139 games that already played to a victory natively, 0 changed their hash (`tools/engine_profile/bench_rv36/unchanged_streams.py`
  on the old and the new verify output: `changed 0` x6). Rules version stays 36. Quick native fingerprint: see "Checks" below.
* The bench baselines are NOT re-recorded (`tools/rulebot/*.json` untouched). The native re-recording is prepared in a copy, see "Re-recording".
* The S1 hypothesis is only partly right, see the last section.

## The causes (61 games) and the fixes

| games | error | root cause | fix | regression test (red without the fix, checked) |
|---|---|---|---|---|
| 31 | `aftermath: Unported(HitsAddedToARoll)` | Valkyrie Particle Weave (Sardakk, "after making combat rolls during a round of ground combat, if your opponent produced 1 or more hits, you produce 1 additional hit") made the native ground round bail before the hit step. The hit step (`GroundRoundHits`) already adds the hits of the hook, both sides' final dice read first, assigned by the opponent; the bail in `driver/windows/invasion/fight.rs` was stale. | the bail and `Unported::HitsAddedToARoll` (and its text) are gone. | `ti4-engine-legacy` lib `invasion_live::tests::valkyrie_particle_weave_adds_its_hit_natively_in_lockstep_with_the_string_window` (12 games, Sardakk invading / defending / both; lockstep against the string window: questions, events, position). Was `the native run stopped: Unported(HitsAddedToARoll)`. |
| 15 | `Marks: mark without a text "NomadTcsNegotiating(..)"` | The Temporal Command Suite window writes the `NomadTcsNegotiating` mark while the transaction runs (the reach it grants). The core wrote `Atom::Num(1)`; the string's value is the text `true` (`nomad:tcs:negotiating:<a>:<b>` = `true`) and the mark's shape is `TRUE` (`Word::True`), so the position could not be shown to the decider while the window ran. | `ce_movement/nomad/query.rs::NEGOTIATING` (`Atom::Word(Word::True)`), used by the effect (`timing.rs`) and the reach test. | `ti4-engine` (next) lib `core_sync::tests::...::the_mark_of_a_temporal_command_suite_transaction_has_a_text` (stores the mark with the effect's value and expects `"true"`). |
| 7 | `aftermath: PendingDestructions` | A Direct Hit that destroys the Yin flagship (Van Hauge): the card's removal is announced in ONE pass after the discard (the string's `reactions::announce` does `for .. in take(pending)` once); the `SHIP_DESTROYED` windows of that pass stage the destruction of every other ship in the system (Van Hauge's ability), and these stay pending for the next announcement of the enclosing window (the string leaves them too). The core called any pending destruction left after the hook "unknown". | `Runner::after_play` returns whether a hook announced; `Announce` is unknown only when the table has NO announcement (`rules/action_cards/announce.rs`). | `ti4-engine-core-factions` lib `cards::tests::what_the_announcement_of_a_play_stages_in_turn_waits_for_the_next_announcement` (Yin seat, Van Hauge destroyed by a played card, cruiser and fighter of the other seat wait). The old unit test of the bail (a table with no announcement) still holds. |
| 4 | `ask: UnportedAftermath(CombatUnported)` | Three different causes, all in the fights of the Mahact hero (Airo Shir Aur, `FightToEnd` over the non-live machine): (a) bench 63: the non-live machine bailed at the first transition while destructions were pending (the live machine goes on and drains them at the string's four fixed points since M5; the non-live one never got that); (b) bench-v4 25 and 87: the texts of the step (`combat_live::hero_fight_types`) named only the SHIPS of the pairs, not the ground forces a Nekro Alastor in the destination brings into the fight (Mordred, infantry) or a Naaz Eidolon Maximum on a planet, so the machine stood at their roller; (c) bench-v2 23 (it appeared only after the other fixes): the pairs together named more than `MAX_TYPES` kinds, past which only the subsets of each seat's own kinds are roll groups, and the hero branch never named any seat's kinds, so no group existed. | (a) `rules/combat/machine.rs::transition`: no bail, drain in both modes; (b) planet-standing combatants of the destination (`fights_as_a_ship`) are named; (c) the kinds are named per seat. (b) and (c) in both copies of `combat_live` (`ti4-engine/src` and `ti4-engine-next/src/ties`). | (a) `ti4-engine-core-tests` `combat_fight::a_fight_run_to_its_end_goes_on_with_the_destructions_an_ability_staged`; (b) `combat_live::tests::the_texts_of_the_step_that_plays_airo_shir_aur_name_the_ground_forces_an_alastor_brings_into_the_fight`; (c) `...name_the_groups_of_each_seat_when_the_pairs_hold_many_kinds`. All three red on the old code. |
| 2 | `aftermath: MissingFact` | Direct Hit on a ship that stands on a planet (a Nekro Alastor ground force, a Naaz Eidolon Maximum fight the space combat from their planet). The effect derived "the exact ship" as the first sustained ship in the space area and bailed whenever the victim's combatants were not all in space, or the sustained ships of the type differed by galvanize token. The string removes the unit the sustain handoff names (`combat:sustain_target`: system, planet, owner, type, damage, galvanize), from its planet, else the first sustained combatant via `remove_combat_ship`. | `cards/mod.rs::direct_hit` follows the string: the handoff mark names the exact unit and its planet; without it `assign::remove_ship` removes the first sustained combatant from where it stands (planet or space). No bail left. | `ti4-engine-core-factions` lib `cards::tests::direct_hit_removes_the_ship_named_by_the_handoff_from_the_planet_it_stands_on` and `..._removes_the_first_sustained_ship_when_the_sustained_ships_differ_by_their_galvanize_token` (replaces the old bail test). A lockstep against the string for the planet case was tried and dropped: the dual cannot lend a state with a unit like that (`the typed state could not be lent`), so it passed vacuously. |
| 2 | `Board: planet entry in an untouched system "mallice"` | The Yin hero (Dannel of the Tenth) may target the open Mallice (user ruling 2026-11-04). The spot names the tile beside the map (`82a|mallice`), but the planet's entries are kept under the system it is printed in (`82b`, `MalliceNexus`); the landing and the control step touched only `82a`, so `82b` held a planet entry in a system nothing touched and the position could not be shown. | `rules/invasion/yin_hero.rs::home`: what lands on / is taken on a planet touches the planet's own system. The option text stays `82a|mallice` (a first version offered `82b|mallice`; that changed the decision streams of 18 games that never chose it, so it was dropped). | `ti4-engine-core-tests` `action_dispatch::dannel_of_the_tenth_on_the_open_mallice_leaves_its_entries_in_the_planets_own_touched_system` (checks that every planet with an entry sits in a touched system after the commit). |

Counts are the 61 of the task (31 + 15 + 7 + 4 + 2 + 2). Several games hid a second cause behind the first: bench seed 70 stopped on `PendingDestructions` first and would have met the Mallice entry later
(bench-v4 / bench-v5 seed 70 are the Mallice stops), and bench-v2 seed 23, which played to a victory on the first stop list, ended in the third kind of `CombatUnported` once the Mahact fights were changed.

Two of the fixes touch the shared machinery and were checked beyond their own test: `Runner::after_play` (two impls) and the non-live machine (also used by the string-led dual windows). The
legacy lockstep suites for combat, Mahact, dual windows, cards, trade and turns were run (see Checks).

## Checks

* Native verify of the six benches with the final binary: `/root/deploytmp/rv36/ver-stops2/` (`bench*.json`, `*.line`); 0 `engine_error`, 6 x 200 victories; compare with the
  string recording (`base35/`) is meaningless for identity (see the S1 section), compare with the old native output: `unchanged_streams.py ver36-out ver-stops2` -> `changed 0` x6.
* Quick native fingerprint: `native_play --quick` = sol 4p seed 1, 1165 steps, fingerprint **538dc68f23dddcc9**, ZERO GAP (unchanged).
* Test suites (`lockstep` profile, log `/root/deploytmp/rv36/finaltests.log`): `ti4-engine-core-factions` lib 159 passed; `ti4-engine` (next) lib 365 passed; `ti4-engine-legacy` lib filtered to
  combat / invasion_live / mahact / dual_window / cards / turn_live / trade_live / core_sync / yin / nomad: 1 303 passed; `ti4-engine-core-tests` all binaries: 2 016 passed, 1 failed
  (`state_basics::a_state_with_a_slice_missing_refuses_to_read_it_in_debug`: a `debug_assertions` test, fails in the optimised `lockstep` profile, not touched by this lane). One existing test changed:
  `combat_machine::staged_destructions_and_cancellations_...` asserted the old bail of the non-live machine at pending destructions; it now asserts the fight goes on and announces them at the end of the round.
  No heavy gate (Z-gate, golden dual, mutation ratchet) was run, as briefed; the dual-lockstep suites above are the string comparison.
* Red check: the fixes were reverted in a scratch worktree and the new tests failed (DH planet, DH galvanize, announcement, mallice with the first version of the test, combat_fight, nomad mark, valkyrie, Alastor texts; the many-kinds test fails on `unit_codes.len() > MAX_TYPES` because the old code returns empty texts there).
* Debugging aid used and removed: an env-triggered note at each `Unknown` / machine bail (not in the tree).

## Re-recording the baselines (prepared, not done)

`tools/engine_profile/bench_rv36/prepare_native_record.sh NATIVE_SELFPLAY OUTDIR` copies `tools/rulebot` to `OUTDIR`, records the six baselines there natively at 36 and verifies them with the
same binary (must print `200 identical, 0 different` x6, which also proves determinism). Run for this branch with the final binary: `OUTDIR=/root/deploytmp/rv36/native36-rb` (log `/root/deploytmp/rv36/prepare.log`): recorded 6 x 200 at 36, verified **200 identical, 0 different x6**; the recording's hashes
are also equal to the hashes of the independent verify run `ver-stops2` (1 200 of 1 200), i.e. the native engine is deterministic across worker counts. Cost: ~15 min recording + ~15 min verify at 6 workers (cpu 1 800 s for `bench`, `bench-v4`, `bench-v5`; 570 s for v1-v3).
To adopt it, in ONE commit (the user decides): `cp native36-rb/baseline*.json native36-rb/golden*.json tools/rulebot/ && git add tools/rulebot && git commit`. The committed `golden*.json` carry rules versions 23/18/11; the native
engine refuses them ("rules version 23 is not supported (the core plays 36 to 36)"), so the copy sets all four to 36 and `verify_benches.sh` (and the three `TI4_CHURN` bench steps) need the same configs.
Note (unchanged from `BENCH_RV36_RESUME.md`): the specs inside a recording carry the rules version, so only a recording made at 36 verifies at 36 by the verify script.

## The S1 hypothesis ("the S1 ruling makes nearly all bench games diverge from the string recording")

Proof attempt, cheap form: a scratch tree of this branch where the NATIVE engine follows the string's reaction order (`Resolver::player_order` returns the seating order of `Game::new`, rotated to start at the active
seat, for every window: env switch `TI4_STRING_S1`; the patch is not in the tree), and the `bench` config (200 games) is run against the string recording at 35.

| engine | games identical to the string recording (bench, 200) |
|---|---:|
| native, the ruling's order (S1 as decided) | 0 |
| native with the string's order in the action and status phases only | not run on the 200 (12 sample games: first differences move later in 4 of them) |
| native with the string's order in every window | **12** |

First difference (decision index) in the 12 sample games of `bench` (4p seeds 1-12), string 35 against: the native with the ruling order (texts of the earlier lane) / the native with the string's order in every window:
603 / 972, 727 / 1323, 1166 / 1338, 14 / 1406, 898 / 980, 670 / 670, 633 / 633, 409 / 409, 284 / 887, 555 / 555, 893 / 893, 9 / 805.

So S1 is a real and early cause (in 7 of the 12 sample games the first difference was a window order, at decision 9 and 14 in two of them; with the string's order the first differences move out to decisions 800-1400; in the other 5 nothing moves), but
**not the only one**: the games then diverge for other reasons that do not depend on the order: different hidden draws (`return_over_secret_hand_limit` offers different secrets at decision 409 / 670, i.e. the draw order of
a stream differs earlier), different option sets (`load_cargo` 4 options against 3), ground-combat commit decisions (`commit_ground_forces` against `end_turn`/`fight`, probably Parley S7 / the Ixthian S6 at version 36, not checked), and
the other recorded string bugs (S2-S5). Nearly every game still contains at least one such difference, so a string recording cannot serve as the identity reference for the benches at 36 whatever is done about S1; the
reference has to be the native recording itself (prepared above) plus the golden dual for equivalence with the string, as `BENCH_RV36_RESUME.md` already suggested.
Caveats: the emulation is only the order (S2's round-flip windows and the status-phase ruling of 2026-11-03 are covered by "every window", S5's unlock check and the others are not); the 200-game number is for `bench` only.

## Files

* Core: `crates/ti4-engine-core/src/{driver/windows/invasion/fight.rs, kernel/ability.rs, kernel/stage.rs, rules/action_cards/announce.rs, rules/combat/machine.rs, rules/invasion/yin_hero.rs, driver/turn/hero.rs}`,
  `crates/ti4-engine-core-factions/src/{cards/mod.rs, ce_movement/nomad/{query,timing}.rs}`, `crates/ti4-engine/src/{combat_live.rs, invasion_live.rs}` (legacy crate: tests and the dual copy),
  `crates/ti4-engine-next/src/ties/combat_live.rs`.
* Tests: see the table. New helper scripts: `tools/engine_profile/bench_rv36/{unchanged_streams.py, prepare_native_record.sh}`; `compare_hashes.py` takes the verify and base dirs as arguments.
* Work dir (kept, small): `/root/deploytmp/rv36/` (`ver-stops2/` = the final verify output, the first one was deleted; `native36-rb/` = prepared recording; `s1b-bench.json` = the S1 emulation run; logs).
