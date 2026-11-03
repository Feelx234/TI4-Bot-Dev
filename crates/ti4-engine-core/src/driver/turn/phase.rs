//! Phase changes: `Game::step_phase` (the typed phase events, `phase::advance_phase` and the round's bookkeeping) and the announcement
//! of the first strategy phase (`strategy_phase_announced`) as jobs.
//!
//! The experiment knobs of the string (`STATUS_INCOME_TRADE_GOODS`, `Game::round_income_trade_goods`: trade goods every seat gains as
//! a phase begins) are zero in every game a player plays; they are the harness's, not the core's, and are not ported.

use super::inline::{emit_typed, gap, sync_clock, Flow};
use crate::driver::turn_flow;
use crate::generated::{Agenda, StrategyCard};
use crate::kernel::stage::Sx;
use crate::kernel::turn_codes::TurnGap;
use crate::query::{laws, reach};
use crate::rules::strategy_flow;
use crate::state::{CoreState, DriverFlags, EventKind, MetaFlags, Outcome, Payload, Phase, PlainKind, PlainRec};

/// What `phase::advance_phase` did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Moved {
    /// The action phase began (the first seat in initiative order has the first turn).
    ActionBegan,
    StatusBegan,
    AgendaBegan,
    /// The round ended; the strategy phase of the next one follows.
    RoundEnded,
}

/// The strategy cards of the game in the order of the mat: by initiative, as the set lists them (`Game::strategy_cards`).
#[must_use]
pub fn mat_cards(core: &CoreState) -> Vec<StrategyCard> {
    let mut cards: Vec<(i32, StrategyCard)> = core.strategy().initiative.iter().map(|(card, n)| (*n, *card)).collect();
    cards.sort_by_key(|(n, _)| *n);
    cards.into_iter().map(|(_, card)| card).collect()
}

/// `phase::advance_phase`: move the game to its next phase.
fn advance_phase(sx: &mut Sx<'_>) -> Moved {
    match sx.state.meta().phase {
        Phase::Strategy => {
            strategy_flow::stock_unclaimed(sx.state);
            // Political Stability is spent here, the initiative tokens of the faction modules are placed (Naalu's "0"), then the order is read.
            turn_flow::reset_stability(sx.state);
            let (k, mut cx) = (sx.kernel, sx.cx());
            k.hooks.strategy.strategy_phase_ended(&mut cx);
            if let Some(first) = turn_flow::strategy_phase_flip(sx.state) {
                turn_flow::begin_action_turn(sx.state, first);
            }
            Moved::ActionBegan
        }
        Phase::Action => {
            turn_flow::action_phase_ended(sx.state);
            Moved::StatusBegan
        }
        Phase::Status => {
            // LRR 8.1, 27.4: the agenda phase exists only once the custodians token has been lifted, and then it runs every round.
            if sx.state.meta().flags.has(MetaFlags::CUSTODIANS_REMOVED) && !sx.state.meta().finished {
                sx.state.meta_mut().phase = Phase::Agenda;
                Moved::AgendaBegan
            } else {
                Moved::RoundEnded
            }
        }
        Phase::Agenda => Moved::RoundEnded,
    }
}

/// Anti-Intellectual Revolution (Against): "At the start of the next strategy phase, each player chooses and exhausts 1 planet for each
/// technology that they own." The string takes the planets in its order (`laws::revolution_levy`: system id, then planet id), no question.
fn revolution_levy(core: &mut CoreState) {
    if laws::elected(core, reach::blanked(core), Agenda::AntiIntellectualRevolution) != Some(Outcome::Against) {
        return;
    }
    let seats: Vec<_> = core.players().iter().map(|p| (p.seat, p.tech.owned.len())).collect();
    for (seat, owed) in seats {
        let ready: Vec<_> =
            crate::query::board::controlled_planets(core, seat).as_slice().iter().map(|(_, planet)| *planet).filter(|planet| !crate::query::board::exhausted(core, *planet)).take(owed).collect();
        for planet in ready {
            crate::kernel::fx::exhaust_planet(core, planet);
        }
    }
}

/// `Game::step_phase`.
pub fn run_phase(sx: &mut Sx<'_>) -> Flow {
    if sx.state.meta().phase == Phase::Action && !turn_flow::all_passed(sx.state) {
        // The string reports `MissingActivePlayer`: a position the loop never reaches (a seat that has not passed always has the turn).
        gap(sx, TurnGap::PhaseNotReady);
        return Flow::Halt;
    }
    // "At the end of the strategy phase" (Naalu Telepathic, Gift of Prescience): typed before the phase advances, while initiative can still
    // be overridden for the action phase.
    if sx.state.meta().phase == Phase::Strategy && emit_typed(sx, EventKind::StrategyPhaseEnded, Payload::new()) == Flow::Halt {
        return Flow::Halt;
    }
    match advance_phase(sx) {
        Moved::ActionBegan => {
            sx.plain(PlainRec::bare(PlainKind::ActionPhaseBegan));
            Flow::Done
        }
        Moved::StatusBegan => {
            sx.plain(PlainRec::bare(PlainKind::StatusPhaseBegan));
            // "At the start of the status phase" (Mitosis): the resolver learns the new phase first, so per-phase limits count in the status phase.
            sync_clock(sx);
            emit_typed(sx, EventKind::StatusPhaseBegan, Payload::new())
        }
        Moved::AgendaBegan => {
            sx.plain(PlainRec::bare(PlainKind::AgendaPhaseBegan));
            // Two cards read "at the start of the agenda phase". The resolver learns the new phase first (integration 18, see the round flip below): the
            // agenda phase orders its windows from the speaker, the status phase before it does not.
            sync_clock(sx);
            emit_typed(sx, EventKind::AgendaPhaseBegan, Payload::new())
        }
        Moved::RoundEnded => {
            let cards = mat_cards(sx.state);
            if turn_flow::begin_next_round(sx.state, &cards).is_err() {
                // A diplomacy promise book that cannot settle: a defect of the position (the feature is refused at creation).
                gap(sx, TurnGap::PromiseBook);
                return Flow::Halt;
            }
            let driver = &mut sx.state.pending_mut().driver;
            driver.set(DriverFlags::STATUS_RESOLVED, false);
            driver.set(DriverFlags::AGENDA_RESOLVED, false);
            sx.plain(PlainRec::bare(PlainKind::RoundBegan));
            // A round begins with the strategy phase, and cards read "at the start of the strategy phase". Only the typed event: the mirror of the
            // resolver's journal puts it into the queue, so a plain line as well would log the phase twice.
            revolution_levy(sx.state);
            // The resolver learns the new phase first (integration 18): its clock was still the phase before (the status phase, or the agenda phase once the
            // Custodians are taken), so the windows of the flip were ordered by THAT phase's rule: with the ruling of 2026-11-03 the status phase offers a
            // window without an active player from the seat after the speaker, the speaker last, where the strategy phase starts at the speaker.
            sync_clock(sx);
            emit_typed(sx, EventKind::StrategyPhaseBegan, Payload::new())
        }
    }
}

/// The first strategy phase announces itself (`step_flow`'s `strategy_announce` block).
pub fn run_announce(sx: &mut Sx<'_>) -> Flow {
    sx.state.pending_mut().driver.set(DriverFlags::STRATEGY_PHASE_ANNOUNCED, true);
    sync_clock(sx);
    emit_typed(sx, EventKind::StrategyPhaseBegan, Payload::new())
}
