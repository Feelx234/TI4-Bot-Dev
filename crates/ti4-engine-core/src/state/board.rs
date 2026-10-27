//! `Board`: systems and planets as chunked copy-on-write arrays indexed by the generated enums.
//! A clone bumps 25 refcounts; a unit move copies one chunk of 16 cells.

use crate::containers::{Chunked, SeatSet, SmallList};
use crate::generated::{Attachment, Planet, System};
use crate::ids::Seat;

use super::piece::Piece;

/// A unit list: the space area or a planet. Order is today's `Vec<Unit>` order (casualty option
/// order and JSON depend on it): never canonicalised.
pub type UnitList = SmallList<Piece, 12>;

/// Flags of a system cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SysFlags(pub u8);

impl SysFlags {
    /// The string state has a `board` entry for this system. An absent entry is not an empty
    /// system (`state.rs:1366-1384`): the flag lets the codec round-trip exactly.
    pub const TOUCHED: u8 = 1;
    /// `GameState.purged_systems`.
    pub const PURGED: u8 = 1 << 1;

    #[must_use]
    pub const fn has(self, flag: u8) -> bool {
        self.0 & flag == flag
    }

    pub fn set(&mut self, flag: u8, on: bool) {
        if on {
            self.0 |= flag;
        } else {
            self.0 &= !flag;
        }
    }
}

/// `SystemState` minus its per-planet maps (those are [`PlanetCell`]s).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SysCell {
    pub units: UnitList,
    pub cmd: SeatSet,
    pub flags: SysFlags,
}

/// Flags of a planet cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PlanetFlags(pub u8);

impl PlanetFlags {
    /// `GameState.exhausted_planets` contains the planet.
    pub const EXHAUSTED: u8 = 1;
    /// `SystemState.purged_planets` contains the planet.
    pub const PURGED: u8 = 1 << 1;
    /// `SystemState.planet_units` has an entry (possibly an empty list) for the planet.
    pub const UNITS_ENTRY: u8 = 1 << 2;
    /// `SystemState.coexisting` has an entry for the planet.
    pub const COEXIST_ENTRY: u8 = 1 << 3;
    /// `GameState.planet_attachments` has an entry (possibly empty) for the planet.
    pub const ATTACH_ENTRY: u8 = 1 << 4;

    #[must_use]
    pub const fn has(self, flag: u8) -> bool {
        self.0 & flag == flag
    }

    pub fn set(&mut self, flag: u8, on: bool) {
        if on {
            self.0 |= flag;
        } else {
            self.0 &= !flag;
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PlanetCell {
    pub control: Option<Seat>,
    pub units: UnitList,
    pub attach: SmallList<Attachment, 3>,
    pub coexist: SeatSet,
    /// `GameState.placed_planets`: the system the planet was placed into, if not its printed one.
    pub placed_in: Option<System>,
    pub flags: PlanetFlags,
}

/// Chunks of 16: 15 for the 231 systems (240 cells), 10 for the 159 planets (160 cells).
const SYS_CHUNKS: usize = System::COUNT.div_ceil(16);
const PLANET_CHUNKS: usize = Planet::COUNT.div_ceil(16);

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Board {
    sys: Chunked<SysCell, 16, SYS_CHUNKS>,
    planet: Chunked<PlanetCell, 16, PLANET_CHUNKS>,
}

impl Board {
    #[must_use]
    pub fn sys(&self, s: System) -> &SysCell {
        self.sys.get(s.index())
    }

    pub fn sys_mut(&mut self, s: System) -> &mut SysCell {
        self.sys.edit(s.index())
    }

    #[must_use]
    pub fn planet(&self, p: Planet) -> &PlanetCell {
        self.planet.get(p.index())
    }

    pub fn planet_mut(&mut self, p: Planet) -> &mut PlanetCell {
        self.planet.edit(p.index())
    }

    /// Every system cell in system order (cells past `System::COUNT` are padding).
    pub fn systems(&self) -> impl Iterator<Item = (System, &SysCell)> {
        System::ALL.iter().map(|s| (*s, self.sys(*s)))
    }

    pub fn planets(&self) -> impl Iterator<Item = (Planet, &PlanetCell)> {
        Planet::ALL.iter().map(|p| (*p, self.planet(*p)))
    }

    /// How many of the 25 chunks are shared with `other`.
    #[must_use]
    pub fn shared_chunks(&self, other: &Self) -> usize {
        self.sys.shared_chunks(&other.sys) + self.planet.shared_chunks(&other.planet)
    }
}
