//! The items of the legacy crate that the glue and the wire types name outside themselves (docs/todo/D2-8-wrapper.md, Appendix A,
//! the 81 ties). Each file here is named after the legacy module the items came from; an item is a COPY (a constant, a pure
//! function over model types), never a re-export, so nothing in this crate names a string rule module.
//!
//! Lane B owns `kinds.rs` and the queries: where lane B provides one of these at its final path, the item here becomes a
//! `pub use` of it. Nothing outside lane A should depend on this module's layout.

pub mod planets;
pub mod strategy_cards;
pub mod tactical;
pub mod timing;
