//! Productions outside a use of PRODUCTION (package B3): Sling Relay's one ship at an unblocked dock and
//! Integrated Economy's units on a conquered planet, as [`Window`]s (the string `sling_relay` and
//! `integrated_economy`, which ask a `Table` in a loop).
//!
//! Both pay with the free payment loop (`PayStep`'s question and application) and place the units
//! themselves. The seat's list of buildable units (`buildable_for`) and the fleet limit are the adapter's
//! facts, as in the production window. What stays outside: the fleet-supply enforcement after Integrated
//! Economy (the caller's), the Keleres agent offer at the payment window and the Naaz mech staging (the
//! callers decline the tables where they could fire, `production_view::unported_window`).

use super::pay::PayStep;
use super::{price, Cx, Extras, placed_of};
use crate::choice::generic::Rule;
use crate::choice::production::{Ask as PAsk, EconomyAsk, Opt as POpt, Shown, SlingAsk, SlingShipAsk, Subtype as PSub};
use crate::choice::{Ask, Choice, Ctx, Opt, Origin, Owed, Pick, Subtype, Target};
use crate::containers::SmallList;
use crate::generated::{Planet, System, Unit, UnitBaseType};
use crate::ids::Seat;
use crate::kernel::fx;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::query::economy::Spend;
use crate::query::fleet::{self, Arrival};
use crate::query::{board, reach};
use crate::state::CoreState;
use crate::tables::Scope;

fn cx<'a>(s: &'a CoreState, k: &'a Kernel) -> Cx<'a> {
    Cx::new(s, k, Scope(u32::from(k.sources.0)), reach::blanked(s))
}

fn small(n: i64) -> i32 {
    i32::try_from(n).unwrap_or(if n < 0 { i32::MIN } else { i32::MAX })
}

fn ctx(s: &CoreState, subtype: PSub, target: Option<Target>, outstanding: SmallList<Owed, 4>) -> Ctx {
    let mut c = Ctx {
        origin: Origin::Rule(Rule::R68),
        subtype: Subtype::Production(subtype),
        optional: false,
        target,
        outstanding,
        space_battle: false,
        invasion_seq: None,
    };
    super::enrich_ctx(s, &mut c);
    c
}

fn units_of(buildable: &[Option<Unit>; 9]) -> impl Iterator<Item = Unit> + '_ {
    buildable.iter().flatten().copied()
}

/// The printed price, without the law that halves a pair's yield (`production::price_of`).
fn printed_price(unit: Unit) -> i64 {
    price(unit.data().cost_halves, false).0
}

// -- Sling Relay ---------------------------------------------------------------------------------------

/// The ships Sling Relay may produce: `(system, unit, cost)`, systems in id order, ships in the seat's
/// buildable order.
pub type Candidates = SmallList<(System, Unit, i64), 48>;

/// The systems of `cands`, once each, in order.
fn systems_of(cands: &Candidates) -> SmallList<System, 8> {
    let mut out = SmallList::new();
    for (system, ..) in cands.as_slice() {
        if !out.as_slice().contains(system) {
            out.push(*system);
        }
    }
    out
}

/// The ships of one system.
fn ships_in(cands: &Candidates, system: System) -> SmallList<(Unit, i64), 9> {
    let mut out = SmallList::new();
    for (sys, unit, cost) in cands.as_slice() {
        if *sys == system {
            out.push((*unit, *cost));
        }
    }
    out
}

/// `sling_relay_candidates`: the systems with one of the seat's docks that nobody blockades, each with the ships
/// the seat can afford and has plastic for, in system id order.
#[must_use]
pub fn sling_candidates(c: &Cx<'_>, who: Seat, buildable: &[Option<Unit>; 9]) -> Candidates {
    let affordable = super::available(c, who, Spend::Resources, &Extras::default());
    let core = c.core();
    let mut out = SmallList::new();
    for (system, cell) in core.board().systems() {
        let is_dock = |p: &crate::state::Piece| p.owner() == who && p.kind().data().base_type == UnitBaseType::Spacedock;
        let dock_planets: SmallList<Planet, 8> = {
            let mut v = SmallList::new();
            for planet in board::planets_of(core, system) {
                if board::planet_units(core, planet).iter().any(is_dock) {
                    v.push(planet);
                }
            }
            v
        };
        // A space dock in the space area (Saar's Floating Factory) is a space dock too.
        let has_dock = !dock_planets.as_slice().is_empty() || cell.units.as_slice().iter().any(|p| is_dock(p) && p.kind().data().space_only);
        // Coexistence rule 4: a coexisting structure is always blockaded.
        let coexisting_dock = dock_planets.as_slice().iter().any(|p| board::is_coexisting(core, system, *p, who));
        let blockaded = coexisting_dock || cell.units.as_slice().iter().any(|p| p.owner() != who && p.kind().data().ship);
        if !has_dock || blockaded {
            continue;
        }
        for unit in units_of(buildable) {
            let cost = printed_price(unit);
            if unit.data().ship && cost <= affordable && placed_of(c, who, unit, 1) > 0 {
                out.push((system, unit, cost));
            }
        }
    }
    out
}

