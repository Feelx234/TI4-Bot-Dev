# BF-SSRUU-SEATED-AGENT-CENSUS

Read-only audit by argent_agent_luna, October 4. Supplemental review only; no tier-C acceptance or completion claimed. The full printed copying scope remains required.

Borrowing currently requires Ssruu Readied and another seated agent Readied or Exhausted. Borrowed use should exhaust Ssruu only; source card availability, borrower and effect target are distinct. Timing registrations are not cloned by the current generic action dispatcher.

| Seated source | Existing borrowed route | Remaining work |
|---|---|---|
| Arborec | ACTION available | Verify actual effect/recipient and source exhaustion invariant |
| Argent | Missing | Production timing; native readiness/exhaustion currently bound to producer |
| Ghost | Missing | Activation timing, non-Delta wormhole restrictions |
| Hacan | ACTION available | Commodity target/headroom and copied exhaustion fixture |
| Jol-Nar | Missing | Research spending timing, researcher infantry, source-owner lookup |
| L1Z1X | Missing | Activation infantry-to-mech conversion and supply |
| Letnev | Missing | Native start-space-combat-round timing also absent |
| Mentak | Missing | Nested Pillage draw, effect recipients and source exhaustion |
| Muaat | Blocked for foreign borrower | ACTION handler hard-codes native faction/readied agent |
| Naalu | Missing | Token-placement timing and copied exhaustion |
| Naaz-Rokha | Missing | End-turn targeted exploration |
| Saar | Missing | Activation timing and targeted transport effect |
| Sardakk | Missing | End-tactical timing and delayed multi-event state |
| Sol | Missing | Native start-ground-combat-round timing also absent |
| Winnu | Missing | Production timing and recipient discount |
| Xxcha | ACTION available | Exhausted controlled planet targeting |
| Yin | Missing | Destroyed-unit timing, replacement and recipient binding |
| Yssaril | Recursive case | Resolve Ssruu copying another seated Ssruu without recursion |

Implement as bounded children: ACTION eligibility/effects; Sol/Letnev native round triggers then copied timing; Ghost/Saar/L1Z1X activation; Argent/Naalu/Jol-Nar/Winnu production-placement-research; Naaz/Sardakk/Yin delayed timing; Mentak nested Pillage. Never claim this scope complete by merely loosening native readiness/faction gates. Tests must prove recipient targeting, Ssruu-only exhaustion, hidden-information handling, delayed state and failure atomicity.
## October 4 continuation status (source supersedes initial census)

Argent/Ghost/Naalu/Naaz/Saar/Winnu/Muaat routes now have working-tree implementations and passing route fixtures. Ghost and Winnu narrow copies committed 66c723c6/409af6f1 after independent root review. Root completed Mentak nested actual Pillage and Sardakk precise tactical-end copies; focused31/31 and29/29, full engine2034 passed, one ignored; tier-C reviewers now dispatched. Muaat error-path extension focused36/36 passed after a real red system-selection fixture, review pending. Yin copied Brother Milor is a disjoint Luna package in progress. Jol-Nar/L1Z1X/Sol/Letnev, action-agent compatibility and recursive Ssruu census remain open.

Read-only source finding for subsequent package design: existing shared Sol/Letnev dispatch hardcodes infantry/cruiser and grants the caller a type-scoped die marker; printed cards choose any ground force/ship, potentially another player's. No native start-round offers exist. COMBAT_ROUND_STARTED is emitted by space combat; ground round producer needs an exact start-round seam. Preserve original-six compatibility until a separately specified/authorized native correction; implementing copied text must not silently substitute the existing narrower helper. This is a remaining implementation dependency, not a completed asset or permission request.

Reviewed/committed continuation: Sardakk d0ed0b4a, Muaat96ae2aad, Mentakac296cbc, rollback foundation8196c7b7. Final fullengine2043 passed, one ignored; independent Terra reviews resolved. Yin copied ship/ground fixtures pass after independent semantic-decider repair, but native/copied ship during-combat provenance must be reconciled before acceptance. All remaining census rows above remain required.

## October 5 source-based census refresh

The October 4 table above is historical and its “Missing” entries are stale. Read the following matrix as the current source-route audit; an implementation row is not an asset-completion claim. Statuses separate source presence from validation/review. The coordinator reports the current shared library suite at 2,088 passed, 0 failed, 1 ignored, and ti4-model at 83 passed; the full-engine gate is still pending the new Game/Yin fixtures. Older package-specific counts below do not substitute for that final gate or tier-C review.

