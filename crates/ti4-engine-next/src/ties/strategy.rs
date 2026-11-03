//! From `strategy.rs` of the legacy crate (copies).

/// Prefix of an option that follows a secondary without spending a strategy token, offered only
/// when a faction module has a waiver (Winnu Acquiescence). The full id is
/// `follow|waived|<module index>|<waiver id>`.
pub const WAIVED_SECONDARY_PREFIX: &str = "follow|waived|";

/// The choice kind for a strategic-action secondary response.
pub const STRATEGY_KIND: &str = "strategy";

/// The choice kind for an ordinary action-phase action.
pub const ACTION_KIND: &str = "action";

/// The historical action id used when a player has exactly one strategic action available.
pub const STRATEGIC_ACTION_ID: &str = "strategic";
