//! Quantum Dissemination (Yin hero; lent-bodies lane 9, the string `yin::use_leader_timed`): "ACTION: Commit up to 3 infantry from your
//! reinforcements to planets that are not in a home system, as if you were invading them; resolve the ground combat there. Then, purge this
//! card."
//!
//! The seat is asked where each of up to `stock` infantry goes (a decline stops the asking; the spots are the adapter's read of the map and
//! the stock the infantry the reinforcements hold, at most three), then every infantry lands (the box caps it), each planet chosen is fought to
//! the end with a child ([`super::fight::CommittedFight`], in the order the planets were first chosen, until the game is won) and the planets the
//! seat still holds ground forces on change hands: the control is set, the planet taken exhausted. The effects of gaining control that the
//! window does not carry (a Minister of Exploration, the Crown of Emphidia, L1Z1X's Fealty Uplink) are declined by the adapter.

use super::fight::{CommittedFight, FightFacts};
use super::{is_ground_force, Rd};
use crate::choice::generic::Content;
use crate::choice::leaders::{Ask as LAsk, Opt as LOpt, Subtype as LSub};
use crate::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use crate::containers::SmallList;
use crate::generated::{Leader, Planet, System, Unit};
use crate::ids::Seat;
use crate::kernel::fx;
use crate::kernel::stage::{Child, Illegal, Ran, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::query::reach;
use crate::state::{CoreState, SysFlags};

/// What the adapter vouches for.
#[derive(Debug, Clone)]
pub struct YinFacts {
    /// The planets that are not in a home system, in the string's order.
    pub spots: Vec<(System, Planet)>,
    /// The infantry the seat may commit: the reinforcements, at most three.
    pub stock: u8,
    /// The form of infantry the seat places.
    pub infantry: Unit,
    /// The facts of a combat on one planet; the system and the planet are the window's own.
    pub fight: FightFacts,
}

/// The suspension points of [`YinHero`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YStage {
    Start,
    /// Waiting on where the next infantry goes.
    Commit,
    /// The planet `at` of the order is fought.
    Fighting,
    Done,
}

#[derive(Debug, Clone)]
pub struct YinHero {
    pub who: Seat,
    facts: YinFacts,
    stage: YStage,
    chosen: Vec<(System, Planet)>,
    /// The distinct planets in the order they were first chosen.
    order: Vec<(System, Planet)>,
    at: usize,
    fight: Child<CommittedFight>,
    /// The card resolved (some infantry was committed).
    pub used: bool,
    /// The scoring windows a won combat earned a feat for, by feat occurrence (the string opens them after the combat).
    pub scoring: Vec<u64>,
    /// A combat the window does not carry: the adapter declines the run.
    pub unsupported: bool,
}

fn ctx() -> Ctx {
    Ctx {
        origin: Origin::FactionAbility(Content::Leader(Leader::DannelOfTheTenth)),
        subtype: Subtype::Leaders(LSub::YinCommit),
        optional: false,
        target: None,
        outstanding: SmallList::new(),
        space_battle: false,
        invasion_seq: None,
    }
}

impl YinHero {
    #[must_use]
    pub fn new(who: Seat, facts: YinFacts) -> Self {
        Self { who, facts, stage: YStage::Start, chosen: Vec::new(), order: Vec::new(), at: 0, fight: Child::idle(), used: false, scoring: Vec::new(), unsupported: false }
    }

    /// Every infantry chosen lands, then the planets are fought in the order first chosen.
    fn land(&mut self, sx: &mut Sx<'_>) {
        let scope = crate::tables::Scope(u32::from(sx.kernel.sources.0));
        for (system, planet) in &self.chosen {
            let _ = fx::place_counted(sx.state, sx.kernel, scope, self.who, *system, Some(*planet), Some(self.facts.infantry), 1);
        }
        for spot in &self.chosen {
            if !self.order.contains(spot) {
                self.order.push(*spot);
            }
        }
        self.at = 0;
        self.used = true;
        self.next_planet(sx);
    }