/// Where Sling Relay has got to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlingStage {
    /// Choosing the dock system.
    System,
    /// Choosing the ship in `system`.
    Ship { system: System },
    /// Paying `cost` for `unit`, `paid` collected so far.
    Paying { system: System, unit: Unit, cost: i32, paid: i32 },
    Done,
}

/// Sling Relay: produce one ship without consuming a dock's PRODUCTION value.
#[derive(Debug, Clone)]
pub struct SlingRelay {
    pub who: Seat,
    pub buildable: [Option<Unit>; 9],
    pub stage: SlingStage,
    /// The ship was paid for and placed.
    pub produced: bool,
}

impl SlingRelay {
    #[must_use]
    pub const fn new(who: Seat, buildable: [Option<Unit>; 9]) -> Self {
        Self { who, buildable, stage: SlingStage::System, produced: false }
    }

    fn pay_step(&self, cost: i32, paid: i32) -> PayStep {
        PayStep { who: self.who, kind: Spend::Resources, cost: i64::from(cost), used_credit: 0, owed: i64::from(cost), paid: i64::from(paid), took: None, stage: super::pay::Stage::Paying }
    }

    fn question(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        let c = cx(s, k);
        match self.stage {
            SlingStage::System => {
                let systems = systems_of(&sling_candidates(&c, self.who, &self.buildable));
                (systems.as_slice().len() > 1).then(|| Choice {
                    actor: self.who,
                    ask: Ask::Production(PAsk::SlingSystem(SlingAsk { who: self.who })),
                    ctx: ctx(s, PSub::SlingSystem, None, SmallList::new()),
                    opts: systems.as_slice().iter().map(|system| Opt::Production(POpt::SlingSystem { system: *system })).collect(),
                })
            }
            SlingStage::Ship { system } => {
                let ships = ships_in(&sling_candidates(&c, self.who, &self.buildable), system);
                (ships.as_slice().len() > 1).then(|| Choice {
                    actor: self.who,
                    ask: Ask::Production(PAsk::SlingShip(SlingShipAsk { who: self.who, system })),
                    ctx: ctx(s, PSub::SlingShip, Some(Target::System(system)), SmallList::new()),
                    opts: ships.as_slice().iter().map(|(unit, cost)| Opt::Production(POpt::SlingShip { unit: *unit, shown: Shown { worth: small(*cost), pool_after: 0 } })).collect(),
                })
            }
            SlingStage::Paying { cost, paid, .. } => self.pay_step(cost, paid).question(s, k),
            SlingStage::Done => None,
        }
    }

    /// The ship chosen: start paying, or end when it cannot be paid.
    fn chosen(&mut self, s: &CoreState, k: &Kernel, system: System, unit: Unit, cost: i64) {
        let owed = cost;
        if owed <= 0 {
            self.stage = SlingStage::Paying { system, unit, cost: 0, paid: 0 };
            return;
        }
        if super::available(&cx(s, k), self.who, Spend::Resources, &Extras::default()) < owed {
            self.stage = SlingStage::Done;
            return;
        }
        self.stage = SlingStage::Paying { system, unit, cost: small(cost), paid: 0 };
    }

    fn finish(&mut self, s: &mut CoreState, system: System, unit: Unit) {
        fx::place_unit(s, self.who, system, None, unit);
        self.produced = true;
        self.stage = SlingStage::Done;
    }
}

impl Window for SlingRelay {
    type Stage = SlingStage;

