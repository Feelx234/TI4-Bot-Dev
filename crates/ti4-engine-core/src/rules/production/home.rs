//! A use of PRODUCTION at a seat's home system outside a tactical action (the Warfare secondary; package W10, lent-bodies lane 9): the
//! wrappers the string `production::resolve_timed` puts around the production window, as one [`Window`] run as a child
//! (`Spawn::Home`).
//!
//! In the order of the string: the discount an earlier tactical action left unspent is dropped (from the rules version that offers Berekar
//! Berekon), Harrugh Gefhara (the Hacan hero) is offered to the use when it is unlocked (the use counter goes up first: the offer names it),
//! and a hero used makes the use free; then the use is the production window of the area ([`ProductionWindow::use_of`]) question by question.
//! The effects at the end of a use run once, after the answer that closed it (a use that asked nothing has none, as in the string): the planet
//! value a swap lasted for is back, Hacan's Auto-Factories pays a command token for three ships that are no fighters, the units made are
//! reported (the announcement `UNITS_PRODUCED` is the adapter's), and Self-Assembly Routines offers a mech (a window of the faction crate, the
//! table's extra link). The Keleres agent's window and Agency Supply Network belong to a seat the production windows do not carry. The
//! Prophecy of Ixth is discarded at the end of a use that made fewer than two fighters (package W13); a planet value swapped by Hegemonic Trade
//! Policy is declared in the tactical action's production and ends with it, so none lingers into a use outside it.

use super::triggered::HeroOffer;
use super::window::ProductionWindow;
use crate::choice::{Choice, Pick};
use crate::generated::{Agenda, Breakthrough, Leader, Technology, UnitBaseType};
use crate::ids::Seat;
use crate::kernel::children::{ErasedChild, HomeFacts, Made, Spawn};
use crate::kernel::seam::observe::{Obs, Phase};
use crate::kernel::stage::{Child, Illegal, Ran, Sx, Window};
use crate::kernel::{fx, Kernel, Next};
use crate::state::{CoreState, LeaderStatus};

/// `ti4_model::state::RULES_SELF_ASSEMBLY`.
const RULES_SELF_ASSEMBLY: u32 = 13;

/// The suspension points of [`HomeProduction`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HStage {
    Start,
    /// The Hacan hero's offer.
    Hero,
    /// The use of PRODUCTION.
    Using,
    /// Self-Assembly Routines' offer.
    Assembling,
    Done,
}

/// The use of PRODUCTION at the home system. [`Self::made`]: what it made, in order. [`Self::unsupported`]: Self-Assembly Routines is owned
/// and the table has no link to its window.
#[derive(Debug, Clone)]
pub struct HomeProduction {
    pub who: Seat,
    facts: HomeFacts,
    stage: HStage,
    window: Option<ProductionWindow>,
    hero: Child<HeroOffer>,
    using: Child<ProductionWindow>,
    answered: u32,
    assembly: ErasedChild,
    made: Made,
    pub unsupported: bool,
}

impl HomeProduction {
    #[must_use]
    pub fn new(facts: HomeFacts) -> Self {
        Self { who: facts.produce.who, facts, stage: HStage::Start, window: None, hero: Child::idle(), using: Child::idle(), answered: 0, assembly: ErasedChild::idle(), made: Made::new(), unsupported: false }
    }

    /// What the use made, in order, with the planet each unit stands on.
    #[must_use]
    pub fn made(&self) -> Made {
        self.made.clone()
    }

    fn hero_unlocked(core: &CoreState, who: Seat) -> bool {
        core.players().by_seat(who).is_some_and(|p| p.leaders.status.get(&Leader::HarrughGefhara) == Some(&LeaderStatus::Unlocked))
    }

    /// The use starts: the offer to the hero, or the production at once.
    fn start(&mut self, sx: &mut Sx<'_>) {
        if self.facts.agent_rules {
            // This use reads no discount an earlier tactical action left unspent.
            sx.state.transient_mut().production.discount_remaining = 0;
        }
        self.window = Some(ProductionWindow::use_of(&self.facts.produce, self.facts.agent_rules));
        if self.facts.produce.limit > 0 && Self::hero_unlocked(sx.state, self.who) {
            sx.state.meta_mut().production_seq += 1;
            self.hero.start(HeroOffer::new(self.who));
            self.stage = HStage::Hero;
        } else {
            self.begin_use();
        }
    }

    fn begin_use(&mut self) {
        if let Some(window) = self.window.take() {
            self.using.start(window);
        }
        self.stage = HStage::Using;
    }

