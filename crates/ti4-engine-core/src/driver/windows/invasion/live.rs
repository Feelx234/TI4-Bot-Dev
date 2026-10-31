//! The live face of the invasion: how it is opened, how the arm runs it, and the scoring window it pauses for.
//!
//! The arm is [`advance_invasion`], a variant of `driver::live::advance::<InvasionMachine>`: a machine that stopped for an event scoring
//! occurrence (`InvasionMachine::pause`) stays on the stack while the window of the occurrence is opened above it; the generic `advance` would close it
//! (a `Next` that is not a question ends the window).

use super::InvasionMachine;
use crate::choice::Pick;
use crate::containers::SmallList;
use crate::driver::live::{ask, Asking, Env, Live, LiveError, Out};
use crate::driver::{Driver, Status};
use crate::generated::System;
use crate::ids::Seat;
use crate::kernel::stage::Held;
use crate::rules::scoring_book::ScoringBook;
use crate::state::{CoreState, IBombard, IReport, IStage, Invasion, NoteHold, Scoring, ScoringTiming, Window as OpenWindow};

/// Open an invasion of `system` by `invader` (the aftermath's `next_step`): the payload on the stack. The first run announces `INVASION_BEGAN`,
/// writes the public boundary and rolls the bombardment.
pub fn open_invasion(core: &mut CoreState, invader: Seat, system: System, notes: NoteHold) {
    let m = InvasionMachine::new(Invasion {
        invader,
        system,
        stage: IStage::Begin,
        report: IReport::default(),
        scoring: Vec::new(),
        ground_occurrence: None,
        notes,
        bombard: IBombard::default(),
        extra_commits: Vec::new(),
        temp_commits: Vec::new(),
        combat_only: false,
    });
    m.to_pending(core);
}

/// Open an invasion at its "Commit Ground Forces" step, skipping bombardment (49.1), as the Sardakk hero does (`at_commit_step`): no dice, no hits;
/// the custodians offer still comes first. The caller owns whatever follows the step.
pub fn open_at_commit(core: &mut CoreState, invader: Seat, system: System, notes: NoteHold) {
    let meta = core.meta_mut();
    meta.feat_occurrence_seq += 1;
    let occurrence = meta.feat_occurrence_seq;
    let m = InvasionMachine::new(Invasion {
        invader,
        system,
        stage: IStage::Custodians(crate::state::ICustodians::Check),
        report: IReport::default(),
        scoring: Vec::new(),
        ground_occurrence: None,
        notes,
        bombard: IBombard { plan: Vec::new(), index: 0, occurrence, announced: true, cursor: None, walked: true, walk: None },
        extra_commits: Vec::new(),
        temp_commits: Vec::new(),
        combat_only: false,
    });
    m.to_pending(core);
}

/// The scoring window the aftermath opens for an occurrence (`open_occurrence_event_scoring(Timing::Action, ..)`): every seat in initiative order,
/// the combat's occurrences capped at one scoring per player.
#[must_use]
pub fn scoring_window(core: &CoreState, occurrence: u64, combat: bool) -> OpenWindow {
    let order: Vec<Seat> = crate::query::seats::initiative_order(core).iter().copied().collect();
    let book = ScoringBook::new(&order, false, combat, true);
    let mut pending = SmallList::new();
    for seat in book.pending() {
        pending.push(*seat);
    }
    OpenWindow::EventScoring(Scoring {
        pending,
        public_used: book.public_used(),
        secret_used: book.secret_used(),
        plot_card: None,
        timing: ScoringTiming::Action,
        occurrence: Some(occurrence),
        one_per_player: combat,
    })
}

/// The question the open invasion waits on (the arm's `AskFn`).
///
/// # Errors
/// As [`ask`].
pub fn ask_invasion(core: &CoreState, env: &Env<'_>) -> Result<Option<Asking>, LiveError> {
    ask::<InvasionMachine>(core, env)
}

/// Run the open invasion to its next question, its next scoring pause, or its end (the arm's `LiveFn`). `Live::Done` is the end of the STEP: the
/// invasion has left the stack, or (a pause) it stays on it with the window of the occurrence opened above it.
///
/// # Errors
/// See [`LiveError`]; the position is then what it was before the call.
pub fn advance_invasion(core: &mut CoreState, env: &Env<'_>, answer: Option<Pick>) -> Result<(Live, Out), LiveError> {
    let before = core.clone();
    let result = advance_inner(core, env, answer);
    if result.is_err() {
        *core = before;
    }
    result
}

fn driver_of(window: InvasionMachine, core: &CoreState, env: &Env<'_>) -> Driver<InvasionMachine> {
    let mut d = Driver::from_state(window, core, env.registry.clone(), env.table.clone()).with_seeds(env.seeds.clone());
    let m = core.meta();
    d.resolver.set_phase(m.phase);
    d.resolver.set_active_player(m.active);
    d.resolver.set_speaker(Some(m.speaker));
    d.resolver.set_seating_order(core.seats().seating_order());
    d.resolver.set_counters(u64::from(m.round), u64::from(m.turn_seq));
    d
}

fn advance_inner(core: &mut CoreState, env: &Env<'_>, answer: Option<Pick>) -> Result<(Live, Out), LiveError> {
    let window = InvasionMachine::from_pending(core, env.kernel).ok_or(LiveError::NotOpen)?;
    let mut d = driver_of(window, core, env);
    let nested = core.pending_mut().nested.take();
    let status = match (nested, answer) {
        (Some(n), Some(pick)) => {
            d.restore_nested(&n);
            let Status::Asking(choice) = d.reask(core, env.kernel)? else { return Err(LiveError::NotAsking) };
            if !is_option(&choice, &pick) {
                return Err(LiveError::Illegal);
            }
            d.resume(core, env.kernel, pick)?
        }
        (Some(n), None) => {
            d.restore_nested(&n);
            d.reask(core, env.kernel)?
        }
        (None, Some(pick)) => {
            if !d.restore_stage(core, env.kernel) {
                return Err(LiveError::NotAsking);
            }
            d.resume(core, env.kernel, pick)?
        }
        (None, None) => d.run(core, env.kernel)?,
    };
    if let Some(why) = d.unknown() {
        return Err(LiveError::Unknown(why));
    }
    let mut out = Out { emitted: std::mem::take(&mut d.emitted), opened: std::mem::take(&mut d.opened) };
    d.store(core);
    match status {
        Status::Asking(choice) => {
            d.window.to_pending(core);
            core.pending_mut().nested = d.nested_journal();
            Ok((Live::Asks(Box::new(crate::driver::live::asking_of(&d, choice))), out))
        }
        Status::Done => {
            if let Some((occurrence, combat)) = d.window.pause {
                // Stopped for an event scoring occurrence: stay open, the window of the occurrence goes above.
                d.window.to_pending(core);
                core.pending_mut().nested = None;
                let w = scoring_window(core, occurrence, combat);
                core.pending_mut().open(w.clone());
                out.opened.push(w);
                return Ok((Live::Done, out));
            }
            d.window.closed(core, env.kernel);
            core.pending_mut().close(InvasionMachine::KIND);
            core.pending_mut().nested = None;
            Ok((Live::Done, out))
        }
    }
}

/// Whether `pick` answers `choice`: one of its options, or a number to a count question.
fn is_option(choice: &crate::choice::Choice, pick: &Pick) -> bool {
    match pick {
        Pick::Opt(o) => choice.opts.iter().any(|x| x == o),
        Pick::Number(_) => matches!(choice.ask, crate::choice::Ask::Generic(crate::choice::generic::Ask::Count { .. })),
    }
}
