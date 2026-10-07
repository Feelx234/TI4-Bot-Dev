# BF-NAAZ-PLACEMENT-CLOSURE

Status: in progress, root implementation; independent review pending.
Permission: P1; writable supply.rs, naaz.rs, exploration.rs, legendary.rs, agenda_effects.rs, scoped game.rs post-agenda hook, decision inventory and inline route tests; no external writes, new folders, deletions, remote changes. Engine tests use one coordinator, 16 build/test workers, debug=0 and incremental=false.

Objective: close Maximum placement restrictions for Local Fabricators, Refit Troops, L1Z1X replacement, Hope's End and Rearmament; propagate Synergy nested-choice errors; announce a fourth mech placed by an agenda after its effect. Normative sources: content `naaz_voltron`/`naazbt`; original-six compatibility remains binding. Independent reviewer hour_naaz_review found all three blockers in current WIP; no other immunity-origin blocker was found. Keep ledger rows unclaimed until actual route tests and rereview pass.

Shared reinforcement eligibility is the common guard for four routes. Rearmament directly places a generic mech without reinforcement eligibility, so requires a separate Naaz-scoped producer correction. Do not alter old six Rearmament unit identity or finite-supply behavior in this package.

Acceptance: route refusals with Maximum and preserved native control; fallible nested choice state/log retry; actual agenda fourth-mech delivery; affected engine/inventory tests. No test result yet.

## Review follow-up / build interruption

Terra `hour_pi_review` found the callback log checkpoint occurs after the outer optional choice, capture from reinforcements must use remaining plastic rather than placement eligibility, and the new agenda branch must use caller sources. Root fixed strict `try_flush_staged_events` external Table log rollback, corrected capture eligibility to `remaining`, and used actual sources. The first capture-edit pattern did not match; rereview caught this and root applied the exact conditional hunk. Nested retry now exercises the real Game staged-event boundary.

Two test builds failed allocating memory before executing tests (default codegen, then codegen16); cargo check --tests also failed metadata allocation. Host commit headroom fell below 1GB while operator model engine strata.exe privately committed about66GB. Tiny rustc std metadata probe passed; reviewer verified tool job has no memory cap. No green results, red-first proof or acceptance claimed. No process terminated; operator was informed physical free RAM differs from available commit capacity. Next build requires adequate commit headroom and one coordinator.

## October 5 continuation review and measured checkpoint

Root reviewed Luna's new exact survivor-state selection (damage/form/galvanize); a real Game nested failure/retry fixture and distinct damaged survivor test are present. The first scripts incorrectly included a `space` answer when only one spot existed; corrected to the actual legal-choice sequence. Independent Luna review confirmed Maximum unit-ability immunity and planet participation are already implemented in current shared combat/invasion source; stale unimplemented statements were withdrawn.

Root found and corrected Direct Hit's space-only removal for a sustained planet Maximum, using the same ships_of supporting-fleet predicate and exact damaged unit value. Actual Direct Hit effect regression passes in the latest lib run. Original-six native removal behavior is retained under the no-BF branch. Root also added Naaz-only NAAZ_MECH_PLACED staging to exploration, Hope End, Refit Troops, native/copied L1Z conversion, and TE Xxcha hero placement; independent Luna review identified the last two missing paths. Refit uses the proper naaz_mech identity and is now capped by actual remaining supply, while other factions retain accepted behavior. Latest full lib: 2088 passed,0 failed,1 ignored; the subsequent inventory failure was the new objective payload vocabulary, not a Naaz result. Refit's final supply-cap change needs the next run; no asset ledger closure or commit is claimed.

## October 5 source-route audit addendum

The older opening paragraph and historical blocker list above are not a current source audit: combat immunity/planetary participation, captured-form normalization, direct placement guards, and strict nested survivor selection now exist in the shared worktree. The actual asset declarations remain intentionally open in `factions/naaz.rs::MODULE`: `naaz_voltron` is absent from `units`, and `naazbt` is absent from `breakthroughs`. Do not claim either row until route closure and independent review are recorded.

The read-only census found one trigger gap in Absolute Synergy: `ProductionWindow::place` wrote units directly and did not announce `NAAZ_MECH_PLACED`. This missed a Naaz mech produced during another seat's off-turn strategic secondary, because `ACTION_COMPLETED` is owner-filtered. Root has now added `stage_naaz_mech_placed` calls to the actual production placement paths (ordinary window, ability/synchronous build); current diff shows those calls adjacent to the unit placements. This specific source gap is closed in code, pending the coordinator's tests. The agenda Rearmament branch already stages the event after its effect; its real Game staged-delivery fixture is present.

The remaining implementation acceptance items are the named Maximum refusal routes (Local Fabricators, Refit Troops, L1Z1X replacement, Hope's End, Rearmament), correct preservation of native non-Naaz behavior, failed nested-choice state/log rollback, and actual production/agenda fourth-mech delivery. The copied Naaz agent has a separate unresolved failure-path issue recorded in `BF-SSRUU-NAAZ-TIMING.md`: nested exploration still uses legacy Option-returning calls that swallow nested-choice errors. Current shared library result is 2,088 passed, 0 failed, 1 ignored and ti4-model 83 passed; the final full-engine gate is pending the new Game/Yin fixtures. No independent tier-C acceptance or ledger closure is claimed here.


## Coordinated October 5 complete engine gate

Root command `cargo test -p ti4-engine -q -j4 -- --test-threads=16` passed:2100 library,0 failures,1 ignored; integration groups1/1,4/4,5/5; doctests passed. Full log target/bf-oct5-engine-final.log. Compilation4 preserves memory headroom; independent tests16 use authorized cores. This includes the actual fixtures described here and all current shared WIP, not an isolated committed tree. Independent acceptance/atomic scoped commits and milestone closure remain separate gates. Historical pending statements above describe earlier checkpoints.
