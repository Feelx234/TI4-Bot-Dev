# BF-SIM-DIAGNOSTIC-PARALLEL

Objective: prepare the already-authorized original-six rebaseline diagnostic with bounded CPU parallelism and complete seed accounting. This is diagnostic preparation before BF23, not accepted 18-faction integration or a new baseline. Permission P1, writable crates/ti4-sim/src/run.rs, examples/rebaseline_behavior.rs, this evidence; no external/network/deletion/other process mutation.

The existing run_with already executes independent seed chunks and sorts by seed. Root capped it at16 workers per operator CPU rule, retaining isolated Game/RNG state and every case. A worker panic now propagates rather than silently dropping its games through filter_map(join.ok()). The example verifies exact ordered seed coverage and rejects errored games before reporting bounds. Metric derivation, bootstrap protocol, seed set, bounds and policy remain unchanged. No performance speedup is claimed.

Validation, independent review and measured diagnostic output pending; no baseline values were edited.


Independent read-only reviewer luna_round_copy_finish found no seed isolation/order defect: chunk-order joining then stable seed sorting preserve deterministic duplicates; the example rejects missing/extra/reordered/error seeds. Worker panic propagation is an intentional runtime change, not a signature change. The16 cap uses OS-visible available_parallelism, not physical-core detection on arbitrary hardware. No speedup claimed; diagnostic and affected tests pending.


Coordinated affected run passed `ti4-sim`51 tests,1 ignored; model83, policy273 before its parallel refactor, bridge62+6+12+3, content134, legacy25; integrations/doctests passed. Overall Cargo exit0, target/bf-oct5-affected.log. Seed-order and repeated-batch fixtures included. Wholly-owned two source files rustfmt formatted and diff checked. Lint/diagnostic/commit pending.


Final coordinated gate: engine2103 passed,1ignored; policy273 passed (parallel campaign included); sim51 passed,1ignored; engine integrations1/1,4/4,5/5 and doctests passed, overall exit0 (target/bf-oct5-closure-green.log). Clippy engine/policy/sim all-targets exit0 with inherited/shared warnings, not warning-clean (target/bf-oct5-closure-clippy.log). Root resolved the two new simulator warnings by documenting panic behavior and replacing map+flatten with equivalent flat_map. Final sim51/1ignored passes and focused sim/policy all-targets clippy exit0 (target/bf-oct5-sim-final.log, target/bf-oct5-focused-clippy.log); no remaining warning in these changed functions. Rustfmt/diff checks pass. This validates the shared tree; isolated committed-tree build not claimed. Independent source review is documented above.


## October 5 diagnostic follow-up

Preparation committed d159edd8; policy parallelization separately c42c8638. First actual original-six diagnostic completes every30 fixed seed with zero errors and completion1.0, but reports5 old metric intervals outside: INVASION_RESOLVED, PRODUCTION_RESOLVED, SHIP_MOVED, SYSTEM_ACTIVATED, TACTICAL_ACTION_BEGAN. Full raw old/new/now table target/bf-oct5-original-six-diagnostic.log. Bounds remain unchanged. Approved no-cargo movement events affect counts and event-share denominators; this is an inference to investigate, not a complete causal explanation or original-six decision-boundary equivalence claim. Shared income/other WIP also exists and cannot be excluded from this diagnostic.

Bounded follow-up scope: example and this evidence only, P1 no external effects. Repeat the same30seed workload, compare every GameResult summary field excluding only wallseconds, reject horizon cutoffs, then derive metrics from the original30cases with unchanged bootstrap. This establishes per-seed summary reproducibility, not full decision-trace equality across old/new trees. Independent review/new run pending; no baseline modification or BF23 closure.


Independent Luna review of follow-up finds exact firstseedarray check + full normalized result-vector equality rejects omissions, duplicates, order changes, errors, cutoffs and outcome differences across every summary field. Only wallseconds is excluded. No decision-boundary compatibility claim or bounds edit.


Follow-up command exits0: all30 recorded seeds complete and reproduce full deterministic result summaries. Five metricintervals remain outside, unchanged. Reviewable old/new/point table and rawlogSHA256 in BF-ORIGINAL-SIX-DIAGNOSTIC-2026-10-05.md. Example lint/format/focused follow-up commit pending; no BF23 closure.
