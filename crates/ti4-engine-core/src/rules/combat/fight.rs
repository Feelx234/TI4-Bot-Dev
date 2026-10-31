//! A whole space combat as one child window (lent-bodies lane 9; the string `combat::resolve_resolving`): the settle machine of
//! [`super::machine`] run from the stage a fight starts in to its end.
//!
//! The Game driver settles one decision at a time: a settle runs to the next question of the players ([`Stop::Pending`]), the question is
//! asked and answered by a machine of its own ([`Machine::awaiting`]), and the settle that follows is a machine again. This window is that
//! loop for a caller that fights the combat to its end by itself (the Mahact hero): it keeps the [`Env`] of the fight (its occurrence
//! moves on as the rounds begin it) and the [`Fields`] the machines hand to each other, asks what a machine asks, and goes on until the
//! fight is announced as over. The synchronous API has no scoring window of its own: a pause for the barrage's feat is consumed and the fight
//! goes on.
//!
//! What the machine does not carry stops the run ([`FightToEnd::unsupported`], the reason in [`FightToEnd::bailed`]): the adapter declines
//! it and the string fights the combat.

use super::machine::{Bail, Env, Fields, Machine, Stop};
use crate::choice::{Choice, Pick};
use crate::ids::Seat;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::state::CoreState;

/// The suspension points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FStage {
    /// A settle runs (it may ask the questions inside a round: a reroll, a hit of a module).
    Settling,
    /// The decision the settle stopped at is asked and answered.
    Answering,
    Done,
}

/// The fight, run to its end.
#[derive(Debug, Clone)]
pub struct FightToEnd {
    machine: Machine,
    stage: FStage,
    /// Settles run so far (the string loop's turns that stopped at a decision or a pause).
    pub settles: u32,
    /// The machine stopped at a position it does not carry, or finished without saying how.
    pub unsupported: bool,
    pub bailed: Option<Bail>,
}

impl FightToEnd {
    /// A fight that starts at `fields.step` (a fresh fight starts at `Step::Opening { round: 1 }`).
    #[must_use]
    pub fn new(env: Env, fields: Fields) -> Self {
        Self { machine: Machine::new(env, fields), stage: FStage::Settling, settles: 0, unsupported: false, bailed: None }
    }

    /// The winner (none for a draw) and the rounds fought, once the fight is over.
    #[must_use]
    pub fn outcome(&self) -> Option<(Option<Seat>, u32)> {
        if self.stage == FStage::Done && !self.unsupported { self.machine.fields.outcome } else { None }
    }

    /// The occurrence of the fight's feats (the first round begins it).
    #[must_use]
    pub fn occurrence(&self) -> u64 {
        self.machine.env.occurrence
    }

    fn bail(&mut self, why: Option<Bail>) {
        self.unsupported = true;
        self.bailed = why;
        self.stage = FStage::Done;
    }

    /// The next machine, after the one that ended.
    fn next_settle(&mut self) {
        let (env, mut fields) = (self.machine.env.clone(), self.machine.fields.clone());
        // The pause for the barrage's feat is consumed.
        fields.scoring = false;
        self.machine = Machine::new(env, fields);
        self.stage = FStage::Settling;
        self.settles += 1;
    }
}

impl Window for FightToEnd {
    type Stage = FStage;

    fn stage(&self) -> FStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        loop {
            match self.stage {
                FStage::Done => return Next::Done,
                FStage::Settling | FStage::Answering => match self.machine.step(sx) {
                    Next::Continue(ask) => return Next::Continue(ask),
                    Next::Done => {}
                    other => return other,
                },
            }
            if self.stage == FStage::Answering {
                // The decision was answered: the settle that follows.
                self.next_settle();
                continue;
            }
            match self.machine.stop {
                Some(Stop::Over) => self.stage = FStage::Done,
                Some(Stop::Pending) => {
                    self.machine = Machine::awaiting(self.machine.env.clone(), self.machine.fields.clone());
                    self.stage = FStage::Answering;
                }
                Some(Stop::Scoring) => self.next_settle(),
                Some(Stop::Bailed(why)) => self.bail(Some(why)),
                None => self.bail(None),
            }
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        if self.stage == FStage::Done { None } else { self.machine.ask_of(s, k) }
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        if self.stage == FStage::Done {
            return Err(Illegal);
        }
        self.machine.answer(sx, pick)
    }
}
