//! The adapter between the thin deciders and the legacy tables lives in the thin crate (`ti4_engine::oracle_adapter`): tests of several crates use it.

pub use ti4_engine::oracle_adapter::{ThinDecider, convert, thin_records};
