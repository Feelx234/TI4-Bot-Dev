//! The adapter of the kernel (package A5): the text tables of events, payload keys and ability ids,
//! the conversion between the string `Event` and the core's `EventRec`, and the dual shim that replays
//! the string resolver's trace through the core `Resolver` (docs/ENGINE_CORE_STATE.md 3.3).

pub mod text;
pub mod convert;
pub use text::{ability_id, ability_kind, ability_text, event_kind, event_text, key_of, key_text};

#[cfg(test)]
mod tests {
    use ti4_engine_core::generated::Timing;
    use ti4_engine_core::kernel::reaction::window_of;

    use super::text::{event_text, relation_text};
    use ti4_engine_legacy::reactions::{EMITTED_EVENTS, window_table};
    use crate::render::Render;
    use ti4_engine_legacy::timing::Relation;

    /// The core's printed-window table binds every window to the event and relation the string table does.
    #[test]
    fn the_window_table_matches_the_string_table() {
        let r = Render::new(vec!["a".to_owned()]);
        let mut checked = 0;
        for (printed, w) in window_table() {
            let timing = Timing::ALL
                .iter()
                .find(|t| format!("{t:?}") == format!("{printed:?}"))
                .unwrap_or_else(|| panic!("no core timing for {printed:?}"));
            let spec = window_of(*timing).expect("a reaction window");
            assert_eq!(event_text(&r, spec.event), w.event, "{printed:?}");
            assert_eq!(relation_text(spec.relation), (match w.relation { Relation::When => "when", Relation::After => "after" }), "{printed:?}");
            checked += 1;
        }
        assert_eq!(checked, Timing::COUNT - 1, "every non-Action timing is in the string table");
    }

    #[test]
    fn the_emitted_events_are_the_same_set() {
        let r = Render::new(vec!["a".to_owned()]);
        let core: Vec<String> =
            ti4_engine_core::kernel::reaction::EMITTED_EVENTS.iter().map(|k| event_text(&r, *k)).collect();
        assert_eq!(core, EMITTED_EVENTS.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>());
    }
}
