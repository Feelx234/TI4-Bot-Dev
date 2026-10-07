# Ssruu combat-round failure and retry evidence

This evidence covers the `Game::step_aftermath` checkpoint boundary for the two borrowed round agents. The new `game.rs` tests enter through the real `Game::step()` and the real aftermath wrapper, while constructing the active combat/invasion continuation directly so each case isolates the failed round transition.

- `invalid_ssruu_letnev_round_copy_in_automatic_space_settle_rolls_back_for_retry` opens `CombatWindow` in its automatic opening stage. The invalid nested unit answer must leave the full game state, event sequence, resolver log and applied-event journal, table decision log, game event log, dice history, and aftermath continuation equal to their pre-step snapshots. The borrower stays readied and the source agent stays readied. Retrying the same automatic opening succeeds, exhausts Ssruu only, preserves Letnev's agent, records a round-copy mark, and emits the round event.
- `invalid_ssruu_sol_round_copy_from_fight_choice_rolls_back_for_retry` creates a live invasion with a committed ground-combatant pair. After the commit step opens the real ground-round choice, its invalid nested target must restore the same snapshots and retain the fight choice. Retrying succeeds; only Ssruu exhausts, Sol's agent remains readied, the target mark is recorded, and the combat sequence advances on the successful round.

The test table deliberately returns `IllegalChoice::ScriptDiverged` from the nested unit-target decision once, then chooses an offered target on retry. This exercises propagation of a real decider failure through timing resolution and `Game::step_aftermath`; it does not fabricate a post-hoc engine error.

Scope limit: these are real driver/checkpoint integration tests for an already-open aftermath, not a full tactical action replay from movement through combat. The focused round module tests separately cover exact per-unit die application and round/planet scoping. This file does not claim final Base Factions acceptance.

Validation: source reviewed; this bounded task did not run Cargo or formatting. The root task owns the current Cargo run and full review.


## Coordinated October 5 complete engine gate

Root command `cargo test -p ti4-engine -q -j4 -- --test-threads=16` passed:2100 library,0 failures,1 ignored; integration groups1/1,4/4,5/5; doctests passed. Full log target/bf-oct5-engine-final.log. Compilation4 preserves memory headroom; independent tests16 use authorized cores. This includes the actual fixtures described here and all current shared WIP, not an isolated committed tree. Independent acceptance/atomic scoped commits and milestone closure remain separate gates. Historical pending statements above describe earlier checkpoints.
