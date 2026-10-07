# BF-SSRUU-MUAAT-ERRORS

## Scoped implementation authorization (P1)

This evidence records P1 authorization before edits. Writable scope: `crates/ti4-engine/src/factions/muaat.rs` and this evidence file only. Preserve the existing Ssruu Umbat helper, tests, and root WIP. No shared API/production edits, Cargo, commits, folders, deletions, or subagents.

## Rule and defect scope

Muaat's Umbat lets the selected player produce up to two units, each costing no more than four. The existing copied path runs this effect as the Ssruu borrower and generic `leaders::use_leader_text` spends Ssruu only when dispatch returns true. Umbat currently discards `produce_by_ability_capped` errors and returns true anyway; after an earlier unit/payment mutation, a later illegal production answer can therefore leave partial game state and spend Ssruu. Fix copied-path failure atomicity only, without altering successful native Muaat play. On copied failure restore all mutable game services transactionally checkpointed by this local effect, return false so shared dispatch does not exhaust Ssruu or call the successful-copy hook. No production/shared API changes are in scope.

## Planned verification

Add an actual `leaders::use_leader_text` test where one purchase and payment have already mutated state before a later invalid production choice. Verify state/log rollback and then retry through the same public copied dispatch on the restored state; assert the retry can produce and spends Ssruu only on success. Tests are coordinated by root and will not be run here.

Usage-limit interruption: local copied-production checkpoint/rollback code was written, but the planned invalid-after-payment regression was not. Root corrected double-dereference assignments in the borrowed TimingContext fields so the partial code compiles. Existing Muaat regressions are being checked; failed-transition atomicity remains unaccepted until its new fixture and independent review are complete. This is a Luna follow-up to Pi's passing implementation, not a Pi defect introduced by its eligibility patch.

Root added a_failed_copied_umbat_rolls_back_paid_production_before_retry: actual shared copied-leader dispatch buys a cruiser (automatic sole-planet payment/placement), then receives an invalid later build. It must refuse without state/leader changes and a legal retry must finish and exhaust only Ssruu. Root removed temporary diagnostic output and corrected the original Pi script's redundant payment answer. No resolver rollback is claimed: Umbat production currently runs with timing None; state/dice/rng/sequence/table-log services are locally checkpointed, and decider script consumption intentionally remains consumed. Independent review remains pending.

## October 4 coordinated validation

Muaat focused module before the extra system-choice regression: 35 passed, zero failures. Root full affected-crate command `cargo test -p ti4-engine -q -j8 -- --test-threads=8`: 2034 library tests passed, zero failures, one ignored; integration groups 1/1, decision inventory 4/4, other 5/5; doctests passed. Eight build/test workers authorized by the operator. This validates the shared working tree, not an isolated commit. Root implementation and shared timing changes still require independent tier-C review; no package or ledger closure is claimed.

Additional actual dispatch regression: two legal war-sun systems force the system-selection choice. Invalid answer initially failed the assertion (dispatch incorrectly returned true). Root distinguishes ask error from a valid offered decline: copied ask error returns false, valid decline still spends Ssruu and places nothing. Native legacy error behavior remains unchanged. Focused retry pending.

Focused retry after the system-choice fix: `cargo test -p ti4-engine --lib factions::muaat -q -j8 -- --test-threads=8`: **36 passed, zero failures**, 2000 filtered. The new fixture proves full state unchanged after the invalid answer, then legal decline spends only Ssruu and leaves board/source unchanged. Independent Terra review dispatched; pending actual findings.

## Independent Terra review and fixes

muaat_mentak_review_terra found the prior table-log checkpoint began after target/system asks. Root wraps umbat_inner with copied-use log checkpoint before any asks; false dispatch restores that log as well as the inner production state/services. The paid-production failure fixture now uses the SAME Table for failure then legal retry, verifies exact failed-use log rollback, and preserves consumed decider input. All Muaat tests passed in the latest shared full run (only new Yin fixture failures). Independent source rereview recommends accepting Muaat with no remaining findings; reviewer ran no builds. Final coordinated full suite remains pending before commit.

Final coordinated full shared-tree run `cargo test -p ti4-engine -q -j8 -- --test-threads=8`: **2043 library passed, zero failures, one ignored**; integrations1/1, decision inventory4/4, other5/5; doctests passed. All new route and failure fixtures passed. No isolated committed-tree build or full BF completion claim.

`cargo clippy -p ti4-engine --lib -q -j8` exited0 with existing/shared-WIP warnings; not warning-clean. Independent Terra final rereview reports no remaining findings.
