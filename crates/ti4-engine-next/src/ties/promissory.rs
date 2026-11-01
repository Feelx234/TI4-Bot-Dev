//! From `promissory.rs` of the legacy crate (copies).

/// The constants live in `crate::kinds` (lane B); this module re-exports them.
pub use crate::kinds::promissory::{SUPPORT_PREFIX};


/// This owner's Support for the Throne, by the faction name that owns it.
#[must_use]
pub fn support(owner_name: &str) -> String {
    format!("{SUPPORT_PREFIX}{owner_name}")
}
