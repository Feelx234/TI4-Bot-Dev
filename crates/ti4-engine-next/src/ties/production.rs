//! From `production.rs` of the legacy crate (copies).

/// The choice kind for producing one unit.
pub const PRODUCE_KIND: &str = "produce";

/// The choice kind for exhausting a planet to pay.
pub const PAY_KIND: &str = "pay";

/// The choice kind for placing a produced unit.
pub const PLACE_KIND: &str = "place";

/// The id standing for a system's space area.
pub const SPACE: &str = "space";

/// Separates the system from the spot in a placement outside the producing system
/// (`"<system>@<planet or space>"`), offered by `EconomyHooks::production_destinations`.
pub const REMOTE_SEPARATOR: char = '@';

pub const AGENT_OPTION_ID: &str = "exhaust_agent|winnuagent";

/// Kind and id of the option that exhausts Berekar Berekon for -2 on this use's combined cost.
pub const AGENT_OPTION_KIND: &str = "production_agent";
