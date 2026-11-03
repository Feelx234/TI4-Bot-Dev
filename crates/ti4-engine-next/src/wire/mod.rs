//! The wire and format types, copied out of the legacy crate (docs/todo/D2-8-wrapper.md 8b1): what crosses between a game and
//! its deciders, its saves and the browser. They are COPIES of `ti4-engine-legacy`'s types with an identical serde and `ts` shape;
//! `tests/wire_parity.rs` is the contract that keeps them equal (a leaf crate shared by both engines was the alternative, DECISION 2).
//! Until the swap commit the public paths (`ti4_engine::choice::Choice` ...) are still the legacy ones; the swap is a `pub use wire::...`.

pub mod choice;
pub mod decision_context;
pub mod dice;
pub mod event;
pub mod fingerprint;
pub mod observation;
pub mod preview;
pub mod rng;
