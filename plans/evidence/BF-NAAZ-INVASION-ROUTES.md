# BF-NAAZ-INVASION-ROUTES — routed ground immunity fixtures

Date: 2026-10-04
Status: tests added; root validation pending
Scope: `crates/ti4-engine/src/invasion.rs` test module only

These regressions drive the real `InvasionWindow` path rather than passing cause labels directly
to `ground_hit_logged` or calling the ground hit absorber. They use the printed Naaz Eidolon
Maximum, actual factions and units, deterministic dice, and the window's normal commit/settle
steps.

| Test | Route and assertion |
|---|---|
| `invasion_window_bombardment_skips_maximum_and_discards_excess_hits` | A Sol War Sun rolls three hits against a Naaz planet holding one Maximum and one infantry. The routed invasion bombardment destroys the infantry; both remaining hits have no eligible victim and are wasted. The Maximum remains undamaged, and no sustain prompt mentions it. |
| `invasion_window_space_cannon_defense_skips_maximum_for_infantry` | A Naaz player commits infantry onto a Sol PDS planet through `InvasionWindow::at_commit_step`. The PDS's actual invasion defense roll removes the committed infantry while the Maximum already standing there remains undamaged; no sustain prompt is offered. |
| `invasion_window_harrow_skips_maximum_for_eligible_infantry` | An L1Z1X dreadnought and infantry invade a Naaz planet with a Maximum and infantry. The test resolves one real ground-combat round, then Harrow fires through the round-ended invasion path. Its hit removes the eligible defender infantry and leaves the Maximum undamaged, with no sustain prompt. |

Support code selects a real non-Mecatol planet from the embedded corpus and clears that system's
deployed units/control before arranging each fixture, so unrelated starting fleets cannot affect
the deterministic rolls. The bombardment fixture uses three War Sun faces of 10; the ground round
uses six misses followed by a Harrow hit.

The prior helper-level coverage remains useful for the pure filter, but is not treated as route
coverage. No production code changed in this package. Tests were not run here; the parent agent owns
shared validation and should run the focused `ti4-engine` tests before accepting these fixtures.

First coordinated compilation found four E0716 temporary-borrow lifetime errors in the new fixtures. Root introduced stable board-view and planet-catalog bindings; no production behavior changed. Runtime validation remains pending.

Root runtime validation: `cargo test -p ti4-engine --lib invasion_window_ -q -j1` passed **4/4**, 2019 filtered, after the lifetime repairs. This includes the three new actual routes and an existing matching invasion fixture. No production behavior changed in these repairs.
