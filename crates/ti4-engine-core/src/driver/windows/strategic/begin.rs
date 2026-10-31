//! The strategic action chosen from the action menu (`Game::apply_choice`, the `Strategic` option; `TurnJob::Strategic`): the card is taken,
//! `STRATEGIC_ACTION_BEGAN` gives the table its chance to cancel it (Coup d'Etat), the primary ability is performed by the card's own window and
//! the follower window opens (or, for Thunder's Edge Warfare, the free tactical action begins and the follower window waits for its end).
//!
//! The menu that offers the option is lane F's (`driver::menu`): whoever answers the menu with a strategic action calls
//! [`begin_strategic`], which runs this job in the same call (the string does the whole of it in the step of the answer).

use crate::driver::live::{Env, Live, LiveError, Out};
use crate::driver::turn::end::run_end;
use crate::driver::turn::inline::{emit_typed, gap, player_payload, sync_clock, Flow};
use crate::driver::turn::advance_job;
use crate::generated::StrategyCard;
use crate::ids::Seat;
use crate::kernel::stage::Sx;
use crate::kernel::turn_codes::TurnGap;
use crate::rules::strategy;
use crate::state::{CoreState, EventKind, Key, PlainArg, PlainKind, PlainRec, TransientFlags, TurnJob, Val, Window as OpenWindow};

use super::{perform, secondary_of};

/// The seats after `from` in seating order, clockwise (`GameState::clockwise_from(..).skip(1)`).
#[must_use]
pub fn others_clockwise(core: &CoreState, from: Seat) -> Vec<Seat> {
    let order = core.seats().seating_order();
    match order.iter().position(|s| *s == from) {
        Some(start) => (1..order.len()).map(|step| order[(start + step) % order.len()]).collect(),
        // (A primary player who is not seated cannot be reached: the dispatch reads the active seat.)
        None => order.iter().copied().filter(|s| *s != from).collect(), // mutants:skip: unreachable
    }
}

/// `Game::apply_choice`'s arm for a strategic action: the job.
pub fn run_begin(sx: &mut Sx<'_>, card: StrategyCard) -> Flow {
    let Some(active) = sx.state.meta().active else {
        gap(sx, TurnGap::ReplayDiverged);
        return Flow::Halt;
    };
    // `begin_strategic_action`: the seat holds the card and has not used it.
    if !strategy::unused_cards(sx.state, active).as_slice().contains(&card) {
        gap(sx, TurnGap::ReplayDiverged);
        return Flow::Halt;
    }
    // Strategic is the one action Extreme Duress does not punish: it lifts quietly (`settle_extreme_duress(.., true)`).
    if let Some(p) = sx.state.players_mut().by_seat_mut(active) {
        p.econ.duress_by = None;
    }
    // "When another player would perform a strategic action": typed, and fired before anything is resolved, so Coup d'Etat can end the turn in
    // time to undo the action entirely. Cleared up front so a stale value from an interrupted step cannot veto this action.
    sx.state.transient_mut().misc.flags.clear(TransientFlags::STRATEGIC_CANCELLED);
    sync_clock(sx);
    if emit_typed(sx, EventKind::StrategicActionBegan, player_payload(active).with(Key::Card, Val::Strategy(card))) == Flow::Halt {
        return Flow::Halt;
    }
    if sx.state.transient().misc.flags.has(TransientFlags::STRATEGIC_CANCELLED) {
        // Coup d'Etat: nothing above changed the state, so passing the turn undoes the action exactly as the card says.
        sx.state.transient_mut().misc.flags.clear(TransientFlags::STRATEGIC_CANCELLED);
        sx.plain(PlainRec::one(PlainKind::StrategicActionCancelled, PlainArg::Card(card)));
        return run_end(sx);
    }
    let mut free = None;
    if perform::run(sx, active, card, true, &mut free) == Flow::Halt {
        return Flow::Halt;
    }
    if perform::resolved(sx, active, card.data().kind) == Flow::Halt {
        return Flow::Halt;
    }
    let followers = others_clockwise(sx.state, active);
    let window = secondary_of(active, card, &followers, true);
    match free {
        Some(_system) => {
            // Thunder's Edge Warfare: the follower window waits for the end of the free tactical action. The action itself is the tactical lane's
            // (`tactical::open_free`); until that window is native the job stops with a counted gap.
            sx.state.pending_mut().open(OpenWindow::StrategySecondaryAfterTactical(window));
            gap(sx, TurnGap::FreeTactical);
            return Flow::Halt;
        }
        None => sx.state.pending_mut().open(OpenWindow::StrategySecondary(window)),
    }
    sx.plain(PlainRec::bare(PlainKind::StrategicActionBegan));
    Flow::Done
}

/// The seat's answer to the action menu was a strategic action of `card`: run the job to its first question or its end.
///
/// # Errors
/// As [`advance_job`]; the gaps of the ability.
pub fn begin_strategic(core: &mut CoreState, env: &Env<'_>, card: StrategyCard) -> Result<(Live, Out), LiveError> {
    advance_job(TurnJob::Strategic(card), core, env, None)
}
