//! Typed ids: the generated per-category enums (integer order = text order of the id), plus the
//! per-game seat index.

pub use crate::generated::{Planet as PlanetIx, System as SystemIx, Unit as UnitIx};

/// The system with this integer value (an index into a [`crate::SysSet`]).
#[inline]
#[must_use]
pub fn system_at(index: usize) -> SystemIx {
    SystemIx::from_index(index).expect("a system index is in range")
}

/// A seat (or any unit owner) of one game: an index into that game's owner table, which the
/// adapter builds in text order of the owner ids. At most 16 per game.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Seat(pub u8);

impl Seat {
    #[must_use]
    pub const fn bit(self) -> u16 {
        1 << self.0
    }
}
