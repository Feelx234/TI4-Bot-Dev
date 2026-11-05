//! `Board`: systems and planets as chunked copy-on-write arrays indexed by the generated enums.
//! A clone bumps 25 refcounts; a unit move copies one chunk of 16 cells.

use std::sync::{Arc, OnceLock};

use crate::bits::{Bits, SysSet};
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

/// A set of planets.
pub type PlanetSet = Bits<{ Planet::COUNT.div_ceil(64) }>;

/// Owners are `Seat`s below 16 (`Piece` stores the owner in four bits).
const OWNERS: usize = 16;

/// What the planet cells say, as sets, so that "the planets the seat controls" and "where the seat has units" are a copy of one set and not a scan of
/// the 159 cells (`Board::planet_index`). A function of the cells: it is built from them on first use and dropped by the next edit access.
#[derive(Debug, Clone)]
pub struct PlanetIndex {
    /// Planets with a `placed_in` (an effect put them into another system).
    pub placed: PlanetSet,
    /// Planets with the `EXHAUSTED` flag.
    pub exhausted: PlanetSet,
    /// `controlled[seat]`: the planets whose `control` is the seat.
    pub controlled: [PlanetSet; OWNERS],
    /// `with_units[seat]`: the planets holding a unit of the seat.
    pub with_units: [PlanetSet; OWNERS],
}

/// The same for the system cells.
#[derive(Debug, Clone)]
pub struct SysIndex {
    /// `with_units[seat]`: the systems whose space area holds a unit of the seat.
    pub with_units: [SysSet; OWNERS],
    /// `with_token[seat]`: the systems holding a command token of the seat.
    pub with_token: [SysSet; OWNERS],
}

impl PlanetIndex {
    fn of(planets: &Chunked<PlanetCell, 16, PLANET_CHUNKS>) -> Self {
        let mut ix = Self { placed: PlanetSet::EMPTY, exhausted: PlanetSet::EMPTY, controlled: [PlanetSet::EMPTY; OWNERS], with_units: [PlanetSet::EMPTY; OWNERS] };
        for (i, cell) in planets.iter().take(Planet::COUNT).enumerate() {
            if cell.placed_in.is_some() {
                ix.placed.insert(i);
            }
            if cell.flags.has(PlanetFlags::EXHAUSTED) {
                ix.exhausted.insert(i);
            }
            if let Some(seat) = cell.control {
                ix.controlled[usize::from(seat.0)].insert(i);
            }
            for owner in owners(cell.units.as_slice()).iter() {
                ix.with_units[owner].insert(i);
            }
        }
        ix
    }
}

impl SysIndex {
    fn of(systems: &Chunked<SysCell, 16, SYS_CHUNKS>) -> Self {
        let mut ix = Self { with_units: [SysSet::EMPTY; OWNERS], with_token: [SysSet::EMPTY; OWNERS] };
        for (i, cell) in systems.iter().take(System::COUNT).enumerate() {
            for owner in owners(cell.units.as_slice()).iter() {
                ix.with_units[owner].insert(i);
            }
            for seat in 0..OWNERS {
                if cell.cmd.contains(Seat(seat as u8)) {
                    ix.with_token[seat].insert(i);
                }
            }
        }
        ix
    }
}

/// The owners of a unit list, as a mask over the owner numbers.
fn owners(units: &[Piece]) -> OwnerMask {
    OwnerMask(units.iter().fold(0u16, |m, p| m | p.owner().bit()))
}

#[derive(Clone, Copy)]
struct OwnerMask(u16);

impl OwnerMask {
    fn iter(self) -> impl Iterator<Item = usize> {
        let mut rest = self.0;
        std::iter::from_fn(move || {
            if rest == 0 {
                return None;
            }
            let bit = rest.trailing_zeros() as usize;
            rest &= rest - 1;
            Some(bit)
        })
    }
}

/// The board. Next to the cells it keeps two **derived indexes** (`PlanetIndex`, `SysIndex`): built from the cells on first use, shared by the clones
/// of a board (an `Arc`, so a clone still allocates nothing), and **dropped by `planet_mut` / `sys_mut`**, the only ways to write a cell (the fields are
/// private), so an index is never read after an edit it does not show. They are not part of the state: `Debug` and `==` see the cells only.
#[derive(Clone, Default)]
pub struct Board {
    sys: Chunked<SysCell, 16, SYS_CHUNKS>,
    planet: Chunked<PlanetCell, 16, PLANET_CHUNKS>,
    planet_ix: OnceLock<Arc<PlanetIndex>>,
    sys_ix: OnceLock<Arc<SysIndex>>,
}

impl std::fmt::Debug for Board {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Board").field("sys", &self.sys).field("planet", &self.planet).finish()
    }
}

impl PartialEq for Board {
    fn eq(&self, other: &Self) -> bool {
        self.sys == other.sys && self.planet == other.planet
    }
}
impl Eq for Board {}

impl Board {
    #[must_use]
    pub fn sys(&self, s: System) -> &SysCell {
        self.sys.get(s.index())
    }

    pub fn sys_mut(&mut self, s: System) -> &mut SysCell {
        self.sys_ix.take();
        self.sys.edit(s.index())
    }

    #[must_use]
    pub fn planet(&self, p: Planet) -> &PlanetCell {
        self.planet.get(p.index())
    }

    pub fn planet_mut(&mut self, p: Planet) -> &mut PlanetCell {
        self.planet_ix.take();
        self.planet.edit(p.index())
    }

    /// The sets the planet cells add up to (see [`PlanetIndex`]): equal to what a scan of [`Board::planets`] finds.
    #[must_use]
    pub fn planet_index(&self) -> &PlanetIndex {
        self.planet_ix.get_or_init(|| Arc::new(PlanetIndex::of(&self.planet)))
    }

    /// The sets the system cells add up to (see [`SysIndex`]).
    #[must_use]
    pub fn sys_index(&self) -> &SysIndex {
        self.sys_ix.get_or_init(|| Arc::new(SysIndex::of(&self.sys)))
    }

    /// The indexes recomputed from the cells (the definition they must agree with; the check of the random-play test).
    #[must_use]
    pub fn indexes_agree_with_a_scan(&self) -> bool {
        let (p, s) = (self.planet_index(), self.sys_index());
        let (fp, fs) = (PlanetIndex::of(&self.planet), SysIndex::of(&self.sys));
        p.placed == fp.placed && p.exhausted == fp.exhausted && p.controlled == fp.controlled && p.with_units == fp.with_units && s.with_units == fs.with_units && s.with_token == fs.with_token
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
