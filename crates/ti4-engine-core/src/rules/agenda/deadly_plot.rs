//! Deadly Plot's effect as a [`Window`] (package B9; string `action_cards::deadly_plot`).
//!
//! "During the agenda phase when an outcome would be resolved: If you voted for or predicted another outcome,
//! discard the agenda instead. ... Then, exhaust all of your planets."
//!
//! The condition is the reaction guard [`Guard::VotedOrPredictedAnother`](crate::kernel::reaction::Guard), which
//! reads the ballot (`agenda.votes`) and the predictions with A5's [`Prediction`](crate::keys::Prediction)
//! semantics (a prediction names its outcome up to the tag; a vote bar `none|<bar>` is never the outcome). By
//! the time the effect runs the guard held: it marks the agenda discarded and exhausts every planet the holder
//! controls (the board's planets, not only those of a known system).

use crate::choice::{Choice, Pick};
use crate::ids::Seat;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::state::{CoreState, PlanetFlags, TransientFlags};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Apply,
    Done,
}

/// The effect of a played Deadly Plot.
#[derive(Debug, Clone)]
pub struct DeadlyPlot {
    holder: Seat,
    stage: Stage,
}

impl DeadlyPlot {
    #[must_use]
    pub const fn new(holder: Seat) -> Self {
        Self { holder, stage: Stage::Apply }
    }
}

impl Window for DeadlyPlot {
    type Stage = Stage;

    fn stage(&self) -> Stage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == Stage::Apply {
            sx.state.transient_mut().misc.flags.set(TransientFlags::AGENDA_DISCARDED);
            let held: crate::containers::SmallList<crate::generated::Planet, 40> =
                sx.state.board().planets().filter(|(_, cell)| cell.control == Some(self.holder)).map(|(p, _)| p).collect();
            for planet in held.as_slice() {
                sx.state.board_mut().planet_mut(*planet).flags.set(PlanetFlags::EXHAUSTED, true);
            }
            self.stage = Stage::Done;
        }
        Next::Done
    }

    fn ask_of(&self, _: &CoreState, _: &Kernel) -> Option<Choice> {
        None
    }

    fn answer(&mut self, _: &mut Sx<'_>, _: Pick) -> Result<(), Illegal> {
        Err(Illegal)
    }
}