    /// The hero was used: every unit of the use is free, and the discount is read again (`ProductionWindow::refresh`).
    fn hero_used(&mut self, core: &CoreState) {
        if let Some(window) = &mut self.window {
            window.discount_remaining = core.transient().production.discount_remaining + self.facts.produce.discount;
            window.free_this_use = true;
        }
    }

    /// The use ended after at least one answer: `finish_use` of the string window.
    fn finish_use(&mut self, sx: &mut Sx<'_>, finished: &ProductionWindow) {
        let who = self.who;
        // A planet-value swap lasts one use of PRODUCTION.
        sx.state.transient_mut().production.value_swapped_planet = None;
        self.made = finished.made();
        // Hacan's Auto-Factories: three ships that are no fighters pay a command token for the fleet pool.
        let ships = finished.produced.as_slice().iter().filter(|(unit, _)| unit.data().ship && unit.data().base_type != UnitBaseType::Fighter).count();
        if ships >= 3 && sx.state.players().by_seat(who).is_some_and(|p| p.cards.breakthrough == Some(Breakthrough::AutoFactories)) {
            fx::gain_token(sx.state, who, crate::choice::strategy::Pool::Fleet, 1);
        }
        // The Prophecy of Ixth: using PRODUCTION discards the law unless two or more fighters were produced (read over the whole use).
        let fighters = finished.produced.as_slice().iter().filter(|(unit, _)| unit.data().base_type == UnitBaseType::Fighter).count();
        let blanked = crate::query::reach::blanked(sx.state);
        if fighters < 2 && crate::query::laws::holds_ministry(sx.state, blanked, who, Agenda::ProphecyOfIxth) {
            sx.state.agenda_mut().laws.remove(&Agenda::ProphecyOfIxth);
        }
        if self.made.as_slice().is_empty() || !sx.state.meta().rules_at_least(RULES_SELF_ASSEMBLY) {
            return self.close(sx);
        }
        let owns = crate::query::tech::owns(sx.state, who, Technology::SelfAssemblyRoutines);
        if !owns {
            return self.close(sx);
        }
        match sx.table.links.start(&Spawn::SelfAssembly { who, system: self.facts.produce.system }) {
            Some(window) => {
                self.assembly.start(window);
                self.stage = HStage::Assembling;
            }
            None => {
                self.unsupported = true;
                self.close(sx);
            }
        }
    }

    /// The window is closed: the observers clear what lasted for the use.
    fn close(&mut self, sx: &mut Sx<'_>) {
        sx.observe(&Obs::ProductionWindowClosed { who: self.who, system: self.facts.produce.system }, Phase::After);
        sx.state.transient_mut().production.value_swapped_planet = None;
        self.stage = HStage::Done;
    }
}

impl Window for HomeProduction {
    type Stage = HStage;

    fn stage(&self) -> HStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        loop {
            match self.stage {
                HStage::Start => self.start(sx),
                HStage::Hero => match self.hero.step(sx) {
                    Some(Ran::Asks(ask)) => return Next::Continue(ask),
                    Some(Ran::Pass(next)) => return next,
                    Some(Ran::Done(offer)) => {
                        if offer.used {
                            self.hero_used(sx.state);
                        }
                        self.begin_use();
                    }
                    None => self.begin_use(),
                },
                HStage::Using => match self.using.step(sx) {
                    Some(Ran::Asks(ask)) => return Next::Continue(ask),
                    Some(Ran::Pass(next)) => return next,
                    Some(Ran::Done(finished)) => {
                        if self.answered > 0 {
                            self.finish_use(sx, &finished);
                        } else {
                            // A use that asked nothing has no end-of-use effects; the window still closes.
                            self.close(sx);
                        }
                    }
                    None => self.close(sx),
                },
                HStage::Assembling => match self.assembly.step(sx) {
                    Some(Ran::Asks(ask)) => return Next::Continue(ask),
                    Some(Ran::Pass(next)) => return next,
                    Some(Ran::Done(_)) | None => self.close(sx),
                },
                HStage::Done => return Next::Done,
            }
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        match self.stage {
            HStage::Hero => self.hero.ask_of(s, k),
            HStage::Using => self.using.ask_of(s, k),
            HStage::Assembling => self.assembly.ask_of(s, k),
            HStage::Start | HStage::Done => None,
        }
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        match self.stage {
            HStage::Hero => self.hero.answer(sx, pick),
            HStage::Using => {
                self.using.answer(sx, pick)?;
                self.answered += 1;
                Ok(())
            }
            HStage::Assembling => self.assembly.answer(sx, pick),
            HStage::Start | HStage::Done => Err(Illegal),
        }
    }
}
