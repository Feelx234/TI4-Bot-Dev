//! The typed rules core: a prototype slice (docs/ENGINE_CORE.md).
//!
//! This crate knows no string ids. Systems, planets, unit types, seats and wormhole kinds are small
//! integers; sets of them are bitsets. The corpus is compiled in as the generated typed tables (`generated/`), with
//! ids numbered in the text order of the id, so integer order is the string order the engine's
//! `BTreeMap`s iterate in.
//!
//! The slice covers what the bots ask most: the board's adjacency ([`galaxy`]) and movement
//! reachability ([`movement`]: the route search of `ti4_engine::movement::MovementRules`, the same
//! budget, bars and modifiers). Everything that speaks strings, the wire, saves, option ids, the
//! faction hooks that decide the modifiers, stays in the string engine; `ti4-engine-core-bridge`
//! translates at the boundary.

pub mod bits;
pub mod board;
pub mod choice;
pub mod containers;
pub mod driver;
pub mod galaxy;
pub mod hex;
pub mod ids;
pub mod kernel;
pub mod keys;
pub mod movement;
pub mod query;
pub mod state;
pub mod tables;

#[rustfmt::skip]
pub mod generated;

pub use bits::{SysSet, UnitSet};
pub use ids::{PlanetIx, Seat, SystemIx, UnitIx};
pub use containers::{Chunked, Cow, DecimalTextOrd, Deck, Idx, Log, Opt32, SeatSet, Set, SmallList, SortedMap};
pub use kernel::hooks;
pub use state::{CoreState, Slice, SliceSet};
