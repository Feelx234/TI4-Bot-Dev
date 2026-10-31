//! The Vuil'raith Cabal: Dimensional Tear, Crucible and the Al'Raith breakthrough.

use ti4_engine_core::generated::{Breakthrough, Faction, System, TileBack, Unit};
use ti4_engine_core::hooks::{HookTable, MoveSite};
use ti4_engine_core::hooks::movement::{Systems};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::keys::MarkKey;
use ti4_engine_core::state::{CoreState, Piece};

use crate::common::{any_piece_in, holds_breakthrough, is_faction, mark, num, systems_with_piece};

pub fn register(t: &mut HookTable) {
    t.movement.gravity_rifts.push(tear_systems);
    t.movement.rift_roll_exempt.push(tear_exempts_roll);
    t.movement.rift_crucible.push(crucible_in_effect);
    t.movement.move_bonus.push(alraith_move_bonus);
}

fn is_tear(s: &CoreState, p: Piece) -> bool {
    matches!(p.kind(), Unit::DimensionalTearI | Unit::DimensionalTearII)
        && is_faction(s, p.owner(), Faction::TheVuilraithCabal)
}

/// Every system that holds a dimensional tear is a gravity rift for everyone.
fn tear_systems(s: &CoreState) -> Systems {
    let mut out = Systems::new();
    // Asked per query: a game with no Cabal seat has no tear, so do not scan the board for one.
    if !s.players().iter().any(|p| p.faction == Faction::TheVuilraithCabal) {
        return out;
    }
    for i in systems_with_piece(s, |p| is_tear(s, p)).iter() {
        out.push(ti4_engine_core::ids::system_at(i));
    }
    out
}

/// A dimensional tear stands in the system (`cabal::owns_tear` for any owner).
pub(crate) fn has_tear(s: &CoreState, system: System) -> bool {
    any_piece_in(s, system, |p| is_tear(s, p))
}

/// Some system holds a dimensional tear (`!cabal::tear_systems(..).is_empty()`).
pub(crate) fn any_tear(s: &CoreState) -> bool {
    !tear_systems(s).is_empty()
}

/// "Your ships do not roll for this gravity rift": the owner of the tear in the system.
fn tear_exempts_roll(s: &CoreState, mover: Seat, system: System) -> bool {
    any_piece_in(s, system, |p| p.owner() == mover && is_tear(s, p))
}

/// Crucible: stamped with this activation.
fn crucible_in_effect(s: &CoreState, mover: Seat) -> bool {
    mark(s, MarkKey::CabalCrucible(mover)).and_then(num) == Some(s.meta().activation_seq)
}

/// Al'Raith Ix Ianovar: +1 for ships leaving a Fracture system.
fn alraith_move_bonus(s: &CoreState, site: MoveSite) -> i32 {
    i32::from(
        holds_breakthrough(s, site.player, Breakthrough::AlRaithIxIanovar)
            && site.origin.data().tile_back == Some(TileBack::Fracture),
    )
}
