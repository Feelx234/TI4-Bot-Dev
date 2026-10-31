//! The hook chain of an activation (`Game::apply_tactical`, the activation arm after `SYSTEM_ACTIVATED`): what happens between the activation
//! of a system and the movement step.
//!
//! Carried here: the two marks the string's stateful abilities write in the "after" window of `SYSTEM_ACTIVATED` and that the core faction crates
//! have not ported (the Sardakk agent T'ro remembers the system for the end of the action; the Winnu note that a tactical action began, which
//! Reclamation reads), E-Res Siphons (automatic) and Ceasefire (automatic; it denies the movement step).
//!
//! Not carried (a counted gap, `TacticalGap::ActivationHook`, decided by [`super::facts::activation_hook_held`]): the hooks that ask, pay or end
//! the turn: Spatial Conduit Cylinders, Scanlink Drone Network, Nullification Field, Mahact (Starlancer, the commander), Minister of Peace and
//! the Dominus Orb. Each needs its question as a stage of this chain.

use crate::generated::{Faction, System, Technology};
use crate::ids::Seat;
use crate::kernel::stage::Sx;
use crate::keys::{Atom, MarkKey, MarkVal, Word};
use crate::query::board;
use crate::state::{LeaderStatus, PlainArg, PlainKind, PlainRec};

/// What the chain decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum After {
    /// On to the movement step.
    Move,
    /// A Ceasefire denies the movement: the step is skipped and the action closes (`finish_tactical`).
    Close,
}

/// Run the chain for `seat`'s activation of `system`.
pub(super) fn run(sx: &mut Sx<'_>, seat: Seat, system: System) -> After {
    marks(sx, seat, system);
    e_res_siphons(sx, seat, system);
    if crate::query::promissory::denies_movement_into(sx.state, seat, system) && crate::rules::trade::use_ceasefire(sx.state, seat).unwrap_or(false) {
        sx.plain(PlainRec::bare(PlainKind::CeasefireUsed));
        return After::Close;
    }
    After::Move
}

/// The "after you activate" marks of the Sardakk agent and of the Winnu.
fn marks(sx: &mut Sx<'_>, seat: Seat, system: System) {
    let sardakk: Vec<Seat> = sx
        .state
        .players()
        .iter()
        .filter(|p| p.faction == Faction::SardakkNorr)
        .filter(|p| matches!(p.leaders.status.get(&crate::generated::Leader::Tro), Some(LeaderStatus::Readied | LeaderStatus::Exhausted)))
        .map(|p| p.seat)
        .collect();
    if !sardakk.is_empty() {
        sx.state.marks_mut().tail.edit().insert(MarkKey::SardakkAgentTactical(seat), MarkVal::Atom(Atom::System(system)));
    }
    if sx.state.players().by_seat(seat).is_some_and(|p| p.faction == Faction::TheWinnu) {
        sx.state.marks_mut().tail.edit().insert(MarkKey::PrivateSeat(seat, Word::WinnuTactical), MarkVal::Atom(Atom::Num(1)));
    }
}

/// E-Res Siphons (`faction_techs::e_res_siphons`): "After another player activates a system that contains 1 or more of your ships, gain 4 trade
/// goods." Not optional, not exhausted; the holders in seat order.
fn e_res_siphons(sx: &mut Sx<'_>, seat: Seat, system: System) {
    let holders: Vec<Seat> = sx
        .state
        .players()
        .iter()
        .map(|p| p.seat)
        .filter(|s| *s != seat)
        .filter(|s| board::space_units(sx.state, system).iter().any(|u| u.owner() == *s && u.kind().data().ship))
        .filter(|s| sx.state.players().by_seat(*s).is_some_and(|p| p.tech.owned.contains(Technology::EResSiphons)))
        .collect();
    let source = sx.table.codes.tactical.faction_technology;
    for holder in holders {
        crate::kernel::fx::gain_trade_goods_staged(sx.state, holder, 4, source);
        sx.plain(PlainRec::one(PlainKind::EResSiphons, PlainArg::Seat(holder)));
    }
}
