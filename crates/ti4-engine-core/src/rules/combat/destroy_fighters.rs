//! Anti-fighter barrage hits without a Waylay (`combat::destroy_fighters` and the announcement loop of
//! `apply_barrage`), as a [`Window`] that never asks: the target loses its first fighters in unit order,
//! all of them, and only then is each loss announced (with staging and a timing resolver).

use super::assign::{announce_loss, remove_ship};
use super::Rd;
use crate::generated::System;
use crate::ids::Seat;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::choice::{Choice, Pick};
use crate::state::{Code, CoreState, Piece};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Destroying,
    Done,
}

/// The facts of one barrage resolution against `target`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fighters {
    pub target: Seat,
    pub system: System,
    pub hits: u16,
    /// The interned cause (`"unit_ability:anti_fighter_barrage"`).
    pub cause: Code,
    pub staging: bool,
    pub announce: bool,
}

/// The window.
#[derive(Debug, Clone, Copy)]
pub struct DestroyFighters {
    pub f: Fighters,
    stage: Stage,
}

impl DestroyFighters {
    #[must_use]
    pub const fn new(f: Fighters) -> Self {
        Self { f, stage: Stage::Destroying }
    }
}

/// Remove the target's first `f.hits` fighters in unit order, then announce each loss (with staging and a
/// timing resolver): the whole of the window's work, shared with the barrage step.
pub fn destroy_first_fighters(sx: &mut Sx<'_>, f: Fighters) {
    let mut destroyed: Vec<Piece> = Vec::new();
    for _ in 0..f.hits {
        let rd = Rd::new(sx.state, sx.kernel);
        let first = sx.state.board().sys(f.system).units.iter().find(|u| u.owner() == f.target && rd.is_fighter(u.kind())).copied();
        let Some(fighter) = first else { break };
        remove_ship(sx, f.system, f.target, fighter);
        destroyed.push(fighter);
    }
    if f.staging {
        for fighter in destroyed {
            announce_loss(sx, f.system, f.target, fighter.kind(), f.cause, true, f.announce);
        }
    }
}

impl Window for DestroyFighters {
    type Stage = Stage;

    fn stage(&self) -> Stage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == Stage::Done {
            return Next::Done;
        }
        destroy_first_fighters(sx, self.f);
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
