//! From `tactical.rs` of the legacy crate.

/// The constants live in `crate::kinds` (lane B); this module re-exports them.
pub use crate::kinds::tactical::{ROUTES_DETAIL, ACTIVATE_KIND};
use crate::wire::choice::Choice;


/// Split a route answer into the id of the option and the number of the route: `move|206|0|r2` is route 2 of `move|206|0`.
/// `None` for an id without a route (the default route).
#[must_use]
pub fn route_suffix(id: &str) -> Option<(&str, usize)> {
    let (base, tail) = id.rsplit_once("|r")?;
    let number: usize = tail.parse().ok()?;
    (number > 0 && tail == number.to_string()).then_some((base, number))
}

/// How many routes the choice lists for `option_id`.
#[must_use]
pub fn route_count(choice: &Choice, option_id: &str) -> usize {
    choice.details.get(ROUTES_DETAIL).and_then(|routes| routes.get(option_id)).and_then(serde_json::Value::as_array).map_or(0, Vec::len)
}