    fn stage(&self) -> SlingStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        loop {
            match self.stage {
                SlingStage::System => {
                    let systems = systems_of(&sling_candidates(&cx(sx.state, sx.kernel), self.who, &self.buildable));
                    match systems.as_slice() {
                        [] => {
                            self.stage = SlingStage::Done;
                            return Next::Done;
                        }
                        [system] => self.stage = SlingStage::Ship { system: *system },
                        _ => return self.ask_next(sx),
                    }
                }
                SlingStage::Ship { system } => {
                    let ships = ships_in(&sling_candidates(&cx(sx.state, sx.kernel), self.who, &self.buildable), system);
                    match ships.as_slice() {
                        [] => {
                            self.stage = SlingStage::Done;
                            return Next::Done;
                        }
                        [(unit, cost)] => {
                            let (unit, cost) = (*unit, *cost);
                            self.chosen(sx.state, sx.kernel, system, unit, cost);
                        }
                        _ => return self.ask_next(sx),
                    }
                }
                SlingStage::Paying { system, unit, cost, paid } => {
                    if paid >= cost {
                        self.finish(sx.state, system, unit);
                        return Next::Done;
                    }
                    let step = self.pay_step(cost, paid);
                    let (_, opts) = super::pay_opts(&cx(sx.state, sx.kernel), self.who, Spend::Resources, i64::from(paid), i64::from(cost), &Extras::default());
                    match opts.as_slice() {
                        [] => {
                            self.stage = SlingStage::Done;
                            return Next::Done;
                        }
                        [only] => {
                            let Some(worth) = step.apply(sx.state, sx.kernel, *only) else {
                                self.stage = SlingStage::Done;
                                return Next::Done;
                            };
                            self.stage = SlingStage::Paying { system, unit, cost, paid: paid + small(worth) };
                        }
                        _ => return self.ask_next(sx),
                    }
                }
                SlingStage::Done => return Next::Done,
            }
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        self.question(s, k)
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        let Pick::Opt(Opt::Production(opt)) = pick else { return Err(Illegal) };
        match (self.stage, opt) {
            (SlingStage::System, POpt::SlingSystem { system }) => {
                self.stage = SlingStage::Ship { system };
                Ok(())
            }
            (SlingStage::Ship { system }, POpt::SlingShip { unit, .. }) => {
                let ships = ships_in(&sling_candidates(&cx(sx.state, sx.kernel), self.who, &self.buildable), system);
                let cost = ships.as_slice().iter().find(|(u, _)| *u == unit).map(|(_, c)| *c).ok_or(Illegal)?;
                self.chosen(sx.state, sx.kernel, system, unit, cost);
                Ok(())
            }
            (SlingStage::Paying { system, unit, cost, paid }, pick) => {
                let Some(worth) = self.pay_step(cost, paid).apply(sx.state, sx.kernel, pick) else {
                    self.stage = SlingStage::Done;
                    return Ok(());
                };
                self.stage = SlingStage::Paying { system, unit, cost, paid: paid + small(worth) };
                Ok(())
            }
            _ => Err(Illegal),
        }
    }
}

impl SlingRelay {
    fn ask_next(&self, sx: &Sx<'_>) -> Next {
        match self.question(sx.state, sx.kernel) {
            Some(c) => Next::Continue(c.ask),
            None => Next::Done,
        }
    }
}

// -- Integrated Economy ----------------------------------------------------------------------------------

/// Where Integrated Economy has got to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EconomyStage {
    Choosing,
    Paying { unit: Unit, made: u8, cost: i32, paid: i32 },
    Done,
}

/// Integrated Economy: produce units on a planet just gained, up to its resource value.
#[derive(Debug, Clone)]
pub struct EconomyWindow {
    pub who: Seat,
    pub system: System,
    pub planet: Planet,
    pub budget: i32,
    pub fleet_limit: i32,
    pub buildable: [Option<Unit>; 9],
    pub stage: EconomyStage,
    pub built: bool,
}

impl EconomyWindow {
    #[must_use]
    pub const fn new(who: Seat, system: System, planet: Planet, budget: i32, fleet_limit: i32, buildable: [Option<Unit>; 9]) -> Self {
        Self { who, system, planet, budget, fleet_limit, buildable, stage: EconomyStage::Choosing, built: false }
    }

    fn ask(&self) -> EconomyAsk {
        EconomyAsk { who: self.who, system: self.system, planet: self.planet, budget: self.budget, fleet_limit: self.fleet_limit, buildable: self.buildable }
    }

    /// The units on offer, `(unit, made, cost)`, in buildable order.
    fn offers(&self, s: &CoreState, k: &Kernel) -> SmallList<(Unit, u8, i64), 9> {
        let c = cx(s, k);
        let affordable = super::available(&c, self.who, Spend::Resources, &Extras::default());
        let half = |u: Unit| k.hooks.combat.fighter_fleet_weight_halves(s, k, self.who, u);
        let mut out = SmallList::new();
        for unit in units_of(&self.buildable) {
            let (cost, pair) = super::price_of(&c, unit);
            if cost <= 0 || cost > i64::from(self.budget) || cost > affordable {
                continue;
            }
            let made = placed_of(&c, self.who, unit, pair);
            if made == 0 {
                continue;
            }
            if fleet::counts_against_supply(unit) {
                let after = fleet::standing(s, c.scope, self.who, self.system, i64::from(self.fleet_limit), Some(Arrival { kind: unit, count: 1, in_space: true }), &half);
                if after.fleet_excess() > 0 {
                    continue;
                }
            }
            out.push((unit, u8::try_from(made).unwrap_or(u8::MAX), cost));
        }
        out
    }

