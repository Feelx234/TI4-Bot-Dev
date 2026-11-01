//! The native queries of the product surface (docs/todo/D2-8-wrapper.md, lane B / 8d): one file per legacy module, the same function names and
//! signatures, answers read from the typed core. `tests/surface_parity.rs` compares each with its legacy original.

pub mod adapter;
pub mod tactical;
