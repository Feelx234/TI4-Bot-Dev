//! The start of one ground combat round (LRR 42.1; the counting part of the string `begin_ground_round`), as a
//! [`Window`] that never asks.
//!
//! The round counter of the active invasion goes up when the active fight is on this planet against this
//! defender, and from rules version 16 the ground round is a numbered combat round (`combat_round_seq`), so
//! what a card marks "for this round" cannot outlive it.
//!
//! A copied round agent (the Sol agent lent by Yssaril, or the Sol agent readied) makes the round numbered in any
//! rules version and announces `GROUND_COMBAT_ROUND_STARTED`.
//!
//! Not covered (the adapter returns `None`, the string result stands): the faction round-start hooks (Naaz Supercharge), Proxima Targeting VI, the Dunlain Reaper, nested
//! activity.

use crate::choice::{Choice, Pick};
use crate::generated::{Planet, System};
use crate::ids::Seat;
use crate::kernel::fx;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::state::{CoreState, EventKind, Key, Payload, Val};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Start,
    Done,
}

/// The facts of one round's start.
#[derive(Debug, Clone, Copy)]
pub struct Facts {
    pub system: System,
    pub planet: Planet,
    pub invader: Seat,
    pub defender: Seat,
    /// A copied round agent listens: the round is numbered and announced (the resolver carries the timing).
    pub copied: bool,
    /// `rules_version >= RULES_FACTION_EFFECTS`: the ground round is a numbered combat round.
    pub numbered: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct BeginGroundRound {
    pub f: Facts,
    stage: Stage,
}

impl BeginGroundRound {
    #[must_use]
    pub const fn new(f: Facts) -> Self {
        Self { f, stage: Stage::Start }
    }
}

impl Window for BeginGroundRound {
    type Stage = Stage;

    fn stage(&self) -> Stage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == Stage::Done {
            return Next::Done;
        }
        let f = self.f;
        if let Some(active) = sx.state.transient_mut().invasion.active.as_mut()
            && active.planet == Some(f.planet)
            && active.defender == Some(f.defender)
        {
            active.edit().ground_round += 1;
        }
        if f.numbered || f.copied {
            sx.state.meta_mut().combat_round_seq = sx.state.meta().combat_round_seq.saturating_add(1);
        }
        if f.copied {
            let payload = Payload::new()
                .with(Key::System, Val::System(f.system))
                .with(Key::Planet, Val::Planet(f.planet))
                .with(Key::Attacker, Val::Seat(f.invader))
                .with(Key::Defender, Val::Seat(f.defender))
                .with(Key::RoundSeq, Val::Int(i64::from(sx.state.meta().combat_round_seq)));
            let _ = sx.emit(fx::event(EventKind::GroundCombatRoundStarted, payload));
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
