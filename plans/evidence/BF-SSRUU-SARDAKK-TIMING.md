# BF-SSRUU-SARDAKK-TIMING

## Package specification and scope

Implement Ssruu, Clever Genome (`yssarilagent`) copying Sardakk N'orr's T'ro (`sardakkagent`). Corpus source: `crates/ti4-content/content/leaders.json`, `sardakkagent` text: “At the end of a player's tactical action: You may exhaust this card: if you do, that player may place 2 infantry from their reinforcements on a planet they control in the active system.” The existing Sardakk native route is in `crates/ti4-engine/src/factions/sardakk.rs`.

Writable paths are limited to `crates/ti4-engine/src/factions/sardakk.rs` and this evidence file. No Cargo/build, shared-file changes, commits, folders, deletion, or generated artifacts. Root owns validation and review.

The copied listener must be a distinct optional `ACTION_COMPLETED` timing ability owned by the Ssruu holder, with static registration from the Sardakk-agent/Yssaril-agent roster and a live exact-source `hooks_cards::borrowable_agents` check. The source agent is legal Readied or Exhausted. On acceptance, use the actual action's player and the system remembered from that player's tactical activation, offer the existing controlled-planet infantry placement with supply limits, exhaust only the borrower's Ssruu, leave the source agent unchanged, consume the delayed mark, and notify the borrowed-agent hook after resolution. Decline leaves both cards and mark unchanged until normal expiry.

The existing activation bookkeeping is necessary because `ACTION_COMPLETED` is also used by strategic actions and fires after the active system is cleared. Preserve native T'ro behavior. Track the activation when a copied route may use a source that is Exhausted; clear its delayed state on the existing `STRATEGIC_ACTION_BEGAN` / `TURN_BEGAN` expiry routes. Do not offer either native or copied T'ro if the acting player has no controlled planet or usable infantry in the activated system.

## Focused actual-route coverage

Planned resolver/Game fixtures must cover:

- Actual tactical Game completion with a Readied Sardakk source while the source owner's native optional is declined, then Ssruu accepts; verify actor-owned infantry appear on the actor's controlled planet, only Ssruu exhausts, and the source remains Readied.
- Same actual route with the source Exhausted; Ssruu can still use printed text, source remains Exhausted, and only the borrower exhausts.
- Native Sardakk route unchanged (source holder controls the optional agent, actor controls placement), plus native decline and no legal actor planet/supply cases.
- Absent/exhausted Ssruu and absent/locked source are inert; illegal/non-tactical completion cannot use a stale activation.
- Delayed activation state expires on the existing strategic-action/new-turn cleanup path; no old target survives into a later action.

No tests have been run. Root coordinates the shared build after handoff. Independent tier-C review remains pending; no asset completion claim is made.

## Usage-limit interruption

The Luna implementer stopped on an account usage limit before writing the borrowed callback or fixtures. Its partial registration referenced an undefined borrowed_agent. Root commented out that incomplete registration so it stays unarmed; partial activation-bookkeeping changes are preserved for continuation. No implementation acceptance or tests claimed. Next: finish source-bound callback and actual tactical-completion tests before re-enabling registration.

Root continuation changes the implementation boundary: a precise TACTICAL_ACTION_ENDED event from Game::close_tactical carries player and system before clearing tactical state, including strategy-card free tactical actions. Native and copied T'ro share this actual timing moment; native stable option ID is retained. Generic ACTION_COMPLETED previously lost system data and could retain stale activation marks. Copy condition/effect reads the event target independently, so native use cannot consume it. Game snapshots state, dice, RNG, sequence, resolver and events around this new end window on failure, retaining aftermath for retry. This covers the new card window; earlier Crown/DET effects before that snapshot remain separately scoped. Original six-faction games without T'ro or a legal copy emit no new event. No broad game.rs formatting.

Added actual Game tactical completion coverage for source readied/exhausted and simultaneous native+copied use (two separate batches), plus ordinary component-action completion and exhausted Ssruu refusal. Existing native fixtures migrate their completion helper to the precise event, retaining no-activation refusal and original choice IDs. Focused/affected validation and independent review pending; this source-owned timing correction is not claimed accepted or committed.

## October 4 coordinated validation

Sardakk focused module: 29 passed, zero failures (2006 filtered). Root full affected-crate command `cargo test -p ti4-engine -q -j8 -- --test-threads=8`: 2034 library tests passed, zero failures, one ignored; integration groups 1/1, decision inventory 4/4, other 5/5; doctests passed. Eight build/test workers authorized by the operator. This validates the shared working tree, not an isolated commit. Root implementation and shared timing changes still require independent tier-C review; no package or ledger closure is claimed.

## Independent Terra review and fixes

Independent reviewer sardakk_timing_review_terra rejected the initial patch for lost completed aftermath on end-window failure, zero-infantry-supply offers, and copied use leaving its activation mark. Root retained the completed AftermathWindow in both close branches, gated native/copy eligibility on positive infantry supply, and consumed the copied mark after successful placement. New actual Game fixture forces a production aftermath via a placed space dock, rejects one copied placement, checks retained system/readied Ssruu/no successful end event, then completes a retry with exactly one end event and two infantry. A zero-supply native/copy fixture is inert. Focused module **31/31 passed**. Reviewer re-read the actual source and fixture path and recommended accepting tier C with no remaining findings; reviewer ran no builds. A subsequent root decision-log audit additionally snapshots/restores Game.table.log around the new window and asserts no failed copied-use trace; narrow independent follow-up pending. Shared Resolver checkpoint foundation must be accepted/committed before this dependent package. Later full suite failures were only the separate new Yin fixtures, never accepted as green; coordinated final validation pending.

Final coordinated full shared-tree run `cargo test -p ti4-engine -q -j8 -- --test-threads=8`: **2043 library passed, zero failures, one ignored**; integrations1/1, decision inventory4/4, other5/5; doctests passed. All new route and failure fixtures passed. No isolated committed-tree build or full BF completion claim.

`cargo clippy -p ti4-engine --lib -q -j8` exited0 with existing/shared-WIP warnings; not warning-clean. Independent Terra final rereview reports no remaining findings.
