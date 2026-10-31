//! The activation answer (`Game::apply_tactical`, the activation arm, up to its hook chain) and the free tactical
//! action (`Game::begin_free_tactical`).

use crate::generated::{PromissoryNote, System};
use crate::ids::Seat;
use crate::kernel::fx;
use crate::kernel::fx_move;
use crate::kernel::stage::{Illegal, Sx};
use crate::kernel::Kernel;
use crate::query::{board, transit};
use crate::state::{CoreState, EventKind, Key, NoteKey, Payload, Val};

use super::{Machine, Plain};

/// `tactical::activation_options_with`: the systems on offer, `None` when the seat has no token or no system.
pub(super) fn ask(core: &CoreState, k: &Kernel, seat: Seat) -> Option<Vec<System>> {
    let p = core.players().by_seat(seat)?;
    if p.econ.tactic_tokens <= 0 {
        return None;
    }
    let list = transit::activatable(core, k, seat, holds_leave(core, k, seat))?;
    (!list.is_empty()).then_some(list)
}

/// Mahact's commander lets the seat activate a system that holds its own command token (`promissory::has_commander_ability(.., "mahactcommander")`).
#[must_use]
pub fn holds_leave(core: &CoreState, k: &Kernel, seat: Seat) -> bool {
    crate::query::leaders::has_commander_ability_with(core, seat, crate::generated::Leader::IlNaViroset, || k.hooks.direct.mahact_imperia_grants(core, seat, crate::generated::Leader::IlNaViroset))
}

/// The activation: the token, the typed placement event, the notes that return, the typed activation event.
pub(super) fn run(m: &mut Machine, sx: &mut Sx<'_>, system: System) -> Result<(), Illegal> {
    let seat = m.seat;
    // The system already held the seat's token: only Mahact's commander gets this far, and the hook chain returns both tokens.
    m.held_before = sx.state.board().sys(system).cmd.contains(seat);
    let leave = holds_leave(sx.state, sx.kernel, seat);
    fx_move::activate(sx.state, seat, system, leave).map_err(|_| Illegal)?;
    let placed = Payload::new().with(Key::Player, Val::Seat(seat)).with(Key::System, Val::System(system)).with(Key::Pool, Val::Code(m.facts.pool));
    let _ = m.emit(sx, fx::event(EventKind::CommandTokenPlaced, placed));
    m.note(sx, Plain::Typed(EventKind::CommandTokenPlaced));
    for owner in spend_support(sx.state, seat, system) {
        m.note(sx, Plain::SupportReturned(owner));
    }
    sx.state.transient_mut().movement.gravleash_move_values = Default::default();
    m.note(sx, Plain::SystemActivated(system));
    activated(m, sx, seat, system);
    Ok(())
}

/// The free tactical action: no token, an already-tokened system is fine (TE Warfare, Z'eu).
pub(super) fn free(m: &mut Machine, sx: &mut Sx<'_>, system: System) {
    let seat = m.seat;
    for owner in spend_support(sx.state, seat, system) {
        m.note(sx, Plain::SupportReturned(owner));
    }
    fx_move::begin_move(sx.state, system);
    m.note(sx, Plain::FreeActivated);
    activated(m, sx, seat, system);
    m.note(sx, Plain::FreeTactical);
}

fn activated(m: &mut Machine, sx: &mut Sx<'_>, seat: Seat, system: System) {
    let payload = Payload::new().with(Key::Player, Val::Seat(seat)).with(Key::System, Val::System(system));
    let _ = m.emit(sx, fx::event(EventKind::SystemActivated, payload));
    m.note(sx, Plain::Typed(EventKind::SystemActivated));
}

/// Who has a unit in the system, in the space area or on a planet.
fn present(core: &CoreState, system: System) -> Vec<Seat> {
    let mut seats: Vec<Seat> = board::space_units(core, system).iter().map(|p| p.owner()).collect();
    for planet in board::planets_of(core, system) {
        seats.extend(board::planet_units(core, planet).iter().map(|p| p.owner()));
    }
    seats.sort_unstable();
    seats.dedup();
    seats
}

/// The one seat playing `faction` (`None`: nobody, or two seats).
fn owner_of(core: &CoreState, faction: crate::generated::Faction) -> Option<Seat> {
    let mut seats = core.players().iter().filter(|p| p.faction == faction).map(|p| p.seat);
    let first = seats.next()?;
    seats.next().is_none().then_some(first)
}

/// `promissory::spend_support_on_activation`: the Supports lent to the activator whose owner has units in the system
/// go home (and their point with them), then the faceup Alliances, Trade Convoys, Blood and Dark Pacts and Antiviruses
/// of owners that have units there. Returns the Support owners, in seat order.
pub(super) fn spend_support(core: &mut CoreState, activator: Seat, system: System) -> Vec<Seat> {
    let here = present(core, system);
    let owners: Vec<Seat> = core
        .trade()
        .support_holders
        .iter()
        .filter(|(owner, holder)| **holder == activator && here.contains(owner))
        .map(|(owner, _)| *owner)
        .collect();
    for owner in &owners {
        super::super::trade::return_support(core, *owner);
    }
    for note in [PromissoryNote::Alliance, PromissoryNote::TradeConvoys, PromissoryNote::BloodPact, PromissoryNote::DarkPact, PromissoryNote::Antivirus] {
        let back: Vec<NoteKey> = core
            .cards()
            .promissory
            .iter()
            .filter(|(key, holder)| {
                **holder == activator
                    && key.note == note
                    && core.cards().promissory_faceup.contains_key(key)
                    && owner_of(core, key.faction).is_some_and(|owner| owner != activator && here.contains(&owner))
            })
            .map(|(key, _)| *key)
            .collect();
        for key in back {
            let _ = super::super::trade::give_back(core, key);
        }
    }
    owners
}
