//! `Piece`: a unit on the board in 16 bits (today's `Unit` is 40 bytes with two `Arc<str>`).
//! (The design document calls it `Unit`; that name is the generated unit-type enum.)

use crate::generated::Unit;
use crate::ids::Seat;

/// Bits 8-15 unit type, 4-7 owner seat, bit 1 sustained damage, bit 0 galvanized. The derived order
/// is `(type, owner, sustained, galvanized)`, the field order of the string `Unit`'s `Ord`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Piece(u16);

impl Piece {
    #[must_use]
    pub fn new(kind: Unit, owner: Seat) -> Self {
        debug_assert!(owner.0 < 16);
        Self(((kind.index() as u16) << 8) | (u16::from(owner.0) << 4))
    }

    #[must_use]
    pub fn kind(self) -> Unit {
        Unit::from_index(usize::from(self.0 >> 8)).expect("a valid unit type")
    }

    #[must_use]
    pub const fn owner(self) -> Seat {
        Seat(((self.0 >> 4) & 15) as u8)
    }

    #[must_use]
    pub const fn sustained(self) -> bool {
        self.0 & 2 != 0
    }

    #[must_use]
    pub const fn galvanized(self) -> bool {
        self.0 & 1 != 0
    }

    #[must_use]
    pub const fn with_sustained(self, on: bool) -> Self {
        Self(if on { self.0 | 2 } else { self.0 & !2 })
    }

    #[must_use]
    pub const fn with_galvanized(self, on: bool) -> Self {
        Self(if on { self.0 | 1 } else { self.0 & !1 })
    }

    #[must_use]
    pub const fn with_owner(self, owner: Seat) -> Self {
        Self((self.0 & !0xF0) | ((owner.0 as u16) << 4))
    }

    /// The same piece (owner, damage and galvanize bits) in another unit form (an upgrade replaces the card).
    #[must_use]
    pub fn with_kind(self, kind: Unit) -> Self {
        Self((self.0 & 0xFF) | ((kind.index() as u16) << 8))
    }

    /// The packed bits (codec and hashing use).
    #[must_use]
    pub const fn bits(self) -> u16 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packs_and_orders_like_the_string_unit() {
        let a = Piece::new(Unit::ALL[3], Seat(2)).with_sustained(true);
        assert_eq!((a.kind(), a.owner(), a.sustained(), a.galvanized()), (Unit::ALL[3], Seat(2), true, false));
        let b = Piece::new(Unit::ALL[3], Seat(2)).with_galvanized(true);
        let c = Piece::new(Unit::ALL[3], Seat(3));
        let d = Piece::new(Unit::ALL[4], Seat(0));
        assert!(a > b); // (.., sustained=true, galv=false) > (.., false, true)
        assert!(b < c && c < d);
        assert_eq!(std::mem::size_of::<Piece>(), 2);
        assert_eq!(a.with_owner(Seat(9)).owner(), Seat(9));
    }
}
