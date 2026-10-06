# BF-SSRUU-MENTAK-TIMING

Permission class: P1. Writable paths: `crates/ti4-engine/src/factions/mentak.rs` and this evidence file only. No Cargo, commits, folders, deletions, or shared-file edits. Preserve the existing Mentak WIP and native agent behavior.

## Objective and rule source

Implement Ssruu's copy of Mentak's agent, Suffi An. The printed leader record is `mentakagent` in `crates/ti4-content/content/leaders.json`: “After the Pillage faction ability is used against another player: You may exhaust this card: if you do, you and that player each draw 1 action card.” This listener is tied to successful uses of the Pillage ability, including its nested timing path; it must not fire merely on any `TRADE_GOODS_GAINED` event. Borrowed eligibility is live and comes from `hooks_cards::borrowable_agents` naming the exact Mentak source agent. The listener belongs to the Ssruu holder, exhausts only their Ssruu, leaves the source unchanged, draws once for the borrower and Pillage target, and calls `borrowed_agent_used` once.

## Scope constraints and acceptance

Keep native Mentak Suffi An behavior intact. Register copied listeners from the static seated source-agent / Ssruu-card roster, with dynamic readiness and exact-source checks at use time. Add tests for actual Pillage invocation, borrowed draw recipients and hidden hands, decline/unavailable cases, dynamic eligibility after resolver arming, and native compatibility. Tests should exercise resolver-driven Pillage/timing behavior instead of only calling helpers. Formatting is scoped to `mentak.rs`; root coordinates Cargo and independent review.

## Implementation record

Pending implementation and coordinated validation.

Usage-limit interruption: incomplete copied-listener registration was saved before its callback existed. Root commented out that registration, preserving partial source for continuation. No tests or acceptance claim. Finish the actual Pillage callback and regressions before registering it.

Root continuation after Luna usage limit: implements the callback and successful nested PILLAGE_USED route directly. Emit only after an actual take and the independently unchanged native agent offer, and only when a legal copy exists; original no-copy event streams stay unchanged. Optional copied listener belongs to borrower, dynamic exact-source eligibility, draws to borrower and pillaged player through ordinary draw hooks, only Ssruu exhausts. Local Pillage transaction restores state/dice/RNG/sequence/resolver if nested copied draw/choice fails. Added actual Pillage readied/exhausted and live readiness/no-Pillage fixtures; focused tests and independent review pending. No new Choice constructor site; existing observed draw/optional delivery used.

## October 4 coordinated validation

Mentak focused module: 31 passed, zero failures. Root full affected-crate command `cargo test -p ti4-engine -q -j8 -- --test-threads=8`: 2034 library tests passed, zero failures, one ignored; integration groups 1/1, decision inventory 4/4, other 5/5; doctests passed. Eight build/test workers authorized by the operator. This validates the shared working tree, not an isolated commit. Root implementation and shared timing changes still require independent tier-C review; no package or ledger closure is claimed.

## Independent Terra review and fixes

muaat_mentak_review_terra found native Suffi An was always resolved before the copy, plus nested failure left decision-log entries. Root emits PILLAGE_USED before native resolution when a copy is available; a non-optional native wrapper delegates to the unchanged native Suffi An prompt/option in the same player-ordered resolver window. No-copy games retain the previous inline path. Root restores the callback's decision log on nested failure. Scarce-deck normal-order and post-selection copied Scheming discard-failure regressions passed in the latest full run (whose unrelated Yin fixtures failed). Reviewer caught and root removed an intermediate change to native optional identity; final source rereview reports no findings and recommends accepting tier C. The proposed event.player==source restriction was withdrawn: printed text permits any successful Pillage; the exact source establishes the right to copy the text, not the Pillage actor. Final coordinated full-suite pass remains pending. The outer Pillage optional selection precedes the local transaction; its trace can remain, while nested native/copy/draw choices roll back. No broader Game transaction claim.

Final coordinated full shared-tree run `cargo test -p ti4-engine -q -j8 -- --test-threads=8`: **2043 library passed, zero failures, one ignored**; integrations1/1, decision inventory4/4, other5/5; doctests passed. All new route and failure fixtures passed. No isolated committed-tree build or full BF completion claim.

`cargo clippy -p ti4-engine --lib -q -j8` exited0 with existing/shared-WIP warnings; not warning-clean. Independent Terra final rereview reports no remaining findings.
