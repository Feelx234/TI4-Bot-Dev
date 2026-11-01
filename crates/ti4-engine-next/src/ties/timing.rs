//! From `timing.rs` of the legacy crate.
use serde::{Deserialize, Serialize};

/// The point relative to an event at which an ability may resolve.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Relation {
    /// Resolve before the event's ordinary effect.
    When,
    /// Resolve after the event's ordinary effect.
    After,
}
