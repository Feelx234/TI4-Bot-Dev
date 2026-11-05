//! Typed ids from corpus id texts: the generated adapter table (the one source file the legacy and thin crates include as well).

#[path = "../../ti4-engine/src/generated/mod.rs"]
#[allow(dead_code, unused_imports, clippy::all)]
pub mod generated;

pub use generated::IdStr;

use crate::error::{SetupError, unmapped};

/// The typed id of a corpus id text.
///
/// # Errors
/// [`SetupError::Unmapped`] for a text the tables do not know.
pub fn id<T: IdStr>(what: &'static str, text: &str) -> Result<T, SetupError> {
    T::from_id(text).ok_or_else(|| unmapped(what, text))
}
