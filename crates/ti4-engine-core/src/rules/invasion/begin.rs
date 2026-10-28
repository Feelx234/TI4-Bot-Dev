//! The start of one ground combat round (LRR 42.1; the counting part of the string `begin_ground_round`), as a
//! [`Window`] that never asks.
//!
//! The round counter of the active invasion goes up when the active fight is on this planet against this
//! defender, and from rules version 16 the ground round is a numbered combat round (`combat_round_seq`), so
//! what a card marks "for this round" cannot outlive it.
//!
//! Not covered (the adapter returns `None`, the string result stands): a copied round agent (its own event and
//! numbering), the faction round-start hooks (Naaz Supercharge), Proxima Targeting VI, the Dunlain Reaper, nested
//! activity.

use crate::choice::{Choice, Pick};
use crate::generated::Planet;
use crate::ids::Seat;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::state::CoreState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Start,
    Done,
}

/// The facts of one round's start.
#[derive(Debug, Clone, Copy)]
pub struct Facts {
    pub planet: Planet,
    pub defender: Seat,
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
        if f.numbered {
            sx.state.meta_mut().combat_round_seq = sx.state.meta().combat_round_seq.saturating_add(1);
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
