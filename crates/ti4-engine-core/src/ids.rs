//! Typed ids: indices into the generated tables (or into a game's seat table).
//!
//! `Ord` is the index order, which `build.rs` makes equal to the text order of the ids, so a
//! sorted collection of indices is sorted the way the string engine sorts the ids.

/// A system of the corpus (`tables::SYSTEMS`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SystemIx(pub u16);

/// A planet of the corpus (`tables::PLANETS`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlanetIx(pub u16);

/// A unit type of the corpus (`tables::UNITS`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UnitIx(pub u16);

/// A seat (or any unit owner) of one game: an index into that game's owner table, which the
/// adapter builds in text order of the owner ids. At most 16 per game.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Seat(pub u8);

impl SystemIx {
    #[must_use]
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

impl UnitIx {
    #[must_use]
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

impl PlanetIx {
    #[must_use]
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

impl Seat {
    #[must_use]
    pub const fn bit(self) -> u16 {
        1 << self.0
    }
}
