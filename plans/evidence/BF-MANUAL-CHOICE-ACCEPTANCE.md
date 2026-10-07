# BF manual-choice acceptance

## Scope

This is acceptance evidence for the bounded BF24 manual-choice presentation check. The intended
contract is that a parked offer carries the producer's prompt, option labels and IDs, and serialized
decision context intact; submitting one offered ID resumes the actual engine callback and changes
the matching target. The source producers currently in scope include the Ssruu copied round-agent
unit choice and copied L1Z1X planet choice. Ground combat sustain/casualty choices are optional for
this pass when a real driver fixture is available.

## Current test

`crates/ti4-replayer/tests/live.rs` adds two fixtures:

- `a_manual_ssruu_round_copy_parks_with_target_labels_and_resumes_on_the_chosen_unit` seats a
  Yssaril borrower, an exhausted Letnev source agent, and two target cruisers, arms the engine's
  real timing resolver, then emits `COMBAT_ROUND_STARTED` through `TimingContext` on a worker thread.
  A manual `ControlledDecider` parks the actual resolver ability choice and copied-unit target
  choice in the public `Gate`. The test submits the ability and second cruiser IDs from those offers,
  checks the pending choice's context subtype and readable unit labels, and verifies only the second
  cruiser gets the round bonus after the callback resumes. It also checks that Ssruu exhausts and the
  already exhausted source stays exhausted.
- `a_manual_ssruu_l1z1x_copy_preserves_planet_choice_and_replaces_that_planets_infantry` seats
  Yssaril, L1Z1X and Sol, prepares the active system with Sol infantry on two content planets, and
  emits `SYSTEM_ACTIVATED` through the armed resolver. It checks that the parked planet offer retains
  its context subtype, human-readable replacement labels and planet IDs, then submits the second
  planet's ID and verifies that only that planet's infantry becomes a Sol mech. The test also checks
  Ssruu and source-agent exhaustion semantics.

The test exercises a genuine resolver callback, `Table::ask`/`ask_seeing`, Gate parking, conversion
to `PendingManualChoice`, answer submission, and resumed effect. It does not claim a full
`LiveBranch`/`Game::step` tactical-round fixture: `LiveBranch` starts from `SimulationConfig`'s
checkpoint and map pool and exposes no caller-built `GameState` injection. The integration test keeps
the existing public boundary and uses engine fixtures plus the same public Gate/ControlledDecider
path rather than synthesizing `PendingManualChoice`.

## Validation status

This test was appended before this evidence file was created, so the intended spec-first order was
not followed; this note records that chronology. No Cargo command was run by this contributor, per
the task instruction that root owns the workspace build. Compilation and execution remain pending
root's coordinated test run. Ground-combat sustain/casualty producer paths are not covered: this
addition does not drive a full invasion combat window, so it does not claim acceptance for those
prompts.


Permission P1: writable only appended live.rs callback fixtures and this evidence; no public API/dependency/network/external process/folders/deletion. Independent root review confirms actual timing callback, manual Table parking, offer submission and resumed selected-unit/planet effects, with RAII Gate shutdown on assertion failure. Focused `cargo test -p ti4-replayer --test live a_manual_ssruu_ -q -j4 -- --test-threads=4` passes2,9filtered,0.05s, Cargoexit0 (target/bf-oct5-manual-callback.log). Absolute pinned libtorch and its DLL directory used. Root removes the one new unusedimport; full live test/lint pending. Dependencies (copied round module/shared activation) still uncommitted, so manual fixtures are not committed ahead of them. Existing lineupNone hunk belongs to other shared work and is excluded from this package. No wholeBF24 closure claimed.


After removing the new unusedimport, full affected `cargo test -p ti4-replayer --test live -q -j4 -- --test-threads=16`:11passed,0failed,2.60s,exit0 (target/bf-oct5-live-all.log). Both new actualcallback tests pass alongside existing live tests. Still no fullBF24/manualground/isolatedcommit claim.
