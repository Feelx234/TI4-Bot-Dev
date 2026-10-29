//! bastion: tier W: the Icon's part of a use of PRODUCTION (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CA; B12).
//!
//! | point | string function | here |
//! |---|---|---|
//! | the spots of a produced ship | `bastion::adjust_ship_spots` | `bastion_adjust_ship_spots` |
//! | a ship was placed away from the producing system | `bastion::ship_placed` | `bastion_ship_placed` |
//!
//! "When you produce ships, you may exhaust this card to place those ships in a system that contains 1 of your command tokens,
//! at least 1 of your ground forces, and no other player's ships." Until a ship is placed the Icon's systems are offered as
//! remote spots while the card is ready; once a ship of the use went to one the only spot is that one; once a ship was placed in
//! the producing system the Icon is not offered. Placing a ship in an Icon system exhausts the card.

use ti4_engine_core::choice::production::{Icon, Spot};
use ti4_engine_core::generated::{Breakthrough, System, Unit};
use ti4_engine_core::hooks::direct::SpotList;
use ti4_engine_core::hooks::HookTable;
use ti4_engine_core::ids::Seat;
use ti4_engine_core::keys::{Atom, MarkKey, MarkVal};
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::query::{board, fleet};
use ti4_engine_core::state::CoreState;

use crate::common::holds_breakthrough;

/// Fill the slots of this tier.
pub fn register(t: &mut HookTable) {
    t.direct.bastion_adjust_ship_spots.push(adjust_ship_spots);
    t.direct.bastion_ship_placed.push(ship_placed);
}

fn exhausted(s: &CoreState, who: Seat) -> bool {
    s.marks().tail.contains_key(&MarkKey::BastionIconExhausted(who))
}

/// Systems other than `producing` where `who`'s Icon could place ships: it holds a command token of the player, at least 1 of the
/// player's ground forces and no other player's ships (`bastion::icon_targets`), in system order.
fn icon_targets(s: &CoreState, who: Seat, producing: System) -> Vec<System> {
    System::ALL
        .iter()
        .copied()
        .filter(|system| {
            let cell = s.board().sys(*system);
            *system != producing
                && cell.cmd.contains(who)
                && !cell.units.as_slice().iter().any(|p| p.owner() != who && p.kind().data().ship)
                && cell.units.as_slice().iter().chain(board::planets_of(s, *system).flat_map(|pl| s.board().planet(pl).units.as_slice().iter())).any(|p| p.owner() == who && fleet::is_ground_force(p.kind()))
        })
        .collect()
}

fn adjust_ship_spots(s: &CoreState, _: &Kernel, who: Seat, producing: System, icon: Icon, spots: &mut SpotList) {
    if !holds_breakthrough(s, who, Breakthrough::TheIcon) {
        return;
    }
    match icon {
        Icon::Remote(spot) => {
            spots.clear();
            spots.push(spot);
            return;
        }
        Icon::Local => return,
        Icon::Fresh => {}
    }
    if exhausted(s, who) {
        return;
    }
    for system in icon_targets(s, who, producing) {
        let spot = Spot::Remote(system, None);
        if !spots.as_slice().contains(&spot) {
            spots.push(spot);
        }
    }
}

fn ship_placed(s: &mut CoreState, _: &Kernel, who: Seat, producing: System, target: System, unit: Unit) {
    if target == producing || !holds_breakthrough(s, who, Breakthrough::TheIcon) || exhausted(s, who) || !unit.data().ship || !icon_targets(s, who, producing).contains(&target) {
        return;
    }
    s.marks_mut().tail.edit().insert(MarkKey::BastionIconExhausted(who), MarkVal::Atom(Atom::Num(1)));
}
