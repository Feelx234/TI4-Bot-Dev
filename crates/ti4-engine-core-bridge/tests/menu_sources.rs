//! The strategic action and the leaders' component actions of the action menu, listed by the core, equal the string's lists (package F2).

use ti4_engine_core_bridge::check_hooks::{hook_positions, hook_variants};
use ti4_engine_core_bridge::check_menu_sources::{card_variants, check_variants, MenuReport};

#[test]
fn the_core_lists_the_strategic_and_leader_options_the_string_lists() {
    let mut report = MenuReport::default();
    for p in hook_positions() {
        let variants = hook_variants(&p);
        check_variants(&p, variants, &mut report);
        let cards = card_variants(&p.state);
        check_variants(&p, cards, &mut report);
    }
    assert!(report.mismatches.is_empty(), "{:#?}", report.mismatches);
    assert_eq!(report.unmapped, 0, "every position is held by the core: {:?}", report.unmapped_at);
    // The comparison reached every shape of the lists.
    assert!(report.bare > 0 && report.several > 0 && report.lone_pass > 0, "strategic: {report:?}");
    assert!(report.own_leaders > 0, "leaders: {report:?}");
    assert!(report.borrowed > 0, "a copy through Ssruu: {report:?}");
    assert!(report.sling > 0, "Sling Relay offered: {report:?}");
    assert!(report.sling < report.seats, "and withheld: {report:?}");
}

#[test]
fn the_cards_and_partners_listed_reach_every_shape() {
    let mut report = MenuReport::default();
    for p in hook_positions() {
        check_variants(&p, card_variants(&p.state), &mut report);
    }
    assert!(report.mismatches.is_empty(), "{:#?}", report.mismatches);
    for source in ["relics", "exploration", "transactions", "cards"] {
        assert!(report.listed.get(source).copied().unwrap_or(0) > 0, "{source} was never offered: {report:?}");
    }
}
