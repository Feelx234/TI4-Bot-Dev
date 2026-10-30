//! Containers of the core state (docs/ENGINE_CORE_STATE.md 1.2): sets over id enums, inline lists,
//! sorted small maps, copy-on-write cells, chunked arrays, append-only logs and decks.
//!
//! Rules every container follows: no `unsafe`, no strings, no maps from `std::collections`.
//! Cloning allocates nothing (except `Log`'s tail chunk on the first append after a clone, and
//! the heap spill of a list that outgrew its inline capacity).

mod chunked;
mod cow;
mod deck;
mod decimal;
mod idx;
mod interner;
mod log;
mod opt32;
mod set;
mod small_list;
mod sorted_map;

pub use chunked::Chunked;
pub use cow::Cow;
pub use deck::Deck;
pub use decimal::DecimalTextOrd;
pub use idx::Idx;
pub use interner::Interner;
pub use log::Log;
pub use opt32::Opt32;
pub use set::{Set, SeatSet};
pub use small_list::SmallList;
pub use sorted_map::SortedMap;
