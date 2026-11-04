//! The printed reaction windows of the action cards, as the product reads them (`reactions::window_table` of the legacy crate, without its
//! guard functions): for each window sentence of the corpus, the event it listens for and whether it resolves before or after it. The rows come
//! from the typed core's window table (`kernel::reaction::window_of`); `tests/reactions_table.rs` compares them with the legacy table.

use std::collections::BTreeMap;
use std::sync::LazyLock;

use ti4_content::ActionCardWindow;
use ti4_engine_core::generated::Timing;
use ti4_engine_core::kernel::reaction::window_of;

use crate::kernel_sync::text::event_text;
use crate::render::Render;
use crate::ties::timing::Relation;

/// Where a printed window text hooks onto the engine's events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    /// The event this window listens for (`SHIP_DESTROYED`).
    pub event: &'static str,
    /// Whether it resolves before or after that event.
    pub relation: Relation,
}

/// Every reaction window of the corpus (all but the component action `Action`), in the order of `ActionCardWindow`.
///
/// # Panics
/// When a printed window has no timing in the core or a window binds an event the core has no text for (the table is checked by tests).
#[must_use]
pub fn window_table() -> &'static BTreeMap<ActionCardWindow, Window> {
    static TABLE: LazyLock<BTreeMap<ActionCardWindow, Window>> = LazyLock::new(|| {
        let render = Render::new(vec!["a".to_owned()]);
        ActionCardWindow::TABLE
            .iter()
            .filter(|(_, window)| !window.is_component_action())
            .map(|(_, window)| {
                let timing = Timing::ALL.iter().find(|t| format!("{t:?}") == format!("{window:?}")).unwrap_or_else(|| panic!("no core timing for {window:?}"));
                let spec = window_of(*timing).unwrap_or_else(|| panic!("{window:?} is not a reaction window"));
                let event: &'static str = Box::leak(event_text(&render, spec.event).into_boxed_str());
                let relation = match spec.relation {
                    ti4_engine_core::choice::reaction::Relation::When => Relation::When,
                    ti4_engine_core::choice::reaction::Relation::After => Relation::After,
                };
                (*window, Window { event, relation })
            })
            .collect()
    });
    &TABLE
}