    /// The next planet is fought, or the control is established.
    fn next_planet(&mut self, sx: &mut Sx<'_>) {
        let Some((system, planet)) = self.order.get(self.at).copied() else { return self.control(sx) };
        let mut facts = self.facts.fight.clone();
        facts.system = system;
        facts.planet = planet;
        self.fight.start(CommittedFight::new(facts));
        self.stage = YStage::Fighting;
    }

    /// `establish_control`: a planet the seat holds ground forces on, does not coexist on and does not control changes hands, taken exhausted.
    fn control(&mut self, sx: &mut Sx<'_>) {
        for (system, planet) in self.order.clone() {
            let holds = {
                let rd = Rd::new(sx.state, sx.kernel).with_blanked(reach::blanked(sx.state));
                let cell = sx.state.board().planet(planet);
                (cell.units.as_slice().iter().any(|p| p.owner() == self.who && is_ground_force(rd, p.kind())), cell.control)
            };
            if !holds.0 || holds.1 == Some(self.who) {
                continue;
            }
            sx.state.board_mut().planet_mut(planet).control = Some(self.who);
            sx.state.board_mut().sys_mut(system).flags.set(SysFlags::TOUCHED, true);
            fx::exhaust_planet(sx.state, planet);
        }
        self.stage = YStage::Done;
    }

    fn options(&self) -> Opts {
        self.facts
            .spots
            .iter()
            .map(|(system, planet)| Opt::Leaders(LOpt::YinSpot(*system, *planet)))
            .chain(std::iter::once(Opt::Decline))
            .collect::<Opts>()
    }
}

impl Window for YinHero {
    type Stage = YStage;

    fn stage(&self) -> YStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        loop {
            match self.stage {
                YStage::Start => {
                    if self.facts.stock == 0 || self.facts.spots.is_empty() {
                        self.stage = YStage::Done;
                    } else {
                        self.stage = YStage::Commit;
                    }
                }
                YStage::Commit => {
                    let n = u8::try_from(self.chosen.len()).unwrap_or(u8::MAX);
                    return Next::Continue(Ask::Leaders(LAsk::YinCommit { n, stock: self.facts.stock }));
                }
                YStage::Fighting => match self.fight.step(sx) {
                    Some(Ran::Asks(ask)) => return Next::Continue(ask),
                    Some(Ran::Pass(next)) => return next,
                    Some(Ran::Done(fight)) => {
                        if fight.unsupported {
                            self.unsupported = true;
                            self.stage = YStage::Done;
                            continue;
                        }
                        self.scoring.extend(fight.scoring.iter().copied());
                        self.at += 1;
                        if sx.state.meta().finished {
                            self.control(sx);
                        } else {
                            self.next_planet(sx);
                        }
                    }
                    None => self.stage = YStage::Done,
                },
                YStage::Done => return Next::Done,
            }
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        match self.stage {
            YStage::Commit => {
                let n = u8::try_from(self.chosen.len()).unwrap_or(u8::MAX);
                Some(Choice { actor: self.who, ask: Ask::Leaders(LAsk::YinCommit { n, stock: self.facts.stock }), ctx: ctx(), opts: self.options() })
            }
            YStage::Fighting => self.fight.ask_of(s, k),
            YStage::Start | YStage::Done => None,
        }
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        match (self.stage, pick) {
            (YStage::Fighting, pick) => self.fight.answer(sx, pick),
            (YStage::Commit, Pick::Opt(Opt::Decline)) => {
                if self.chosen.is_empty() {
                    self.stage = YStage::Done;
                } else {
                    self.land(sx);
                }
                Ok(())
            }
            (YStage::Commit, Pick::Opt(Opt::Leaders(LOpt::YinSpot(system, planet)))) => {
                if !self.facts.spots.contains(&(system, planet)) {
                    return Err(Illegal);
                }
                self.chosen.push((system, planet));
                if self.chosen.len() >= usize::from(self.facts.stock) {
                    self.land(sx);
                }
                Ok(())
            }
            _ => Err(Illegal),
        }
    }
}
