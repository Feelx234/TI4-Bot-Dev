//! The Argent Flight: Aerie Hololattice and Aerie Sentinel.

use ti4_engine_core::generated::{System, Technology, Unit};
use ti4_engine_core::hooks::HookTable;
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::state::{CoreState, Piece};

use crate::common::{any_piece_in, has_tech_text, scope};

pub fn register(t: &mut HookTable) {
    t.movement.blocks_passage.push(blocks_passage);
    t.movement.free_cargo.push(free_cargo);
}

/// "Other players cannot move ships through systems that contain your structures."
fn blocks_passage(s: &CoreState, k: &Kernel, who: Seat, system: System) -> bool {
    let scope = scope(k);
    s.players().iter().any(|p| {
        p.seat != who
            && has_tech_text(s, p.seat, Technology::AerieHololattice)
            && any_piece_in(s, system, |u| u.owner() == p.seat && scope.unit(u.kind()).is_some_and(|d| d.structure))
    })
}

fn free_cargo(_: &CoreState, _: &Kernel, unit: Piece) -> bool {
    unit.kind() == Unit::AerieSentinel
}
