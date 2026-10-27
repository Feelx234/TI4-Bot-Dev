//! The Ral Nel Consortium: structures are free cargo.

use ti4_engine_core::generated::Faction;
use ti4_engine_core::hooks::HookTable;
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::state::{CoreState, Piece};

use crate::common::{is_faction, scope};

pub fn register(t: &mut HookTable) {
    t.movement.free_cargo.push(free_cargo);
}

fn free_cargo(s: &CoreState, k: &Kernel, unit: Piece) -> bool {
    is_faction(s, unit.owner(), Faction::TheRalNelConsortium) && scope(k).unit(unit.kind()).is_some_and(|d| d.structure)
}
