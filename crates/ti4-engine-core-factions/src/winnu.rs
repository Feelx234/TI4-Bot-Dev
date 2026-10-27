//! The Winnu: Lazax Gate Folding and the Imperator breakthrough.

use ti4_engine_core::generated::{Breakthrough, System, Technology};
use ti4_engine_core::hooks::{HookTable, MoveSite};
use ti4_engine_core::hooks::movement::{Pairs};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::keys::Word;
use ti4_engine_core::query::board;
use ti4_engine_core::state::{CoreState, SysFlags};
use ti4_engine_core::tables::WH_ALPHA_BETA;

use crate::common::{has_tech_text, holds_breakthrough, num, private, rec, system, unit};

pub fn register(t: &mut HookTable) {
    t.movement.linked_systems.push(linked_systems);
    t.movement.move_bonus.push(move_bonus);
}

/// `seating::mecatol_on`: the Thunder's Edge tile when the board has an entry for it.
fn mecatol_on(s: &CoreState) -> System {
    if s.board().sys(System::MecatolRex112).flags.has(SysFlags::TOUCHED) {
        System::MecatolRex112
    } else {
        System::MecatolRex18
    }
}

fn controls_mecatol(s: &CoreState, who: Seat) -> bool {
    board::planets_of(s, mecatol_on(s)).any(|p| board::controls(s, who, p))
}

/// Lazax Gate Folding: during the Winnu's tactical action, without Mecatol Rex, Mecatol Rex counts as
/// having an alpha and a beta wormhole.
fn linked_systems(s: &CoreState, _: &Kernel, who: Seat) -> Pairs {
    let mut pairs = Pairs::new();
    let Some(galaxy) = s.galaxy() else { return pairs };
    let meta = s.meta();
    if !has_tech_text(s, who, Technology::LazaxGateFolding)
        || meta.active != Some(who)
        || meta.active_system.is_none()
        || controls_mecatol(s, who)
    {
        return pairs;
    }
    let mecatol = if galaxy.coord_of(System::MecatolRex112).is_some() { System::MecatolRex112 } else { System::MecatolRex18 };
    if galaxy.coord_of(mecatol).is_none() {
        return pairs;
    }
    for other in galaxy.wormhole_systems().iter() {
        let other = ti4_engine_core::ids::system_at(other);
        if other != System::MecatolRex18 && other != System::MecatolRex112 && galaxy.wormhole_kinds(other) & WH_ALPHA_BETA != 0 {
            pairs.push((mecatol, other));
        }
    }
    pairs
}

/// Imperator: +1 for the one ship the recorded mark names.
fn move_bonus(s: &CoreState, site: MoveSite) -> i32 {
    let Some(index) = site.index else { return 0 };
    let Some(f) = private(s, site.player, Word::WinnuImperator).and_then(rec) else { return 0 };
    let [origin, kind, damaged, _count] = f else { return 0 };
    let (Some(origin), Some(kind), Some(damaged)) = (system(origin), unit(kind), num(damaged)) else { return 0 };
    let damaged = damaged == 1;
    let first = s
        .board()
        .sys(origin)
        .units
        .as_slice()
        .iter()
        .filter(|p| p.owner() == site.player)
        .position(|p| p.kind() == kind && p.sustained() == damaged);
    i32::from(
        origin == site.origin
            && first == Some(usize::from(index))
            && holds_breakthrough(s, site.player, Breakthrough::Imperator),
    )
}
