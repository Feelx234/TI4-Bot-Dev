//! `fleet::enforce_everywhere` as one window (package D1b2): at every turn end the fleet supply and the capacity of every seat
//! are enforced in every system it has ships in, seat by seat in the table's order and system by system in the board's order,
//! asking the owner which unit goes where something is over (`rules::tactical::fleet::Enforce` per pair).
//!
//! The pairs, and the limits each is judged by, are the caller's facts (the string works the limits out from pool, laws and
//! abilities); a pair where the seat has no ship in the space area cannot ask anything and is left out by the caller.

use crate::choice::{Choice, Pick};
use crate::generated::System;
use crate::ids::Seat;
use crate::kernel::stage::{Child, Illegal, Ran, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::rules::tactical::fleet::{Enforce, Limits};
use crate::state::CoreState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Running,
    Done,
}

#[derive(Debug, Clone)]
pub struct EnforceAll {
    pairs: Vec<(Seat, System, Limits)>,
    at: usize,
    child: Child<Enforce>,
    stage: Stage,
    /// Units removed in all.
    pub removed: u32,
}

impl EnforceAll {
    /// `pairs`: the seats and systems to enforce, in the order the string walks them.
    #[must_use]
    pub fn new(pairs: Vec<(Seat, System, Limits)>) -> Self {
        Self { pairs, at: 0, child: Child::idle(), stage: Stage::Running, removed: 0 }
    }
}

impl Window for EnforceAll {
    type Stage = Stage;

    fn stage(&self) -> Stage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        loop {
            if self.stage == Stage::Done {
                return Next::Done;
            }
            if !self.child.running() {
                let Some((seat, system, limits)) = self.pairs.get(self.at).copied() else {
                    self.stage = Stage::Done;
                    continue;
                };
                self.child.start(Enforce::new(seat, system, limits));
            }
            match self.child.step(sx) {
                Some(Ran::Asks(ask)) => return Next::Continue(ask),
                Some(Ran::Pass(next)) => return next,
                Some(Ran::Done(e)) => {
                    self.removed += e.removed;
                    self.at += 1;
                }
                None => self.at += 1, // mutants:skip: a child that was just started cannot be missing
            }
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        self.child.ask_of(s, k)
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        self.child.answer(sx, pick)
    }
}
