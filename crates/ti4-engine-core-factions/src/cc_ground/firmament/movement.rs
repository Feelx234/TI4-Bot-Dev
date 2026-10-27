//! The Firmament: the Heaven's Eye bonus and the agent.

use ti4_engine_core::generated::Unit;
use ti4_engine_core::hooks::{HookTable, MoveSite};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::keys::MarkKey;
use ti4_engine_core::query::board;
use ti4_engine_core::state::CoreState;

use crate::common::{flagship_has_text, player, stamped_for};

pub fn register(t: &mut HookTable) {
    t.movement.move_bonus.push(move_bonus);
    t.movement.unladen_pass.push(unladen_pass);
}

/// `firmament::puppeted`: everyone holding a token on one of `owner`'s plots.
fn puppeted(s: &CoreState, owner: Seat) -> ti4_engine_core::containers::SeatSet {
    let mut out = ti4_engine_core::containers::SeatSet::EMPTY;
    if let Some(p) = player(s, owner) {
        for plot in p.cards.plots.as_slice() {
            for t in plot.tokens.iter() {
                out.insert(t);
            }
        }
    }
    out
}

/// Heaven's Eye: +1 when the active system holds units of a player the owner has puppeted.
fn move_bonus(s: &CoreState, site: MoveSite) -> i32 {
    if !flagship_has_text(s, site.player, site.ship, Unit::HeavensEye) {
        return 0;
    }
    let Some(active) = s.meta().active_system else { return 0 };
    let puppets = puppeted(s, site.player);
    let held = s.board().sys(active).units.as_slice().iter().any(|p| puppets.contains(p.owner()))
        || board::planets_of(s, active)
            .any(|planet| s.board().planet(planet).units.as_slice().iter().any(|p| puppets.contains(p.owner())));
    i32::from(held)
}

/// The agent: unladen ships of the player the mark names may pass other players' ships.
fn unladen_pass(s: &CoreState, who: Seat) -> bool {
    stamped_for(s, MarkKey::FirmamentAgent(), who)
}
