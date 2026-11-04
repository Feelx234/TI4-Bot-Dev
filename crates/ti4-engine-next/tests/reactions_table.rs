//! D2-8f: `reactions::window_table` of the thin crate (read from the typed core) lists the windows of the legacy string table, with the same
//! event and relation for each.

#[test]
fn the_window_table_is_the_string_table() {
    let native = ti4_engine::reactions::window_table();
    let legacy = ti4_engine_legacy::reactions::window_table();
    assert_eq!(native.len(), legacy.len(), "same windows");
    assert!(native.len() > 40, "{} windows", native.len());
    for (window, old) in legacy {
        let new = native.get(window).unwrap_or_else(|| panic!("{window:?} missing"));
        assert_eq!(new.event, old.event, "{window:?}");
        assert_eq!(format!("{:?}", new.relation), format!("{:?}", old.relation), "{window:?}");
    }
    assert!(native.keys().eq(legacy.keys()), "same order");
}
