//! KF2: the play facts the live loop computes from the core position (`ti4_engine::live::plays`) agree with the string engine's
//! (`reactions_core::play_facts`) over the opening positions of the maps the host knows and the variants that matter to a played
//! card (unit upgrades, Reveal Prototype in hand with ships in the active system, a Mahact fleet pool).

use ti4_engine_core_bridge::check_play_facts::{check_position, PlayReport};
use ti4_engine_core_bridge::workload::template_openings;

#[test]
fn the_play_facts_from_the_core_agree_with_the_string_facts_on_the_template_openings() {
    let mut report = PlayReport::default();
    let (positions, _) = template_openings();
    for p in positions.iter().take(8) {
        check_position(p, &mut report);
    }
    assert!(report.mismatches.is_empty(), "{:#?}", report.mismatches);
    assert_eq!(report.unmapped, 0);
    println!("gaps (the string declines, the core answers): {:?}", report.gaps);
    // A check that compared only plain seats and empty sets would agree with anything.
    assert!(report.answers > 2000, "{} answers", report.answers);
    assert!(report.nonempty_reveal > 0, "no seat had a Reveal Prototype set");
    assert!(report.payable > 0, "Reveal Prototype was never payable");
    assert!(report.special_forms > 0, "no seat placed anything but the plain infantry and cruiser");
}

#[test]
fn a_mahact_pool_is_seen_by_both_sides() {
    let mut report = PlayReport::default();
    let (positions, _) = template_openings();
    for p in positions.iter().filter(|p| p.state.players.iter().any(|s| s.faction.as_str() == "mahact")) {
        check_position(p, &mut report);
    }
    assert!(report.mismatches.is_empty(), "{:#?}", report.mismatches);
    assert!(report.mahact_held > 0, "no seat held a token in a Mahact pool; positions with a Mahact seat: {}", report.positions);
}
