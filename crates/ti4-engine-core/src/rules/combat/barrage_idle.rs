//! The anti-fighter barrage step of round 1 when neither side has a barrage to roll (string
//! `CombatWindow::barrage_step` with an empty plan for both sides), as a [`Window`] that never asks.
//!
//! Per side, attacker first: `ANTI_FIGHTER_BARRAGE_STARTED` is announced (the reaction window where Waylay
//! binds), the presentation phase is `barrage`, nothing is rolled or staged, and no hits are applied. The
//! adapter checks the typed plan of both sides is empty; positions with a barrage (dice, reroll windows,
//! Fight with Precision, the hit windows) stay with the string side until the core window holds dice.

use crate::choice::{Choice, Pick};
use crate::generated::System;
use crate::ids::Seat;
use crate::kernel::fx;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::state::{Code, CoreState, EventKind, Key, Payload, Val};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Start,
    Done,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Facts {
    pub attacker: Seat,
    pub defender: Seat,
    pub system: System,
    pub round: u32,
    /// The interned presentation phase (`"barrage"`).
    pub phase: Code,
    pub announce: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct BarrageIdle {
    pub f: Facts,
    stage: Stage,
}

impl BarrageIdle {
    #[must_use]
    pub const fn new(f: Facts) -> Self {
        Self { f, stage: Stage::Start }
    }
}

impl Window for BarrageIdle {
    type Stage = Stage;

    fn stage(&self) -> Stage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == Stage::Done {
            return Next::Done;
        }
        let f = self.f;
        for side in [f.attacker, f.defender] {
            if f.announce {
                let payload = Payload::new()
                    .with(Key::System, Val::System(f.system))
                    .with(Key::Player, Val::Seat(side))
                    .with(Key::Round, Val::Int(i64::from(f.round)));
                let _ = sx.emit(fx::event(EventKind::AntiFighterBarrageStarted, payload));
            }
            sx.state.transient_mut().combat.presentation.edit().phase = f.phase;
        }
        let combat = &mut sx.state.transient_mut().combat;
        combat.last_reroll_player = None;
        combat.presentation.edit().phase = f.phase;
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
