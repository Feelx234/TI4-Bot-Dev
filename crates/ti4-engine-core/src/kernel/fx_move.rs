//! `fx_move`: the state changes of the tactical action that no other area shares (package A2b): the tactic
//! token and the activation, the boosts of a move, the unit moves of a sail and the loss of a ship with its
//! hold. Each is the string engine's function of the same name, line for line (`tactical::activate`,
//! `technology::use_gravity_drive`, `legendary::use_ionian`, `transit::apply_move`, `exploration::flip_ion_storm`).
//! Nothing here asks a question or emits an event.

use crate::generated::{Planet, System, Technology, Unit, Wormhole};
use crate::ids::Seat;
use crate::keys::{Atom, DecimalTextOrd, MarkKey, MarkVal};
use crate::state::{CoreState, PendingTag, Piece, SysFlags};

/// Why an activation was refused (`tactical::TacticalError`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivateErr {
    /// The seat holds a command token in the system (89.1b) and has no Mahact commander's leave.
    AlreadyActivated,
    /// The tactic pool is empty (or the seat does not exist).
    NoTacticToken,
}

/// `Player::spend_token(Tactic)`: whether the pool had a token.
pub fn spend_tactic_token(s: &mut CoreState, who: Seat) -> bool {
    let Some(p) = s.players_mut().by_seat_mut(who) else { return false };
    if p.econ.tactic_tokens > 0 {
        p.econ.tactic_tokens -= 1;
        true
    } else {
        false
    }
}

/// `tactical::activate` (89.1): spend a tactic token, place a command token in the system, make it the active
/// system, mark the pending move and count the activation. `mahact_leave`: the seat's commander lets it activate a
/// system holding its token (the string checks `has_commander_ability`).
///
/// # Errors
/// [`ActivateErr`]; nothing has changed then.
pub fn activate(s: &mut CoreState, who: Seat, system: System, mahact_leave: bool) -> Result<(), ActivateErr> {
    if s.board().sys(system).cmd.contains(who) && !mahact_leave {
        return Err(ActivateErr::AlreadyActivated);
    }
    if !spend_tactic_token(s, who) {
        return Err(ActivateErr::NoTacticToken);
    }
    let cell = s.board_mut().sys_mut(system);
    cell.flags.set(SysFlags::TOUCHED, true);
    cell.cmd.insert(who);
    begin_move(s, system);
    Ok(())
}

/// The part of an activation every path shares (the free tactical action of a card or an ability has no token):
/// the active system, the pending tag, one more activation.
pub fn begin_move(s: &mut CoreState, system: System) {
    s.meta_mut().active_system = Some(system);
    s.pending_mut().tag = Some(PendingTag::Move);
    let meta = s.meta_mut();
    meta.activation_seq = meta.activation_seq.saturating_add(1);
}

/// `technology::gravity_drive_available`.
#[must_use]
pub fn gravity_drive_available(s: &CoreState, who: Seat) -> bool {
    s.players()
        .by_seat(who)
        .is_some_and(|p| p.tech.owned.contains(Technology::GravityDrive) && !p.timers.gravity_drive_used_activation.is(s.meta().activation_seq))
}

/// `technology::use_gravity_drive`.
pub fn use_gravity_drive(s: &mut CoreState, who: Seat) -> bool {
    if !gravity_drive_available(s, who) {
        return false;
    }
    let activation = s.meta().activation_seq;
    match s.players_mut().by_seat_mut(who) {
        Some(p) => {
            p.timers.gravity_drive_used_activation = crate::containers::Opt32::some(activation);
            true
        }
        None => false,
    }
}

/// `legendary::ionian_available`: the seat controls Tempesta and its card is ready.
#[must_use]
pub fn ionian_available(s: &CoreState, who: Seat) -> bool {
    s.players().by_seat(who).is_some_and(|p| !p.cards.legendary_exhausted.contains_key(&Planet::Tempesta))
        && crate::query::board::controls(s, who, Planet::Tempesta)
}

/// `legendary::use_ionian`.
pub fn use_ionian(s: &mut CoreState, who: Seat) -> bool {
    if !ionian_available(s, who) {
        return false;
    }
    if let Some(p) = s.players_mut().by_seat_mut(who) {
        p.cards.legendary_exhausted.insert(Planet::Tempesta, ());
    }
    true
}

