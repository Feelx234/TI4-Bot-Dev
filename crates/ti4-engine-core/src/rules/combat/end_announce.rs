//! The announcement of the end of a space combat (string `CombatWindow::announce_end`), as a [`Window`] that
//! never asks: `SPACE_COMBAT_ENDED` with its winner, losers, reason, retreats and lost ships
//! ([`super::end::end_report`]).
//!
//! The window carries the facts the string window holds in fields (the outcome, the retreats, the fleets at
//! the start). A dual-form unit flips back to its ground form after the report is read and before it is announced (Z-Grav
//! Eidolon, package W3). Not covered (the adapter returns `None`): a mobile dock destroyed by the blockade, faction notes cleared at the end (the
//! Bastion retreat notes, the Alastor participants), destructions still to announce, and neutral sides.

use super::end::end_report;
use super::end::EndReason;
use super::Rd;
use crate::choice::{Choice, Pick};
use crate::containers::SmallList;
use crate::generated::{System, Unit};
use crate::ids::Seat;
use crate::kernel::fx;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::query::reach;
use crate::state::{Code, CoreState, EventKind, Key, List, Payload, Val};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Start,
    Done,
}

/// Interned texts of the payload (adapter-interned).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Codes {
    pub losers: Code,
    pub retreated: Code,
    /// `retreat`, `draw`, `destruction`, `round_limit`.
    pub reasons: [Code; 4],
}

/// The facts of one ending.
#[derive(Debug, Clone)]
pub struct Facts {
    pub attacker: Seat,
    pub defender: Seat,
    pub system: System,
    pub rounds: u32,
    pub winner: Option<Seat>,
    pub declared_draw: bool,
    pub fled: Vec<(Seat, Vec<Unit>)>,
    pub at_start: Vec<(Seat, Vec<Unit>)>,
    pub codes: Codes,
    /// The resolver carries the timing machinery (`ctx.emit` announces).
    pub announce: bool,
}

#[derive(Debug, Clone)]
pub struct EndAnnounce {
    pub f: Facts,
    stage: Stage,
}

impl EndAnnounce {
    #[must_use]
    pub const fn new(f: Facts) -> Self {
        Self { f, stage: Stage::Start }
    }
}

impl Window for EndAnnounce {
    type Stage = Stage;

    fn stage(&self) -> Stage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == Stage::Done {
            return Next::Done;
        }
        let f = &self.f;
        // The report is read before the flip: a Z-Grav Eidolon counts as a ship in it.
        let rd = Rd::new(sx.state, sx.kernel).with_blanked(reach::blanked(sx.state));
        let scope = rd.scope();
        let report = f.announce.then(|| end_report(rd, f.system, [f.attacker, f.defender], f.winner, f.declared_draw, &f.fled, &f.at_start));
        // "At the end of a space battle in the active system, flip this card" (Z-Grav Eidolon): the survivors in the space
        // area return to their ground form (`fleet::flip_to_ground_forms`).
        crate::kernel::fx_strategy::flip_space_area(sx.state, scope, &[f.attacker, f.defender], f.system, false);
        if let Some(r) = report {
            let reason = match r.reason {
                EndReason::Retreat => f.codes.reasons[0],
                EndReason::Draw => f.codes.reasons[1],
                EndReason::Destruction => f.codes.reasons[2],
                EndReason::RoundLimit => f.codes.reasons[3],
            };
            let mut payload = Payload::new()
                .with(Key::System, Val::System(f.system))
                .with(Key::Attacker, Val::Seat(f.attacker))
                .with(Key::Defender, Val::Seat(f.defender))
                .with(Key::Rounds, Val::Int(i64::from(f.rounds)))
                .with(Key::Reason, Val::Code(reason));
            if let Some(w) = f.winner {
                payload.set(Key::Winner, Val::Seat(w));
            }
            let seats = |v: &SmallList<Seat, 2>| v.iter().copied().collect::<SmallList<Seat, 4>>();
            payload.set_list(Key::Other(f.codes.losers), List::Seats(seats(&r.losers)));
            payload.set_list(Key::Other(f.codes.retreated), List::Seats(seats(&r.retreated)));
            payload.set_list(Key::Destroyed, List::Units(r.destroyed.iter().copied().collect()));
            let _ = sx.emit(fx::event(EventKind::SpaceCombatEnded, payload));
        }
        // The ground forces an Alastor brought into the battle are no longer in it (`Obs::ParticipantsCleared`), announced or not.
        {
            let (k, mut cx) = (sx.kernel, sx.cx());
            k.hooks.direct.nekro_units_alastor_clear(&mut cx, f.system, None);
        }
        self.stage = Stage::Done;
        Next::Done
    }

    fn ask_of(&self, _: &CoreState, _: &Kernel) -> Option<Choice> {
        None
    }

    fn answer(&mut self, _: &mut Sx<'_>, _: Pick) -> Result<(), Illegal> {
        Err(Illegal)
    }
}
