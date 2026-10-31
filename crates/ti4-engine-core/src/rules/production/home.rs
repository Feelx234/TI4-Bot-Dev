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
use super::Cx;
use crate::generated::{Agenda, Breakthrough, Leader, System, Technology, Unit, UnitBaseType};
use crate::ids::Seat;
use crate::kernel::children::{ErasedChild, HomeFacts, Made, ProduceFacts, Spawn};
use crate::kernel::seam::observe::{Obs, Phase};
use crate::kernel::stage::{Child, Illegal, Ran, Sx, Window};
use crate::kernel::{fx, Kernel, Next};
use crate::query::tech::{self, UnitRoster};
use crate::state::{CoreState, LeaderStatus};
use crate::tables::Scope;

/// `ti4_model::state::RULES_SELF_ASSEMBLY`.
const RULES_SELF_ASSEMBLY: u32 = 13;

/// `production::BUILDABLE`: the base types a seat may produce, in the order of the offer.
const BUILDABLE: [UnitBaseType; 9] = [
    UnitBaseType::Fighter,
    UnitBaseType::Infantry,
    UnitBaseType::Carrier,
    UnitBaseType::Cruiser,
    UnitBaseType::Destroyer,
    UnitBaseType::Dreadnought,
    UnitBaseType::Mech,
    UnitBaseType::Flagship,
    UnitBaseType::Warsun,
];

/// The fleet bill of a seat whose fleet supply is unlimited for the round (`fleet::UNLIMITED_FLEET_BILL`).
const UNLIMITED_FLEET_BILL: i64 = 10_000;

/// `production_view::buildable_for` / `production::buildable_for`: what `who` may produce, in the order of the offer. A unit upgrade owned
/// replaces the unit it covers, the faction's own unit stands in for a plain one, a mech and a flagship exist only as the faction's own, a war
/// sun needs the technology (or a faction that prints its own), and the hook table may name another form (Mentak's Corsair). `None`: no such seat.
#[must_use]
pub fn buildable_for(core: &CoreState, k: &Kernel, scope: Scope, roster: &UnitRoster, who: Seat) -> Option<[Option<Unit>; 9]> {
    let p = core.players().by_seat(who)?;
    let mut out = [None; 9];
    let mut slots = out.iter_mut();
    for base in BUILDABLE {
        let own = tech::faction_unit(roster, p.faction, base);
        if base == UnitBaseType::Warsun && !p.tech.owned.contains(Technology::WarSun) && own.is_none() {
            continue;
        }
        let chosen = tech::unlocked_upgrade(scope, base, p.faction, &p.tech.owned)
            .or(own)
            .or_else(|| if matches!(base, UnitBaseType::Mech | UnitBaseType::Flagship) { None } else { tech::generic_unit(scope, base) });
        // (The hook is asked for a base type with no form too: the string passes it an empty form. The one rule it carries names a cruiser.)
        let form = match chosen {
            Some(unit) => Some(k.hooks.strategy.unit_form_override(core, k, who, base, unit).unwrap_or(unit)),
            None => None,
        };
        if let (Some(form), Some(slot)) = (form, slots.next()) {
            *slot = Some(form);
        }
    }
    Some(out)
}

/// The fleet limit one seat judges a production by (`fleet::limit`, or the unlimited bill for the round of Darktalon Treilla's hero): the pool
/// with the tokens other players hold in it, held by Fleet Regulations, lifted by Armada and the hook table.
#[must_use]
pub fn fleet_limit(core: &CoreState, k: &Kernel, blanked: bool, who: Seat) -> i64 {
    if crate::query::fleet::is_unlimited(core, who) {
        return UNLIMITED_FLEET_BILL;
    }
    let foreign = k.hooks.direct.mahact_foreign_tokens(core, who);
    let capped = crate::query::laws::fleet_pool_cap(core, blanked, crate::query::fleet::pool_base(core, who, foreign));
    let lifted = k.hooks.general.fleet_supply(core, k, who, capped + crate::rules::abilities::armada_bonus(core, who));
    i64::from(lifted).max(0)
}

