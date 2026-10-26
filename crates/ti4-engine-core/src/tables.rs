//! Typed content as the movement slice reads it: views over the generated tables
//! (`crate::generated`, written by `cargo xtask gen-cards`; docs/CONTENT_GENERATOR.md).

use crate::generated::{Source, System, SystemData, Unit, UnitData, Wormhole};

pub const SYSTEM_COUNT: usize = System::COUNT;
pub const UNIT_COUNT: usize = Unit::COUNT;
pub const WORMHOLE_KIND_COUNT: usize = Wormhole::COUNT;

/// One bit per wormhole kind, in the enum's (text) order.
pub const fn wormhole_bit(kind: Wormhole) -> u8 {
    1 << (kind as u8)
}
pub const WH_ALPHA: u8 = wormhole_bit(Wormhole::Alpha);
pub const WH_BETA: u8 = wormhole_bit(Wormhole::Beta);
/// Bits of the alpha and beta kinds (Wormhole Reconstruction, Lost Star Chart, Nexus Sovereignty).
pub const WH_ALPHA_BETA: u8 = WH_ALPHA | WH_BETA;

/// The printed wormholes of a system as bits.
#[must_use]
pub fn printed_wormholes(system: System) -> u8 {
    system.data().wormholes.iter().fold(0, |m, k| m | wormhole_bit(*k))
}

/// The Wormhole Nexus faces (ids `82a`, `82b`), whose alpha and beta Nexus Sovereignty switches
/// off (`Galaxy::wormhole_kinds` tests the id prefix `82`; the bridge tests the two agree).
#[must_use]
pub const fn is_nexus(system: System) -> bool {
    matches!(system, System::MalliceLockedNexus | System::MalliceNexus)
}

/// Flags of a system record, for `has`.
pub struct SystemFlags;

impl SystemFlags {
    pub const NEBULA: u8 = 1;
    pub const SUPERNOVA: u8 = 1 << 1;
    pub const ASTEROID_FIELD: u8 = 1 << 2;
    pub const GRAVITY_RIFT: u8 = 1 << 3;
    pub const HYPERLANE: u8 = 1 << 4;
    pub const SCAR: u8 = 1 << 5;
}

/// Flag tests on a generated system row.
pub trait Flags {
    fn has(&self, flag: u8) -> bool;
}

impl Flags for SystemData {
    #[inline]
    fn has(&self, flag: u8) -> bool {
        let a = &self.anomalies;
        match flag {
            SystemFlags::NEBULA => a.nebula,
            SystemFlags::SUPERNOVA => a.supernova,
            SystemFlags::ASTEROID_FIELD => a.asteroid_field,
            SystemFlags::GRAVITY_RIFT => a.gravity_rift,
            SystemFlags::HYPERLANE => self.hyperlane,
            SystemFlags::SCAR => a.scar,
            _ => false,
        }
    }
}

/// The bit of a source in a [`Scope`].
#[must_use]
pub const fn source_bit(source: Source) -> u32 {
    1 << (source as u32)
}

/// A set of sources (expansions) as a bit mask over [`source_bit`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Scope(pub u32);

impl Scope {
    #[inline]
    #[must_use]
    pub fn system(self, system: System) -> Option<&'static SystemData> {
        let data = system.data();
        (source_bit(data.source) & self.0 != 0).then_some(data)
    }

    #[must_use]
    pub fn unit(self, unit: Unit) -> Option<&'static UnitData> {
        let data = unit.data();
        (source_bit(data.source) & self.0 != 0).then_some(data)
    }
}

/// `UnitType::moves_as_ship`: a ship, or a space-only structure with a move value.
#[must_use]
pub fn moves_as_ship(data: &UnitData) -> bool {
    data.ship || (data.structure && data.space_only && data.movement.unwrap_or(0) > 0)
}

/// The printed lanes of each hyperlane tile after its turn, by [`System`] index; supplied by the
/// adapter (the lane table is printed data of `ti4_content::hyperlane`, not in the corpus JSON).
pub type LaneTable = [Option<&'static [(u8, u8)]>];
