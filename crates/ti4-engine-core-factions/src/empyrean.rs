//! The Empyrean: Voidborn, Aetherstream, Aetherpassage and the Void Tether.

use ti4_engine_core::generated::{Breakthrough, Faction, System};
use ti4_engine_core::hooks::{HookTable, MoveSite};
use ti4_engine_core::hooks::movement::{Owners, Pairs};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::keys::MarkKey;
use ti4_engine_core::state::CoreState;

use crate::common::{holds_breakthrough, is_faction, mark, num, rec, seat, stamped_for, system};

pub fn register(t: &mut HookTable) {
    t.movement.move_bonus.push(move_bonus);
    t.movement.ignores_nebulae.push(ignores_nebulae);
    t.movement.passable_owners.push(passable_owners);
    t.movement.blocked_borders.push(blocked_borders);
}

fn is_empyrean(s: &CoreState, who: Seat) -> bool {
    is_faction(s, who, Faction::TheEmpyrean)
}

/// Aetherstream: +1 for the player the mark names this activation.
fn move_bonus(s: &CoreState, site: MoveSite) -> i32 {
    i32::from(stamped_for(s, MarkKey::EmpyreanAetherstream(), site.player))
}

/// Voidborn: "Nebulae do not affect your ships' movement."
fn ignores_nebulae(s: &CoreState, who: Seat) -> bool {
    is_empyrean(s, who)
}

/// Aetherpassage: the owner the mark lets `mover` pass this activation.
fn passable_owners(s: &CoreState, mover: Seat) -> Owners {
    let mut out = Owners::new();
    if let Some([stamp, who, owner]) = mark(s, MarkKey::EmpyreanPassage()).and_then(rec)
        && num(stamp) == Some(s.meta().activation_seq)
        && seat(who) == Some(mover)
        && let Some(owner) = seat(owner)
    {
        out.push(owner);
    }
    out
}

fn allowed_this_activation(s: &CoreState, viewer: Seat, owner: Seat) -> bool {
    mark(s, MarkKey::EmpyreanTetherAllow()).and_then(rec).is_some_and(|f| {
        matches!(f, [stamp, who, by]
            if num(stamp) == Some(s.meta().activation_seq) && seat(who) == Some(viewer) && seat(by) == Some(owner))
    })
}

/// Void Tether: the tethered borders of every other Empyrean holding the breakthrough, unless allowed.
fn blocked_borders(s: &CoreState, viewer: Seat) -> Pairs {
    let mut out = Pairs::new();
    for p in s.players().iter() {
        let owner = p.seat;
        if owner == viewer
            || !(is_empyrean(s, owner) && holds_breakthrough(s, owner, Breakthrough::VoidTether))
            || allowed_this_activation(s, viewer, owner)
        {
            continue;
        }
        if let Some(ti4_engine_core::keys::MarkVal::List(items)) = mark(s, MarkKey::EmpyreanTether(owner)) {
            for item in items {
                if let Some([a, b]) = rec(item)
                    && let (Some(a), Some(b)) = (system(a), system(b))
                {
                    let (a, b): (System, System) = if a < b { (a, b) } else { (b, a) };
                    out.push((a, b));
                }
            }
        }
    }
    out
}
