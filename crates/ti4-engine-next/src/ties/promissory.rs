//! From `promissory.rs` of the legacy crate (copies).

/// Support for the Throne is keyed by its owner rather than by an alias.
pub const SUPPORT_PREFIX: &str = "support:";

/// This owner's Support for the Throne, by the faction name that owns it.
#[must_use]
pub fn support(owner_name: &str) -> String {
    format!("{SUPPORT_PREFIX}{owner_name}")
}
