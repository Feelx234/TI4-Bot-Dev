//! The stages of the string `InvasionWindow` that ask a question of their own, one core [`Window`] each:
//! `Custodians` (27.2, 27.3) and `FightRound` (42). Their string counterparts are
//! `InvasionWindow::{ask_custodians, answer_custodians, ask_fight_round, answer_fight}`.
//!
//! # `Custodians`
//!
//! `step` ends the window when the offer does not apply (the token is gone, this is not Mecatol Rex, no ground
//! force could land there, or the invader cannot pay: `affordable`, the payment rules' answer from package B3)
//! and otherwise asks. The answer "no" leaves the token. The answer "yes" is complete only when it costs
//! nothing (Winnu blood ties): it lifts the token and gains the victory point (capped at the target). A paid
//! "yes" needs the payment rules and is refused (`Illegal`); the adapter bows out before that answer.
//!
//! # `FightRound`
//!
//! Only the question: the round itself (dice, rerolls, hit windows, Harrow, the end-of-round events) is the
//! string `answer_fight`. The adapter compares the question and never runs the answer.

use super::{custodians_cost, custodians_gate, Rd};
use crate::choice::generic::Rule;
use crate::choice::invasion::{Ask as IAsk, Opt as IOpt, Subtype as ISub};
use crate::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype, Target};
use crate::containers::SmallList;
use crate::generated::{Planet, System};
use crate::ids::Seat;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{fx, Kernel, Next};
use crate::query::reach;
use crate::state::{CoreState, LedgerReason, MetaFlags};

/// The victory point target (`objectives::VICTORY_TARGET`): the custodians point is capped at it.
pub use crate::kernel::fx::VICTORY_TARGET;

/// The suspension points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Checking,
    Asking,
    Done,
}

fn rd<'a>(s: &'a CoreState, k: &'a Kernel) -> Rd<'a> {
    Rd::new(s, k).with_blanked(reach::blanked(s))
}

fn ctx(subtype: ISub, origin: Origin, target: Target) -> Ctx {
    Ctx { origin, subtype: Subtype::Invasion(subtype), optional: false, target: Some(target), outstanding: SmallList::new(), space_battle: false, invasion_seq: None }
}

/// The custodians offer of `invader` in `system`.
#[derive(Debug, Clone, Copy)]
pub struct Custodians {
    pub invader: Seat,
    pub system: System,
    /// The invader can pay the cost (`production::available >= cost`): the payment rules' (B3).
    pub affordable: bool,
    /// The token was lifted by this window.
    pub removed: bool,
    stage: Stage,
}

impl Custodians {
    #[must_use]
    pub const fn new(invader: Seat, system: System, affordable: bool) -> Self {
        Self { invader, system, affordable, removed: false, stage: Stage::Checking }
    }

    fn cost(&self, s: &CoreState, k: &Kernel) -> u8 {
        u8::try_from(custodians_cost(rd(s, k), self.invader)).unwrap_or(u8::MAX)
    }
}

impl Window for Custodians {
    type Stage = Stage;

    fn stage(&self) -> Stage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == Stage::Checking {
            let r = rd(sx.state, sx.kernel);
            let offered = custodians_gate(r, self.invader, self.system) && (self.affordable || custodians_cost(r, self.invader) == 0);
            self.stage = if offered { Stage::Asking } else { Stage::Done };
        }
        match self.stage {
            Stage::Asking => Next::Continue(Ask::Invasion(IAsk::RemoveCustodians { system: self.system, cost: self.cost(sx.state, sx.kernel) })),
            Stage::Checking | Stage::Done => Next::Done,
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        (self.stage == Stage::Asking).then(|| {
            let mut opts = Opts::default();
            opts.push(Opt::Invasion(IOpt::KeepCustodians));
            opts.push(Opt::Invasion(IOpt::RemoveCustodians));
            Choice {
                actor: self.invader,
                ask: Ask::Invasion(IAsk::RemoveCustodians { system: self.system, cost: self.cost(s, k) }),
                ctx: ctx(ISub::RemoveCustodians, Origin::Rule(Rule::R27_2), Target::System(self.system)),
                opts,
            }
        })
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        if self.stage != Stage::Asking {
            return Err(Illegal);
        }
        match pick {
            Pick::Opt(Opt::Invasion(IOpt::KeepCustodians)) => {}
            Pick::Opt(Opt::Invasion(IOpt::RemoveCustodians)) => {
                if self.cost(sx.state, sx.kernel) != 0 {
                    // The payment is the payment rules' (B3).
                    return Err(Illegal);
                }
                sx.state.meta_mut().flags.set(MetaFlags::CUSTODIANS_REMOVED, true);
                fx::adjust_victory_points(sx.state, self.invader, 1, LedgerReason::Custodians);
                self.removed = true;
            }
            _ => return Err(Illegal),
        }
        self.stage = Stage::Done;
        Ok(())
    }
}

/// The question "fight a round on {planet}" of the invader.
#[derive(Debug, Clone, Copy)]
pub struct FightRound {
    pub invader: Seat,
    pub system: System,
    pub planet: Planet,
    stage: Stage,
}

impl FightRound {
    #[must_use]
    pub const fn new(invader: Seat, system: System, planet: Planet) -> Self {
        Self { invader, system, planet, stage: Stage::Checking }
    }
}

impl FightRound {
    /// The question, which reads nothing from the state.
    #[must_use]
    pub fn question(&self) -> Choice {
        let mut opts = Opts::default();
        opts.push(Opt::Invasion(IOpt::FightRound { planet: self.planet }));
        Choice {
            actor: self.invader,
            ask: Ask::Invasion(IAsk::FightRound { system: self.system, planet: self.planet }),
            ctx: ctx(ISub::FightGroundCombatRound, Origin::Rule(Rule::R42), Target::Planet { system: self.system, planet: self.planet }),
            opts,
        }
    }
}

impl Window for FightRound {
    type Stage = Stage;

    fn stage(&self) -> Stage {
        self.stage
    }

    fn step(&mut self, _sx: &mut Sx<'_>) -> Next {
        if self.stage == Stage::Checking {
            self.stage = Stage::Asking;
        }
        match self.stage {
            Stage::Asking => Next::Continue(Ask::Invasion(IAsk::FightRound { system: self.system, planet: self.planet })),
            Stage::Checking | Stage::Done => Next::Done,
        }
    }

    fn ask_of(&self, _s: &CoreState, _k: &Kernel) -> Option<Choice> {
        (self.stage == Stage::Asking).then(|| self.question())
    }

    fn answer(&mut self, _sx: &mut Sx<'_>, _pick: Pick) -> Result<(), Illegal> {
        // The round is the string side's (dice, rerolls, hit windows); the adapter never answers.
        Err(Illegal)
    }
}
