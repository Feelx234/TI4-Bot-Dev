//! Package B7: the typed leader reads agree with the string engine (the gate itself is
//! `board_equivalence leaders ...` over the golden games; this is its smoke test).

use ti4_engine_core_bridge::check_leaders::{LeaderReport, check_leaders_content, check_leaders_position};
use ti4_engine_core_bridge::workload::template_openings;

#[test]
fn printed_leader_data_matches_the_content_store() {
    let mut report = LeaderReport::default();
    check_leaders_content(&mut report);
    assert!(report.mismatches.is_empty(), "{:#?}", report.mismatches);
    assert!(report.answers > 200);
}

#[test]
fn template_openings_agree_on_every_seat_and_leader() {
    let mut report = LeaderReport::default();
    let (positions, _) = template_openings();
    for p in positions.iter().take(2) {
        check_leaders_position(p, &mut report);
    }
    assert!(report.mismatches.is_empty(), "{:#?}", report.mismatches);
    assert_eq!(report.unmapped, 0);
    assert!(report.unlock_hits > 0);
}