    fn question(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        match self.stage {
            EconomyStage::Choosing => {
                if self.budget <= 0 {
                    return None;
                }
                let offers = self.offers(s, k);
                if offers.as_slice().is_empty() {
                    return None;
                }
                let mut opts: crate::choice::Opts = offers
                    .as_slice()
                    .iter()
                    .map(|(unit, made, cost)| Opt::Production(POpt::Economy { unit: *unit, made: *made, shown: Shown { worth: small(*cost), pool_after: 0 } }))
                    .collect();
                opts.push(Opt::Production(POpt::Stop));
                Some(Choice { actor: self.who, ask: Ask::Production(PAsk::Economy(self.ask())), ctx: ctx(s, PSub::Economy, Some(Target::System(self.system)), SmallList::new()), opts })
            }
            EconomyStage::Paying { cost, paid, .. } => self.pay_step(cost, paid).question(s, k),
            EconomyStage::Done => None,
        }
    }

    fn pay_step(&self, cost: i32, paid: i32) -> PayStep {
        PayStep { who: self.who, kind: Spend::Resources, cost: i64::from(cost), used_credit: 0, owed: i64::from(cost), paid: i64::from(paid), took: None, stage: super::pay::Stage::Paying }
    }

    /// The unit is chosen: pay for it (`cost <= 0` or beyond the budget ends the use).
    fn chosen(&mut self, s: &CoreState, k: &Kernel, unit: Unit, made: u8) {
        let cost = printed_price(unit);
        if cost <= 0 || cost > i64::from(self.budget) {
            self.stage = EconomyStage::Done;
            return;
        }
        if super::available(&cx(s, k), self.who, Spend::Resources, &Extras::default()) < cost {
            self.stage = EconomyStage::Done;
            return;
        }
        self.stage = EconomyStage::Paying { unit, made, cost: small(cost), paid: 0 };
    }

    /// Paid in full: the units arrive (as many as the box allows now) and the budget falls by their price.
    fn arrive(&mut self, s: &mut CoreState, k: &Kernel, unit: Unit, made: u8, cost: i32) {
        let d = unit.data();
        let in_space = d.ship || d.base_type == UnitBaseType::Fighter;
        let made = placed_of(&cx(s, k), self.who, unit, usize::from(made));
        for _ in 0..made {
            fx::place_unit(s, self.who, self.system, if in_space { None } else { Some(self.planet) }, unit);
        }
        self.built |= made > 0;
        self.budget -= cost;
        self.stage = EconomyStage::Choosing;
    }
}

impl Window for EconomyWindow {
    type Stage = EconomyStage;

    fn stage(&self) -> EconomyStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        loop {
            match self.stage {
                EconomyStage::Choosing => {
                    if self.budget <= 0 || self.question(sx.state, sx.kernel).is_none() {
                        self.stage = EconomyStage::Done;
                        return Next::Done;
                    }
                    return self.ask_next(sx);
                }
                EconomyStage::Paying { unit, made, cost, paid } => {
                    if paid >= cost {
                        self.arrive(sx.state, sx.kernel, unit, made, cost);
                        continue;
                    }
                    let (_, opts) = super::pay_opts(&cx(sx.state, sx.kernel), self.who, Spend::Resources, i64::from(paid), i64::from(cost), &Extras::default());
                    match opts.as_slice() {
                        [] => {
                            self.stage = EconomyStage::Done;
                            return Next::Done;
                        }
                        [only] => {
                            let Some(worth) = self.pay_step(cost, paid).apply(sx.state, sx.kernel, *only) else {
                                self.stage = EconomyStage::Done;
                                return Next::Done;
                            };
                            self.stage = EconomyStage::Paying { unit, made, cost, paid: paid + small(worth) };
                        }
                        _ => return self.ask_next(sx),
                    }
                }
                EconomyStage::Done => return Next::Done,
            }
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        self.question(s, k)
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        let Pick::Opt(Opt::Production(opt)) = pick else { return Err(Illegal) };
        match (self.stage, opt) {
            (EconomyStage::Choosing, POpt::Stop) => {
                self.stage = EconomyStage::Done;
                Ok(())
            }
            (EconomyStage::Choosing, POpt::Economy { unit, made, .. }) => {
                self.chosen(sx.state, sx.kernel, unit, made);
                Ok(())
            }
            (EconomyStage::Paying { unit, made, cost, paid }, pick) => {
                let Some(worth) = self.pay_step(cost, paid).apply(sx.state, sx.kernel, pick) else {
                    self.stage = EconomyStage::Done;
                    return Ok(());
                };
                self.stage = EconomyStage::Paying { unit, made, cost, paid: paid + small(worth) };
                Ok(())
            }
            _ => Err(Illegal),
        }
    }
}

impl EconomyWindow {
    fn ask_next(&self, sx: &Sx<'_>) -> Next {
        match self.question(sx.state, sx.kernel) {
            Some(c) => Next::Continue(c.ask),
            None => Next::Done,
        }
    }
}
