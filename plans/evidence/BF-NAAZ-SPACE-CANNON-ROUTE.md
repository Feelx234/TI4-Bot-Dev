# BF-NAAZ-SPACE-CANNON-ROUTE — tactical aftermath immunity fixture

Date: 2026-10-04
Status: test added; root validation pending
Scope: `crates/ti4-engine/src/game.rs` test module only

`a_planetary_maximum_is_immune_to_the_actual_space_cannon_offense_route` starts a real tactical
action on a system with a printed planet. Player A is seated as Naaz, with an Eidolon Maximum on
the controlled planet and a cruiser in the space area to satisfy the supporting-fleet condition.
Player B has a destroyer in space and two PDS on the planet. The two PDS roll deterministic hits.

The fixture enters `Game::step` through the tactical activation and `AftermathWindow::new`, which
fires `combat::space_cannon_offense`, emits `SPACE_CANNON_HITS`, and assigns the resulting hits
through `absorb_hits_seeing_with(..., HitOrigin::UnitAbility)`. The test stops as soon as that event
is present, before ordinary space-combat dice are rolled. It asserts both cannon dice hit, the only
eligible ship (the cruiser) is removed, the Maximum remains undamaged, and the second hit is wasted
after no normal supporting ship remains. A recording decider declines a sustain offer if one is
present and records every prompt; assertions reject any Maximum sustain or casualty offer.

The prior direct assignment fixtures continue to test the shared filter. This fixture proves the
actual space-cannon producer labels and routes its hits as unit-ability hits. No production code
changed here. Tests were not run; focused validation is pending with the parent agent.

Root first compilation corrected an E0716 temporary board-view borrow. First runtime reached the actual hit route and failed a fixture count assertion: two PDS plus Maximum are three planet units, not two. Root corrected the assertion, independently counts the two defender PDS, and explicitly checks the Maximum remains undamaged. Retry/full validation pending. No rule change was needed.
