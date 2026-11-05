//! Running a window to its end INSIDE one step of a turn-flow job, and the small helpers every job uses.
//!
//! A job is one `Window::step`. The windows it composes (the free technology windows, the Minister of War, the legendary planets, the
//! fleet sweep) ask stage questions of their own; inside a job every question goes to the TAPE instead (`Sx::ask`): when the tape is
//! dry the driver rolls the whole step back and reports the question (`Driver::settle`), and `driver::live` stores the answers given
//! so far (`Pending::nested`) so a caller that returns between questions replays the job to the same place. Nothing of a half-run job
//! is ever kept: the payload on the stack is only which job it is.

use crate::choice::Pick;
use crate::kernel::ability::Unknown;
use crate::kernel::stage::{Sx, Window};
use crate::kernel::turn_codes::TurnGap;
use crate::kernel::{fx, Next};
use crate::state::{CoreState, EventKind, EventRec, Key, Payload, Val};

/// How a part of a job ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flow {
    /// It ran to its end.
    Done,
    /// The step is doomed: a question found the tape dry (the driver rolls the step back and asks it), or a gap was noted. The job
    /// returns at once and writes nothing more.
    Halt,
}

/// Whether the step can no longer succeed (a question is waiting for its answer, or a gap was noted).
#[must_use]
// mutants:skip: it only spares work the driver rolls back when the step is doomed; the questions and gaps are tested through the jobs.
pub fn halted(sx: &Sx<'_>) -> bool {
    sx.tape.needed().is_some() || sx.tape.unknown.get().is_some()
}

/// Note a gap: the live turn flow cannot follow the string here (a counted bail of the live path, never a silent skip).
pub fn gap(sx: &Sx<'_>, why: TurnGap) {
    if sx.tape.unknown.get().is_none() {
        sx.tape.unknown.set(Some(Unknown::TurnGap(why)));
    }
}

/// [`gap`], and `Flow::Halt` (the shape every job returns it in).
pub fn gap_flow(sx: &Sx<'_>, why: TurnGap) -> Flow {
    gap(sx, why);
    Flow::Halt
}

/// Run `w` to its end, putting its questions to the tape. The window must not open other windows.
pub fn run<W: Window>(sx: &mut Sx<'_>, w: &mut W) -> Flow {
    loop {
        if halted(sx) {
            return Flow::Halt;
        }
        let mut asked = false;
        crate::driver::probe::seen(w);
        for next in w.step(sx).flatten() {
            match next {
                Next::Done | Next::Chain(_) => {}
                Next::Open(_) => {
                    gap(sx, TurnGap::OpenedWindow);
                    return Flow::Halt;
                }
                Next::Emit(rec) => {
                    if sx.emit(rec).is_err() { // mutants:skip: no window of the turn flow returns `Next::Emit`
                        gap(sx, TurnGap::EmissionRefused);
                        return Flow::Halt;
                    }
                }
                Next::Continue(_) => {
                    let Some(choice) = w.ask_of(sx.state, sx.kernel) else {
                        gap(sx, TurnGap::ReplayDiverged);
                        return Flow::Halt;
                    };
                    let pick: Pick = sx.ask(&choice);
                    if halted(sx) {
                        return Flow::Halt;
                    }
                    if w.answer(sx, pick).is_err() {
                        gap(sx, TurnGap::ReplayDiverged);
                        return Flow::Halt;
                    }
                    asked = true;
                    break;
                }
            }
        }
        if !asked {
            crate::driver::probe::seen(w);
            return Flow::Done;
        }
    }
}

/// The resolver's clock is the position's own (what the string's `sync_timing_context` sets before a typed event of a phase or a turn).
// mutants:skip: it moves the resolver's frequency scopes (once per phase, per turn); the lockstep against the string covers the events it orders.
pub fn sync_clock(sx: &mut Sx<'_>) {
    let m = sx.state.meta();
    let (phase, active, speaker, round, turn) = (m.phase, m.active, m.speaker, m.round, m.turn_seq);
    let order = sx.state.seats().seating_order();
    sx.resolver.set_phase(phase);
    sx.resolver.set_active_player(active);
    sx.resolver.set_speaker(Some(speaker));
    sx.resolver.set_seating_order(order);
    sx.resolver.set_counters(u64::from(round), u64::from(turn));
}

/// A typed event the way the string's `emit_typed` sends it: through the reaction windows, mirrored into the event queue, and then the card moves a
/// faction effect staged in its windows (Scheming's discard) are announced (`Live::emit_typed` ends with `announce_staged_cards`, and every typed event of
/// the string goes through it). `Flow::Halt` when a question found the tape dry or the resolver refused.
pub fn emit_typed(sx: &mut Sx<'_>, kind: EventKind, payload: Payload) -> Flow {
    if emit_typed_bare(sx, kind, payload) == Flow::Halt {
        return Flow::Halt;
    }
    // (The marks are part of the lend of some windows only: nothing can be staged where they are not.)
    if sx.state.loaded.has(crate::state::Slice::Marks) { crate::driver::windows::status_finish::announce_staged_cards(sx) } else { Flow::Done }
}

/// [`emit_typed`] without the announcement of the staged card moves (the announcement itself emits typed events).
pub fn emit_typed_bare(sx: &mut Sx<'_>, kind: EventKind, payload: Payload) -> Flow {
    if halted(sx) {
        return Flow::Halt;
    }
    if sx.emit_mirrored(fx::event(kind, payload)).is_err() {
        // A dry tape rolls the step back whatever the emission returned; a refusal on a wet tape is a defect.
        if !halted(sx) { // mutants:skip: the refusal of the resolver is a defect no position reaches
            gap(sx, TurnGap::EmissionRefused);
        }
        return Flow::Halt;
    }
    if halted(sx) { Flow::Halt } else { Flow::Done }
}

/// The payload `{player: seat}` of the turn events.
#[must_use]
pub fn player_payload(seat: crate::ids::Seat) -> Payload {
    Payload::new().with(Key::Player, Val::Seat(seat))
}

/// `Game::events.last()` is the plain `kind` (the string reads the text of its last event).
#[must_use]
pub fn last_plain_is(core: &CoreState, kind: crate::state::PlainKind) -> bool {
    core.ledger.events.last().and_then(|e| e.plain()).is_some_and(|p| p.kind == kind)
}

/// The record of an event of `kind` with `payload` (id 0: the emitter takes the next trace id).
#[must_use]
pub fn event(kind: EventKind, payload: Payload) -> EventRec {
    fx::event(kind, payload)
}
