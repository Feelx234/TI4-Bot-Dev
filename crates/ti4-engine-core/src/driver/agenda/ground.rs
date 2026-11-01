//! The ground forces an agenda's effect destroyed, announced (`hooks_ground::try_announce_staged_events`): the effect removes the units and
//! STAGES a row per ground force that fell (`fx_ground::stage_destroyed`); the next step's reconcile turns each into a
//! `GROUND_FORCE_DESTROYED` through the reaction windows (Infantry II, Saar's Trade Convoy ...), in the order they were staged.
//!
//! The rows of an agenda's effect (`DestroyReason::Agenda`: their cause text is one of the agenda codes) and of the other effects that stage a
//! ground force and leave it for the next flush ([`GROUND_OTHER`]: Plague, Unstable Planet, Nova Seed, the Nekro hero) are announced here, all in
//! the order staged, as the string's flush does. A row of a cause nobody gave a text to is left for the lane that stages it.

use crate::driver::turn::inline::{halted, Flow};
use crate::kernel::agenda_codes::{AgendaCodes, GROUND_OTHER};
use crate::kernel::stage::Sx;
use crate::keys::{Atom, DestroyReason, MarkKey, MarkVal, Word};
use crate::state::{Code, CoreState, EventKind, Key, Payload, Val};

/// The text of the cause a staged ground force carries: an agenda's, or one of [`GROUND_OTHER`]; `None` for a cause nobody announces here.
fn cause_code(codes: &AgendaCodes, reason: DestroyReason) -> Option<Code> {
    match reason {
        // (An agenda's index is below its count; the guard spares an index panic.)
        DestroyReason::Agenda(agenda) => codes.ground_cause.get(agenda as usize).copied(), // mutants:skip: unreachable
        other => GROUND_OTHER.iter().position(|r| *r == other).map(|at| codes.ground_other[at]),
    }
}

/// The staged rows of the ground destructions to announce, in staging order: the key, the payload of the event.
fn staged(sx: &Sx<'_>) -> Vec<(MarkKey, Payload)> {
    let codes = sx.table.codes.agenda;
    let core: &CoreState = sx.state;
    let mut rows: Vec<(u32, MarkKey, Payload)> = Vec::new();
    for (key, value) in core.marks().tail.iter() {
        let MarkKey::PrivateStagedGround(n) = key else { continue };
        let MarkVal::Rec(f) = value else { continue };
        let [MarkVal::Atom(Atom::Word(Word::Destroyed)), MarkVal::Atom(Atom::System(system)), MarkVal::Atom(Atom::Planet(planet)), MarkVal::Atom(Atom::Seat(owner)), MarkVal::Atom(Atom::Unit(unit)), MarkVal::Atom(Atom::Word(damaged)), MarkVal::Atom(Atom::Destroy(reason)), rest @ ..] = f.as_slice() else { continue };
        let Some(cause) = cause_code(&codes, *reason) else { continue };
        let mut payload = Payload::new()
            .with(Key::System, Val::System(*system))
            .with(Key::Planet, Val::Planet(*planet))
            .with(Key::Player, Val::Seat(*owner))
            .with(Key::Unit, Val::Unit(*unit))
            .with(Key::Damaged, Val::Flag(*damaged == Word::True));
        if rest.iter().any(|v| matches!(v, MarkVal::Atom(Atom::Word(Word::Galvanized)))) {
            payload.set(Key::Galvanized, Val::Flag(true));
        }
        payload.set(Key::Cause, Val::Code(cause));
        rows.push((*n, *key, payload));
    }
    rows.sort_by_key(|(n, _, _)| *n);
    rows.into_iter().map(|(_, key, payload)| (key, payload)).collect()
}

/// Announce the staged ground destructions of agenda effects, in the order staged (a reaction may stage more).
pub fn announce(sx: &mut Sx<'_>) -> Flow {
    loop {
        let rows = staged(sx);
        if rows.is_empty() {
            return Flow::Done;
        }
        for (key, payload) in rows {
            if halted(sx) {
                return Flow::Halt;
            }
            sx.state.marks_mut().tail.edit().remove(&key);
            if sx.emit_mirrored(crate::kernel::fx::event(EventKind::GroundForceDestroyed, payload)).is_err() {
                if !halted(sx) { // mutants:skip: a refusal on a wet tape is a defect no position reaches; a dry tape rolls the step back whatever is noted
                    crate::driver::turn::inline::gap(sx, crate::kernel::turn_codes::TurnGap::EmissionRefused);
                }
                return Flow::Halt;
            }
        }
    }
}
