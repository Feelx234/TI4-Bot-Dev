//! The ground forces an agenda's effect destroyed, announced (`hooks_ground::try_announce_staged_events`): the effect removes the units and
//! STAGES a row per ground force that fell (`fx_ground::stage_destroyed`); the next step's reconcile turns each into a
//! `GROUND_FORCE_DESTROYED` through the reaction windows (Infantry II, Saar's Trade Convoy ...), in the order they were staged.
//!
//! Only the rows of an agenda's effect (`DestroyReason::Agenda`: their cause text is one of the agenda codes) and of UNIT.DSGN.FLAYESH (`nekrohero`)
//! are announced here. A row of another cause is left for the lane that stages it (the action cards, the technologies).

use crate::driver::turn::inline::{halted, Flow};
use crate::kernel::agenda_codes::GROUND_CAUSES;
use crate::kernel::stage::Sx;
use crate::keys::{Atom, DestroyReason, MarkKey, MarkVal, Word};
use crate::state::{CoreState, EventKind, Key, Payload, Val};

/// The staged rows of an agenda's ground destructions, in staging order: the key, the payload of the event.
fn staged(sx: &Sx<'_>) -> Vec<(MarkKey, Payload)> {
    let codes = sx.table.codes.agenda;
    let core: &CoreState = sx.state;
    let mut rows: Vec<(u32, MarkKey, Payload)> = Vec::new();
    for (key, value) in core.marks().tail.iter() {
        let MarkKey::PrivateStagedGround(n) = key else { continue };
        let MarkVal::Rec(f) = value else { continue };
        let [MarkVal::Atom(Atom::Word(Word::Destroyed)), MarkVal::Atom(Atom::System(system)), MarkVal::Atom(Atom::Planet(planet)), MarkVal::Atom(Atom::Seat(owner)), MarkVal::Atom(Atom::Unit(unit)), MarkVal::Atom(Atom::Word(damaged)), MarkVal::Atom(Atom::Destroy(reason)), rest @ ..] = f.as_slice() else { continue };
        let cause = match reason {
            DestroyReason::Agenda(agenda) => {
                let at = *agenda as usize;
                if at >= GROUND_CAUSES { // mutants:skip: unreachable (an agenda's index is below its count), the guard spares an index panic
                    continue;
                }
                codes.ground_cause[at]
            }
            // UNIT.DSGN.FLAYESH (Nekro's hero) is announced the same way, its own cause.
            DestroyReason::LeaderBare(crate::generated::Leader::UNITDSGNFLAYESH) => sx.table.codes.turn.nekrohero,
            _ => continue,
        };
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
