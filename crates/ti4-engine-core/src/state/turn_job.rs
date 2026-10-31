//! The jobs of the turn flow that run as ONE step of the live loop (package KP, lane B): the per-turn prelude, the end of an action,
//! the end of a turn, a phase change and the announcement of the first strategy phase.
//!
//! A job asks its questions from inside the step (the reaction windows of the events it emits, the free technology windows, the
//! Minister of War); the driver rolls the step back at such a question and replays it with the answers given so far
//! (`Pending::nested`). So the payload is only WHICH job is open: everything else is the position the job started from.

/// A job of the turn flow, open on the `Pending` stack while its step is suspended on a question.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TurnJob {
    /// The reconcile hooks every step starts with (staged events, gains, the nexus latch, Styx, Spec Ops rolls).
    Reconcile,
    /// The start of a turn: the start-of-turn technologies, the commander unlocks, Military Support, Spec Ops, `TURN_BEGAN`.
    Prelude,
    /// The end of an action: `ACTION_COMPLETED`, then the closing menu or the end of the turn.
    Finish,
    /// The end of the turn: Minister of War, the end-of-turn windows, the fleet sweep, the turn passes on.
    End,
    /// A phase change (`step_phase`): the typed phase events and the round's bookkeeping.
    Phase,
    /// The first strategy phase announces itself (`STRATEGY_PHASE_BEGAN`).
    Announce,
    /// A strategy card is taken off the mat at the draft (`take_timed_strategy_card`): `STRATEGY_CARD_CHOSEN` with its reaction windows.
    Draft(crate::generated::StrategyCard),
}