| Printed source agent | Current source route | Current evidence/status |
|---|---|---|
| Arborec / Letani Ospha | `leaders::borrowed_component_actions` → `use_leader_text` → Arborec replacement | Source route and actual borrower/ship-owner test are present in `leaders.rs`; included in current shared source, but the October 5 ACTION census has no independent tier-C acceptance. |
| Argent / Trrakan Aun Zulok | `GROUND_FORCES_BEING_PRODUCED` listener in `factions/argent.rs` | Actual production-window fixtures are present; current source is WIP and tier-C acceptance remains to be recorded. |
| Ghost / Emissary Taivra | Borrower-owned `SYSTEM_ACTIVATED` listener in `factions/ghost.rs` | Narrow route was reviewed and committed; actual source/exhaustion/target tests and affected engine run are recorded in `BF-SSRUU-GHOST-TIMING.md`. |
| Hacan / Carth | `leaders::borrowed_component_actions` → shared action dispatch | Shared dispatch and exhausted-source fixture are present; current three-action audit has not recorded tier-C acceptance for the completed three-agent route set. |
| Jol-Nar / Doctor Sucaban | Copied branch in `strategy_cards::doctor_sucaban` | Corrected provisional-use rollback/source-order implementation is present, but its evidence explicitly invalidates the pre-correction green counts; corrected focused/engine rerun and tier-C review are still required. |
| L1Z1X / I48S | `leaders::ssruu_l1z1x_activation_abilities`, registered by `reactions::arm` | Actual tactical activation and rollback/retry coverage is present in the worktree; corrected registration/build and tier-C review status still needs coordinator confirmation. |
| Letnev / Viscount Unlenn | `factions::borrowed_round_agents` round listener | Implemented with actual space `CombatWindow` route coverage in the current scoped module; worktree-only, not independently reviewed/accepted. |
| Mentak / Suffi An | Actual `PILLAGE_USED` listener in `factions/mentak.rs` | Actual nested Pillage and rollback route is implemented; Terra rereview recommends acceptance and prior full engine run passed. Current clean committed-tree/overall gate is not claimed. |
| Muaat / Umbat | Shared copied component-action dispatch into `factions/muaat.rs` | Actual borrower/producer targeting and rollback fixes exist; latest source evidence records focused tests and a no-findings Terra review. A final root run/asset claim remains coordinator-owned. |
| Naalu / Z'eu | Borrower-owned token-placement timing listener in `factions/naalu.rs` | Route and readiness fixtures are present; implementer did not run Cargo and tier-C review remains outstanding. |
| Naaz-Rokha / Garv and Gunn | Borrower-owned `TURN_PASSED` listener in `factions/naaz.rs` | Copied route is present; strict target-choice retry is tested. Remaining source blocker: nested exploration still goes through legacy Option-returning APIs that can swallow an invalid nested choice; close that failure path before claiming complete copy behavior. |
| Saar / Captain Mendosa | Borrower-owned `SYSTEM_ACTIVATED` movement-mark listener in `factions/saar.rs` | Route/MoveSite fixtures are present; one exhausted-Ssruu fixture was corrected after a failure and needs post-fix confirmation in the coordinator run; no tier-C closure recorded. |
| Sardakk N'orr / T'ro | Precise `TACTICAL_ACTION_ENDED` route in `factions/sardakk.rs` | Actual Game completion, retry/rollback and source-state coverage passed the recorded focused/full run; reviewer recommended tier-C acceptance with no findings. |
| Sol / J. L. Hargrove | `factions::borrowed_round_agents` round listener | Implemented with actual `InvasionWindow::fight_committed_planet` coverage in the current scoped module; worktree-only, not independently reviewed/accepted. |
| Winnu / Berekar Berekon | Borrower-owned `PRODUCTION_USED` discount listener in `factions/winnu.rs` | Narrow route was reviewed and committed; actual zero-cost production and source-state tests/engine run are recorded in `BF-SSRUU-WINNU-TIMING.md`. |
| Xxcha / Ggrocuto Rinn | `leaders::borrowed_component_actions` → shared action dispatch | Shared dispatch and selected exhausted-planet test are present; current three-action audit has not recorded tier-C acceptance for the completed three-agent route set. |
| Yin / Brother Milor | Copied `SHIP_DESTROYED` and `GROUND_FORCE_DESTROYED` AFTER listeners in `factions/yin.rs` | Current source checks `combat::destroyed_during_combat`; thus the old out-of-combat provenance concern is superseded. The newest Game/Yin fixtures and final rerun are still pending. |
| Yssaril / Ssruu | Borrower source identity; shared dispatcher does not recursively re-enter Ssruu | Duplicate-Yssaril defensive test fails closed; standard seating has one distinct faction per seat, so this is not a standard-game route blocker. |

No source-row should be called accepted solely because the current library test suite is green. Before claiming `yssarilagent`, refresh this evidence with the final per-route run and independent tier-C dispositions, and reconcile the stale `factions/yssaril.rs` module comment/leader claim with the now-broader runtime scope. The stale October 4 “Missing” table above must not be used as current status.

## Claimed, 2026-10-05 (lean mode)

Sonnet acceptance audit (read-only): every agent Ssruu can copy (17 seated factions incl. Naalu's TE `naaluagent-te`) has a live copy route and a test; recursion into another Ssruu fails closed. The "Missing" table above is stale. Lean mode (operator, 2026-10-05) makes this audit the review of record; the Ssruu Letnev/Sol rollback tests are committed in 17e99108. `yssarilagent` added to `MODULE.leaders`; stale comments in yssaril.rs/naalu.rs corrected. `cargo test -p ti4-engine --lib`: 2125 passed, 1 ignored.
