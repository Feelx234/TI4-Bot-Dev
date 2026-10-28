//! The Nomad: Memoria and the hero.

use ti4_engine_core::generated::{Unit, UnitBaseType};
use ti4_engine_core::hooks::{HookTable};
use ti4_engine_core::hooks::movement::{Systems};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::keys::MarkKey;
use ti4_engine_core::state::CoreState;

use crate::common::{flagship_has_text, mark, num, scope, systems_with_piece};

pub fn register(t: &mut HookTable) {
    t.movement.unit_adjacent_systems.push(memoria_adjacent);
    t.movement.ignores_command_tokens.push(hero_ignores_command_tokens);
}

fn is_flagship(ship: Unit) -> bool {
    matches!(ship, Unit::MemoriaI | Unit::MemoriaII)
}

/// Memoria: "treat this unit as if it were adjacent to systems that contain 1 or more of your mechs."
fn memoria_adjacent(s: &CoreState, k: &Kernel, who: Seat, ship: Unit) -> Systems {
    let mut out = Systems::new();
    if !is_flagship(ship) && !flagship_has_text(s, who, ship, Unit::MemoriaI) {
        return out;
    }
    let scope = scope(k);
    let set = systems_with_piece(s, |p| {
        p.owner() == who && scope.unit(p.kind()).is_some_and(|d| d.base_type == UnitBaseType::Mech)
    });
    for i in set.iter() {
        out.push(ti4_engine_core::ids::system_at(i));
    }
    out
}

/// The hero: the flagship and its cargo may leave command-token systems this round.
fn hero_ignores_command_tokens(s: &CoreState, _: &Kernel, who: Seat, ship: Unit) -> bool {
    is_flagship(ship) && mark(s, MarkKey::NomadHero(who)).and_then(num) == Some(s.meta().round)
}
