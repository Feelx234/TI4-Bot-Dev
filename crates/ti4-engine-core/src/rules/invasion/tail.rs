//! The end of the "Roll Dice" step of a ground combat round (LRR 42.2; the part of the string `roll_ground_step`
//! between the two sides' rolls and `GROUND_COMBAT_ROLL_STEP_ENDED`), as a [`Window`] that never asks.
//!
//! The round number moves on (unless a copied round agent numbered it already), "after your ground forces make
//! combat rolls" (`GROUND_ROLLS_MADE`) is announced for the invader and the defender, the hits are read back
//! from the staged dice (the reroll windows the announcements opened may have changed them), the dice are
//! listed as the board shows them for the invasion step's report, and the staging is cleared.
//!
//! The end-of-step event (`GROUND_COMBAT_ROLL_STEP_ENDED`: a readied The Thundarian) and its replays are the composer's
//! (`driver::windows::invasion`, `round_body`), which runs this window again for each replay.

use crate::choice::{Choice, Pick};
use crate::generated::{Planet, System};
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

/// The facts of one step's tail.
#[derive(Debug, Clone, Copy)]
pub struct Facts {
    pub invader: Seat,
    pub defender: Seat,
    pub system: System,
    pub planet: Planet,
    /// The hits the two rolls produced (invader, defender).
    pub rolled: (u32, u32),
    /// No copied round agent numbered the round yet.
    pub bump: bool,
    /// The resolver carries the timing machinery.
    pub announce: bool,
    /// Per side (invader, defender): X-89 Bacterial Weapon doubles the hits the staged dice show now (package W10; `rolled` is doubled already).
    pub x89: [bool; 2],
}

/// One die as the board shows it: (side, pool name, face, target, hit).
pub type Die = (Seat, Code, u32, u32, bool);

#[derive(Debug, Clone)]
pub struct GroundRollTail {
    pub f: Facts,
    /// The hits after the reroll windows (invader, defender).
    pub hits: (u32, u32),
    pub dice: Vec<Die>,
    stage: Stage,
}

impl GroundRollTail {
    #[must_use]
    pub fn new(f: Facts) -> Self {
        Self { f, hits: (0, 0), dice: Vec::new(), stage: Stage::Start }
    }
}

/// `RerollEntry::hits`: a die hits when its face plus its delta reaches the number.
fn entry_hits(faces: &[u8], deltas: &impl Fn(u8) -> i64, on: Option<u8>) -> u32 {
    let Some(on) = on else { return 0 };
    let n = faces.iter().enumerate().filter(|(i, face)| i64::from(**face) + deltas(u8::try_from(*i).unwrap_or(u8::MAX)) >= i64::from(on)).count();
    u32::try_from(n).unwrap_or(u32::MAX)
}

impl Window for GroundRollTail {
    type Stage = Stage;

    fn stage(&self) -> Stage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == Stage::Done {
            return Next::Done;
        }
        let f = self.f;
        if f.bump {
            sx.state.meta_mut().combat_round_seq = sx.state.meta().combat_round_seq.saturating_add(1);
        }
        if f.announce {
            for (who, hits) in [(f.invader, f.rolled.0), (f.defender, f.rolled.1)] {
                let payload = Payload::new()
                    .with(Key::System, Val::System(f.system))
                    .with(Key::Planet, Val::Planet(f.planet))
                    .with(Key::Player, Val::Seat(who))
                    .with(Key::Hits, Val::Int(i64::from(hits)));
                let _ = sx.emit(fx::event(EventKind::GroundRollsMade, payload));
            }
        }
        let combat = &sx.state.transient().combat;
        let mut hits = [f.rolled.0, f.rolled.1];
        let mut dice = Vec::new();
        for (slot, side) in [f.invader, f.defender].into_iter().enumerate() {
            let Some((_, set)) = combat.reroll_staging.iter().find(|(s, _)| *s == side) else { continue };
            let delta = |e: &crate::state::RerollEntry| {
                let d = e.deltas.clone();
                move |i: u8| d.get(&i).map_or(0, |v| i64::from(*v))
            };
            hits[slot] = set.rolls.iter().map(|e| entry_hits(&e.faces, &delta(e), e.hits_on)).sum::<u32>() * if f.x89[slot] { 2 } else { 1 };
            for e in set.rolls.iter().filter(|e| e.planet == Some(f.planet)) {
                let target = e.hits_on.map_or(u32::MAX, u32::from);
                for (i, face) in e.faces.iter().enumerate() {
                    let adjusted = i64::from(*face) + delta(e)(u8::try_from(i).unwrap_or(u8::MAX));
                    dice.push((side, e.unit, u32::from(*face), target, adjusted >= i64::from(target)));
                }
            }
        }
        self.hits = (hits[0], hits[1]);
        self.dice = dice;
        let combat = &mut sx.state.transient_mut().combat;
        combat.reroll_staging.edit().clear();
        combat.last_reroll_player = None;
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
