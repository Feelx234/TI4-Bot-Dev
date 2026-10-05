# Ssruu Naaz nested exploration errors

This evidence covers the copied Garv and Gunn route when a player's turn ends and the chosen
exploration card opens another decision. The scenario uses a real `Game::step()` pass, an
industrial target planet, and Local Fabricators (`lf1`). The outer pass is committed before the
turn-end continuation starts; a nested-choice error must restore the continuation to its
post-pass checkpoint.

The regression test in `crates/ti4-engine/src/factions/naaz.rs` makes the first Local Fabricators
answer invalid, then retries through the same `Game` and `Table`. It checks that the error leaves
the actor passed and retains `PLAYER_PASSED`, while restoring the exploration deck and log,
trade goods, planet units, and Ssruu's readied status. It also checks that the copied Naaz source
leader remains unchanged. The successful retry must draw only `lf1`, place one Sol mech, spend one
trade good, exhaust Ssruu, and retain one successful borrowed-agent decision in the table log.

This is a focused nested-choice rollback regression, not an end-to-end completion claim for every
Ssruu copied agent. Test execution is owned by the root task; this evidence does not assert a
passing Cargo run.


## Coordinated October 5 complete engine gate

Root command `cargo test -p ti4-engine -q -j4 -- --test-threads=16` passed:2100 library,0 failures,1 ignored; integration groups1/1,4/4,5/5; doctests passed. Full log target/bf-oct5-engine-final.log. Compilation4 preserves memory headroom; independent tests16 use authorized cores. This includes the actual fixtures described here and all current shared WIP, not an isolated committed tree. Independent acceptance/atomic scoped commits and milestone closure remain separate gates. Historical pending statements above describe earlier checkpoints.
