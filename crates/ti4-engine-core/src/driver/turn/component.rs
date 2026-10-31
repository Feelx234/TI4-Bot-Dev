//! What every component action of the action menu shares (package KM2, lane F2b): the Extreme Duress a seat owes once it takes an action, the
//! announcements of what the body staged, and the end of the action (`COMPONENT_ACTION_RESOLVED` / `COMPONENT_ACTION_FAILED`, the failed-action
//! withholding, `finish_action`).
//!
//! The string functions this mirrors are the arms of `Game::apply_choice`'s `Phase::Action` branch (`faction|`, `component|leader|`,
//! `component|tech|`, `action_card|`, relics and Enigmatic Device cards): `play_*`/`perform_*`, then `announce_staged_destructions`,
//! `announce_staged_cards`, `settle_extreme_duress`, the verdict event and `finish_action`. Each route of [`super::action`] is a body that
//! returns whether the action resolved; [`conclude`] is everything after it.

use super::end::run_finish;
use super::inline::{emit_typed, sync_clock, Flow};
use crate::ids::Seat;
use crate::kernel::stage::Sx;
use crate::kernel::fx;
use crate::state::{Code, EventKind, Key, Payload, PlainArg, PlainKind, PlainRec, Val};

/// Extreme Duress settles when the target takes an action (`Game::settle_extreme_duress`). A strategic action is the one case the card does
/// not punish: the duress lifts quietly. Every other action triggers the punishment: the seat's trade goods go to the holder, every action card
/// left in its hand is discarded (and announced, one by one: a Reverse Engineer may take a component action). A pass neither triggers nor lifts it.
///
/// `Flow::Halt` when a discard's reaction window found the tape dry.
pub fn settle_extreme_duress(sx: &mut Sx<'_>, who: Seat, strategic: bool) -> Flow {
    let Some(by) = sx.state.players().by_seat(who).and_then(|p| p.econ.duress_by) else { return Flow::Done };
    if strategic {
        if let Some(p) = sx.state.players_mut().by_seat_mut(who) {
            p.econ.duress_by = None;
        }
        return Flow::Done;
    }
    let (goods, discarded) = {
        let Some(p) = sx.state.players_mut().by_seat_mut(who) else { return Flow::Done };
        let goods = i32::from(p.econ.trade_goods);
        p.econ.trade_goods = 0;
        // The cards are named one by one below, so they are taken out, not cleared away.
        let discarded: Vec<_> = p.cards.hand.as_slice().to_vec();
        p.cards.hand.clear();
        p.econ.duress_by = None;
        (goods, discarded)
    };
    if let Some(holder) = sx.state.players_mut().by_seat_mut(by) {
        holder.econ.trade_goods = holder.econ.trade_goods.saturating_add(i16::try_from(goods).unwrap_or(0));
    }
    let source = sx.table.codes.turn.extreme_duress;
    fx::stage_gain(sx.state, by, goods, source);
    sx.plain(PlainRec::one(PlainKind::ExtremeDuress, PlainArg::Seat(who)));
    for card in discarded {
        sx.state.cards_mut().discarded_action.edit().push(card);
        sync_clock(sx);
        let payload = Payload::new().with(Key::Player, Val::Seat(who)).with(Key::Card, Val::Action(card));
        if emit_typed(sx, EventKind::ActionCardDiscarded, payload) == Flow::Halt {
            return Flow::Halt;
        }
    }
    Flow::Done
}

/// A cause no staged destruction carries: `drain_staged` renames the destructions of a retreat, and a component action has none.
const NO_RETREAT: (Code, Code) = (Code(u16::MAX), Code(u16::MAX));

/// `announce_staged_destructions` and `announce_staged_cards` after a body: the ground forces and ships an effect destroyed without a resolver
/// in reach, then the card moves an effect staged.
pub fn announce_staged(sx: &mut Sx<'_>) -> Flow {
    if crate::driver::agenda::ground::announce(sx) == Flow::Halt {
        return Flow::Halt;
    }
    if !sx.state.transient().combat.pending_destructions.is_empty() {
        sync_clock(sx);
        crate::rules::combat::assign::drain_staged(sx, true, NO_RETREAT);
        if super::inline::halted(sx) {
            return Flow::Halt;
        }
    }
    crate::driver::windows::status_finish::announce_staged_cards(sx)
}

/// How a body ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// The body resolved: `COMPONENT_ACTION_RESOLVED`, then the end of the action.
    Resolved,
    /// The body could not resolve: `COMPONENT_ACTION_FAILED`, and (`withhold`) the option is not offered again this turn.
    Failed { withhold: bool },
}

/// Everything after a body (`Game::apply_choice`): what the body staged is announced, Extreme Duress settles, the verdict is told, and a
/// resolved action ends (`finish_action`). `key` is the option's key (the failed-action withholding).
pub fn conclude(sx: &mut Sx<'_>, who: Seat, key: u32, verdict: Verdict) -> Flow {
    if announce_staged(sx) == Flow::Halt || settle_extreme_duress(sx, who, false) == Flow::Halt {
        return Flow::Halt;
    }
    match verdict {
        Verdict::Resolved => {
            sx.plain(PlainRec::bare(PlainKind::ComponentActionResolved));
            run_finish(sx)
        }
        Verdict::Failed { withhold } => {
            sx.plain(PlainRec::bare(PlainKind::ComponentActionFailed));
            if withhold {
                sx.state.pending_mut().note_failed_key(key);
            }
            Flow::Done
        }
    }
}
