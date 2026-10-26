//! The occupancy facts movement reads, typed: who has ships, units and command tokens where.
//!
//! A per-system summary rather than the unit lists of `GameState::board`: owner bitmasks over the
//! game's seats, so "systems holding another player's ships" is one mask test per system.

use crate::bits::SysSet;
use crate::ids::{Seat, SystemIx, UnitIx};
use crate::tables::SYSTEM_COUNT;

/// One ship (or a unit that moves as one) in space, for the movable-units query.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShipAt {
    pub system: SystemIx,
    pub owner: Seat,
    pub unit: UnitIx,
    /// Position among the owner's ships in that system (`GameState::ships_of` order).
    pub index: u16,
}

#[derive(Debug, Clone, Default)]
pub struct CoreBoard {
    /// Per system: owners with at least one ship in space (bit per seat).
    pub ship_owners: Vec<u16>,
    /// Per system: owners with any unit there, in space or on a planet.
    pub unit_owners: Vec<u16>,
    /// Per system: owners with a command token there.
    pub command_tokens: Vec<u16>,
    /// Systems that are keys of the board (hold a `SystemState`).
    pub systems: SysSet,
    pub ships: Vec<ShipAt>,
}

impl CoreBoard {
    #[must_use]
    pub fn new() -> Self {
        Self {
            ship_owners: vec![0; SYSTEM_COUNT],
            unit_owners: vec![0; SYSTEM_COUNT],
            command_tokens: vec![0; SYSTEM_COUNT],
            systems: SysSet::EMPTY,
            ships: Vec::new(),
        }
    }

    /// Systems holding ships of anybody but `seat` (`movement::Board::for_player`).
    #[must_use]
    pub fn enemy_ships(&self, seat: Seat) -> SysSet {
        let mut out = SysSet::EMPTY;
        for i in self.systems.iter() {
            if self.ship_owners[i] & !seat.bit() != 0 {
                out.insert(i);
            }
        }
        out
    }

    /// Systems holding `seat`'s command tokens.
    #[must_use]
    pub fn own_command_tokens(&self, seat: Seat) -> SysSet {
        let mut out = SysSet::EMPTY;
        for i in self.systems.iter() {
            if self.command_tokens[i] & seat.bit() != 0 {
                out.insert(i);
            }
        }
        out
    }

    /// Systems holding any unit of `seat`.
    #[must_use]
    pub fn unit_systems(&self, seat: Seat) -> SysSet {
        let mut out = SysSet::EMPTY;
        for i in self.systems.iter() {
            if self.unit_owners[i] & seat.bit() != 0 {
                out.insert(i);
            }
        }
        out
    }
}
