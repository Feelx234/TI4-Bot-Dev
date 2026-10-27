//! State sync between the string `GameState` and the typed `CoreState` (docs/ENGINE_CORE_STATE.md 3.2,
//! package F2): lend with declared footprints.
//!
//! * [`Ctx`]: the per-game tables the codecs need and the core must not hold: the owner table
//!   ([`Owners`], seat `i` = the `i`-th owner id in text order) and the interner for open texts
//!   ([`Texts`], the `Code(u16)` values of the state).
//! * [`codec`]: one file per slice. `load` converts the slice from a `GameState` (other slices stay
//!   default, flagged unloaded); `store` writes the slice back **through the existing `GameState`
//!   structs**, so the JSON is still produced by the old serialiser from an old state.
//! * [`Footprint`]: the slices a ported entry point reads and writes (`const`).
//! * [`with_core`] / [`region`] / [`CoreView`]: the lend itself.
//!
//! A value the typed state cannot hold (an id the tables do not know, a counter out of range, a text
//! form the codec cannot reproduce byte for byte) is reported as [`Unmapped`]: the caller answers with
//! the string engine, exactly as `core_adapter` does. Nothing here decides a rule.

mod conv;
mod ctx;
mod error;
mod footprint;
mod lend;

pub mod codec;
/// Text codec of every `String`-typed key and value of `GameState` (package X1).
pub mod keys;

pub use ctx::{Ctx, Owners, Texts};
pub use error::{Unmapped, R};
pub use footprint::Footprint;
pub use codec::{Codec, Sources, SliceCodec};
pub use lend::{CoreView, Kernel, Lend, LendStats, Region, SliceCost, region, with_core};
pub use ti4_engine_core::state::{Slice, SliceSet};

#[cfg(test)]
mod tests;
