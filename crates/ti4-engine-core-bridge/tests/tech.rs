//! Package A3: the typed technology queries agree with the string engine (the gate itself is the
//! `tech_equivalence` binary over the golden games; this is its smoke test).

use ti4_engine_core_bridge::check_tech::{TechReport, check_tech_content, check_tech_position};
use ti4_engine_core_bridge::workload::template_openings;

#[test]
fn printed_technology_data_matches_the_content_store() {
    let mut report = TechReport::default();
    check_tech_content(&mut report);
    assert!(report.mismatches.is_empty(), "{:#?}", report.mismatches);
    assert!(report.content_answers > 3000);
}

#[test]
fn template_openings_agree_on_every_seat_and_technology() {
    let mut report = TechReport::default();
    let (positions, _) = template_openings();
    for p in positions.iter().take(2) {
        check_tech_position(p, &mut report);
    }
    assert!(report.mismatches.is_empty(), "{:#?}", report.mismatches);
    assert_eq!(report.unmapped, 0);
    assert!(report.tech_answers > 1000);
}
