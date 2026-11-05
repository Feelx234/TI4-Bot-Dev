//! The Ghosts of Creuss: the Hil Colish delta wormhole, Quantum Entanglement, Slipstream.

use ti4_engine_core::generated::{Faction, System, Unit, Wormhole};
use ti4_engine_core::hooks::{HookTable, MoveSite};
use ti4_engine_core::hooks::movement::{ExtraWormholes, Pairs};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::keys::MarkKey;
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::state::CoreState;
use ti4_engine_core::tables::{WH_ALPHA_BETA, wormhole_bit};

use crate::common::{flagship_has_text, is_faction, mark, num, player, rec, system};

pub fn register(t: &mut HookTable) {
    t.movement.extra_wormholes.push(extra_wormholes);
    t.movement.linked_systems.push(linked_systems);
    t.movement.move_bonus.push(move_bonus);
}

/// "This ship's system contains a delta wormhole." Space area only, as the string hook reads it.
fn extra_wormholes(s: &CoreState) -> ExtraWormholes {
    let mut out = ExtraWormholes::new();
    // (Only the systems that hold a unit at all: the board's index names them.)
    for sys in ti4_engine_core::query::board::systems_of(ti4_engine_core::query::board::systems_with_any_units(s)) {
        let cell = s.board().sys(sys);
        if cell.units.as_slice().iter().any(|p| flagship_has_text(s, p.owner(), p.kind(), Unit::HilColish)) {
            out.push((sys, Wormhole::Delta));
        }
    }
    out
}

/// The system an agent linked to every wormhole system this activation (`ghost::agent_link`).
fn agent_link(s: &CoreState) -> Option<System> {
    let f = mark(s, MarkKey::GhostAgentLink()).and_then(rec)?;
    let [seq, sys] = f else { return None };
    let sys = system(sys)?;
    (num(seq) == Some(s.meta().activation_seq) && s.meta().active_system == Some(sys)).then_some(sys)
}

fn linked_systems(s: &CoreState, _: &Kernel, who: Seat) -> Pairs {
    let mut pairs = Pairs::new();
    let Some(galaxy) = s.galaxy() else { return pairs };
    if is_faction(s, who, Faction::TheGhostsOfCreuss) {
        // "Game effects cannot prevent you": the kinds with Nexus Sovereignty's suppression lifted.
        let holders: Vec<System> = System::ALL
            .iter()
            .copied()
            .filter(|sys| galaxy.wormhole_kinds_unsuppressed(*sys) & WH_ALPHA_BETA != 0)
            .collect();
        for (i, a) in holders.iter().enumerate() {
            for b in &holders[i + 1..] {
                pairs.push((*a, *b));
            }
        }
    }
    if let Some(linked) = agent_link(s) {
        for other in galaxy.wormhole_systems().iter() {
            let other = ti4_engine_core::ids::system_at(other);
            if other != linked {
                pairs.push((linked, other));
            }
        }
    }
    pairs
}

/// `hooks_movement::has_alpha_or_beta`: the wormholes at a system as the map sees them plus the
/// ion storm's face and the wormhole tokens.
fn has_alpha_or_beta(s: &CoreState, sys: System) -> bool {
    let mut kinds = ti4_engine_core::tables::printed_wormholes(sys);
    for (kind, at) in s.explore().wormhole_tokens.iter() {
        if *at == sys {
            kinds |= wormhole_bit(*kind);
        }
    }
    if let Some((at, face)) = s.meta().ion_storm
        && at == sys
    {
        kinds |= wormhole_bit(face);
    }
    if s.meta().flags.has(ti4_engine_core::state::MetaFlags::NEXUS_UNLOCKED) && sys == System::MalliceLockedNexus {
        kinds |= WH_ALPHA_BETA;
    }
    // The pieces' own wormholes (Hil Colish's delta) are never alpha or beta.
    kinds & WH_ALPHA_BETA != 0
}

/// Slipstream: +1 for a Ghost ship that starts in the home system or at an alpha or beta wormhole.
fn move_bonus(s: &CoreState, site: MoveSite) -> i32 {
    let Some(p) = player(s, site.player) else { return 0 };
    i32::from(
        p.faction == Faction::TheGhostsOfCreuss
            && (p.home_system == Some(site.origin) || has_alpha_or_beta(s, site.origin)),
    )
}
