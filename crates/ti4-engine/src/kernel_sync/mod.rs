//! The adapter of the kernel (package A5): the text tables of events, payload keys and ability ids,
//! the conversion between the string `Event` and the core's `EventRec`, and the dual shim that replays
//! the string resolver's trace through the core `Resolver` (docs/ENGINE_CORE_STATE.md 3.3).

pub mod text;
pub use text::{ability_id, ability_text, event_kind, event_text, key_of, key_text};