/// Letnev's Gravleash Maneuvers: the highest printed-plus-bonus move among the ships already selected to move
/// from `origin` (`game::begin_one_move`).
pub fn anchor_gravleash(s: &mut CoreState, origin: System, own_move: i32) {
    let map = &mut s.transient_mut().movement.gravleash_move_values;
    let value = map.get(&origin).map_or(own_move, |v| (*v).max(own_move));
    map.insert(origin, value);
}

/// `GameState::move_units` for one ship: the first equal piece leaves the space area of `origin`, the piece joins
/// the space area of `destination` (a system gets its board entry by being written).
pub fn move_ship(s: &mut CoreState, origin: System, destination: System, piece: Piece) {
    remove_first(s, origin, None, piece);
    push_space(s, destination, piece);
}

/// A passenger lifted out of its pickup system (`transit::take_aboard`): a ship of the space area keeps its place
/// in the destination's list order; a ground unit leaves its planet and joins the destination's space area.
pub fn take_aboard(s: &mut CoreState, from: System, destination: System, piece: Piece, source: Option<Planet>) {
    remove_first(s, from, source, piece);
    push_space(s, destination, piece);
}

/// `GameState::destroy_units` for the ship that a gravity rift took (`transit::apply_move`, the lost branch).
pub fn destroy_ship(s: &mut CoreState, origin: System, piece: Piece) {
    remove_first(s, origin, None, piece);
}

/// 95.1b: a passenger goes down with the ship (`faction_techs::note_destroyed` counts a Spec Ops II).
pub fn lose_passenger(s: &mut CoreState, from: System, piece: Piece, source: Option<Planet>) {
    remove_first(s, from, source, piece);
    if piece.kind() == Unit::SpecOpsII
        && let Some(p) = s.players_mut().by_seat_mut(piece.owner())
    {
        p.misc.spec_ops_destroyed += 1;
    }
}

fn remove_first(s: &mut CoreState, system: System, planet: Option<Planet>, piece: Piece) {
    // `system_mut` writes an entry for a system the board did not list.
    s.board_mut().sys_mut(system).flags.set(SysFlags::TOUCHED, true);
    let list = match planet {
        Some(p) => &mut s.board_mut().planet_mut(p).units,
        None => &mut s.board_mut().sys_mut(system).units,
    };
    if let Some(at) = list.as_slice().iter().position(|u| *u == piece) {
        list.remove(at);
    }
}

fn push_space(s: &mut CoreState, system: System, piece: Piece) {
    let cell = s.board_mut().sys_mut(system);
    cell.flags.set(SysFlags::TOUCHED, true);
    cell.units.push(piece);
}

/// `exploration::flip_ion_storm`: a move that used the storm's wormhole turns the token over. Whether it flipped.
pub fn flip_ion_storm(s: &mut CoreState, from: System, to: System) -> bool {
    let Some((system, face)) = s.meta().ion_storm else { return false };
    if system != from && system != to {
        return false;
    }
    let flipped = if face == Wormhole::Alpha { Wormhole::Beta } else { Wormhole::Alpha };
    s.meta_mut().ion_storm = Some((system, flipped));
    true
}

/// `private:#moved:{activation}`: the ships that arrived in this activation (`note_arrival`). Returns the count now.
pub fn count_arrival(s: &mut CoreState) -> u32 {
    let key = MarkKey::PrivateMoved(DecimalTextOrd(s.meta().activation_seq));
    let marks = s.marks_mut().tail.edit();
    let moved = match marks.get(&key) {
        Some(MarkVal::Atom(Atom::Num(n))) => *n,
        _ => 0,
    };
    marks.insert(key, MarkVal::Atom(Atom::Num(moved + 1)));
    moved + 1
}

/// The count of `private:#moved:{activation}` (`factions::ships_moved_this_activation`).
#[must_use]
pub fn ships_moved(s: &CoreState) -> u32 {
    let key = MarkKey::PrivateMoved(DecimalTextOrd(s.meta().activation_seq));
    match s.marks().tail.get(&key) {
        Some(MarkVal::Atom(Atom::Num(n))) => *n,
        _ => 0,
    }
}

/// The count is dropped once the movement step's windows have read it (`finish_tactical_inner`).
pub fn clear_arrivals(s: &mut CoreState) {
    let key = MarkKey::PrivateMoved(DecimalTextOrd(s.meta().activation_seq));
    s.marks_mut().tail.edit().remove(&key);
}
