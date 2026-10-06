# BF-POLICY-CAMPAIGN-PARALLEL

## Scope

Parallelize only `ti4-policy`'s long `scored_games_stay_legal_and_deterministic_across_nested_windows` test. Keep all 51 `(rotation, seed)` cases and both executions per case (102 games total), plus the existing replay equality, legal-seat, secret-return, secret-offer, scored-offer, and non-vacuity assertions. No production behavior or new dependency is in scope.

## Implementation contract

- Construct work in canonical rotation-major, then seed order.
- Run each case's two `scored_game(seed, rotation)` calls sequentially in one worker; cases are independent.
- Use `std::thread::scope` with `min(16, available_parallelism)` workers (at least one). Each case owns its game state; no shared counters or mutable test state.
- Keep each case result in its canonical slot. Surface every worker panic and every `scored_game` error; no result may be silently dropped.
- Perform existing replay/legality assertions and offer aggregation sequentially over canonical results. Preserve aggregate integer counts and both non-vacuity assertions.

## Verification status

Source-only change; Cargo and tests were not run in this bounded task. The already-running serial test process was built before this edit and cannot verify the new code. No speedup claim is made. The parent will coordinate the rerun and independent source review.


Independent root source review: all51 case pairs and original assertions retained; workers only mutate their local result lists and an atomic index. Canonical slots reject missing/duplicate jobs; worker panics and case errors remain failures. Root moved the atomic index outside the scope closure to ensure it outlives all scoped worker borrows. Validation pending. The preceding unmodified serial binary passed273 policy tests in470.21s; this is a checkpoint, not a controlled speedup measurement.


Permission P1: only bot.rs test/helpers and this evidence; no dependency, network, external-state/process mutation, folders or deletion. Independent root review of Luna implementation retained every canonical job and predicate; the helper uses isolated games and bounded16 OS-visible workers. Focused command `cargo test -p ti4-policy --lib scored_games_stay_legal_and_deterministic_across_nested_windows -q -j4 -- --test-threads=16` passed1,272 filtered,53.34s (target/bf-oct5-policy-parallel.log). Original serial full-policy run passed273,470.21s. These observed runtimes are not a controlled speedup protocol. Formatting/whitespace pass; affected crate/lint after this edit and focused commit pending.


Final coordinated gate: engine2103 passed,1ignored; policy273 passed (parallel campaign included); sim51 passed,1ignored; engine integrations1/1,4/4,5/5 and doctests passed, overall exit0 (target/bf-oct5-closure-green.log). Clippy engine/policy/sim all-targets exit0 with inherited/shared warnings, not warning-clean (target/bf-oct5-closure-clippy.log). Root resolved the two new simulator warnings by documenting panic behavior and replacing map+flatten with equivalent flat_map. Final sim51/1ignored passes and focused sim/policy all-targets clippy exit0 (target/bf-oct5-sim-final.log, target/bf-oct5-focused-clippy.log); no remaining warning in these changed functions. Rustfmt/diff checks pass. This validates the shared tree; isolated committed-tree build not claimed. Independent source review is documented above.
