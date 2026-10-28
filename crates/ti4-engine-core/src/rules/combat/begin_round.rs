//! The opening of a space combat round (string `CombatWindow::begin_round`), as a [`Window`] that never asks.
//!
//! The round counter and the presentation are set, last round's hit and dice records are cleared, round 1
//! keeps the barrage's starting fleet, and `SPACE_COMBAT_STARTED` (round 1) and `COMBAT_ROUND_STARTED` are
//! announced. The window carries its facts in fields (package B3's approach): `fleet_at_start` is not touched
//! here because the round-1 form flip that would change it is a bail.
//!
//! Not covered (the adapter returns `None`, the string result stands; see `dual_window::begin_round`):
//! diplomacy enabled (the attack-promise evaluation), a Naaz or Letnev seat (Voltron repair, Supercharge, Munitions), a dual-form unit in
//! the space area (Eidolon flip), and a self-repairing flagship damaged in the system.

use super::{ships_of, Rd};
use crate::choice::{Choice, Pick};
use crate::generated::System;
use crate::ids::Seat;
use crate::kernel::fx;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::query::reach;
use crate::state::{Code, CoreState, EventKind, Key, Payload, Val};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Start,
    Done,
}

/// The facts of one round opening.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Facts {
    pub attacker: Seat,
    pub defender: Seat,
    pub system: System,
    pub round: u32,
    /// The interned presentation phase (`"pre_roll"`).
    pub phase: Code,
    /// The resolver carries the timing machinery (`ctx.emit` announces).
    pub announce: bool,
    /// A round agent (Letnev's own, or a Yssaril copy of it) listens: `COMBAT_ROUND_STARTED` carries `round_seq`.
    pub round_seq_listens: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct BeginRound {
    pub f: Facts,
    stage: Stage,
}

impl BeginRound {
    #[must_use]
    pub const fn new(f: Facts) -> Self {
        Self { f, stage: Stage::Start }
    }
}

impl Window for BeginRound {
    type Stage = Stage;

    fn stage(&self) -> Stage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == Stage::Done {
            return Next::Done;
        }
        let f = self.f;
        sx.state.meta_mut().combat_round_seq = sx.state.meta().combat_round_seq.saturating_add(1);
        let rd = Rd::new(sx.state, sx.kernel).with_blanked(reach::blanked(sx.state));
        let mut round_start = ships_of(rd, f.attacker, f.system);
        round_start.extend(ships_of(rd, f.defender, f.system));
        let combat = &mut sx.state.transient_mut().combat;
        combat.combat_round_hits.clear();
        combat.combat_round_dice = Default::default();
        let p = combat.presentation.edit();
        p.phase = f.phase;
        p.round = f.round;
        p.remaining_hits.clear();
        if f.round == 1 {
            p.barrage_start = round_start.clone();
            p.barrage_hits.clear();
            p.barrage_dice.clear();
        }
        p.round_start = round_start;
        let base = Payload::new()
            .with(Key::System, Val::System(f.system))
            .with(Key::Attacker, Val::Seat(f.attacker))
            .with(Key::Defender, Val::Seat(f.defender))
            .with(Key::Round, Val::Int(i64::from(f.round)));
        if f.announce {
            if f.round == 1 {
                let _ = sx.emit(fx::event(EventKind::SpaceCombatStarted, base.clone().with(Key::Player, Val::Seat(f.attacker))));
            }
            let announced = if f.round_seq_listens {
                let seq = i64::from(sx.state.meta().combat_round_seq);
                base.with(Key::RoundSeq, Val::Int(seq))
            } else {
                base
            };
            let _ = sx.emit(fx::event(EventKind::CombatRoundStarted, announced));
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