/// The facts of the use of PRODUCTION `who` would open in `system` outside a tactical action, read from the position as it stands now
/// (`ProductionWindow::new` and `to_core`): the capacity, the exemptions of the Cabal commander, the cost reduction of the hooks, the fleet
/// limit, the units that may be built. `None`: a seat that does not exist or a number beyond 32 bits. A secondary performed before it that
/// changes what a production reads (Leadership's tokens, a researched unit upgrade, a space dock) is therefore seen.
#[must_use]
pub fn home_facts(core: &CoreState, k: &Kernel, roster: &UnitRoster, who: Seat, system: System, agent_rules: bool) -> Option<HomeFacts> {
    let scope = Scope(u32::from(k.sources.0));
    let blanked = crate::query::reach::blanked(core);
    let cx = Cx::new(core, k, scope, blanked);
    Some(HomeFacts {
        produce: ProduceFacts {
            who,
            system,
            limit: i32::try_from(super::capacity(&cx, who, system).max(0)).ok()?,
            max_unit_cost: None,
            small_exemptions: if crate::query::leaders::has_commander_ability_in(core, who, Leader::ThatWhichMoldsFlesh) { 2 } else { 0 },
            discount: i32::try_from(k.hooks.economy.production_cost_reduction(core, who, system).max(0)).ok()?,
            fleet_limit: i32::try_from(fleet_limit(core, k, blanked, who)).ok()?,
            armada: crate::rules::abilities::armada_bonus(core, who),
            flagship_free: crate::query::leaders::has_commander_ability_in(core, who, Leader::NavarchFeng),
            buildable: buildable_for(core, k, scope, roster, who)?,
        },
        agent_rules,
    })
}

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
    system: System,
    /// The facts the use opens with: given ([`Self::new`]) or read from the position when the use starts ([`Self::at`]).
    facts: Option<HomeFacts>,
    /// [`Self::at`]: the roster of the faction units and the rule of Berekar Berekon the facts are read with.
    live: Option<(&'static UnitRoster, bool)>,
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
        Self { facts: Some(facts), ..Self::blank(facts.produce.who, facts.produce.system) }
    }

    /// The use of PRODUCTION `who` has in `system`, its facts read from the position when it starts ([`home_facts`]): what an ability performed
    /// before it changed is seen. A system with no PRODUCTION to use does nothing, and neither opens a window nor announces one.
    #[must_use]
    pub fn at(who: Seat, system: System, roster: &'static UnitRoster, agent_rules: bool) -> Self {
        Self { live: Some((roster, agent_rules)), ..Self::blank(who, system) }
    }

    fn blank(who: Seat, system: System) -> Self {
        Self { who, system, facts: None, live: None, stage: HStage::Start, window: None, hero: Child::idle(), using: Child::idle(), answered: 0, assembly: ErasedChild::idle(), made: Made::new(), unsupported: false }
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
        if let Some((roster, agent_rules)) = self.live {
            match home_facts(sx.state, sx.kernel, roster, self.who, self.system, agent_rules) {
                Some(facts) if facts.produce.limit > 0 => self.facts = Some(facts),
                // Nothing to produce with: the string does not open a window at all.
                Some(_) => {
                    self.stage = HStage::Done;
                    return;
                }
                None => {
                    self.unsupported = true;
                    self.stage = HStage::Done;
                    return;
                }
            }
        }
        let Some(facts) = self.facts else {
            self.stage = HStage::Done;
            return;
        };
        if facts.agent_rules {
            // This use reads no discount an earlier tactical action left unspent.
            sx.state.transient_mut().production.discount_remaining = 0;
        }
        self.window = Some(ProductionWindow::use_of(&facts.produce, facts.agent_rules));
        if facts.produce.limit > 0 && Self::hero_unlocked(sx.state, self.who) {
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
            window.discount_remaining = core.transient().production.discount_remaining + self.facts.map_or(0, |facts| facts.produce.discount);
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
        match sx.table.links.start(&Spawn::SelfAssembly { who, system: self.system }) {
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
        sx.observe(&Obs::ProductionWindowClosed { who: self.who, system: self.system }, Phase::After);
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
