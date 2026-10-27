//! The Crimson Rebellion: Sundered, Sever, the breaches and the Resonance Generator.

use ti4_engine_core::generated::{Breakthrough, Faction, System, Wormhole};
use ti4_engine_core::hooks::{HookTable, MoveSite};
use ti4_engine_core::hooks::movement::{Pairs, Systems, WormSet};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::keys::MarkKey;

use ti4_engine_core::state::CoreState;

use crate::common::{holds_breakthrough, is_faction, mark, player, rec, system};

pub fn register(t: &mut HookTable) {
    t.movement.usable_wormhole_kinds.push(usable_wormhole_kinds);
    t.movement.severed_systems.push(severed_systems);
    t.movement.linked_systems.push(linked_systems);
    t.movement.move_bonus.push(move_bonus);
}

/// Sundered: "You cannot use wormholes other than epsilon wormholes."
fn usable_wormhole_kinds(s: &CoreState, who: Seat) -> Option<WormSet> {
    is_faction(s, who, Faction::TheCrimsonRebellion).then(|| {
        let mut kinds = WormSet::new();
        kinds.insert(Wormhole::Epsilon);
        kinds
    })
}

/// Sever: the system of every sever token (the mark's value is `<seat>|<system>`).
fn severed_systems(s: &CoreState) -> Systems {
    let mut out = Systems::new();
    for (key, value) in s.marks().tail.iter() {
        if matches!(key, MarkKey::CrimsonSever(..))
            && let Some([_, sys]) = rec(value)
            && let Some(sys) = system(sys)
        {
            out.push(sys);
        }
    }
    out
}

fn is_active(s: &CoreState, sys: System) -> bool {
    s.explore().breach.contains(sys) && mark(s, MarkKey::CrimsonBreachActive(sys)).is_some()
}

/// Active breaches are adjacent to each other (for every player).
fn linked_systems(s: &CoreState, _: &Kernel, _: Seat) -> Pairs {
    let active: Vec<System> = s.explore().breach.iter().filter(|b| mark(s, MarkKey::CrimsonBreachActive(*b)).is_some()).collect();
    let mut pairs = Pairs::new();
    for (i, a) in active.iter().enumerate() {
        for b in &active[i + 1..] {
            pairs.push((*a, *b));
        }
    }
    pairs
}

/// The Resonance Generator: +1 from the home system or an active breach.
fn move_bonus(s: &CoreState, site: MoveSite) -> i32 {
    if !holds_breakthrough(s, site.player, Breakthrough::ResonanceGenerator) {
        return 0;
    }
    let at_home = player(s, site.player).and_then(|p| p.home_system) == Some(site.origin);
    i32::from(at_home || is_active(s, site.origin))
}
