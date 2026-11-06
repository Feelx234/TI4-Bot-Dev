//! L1Z1X's Harrow (LRR 15.1f bombardment, the string `assign_ground_round_hits` after the round's removals): "at the
//! end of each round of ground combat, you may produce one hit against the defender for each of your bombarding
//! units in the space area". It is a window of its own run after [`super::round::GroundRoundHits`]' removals: the
//! dice are drawn only after those removals (the string order), the hits go through the hits step (window, then
//! the removal) with the cause `harrow`, and the events stay staged for the round's announcement.
//!
//! The facts the string answers: that the invader holds the Harrow ability. Whether the planet can be bombarded is
//! read here ([`super::bombardable`]). Each unit of the invader's in the space area with a BOMBARDMENT value rolls
//! its dice in space order; Plasma Scoring gives the first such roller one die more (no Assail, Bunker or
//! galvanizing: Harrow is not a unit's roll).
//!
//! Not covered (the adapter returns `None`): Proxima Targeting VI at the table, a neutral side, test faces.

use super::hits::{self as step, Batch};
use super::{bombardable, has_plasma_scoring, Rd, DIE_SIDES};
use crate::choice::{Choice, Pick};
use crate::generated::{Planet, System};
use crate::ids::Seat;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::query::reach;
use crate::state::{Code, CoreState, RngDomain, RollPurpose, RollRec};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Start,
    Done,
}

/// The facts of one Harrow.
#[derive(Debug, Clone, Copy)]
pub struct Facts {
    pub invader: Seat,
    pub defender: Seat,
    pub system: System,
    pub planet: Planet,
    pub announce: bool,
    /// The interned event cause `harrow`.
    pub cause: Code,
}

#[derive(Debug, Clone)]
pub struct Harrow {
    pub f: Facts,
    /// The rolls in order: (hit number, faces), for the dual to compare with the string dice.
    pub drawn: Vec<(u32, Vec<u32>)>,
    /// The hits assigned after the window (`InvasionStep.harrow_hits`).
    pub assigned: u32,
    stage: Stage,
}

impl Harrow {
    #[must_use]
    pub fn new(f: Facts) -> Self {
        Self { f, drawn: Vec::new(), assigned: 0, stage: Stage::Start }
    }
}

impl Window for Harrow {
    type Stage = Stage;

    fn stage(&self) -> Stage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == Stage::Done {
            return Next::Done;
        }
        self.stage = Stage::Done;
        let f = self.f;
        let rd = Rd::new(sx.state, sx.kernel).with_blanked(reach::blanked(sx.state));
        if !bombardable(rd, f.system, f.planet, f.invader) {
            return Next::Done;
        }
        let mut plasma = has_plasma_scoring(rd, f.invader);
        let mut rolls: Vec<(crate::generated::Unit, u32, usize)> = Vec::new();
        for piece in sx.state.board().sys(f.system).units.iter().filter(|p| p.owner() == f.invader) {
            let Some(roll) = rd.data(piece.kind()).and_then(|d| d.bombardment) else { continue };
            if roll.hits_on == 0 || roll.dice == 0 {
                continue;
            }
            rolls.push((piece.kind(), u32::from(roll.hits_on), usize::from(roll.dice) + usize::from(std::mem::take(&mut plasma))));
        }
        let mut total = 0_u32;
        for (unit, on, dice) in rolls {
            let faces: Vec<u32> = sx.dice(RngDomain::Dice, usize::try_from(dice).unwrap_or(0), DIE_SIDES);
            sx.state.ledger_mut().dice.push(RollRec {
                purpose: RollPurpose::Bombardment,
                owner: Some(f.invader),
                unit: Some(unit),
                planet: None,
                scene: Some(f.system),
                hits_on: Some(u8::try_from(on).unwrap_or(u8::MAX)),
                faces: faces.iter().map(|x| u8::try_from(*x).unwrap_or(u8::MAX)).collect(),
                rerolled: 0,
            });
            total += u32::try_from(faces.iter().filter(|x| **x >= on).count()).unwrap_or(u32::MAX);
            self.drawn.push((on, faces));
        }
        let b = Batch { victim: f.defender, system: f.system, planet: f.planet, cause: f.cause, unit_ability: true, announce: f.announce };
        let remaining = step::open(sx, &b, total);
        step::apply(sx.state, sx.kernel, &b, remaining);
        self.assigned = remaining;
        Next::Done
    }

    fn ask_of(&self, _: &CoreState, _: &Kernel) -> Option<Choice> {
        None
    }

    fn answer(&mut self, _: &mut Sx<'_>, _: Pick) -> Result<(), Illegal> {
        Err(Illegal)
    }
}
