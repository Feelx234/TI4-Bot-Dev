//! From `tactical.rs` of the legacy crate.

/// The constants live in `crate::kinds` (lane B); this module re-exports them.
pub use crate::kinds::tactical::{ROUTES_DETAIL, ACTIVATE_KIND};


// The route-id helpers are defined once, in `queries::tactical`.
pub use crate::queries::tactical::{route_count, route_suffix};
