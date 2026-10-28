//! Which planet the invasion fights on next (LRR 49 and 42; the selection part of the string `advance_fighting`),
//! as a [`Window`] that never asks.
//!
//! From a planet index on, a planet is fought over when the invader has a ground force there, does not coexist
//! there and did not choose to coexist this invasion (Research Team on defense). The defender is the planet's
//! controller when that player has ground forces there (coexistence 11), else the first other owner of a
//! ground force. A planet with no defender but coexisters left offers the next combat (coexistence 9 and 12).
//! The window only decides; starting the combat stays with the string (it announces and may remove a side).
//!
//! Not covered (the adapter returns `None`, the string result stands): seats not in id order (the string
//! breaks ties by player id), neutral sides, nested activity.

use super::{is_ground_force_here, Rd};
use crate::choice::{Choice, Pick};
use crate::containers::SeatSet;
use crate::generated::{Planet, System};
use crate::ids::Seat;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::query::{board as qboard, reach};
use crate::state::CoreState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Start,
    Done,
}

/// What the selection found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Upcoming {
    /// Fight on `planets[index]` against `defender`.
    Fight { index: usize, defender: Seat },
    /// Offer the next combat on `planets[index]` against the coexisters.
    Coexisters { index: usize, remaining: Vec<Seat> },
    /// No planet is left to fight over.
    Exhausted,
}

#[derive(Debug, Clone)]
pub struct Facts {
    pub invader: Seat,
    pub system: System,
    pub planets: Vec<Planet>,
    pub index: usize,
    /// The planets the invasion report marks as coexisted by choice.
    pub coexisted: Vec<Planet>,
}

#[derive(Debug, Clone)]
pub struct Advance {
    pub f: Facts,
    pub pick: Option<Upcoming>,
    stage: Stage,
}

impl Advance {
    #[must_use]
    pub fn new(f: Facts) -> Self {
        Self { f, pick: None, stage: Stage::Start }
    }
}

impl Window for Advance {
    type Stage = Stage;

    fn stage(&self) -> Stage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == Stage::Done {
            return Next::Done;
        }
        let rd = Rd::new(sx.state, sx.kernel).with_blanked(reach::blanked(sx.state));
        let f = &self.f;
        let mut index = f.index;
        let mut pick = Upcoming::Exhausted;
        while index < f.planets.len() {
            let planet = f.planets[index];
            let mut owners = SeatSet::EMPTY;
            for u in rd.s.board().planet(planet).units.iter() {
                if is_ground_force_here(rd, u.owner(), f.system, planet, *u) {
                    owners.insert(u.owner());
                }
            }
            if !owners.contains(f.invader) || qboard::is_coexisting(rd.s, f.system, planet, f.invader) || f.coexisted.contains(&planet) {
                index += 1;
                continue;
            }
            let controller = qboard::control(rd.s, planet).filter(|h| *h != f.invader && owners.contains(*h));
            if let Some(defender) = controller.or_else(|| owners.iter().find(|o| *o != f.invader)) {
                pick = Upcoming::Fight { index, defender };
                break;
            }
            let remaining: Vec<Seat> = qboard::coexisters(rd.s, f.system, planet).iter().filter(|p| *p != f.invader).collect();
            if !remaining.is_empty() {
                pick = Upcoming::Coexisters { index, remaining };
                break;
            }
            index += 1;
        }
        self.pick = Some(pick);
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
