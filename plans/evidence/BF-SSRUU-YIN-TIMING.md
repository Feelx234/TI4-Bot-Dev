# BF-SSRUU-YIN-TIMING

## Package specification and scoped authorization

Implement Ssruu, Clever Genome (`yssarilagent`) copying Brother Milor (`yinagent`) at the actual `SHIP_DESTROYED` and `GROUND_FORCE_DESTROYED` AFTER timing windows. The normative latest printed text is `crates/ti4-content/content/leaders.json`: "After a player's unit is destroyed during combat: You may exhaust this card to allow that player to place 2 fighters in the destroyed unit's system if it was a ship, or 2 infantry on its planet if it was a ground force." Existing Yin-native routes and their source-only choice IDs are in `crates/ti4-engine/src/factions/yin.rs` (`brother_milor_ship`, `brother_milor_ground`). `plans/evidence/BF-SSRUU-SEATED-AGENT-CENSUS.md` identifies Yin as delayed destroyed-unit timing.

Permission class required: P1.
Writable paths: `crates/ti4-engine/src/factions/yin.rs`, `plans/evidence/BF-SSRUU-YIN-TIMING.md` only.
Read-only paths: repository instructions, base-factions plan, leader corpus, shared borrowed-agent API, and adjacent timing implementations.
Network access: none.
Processes/ports: none; do not run Cargo/build/test commands.
Expected generated artifacts and maximum size: none.
Destructive actions: none.
External-state changes: none.

The static timing roster registers copied listeners for each seated Yin source and each other seated Yssaril borrower, while both their condition and callback dynamically require that the exact `(Yin owner, yinagent)` pair is returned by `hooks_cards::borrowable_agents`. Thus source status may be Readied or Exhausted, but the copied source must be another seat and the borrower must have a Readied Ssruu. The borrower's optional choice is distinct from the source-owned native choice; acceptance exhausts only the borrower's Ssruu, while placement uses the destroyed event's unit owner as recipient and preserves all existing supply and placement rules. Source readiness/status is unchanged. The existing native Yin routes and their stable option IDs remain intact.

## Focused resolver fixtures

Inline fixtures in Yin's existing module cover ship and ground destroyed-unit windows with Readied and Exhausted Yin sources; actor-owned fighters/infantry are placed in the event's destroyed location; only Ssruu exhausts; an already-exhausted source stays Exhausted; locked source and exhausted borrower are inert; source status can change between resolver arming and dispatch; and native Yin's stable choice ID and native exhaustion/recipient behavior remain compatible. The ship fixture destroys a real board cruiser into `pending_destructions` and drains that event through the existing combat `Resolver` announcement path.

## Implementation status

Implemented in `crates/ti4-engine/src/factions/yin.rs`: Yin's static timing roster registers separate copied ship and ground listeners per seated source/borrower pair. Each condition and resolver callback rechecks exact `yinagent` borrowability through `hooks_cards::borrowable_agents`, which permits Readied and Exhausted sources and requires live Readied Ssruu. Acceptance exhausts only borrower `yssarilagent`, places from the destroyed event's actual owner/location through the existing supply-aware placement helper, and reports the use through `borrowed_agent_used`. The native Brother Milor listeners and IDs are unchanged.

Focused fixtures were added for an actual staged ship destruction with the native branch declined and copied branch accepted; an Exhausted-source ground casualty; locked-source and exhausted-borrower ineligibility; source status changing after Resolver arming; and native Milor accepting its original stable ID while copied Ssruu declines. Rustfmt and `git diff --check` passed on the scoped source/evidence paths. Root's initial Cargo validation reported a test payload key type error (`&str` versus `String`); it was corrected within the Yin source scope. Root's recheck, affected crate result, and independent tier-C review are pending. No Cargo commands, commits, or shared-file edits were made by this implementer.

## Fixture corrections after root validation

Root's full-engine run exposed three fixture issues, not implementation failures: a last-ship WHEN reaction consumes an answer before the native Milor AFTER window, so the staged ship fixture now explicitly declines both windows before accepting the copied choice; the shared ground-event fixture names player `b`, so the copied test now sets its actor to `c`; and the exhausted-Ssruu case now explicitly declines native Milor so it isolates copied-listener ineligibility. Assertions still require exactly two units for the copied effect and no copied placement when Ssruu is unavailable. Root recheck is pending.

## Second-failure independent diagnosis

Root's next full-engine run still failed the two ship fixtures: the actual staged-destruction case placed zero fighters, while the native-compatibility case placed four instead of two. The common cause was positional scripting across a timing window whose ask count is intentionally dynamic. A decline only passes that player for the current resolver pass; after another seat resolves an ability, the resolver clears the pass set and may offer the previously declined listener again. Consequently, adding another literal `decline` could move the copied/native answer to a different WHEN or AFTER ask, and an exhausted script falls back to the first option. In the native case that fallback accepted copied Milor on the revisited borrower ask, producing the extra two fighters.

The ship fixtures now use a test-only semantic `Decider` in `yin.rs`. It selects one requested stable ability ID whenever that ID is offered and declines every other reaction window for as many resolver passes as occur. The real casualty fixture requests the copied Ssruu ID, thereby explicitly declining native Milor and unrelated last-ship reactions before the automatic fighter placement. The compatibility fixture requests native Milor's original ID and continuously declines the copied listener, so only the source agent exhausts and exactly two fighters are placed. Recipient, source-status, borrower-status, and stable-ID assertions remain intact.

This fresh diagnosis made no production-code change. `git diff --check -- crates/ti4-engine/src/factions/yin.rs` passed. Per the package restriction, this implementer ran no Cargo/build/test command; root's focused and affected-engine rerun remains pending.

Printed-combat scope remains an open review point: the corpus text says "during combat", while the existing ship branch assumes every `SHIP_DESTROYED` event is combat-origin. Other engine effects can now emit that event. This package preserves native Milor behavior and does not add a new origin discriminator; tier-C review must resolve/document the copied ship listener's reach beyond combat.

Final coordinated full shared-tree run `cargo test -p ti4-engine -q -j8 -- --test-threads=8`: **2043 library passed, zero failures, one ignored**; integrations1/1, decision inventory4/4, other5/5; doctests passed. All new route and failure fixtures passed. No isolated committed-tree build or full BF completion claim.
