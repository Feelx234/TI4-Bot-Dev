//! The strategic action and the leaders' component actions of the action menu, listed by the core, equal the string's lists (package F2).

use ti4_engine_core_bridge::check_hooks::{hook_positions, hook_variants};
use ti4_engine_core_bridge::check_menu_sources::{check_variants, MenuReport};

#[test]
fn the_core_lists_the_strategic_and_leader_options_the_string_lists() {
    let mut report = MenuReport::default();
    for p in hook_positions() {
        let variants = hook_variants(&p);
        check_variants(&p, variants, &mut report);
    }
    assert!(report.mismatches.is_empty(), "{:#?}", report.mismatches);
    assert_eq!(report.unmapped, 0, "every position is held by the core");
    // The comparison reached every shape of the lists.
    assert!(report.bare > 0 && report.several > 0 && report.lone_pass > 0, "strategic: {report:?}");
    assert!(report.own_leaders > 0, "leaders: {report:?}");
    assert!(report.borrowed > 0, "a copy through Ssruu: {report:?}");
}
