//! One unit outside a use of PRODUCTION (package W12, lent-bodies lane 7; the string `produce_one_from`): Chaos Mapping's single unit,
//! Freelancers (an exploration card; influence pays as if it were resources) and the single units of the Minister of Industry before
//! its window.
//!
//! "You may produce 1 unit in this system" (Freelancers adds: "You may spend influence as if it were resources to produce this unit",
//! which the position carries as `Meta::influence_pays_for_units`, set by the caller around the production): the units the seat can afford, place and has plastic for are offered with a decline; the
//! chosen one is paid for (the free payment loop of [`super::pay::PayStep`]), placed (asked when it has more than one spot) and the
//! fleet supply is judged. What stays outside: the removal questions of the fleet-supply enforcement the string asks after the unit
//! stands, which the window cannot ask (it ends [`ProduceOne::unsupported`]) when the system is over its supply or its capacity.

use super::pay::PayStep;
use super::{Cx, Held, Spot, placed_of, placements, price};
use crate::choice::generic::Content;
use crate::choice::production::{Ask as PAsk, OneAsk, Opt as POpt, Shown, Subtype as PSub};
use crate::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype, Target};
use crate::containers::SmallList;
use crate::generated::{System, Unit};
use crate::ids::Seat;
use crate::kernel::fx;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::query::economy::Spend;
use crate::query::fleet::{self, Arrival};
use crate::query::reach;
use crate::state::CoreState;
use crate::tables::Scope;

fn cx<'a>(s: &'a CoreState, k: &'a Kernel) -> Cx<'a> {
    Cx::new(s, k, Scope(u32::from(k.sources.0)), reach::blanked(s))
}

fn small(n: i64) -> i32 {
    i32::try_from(n).unwrap_or(if n < 0 { i32::MIN } else { i32::MAX })
}

/// Why a unit the seat may build is not offered, the first that applies (the string `not_offered` of the question's details).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Why {
    /// "costs {cost}, you can pay {can_pay}".
    Costs { cost: i32, can_pay: i32 },
    /// No spot takes it in this system (ground forces need a planet with a producer).
    NoPlace,
    /// The fleet supply of the system would break.
    FleetFull,
    /// The box holds none (or a faction rule bars it).
    Empty,
}

/// What the seat may build in `system` now and what it may not, with the reason.
#[derive(Debug, Clone, Default)]
pub struct Sorted {
    pub candidates: SmallList<(Unit, i64), 9>,
    pub not_offered: SmallList<(Unit, Why), 9>,
}

/// `produce_one_from`'s candidates: the units of `buildable` in order, each offered or refused for the first reason that applies.
#[must_use]
pub fn sort(c: &Cx<'_>, who: Seat, system: System, buildable: &[Option<Unit>; 9], fleet_limit: i32) -> Sorted {
    let held = Held::of(c, who);
    let affordable = super::available(c, who, Spend::Resources, &held.extras());
    let (s, k) = (c.core(), c.q.k);
    let half = |u: Unit| k.hooks.combat.fighter_fleet_weight_halves(s, k, who, u);
    let mut out = Sorted::default();
    for unit in buildable.iter().flatten().copied() {
        let cost = price(unit.data().cost_halves, false).0;
        let within_supply = !fleet::counts_against_supply(unit)
            || fleet::standing(s, c.scope, who, system, i64::from(fleet_limit), Some(Arrival { kind: unit, count: 1, in_space: true }), &half).fleet_excess() == 0;
        let blocked = if cost > affordable {
            Some(Why::Costs { cost: small(cost), can_pay: small(affordable) })
        } else if placements(c, who, system, unit).as_slice().is_empty() {
            Some(Why::NoPlace)
        } else if !within_supply {
            Some(Why::FleetFull)
        } else if placed_of(c, who, unit, 1) == 0 {
            Some(Why::Empty)
        } else {
            None
        };
        match blocked {
            None => out.candidates.push((unit, cost)),
            Some(why) => out.not_offered.push((unit, why)),
        }
    }
    out
}

pub use crate::kernel::children::OneFacts;

/// The suspension points of [`ProduceOne`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OStage {
    Start,
    Choosing,
    Paying { unit: Unit, cost: i32, paid: i32 },
    Placing { unit: Unit },
    Done,
}

/// "Produce one unit in `system`" (see the module). [`Self::made`]: the unit that stands and where.
#[derive(Debug, Clone)]
pub struct ProduceOne {
    pub who: Seat,
    pub system: System,
    facts: OneFacts,
    stage: OStage,
    pub made: Option<(Unit, Spot)>,
    /// The unit stands over the fleet supply or the capacity of its system: the string asks which units go, which the window cannot.
    pub unsupported: bool,
}

impl ProduceOne {
    #[must_use]
    pub const fn new(who: Seat, system: System, facts: OneFacts) -> Self {
        Self { who, system, facts, stage: OStage::Start, made: None, unsupported: false }
    }

    fn one_ask(&self) -> OneAsk {
        OneAsk { who: self.who, system: self.system, name: self.facts.name, alias: self.facts.alias, buildable: self.facts.buildable, fleet_limit: self.facts.fleet_limit }
    }

    fn ctx(&self, s: &CoreState, subtype: Subtype, with_source: bool) -> Ctx {
        let origin = match (with_source, self.facts.alias) {
            (true, Some(alias)) => Origin::Content(Content::Other(alias)),
            _ => Origin::Rule(crate::choice::generic::Rule::R68),
        };
        let mut ctx = Ctx { origin, subtype, optional: with_source && self.facts.alias.is_some(), target: Some(Target::System(self.system)), outstanding: SmallList::new(), space_battle: false, invasion_seq: None };
        super::enrich_ctx(s, &mut ctx);
        ctx
    }

    fn pay_step(&self, cost: i32, paid: i32) -> PayStep {
        PayStep { who: self.who, kind: Spend::Resources, cost: i64::from(cost), used_credit: 0, owed: i64::from(cost), paid: i64::from(paid), took: None, stage: super::pay::Stage::Paying }
    }

    /// The window is over. (Freelancers' permission to pay with influence is the position's: `Meta::influence_pays_for_units`, set and
    /// cleared by the caller around the production.)
    fn finish(&mut self, _s: &mut CoreState) {
        self.stage = OStage::Done;
    }

    fn question(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        let c = cx(s, k);
        match self.stage {
            OStage::Choosing => {
                let sorted = sort(&c, self.who, self.system, &self.facts.buildable, self.facts.fleet_limit);
                if sorted.candidates.as_slice().is_empty() {
                    return None;
                }
                let mut opts: Opts = sorted
                    .candidates
                    .as_slice()
                    .iter()
                    .map(|(unit, cost)| Opt::Production(POpt::One { unit: *unit, shown: Shown { worth: small(*cost), pool_after: 0 } }))
                    .collect();
                opts.push(Opt::Decline);
                Some(Choice { actor: self.who, ask: Ask::Production(PAsk::One(self.one_ask())), ctx: self.ctx(s, Subtype::Production(PSub::One), true), opts })
            }
            OStage::Paying { cost, paid, .. } => self.pay_step(cost, paid).question(s, k),
            OStage::Placing { unit } => {
                let spots = placements(&c, self.who, self.system, unit);
                (spots.as_slice().len() > 1).then(|| Choice {
                    actor: self.who,
                    ask: Ask::Production(PAsk::OnePlace { unit, system: self.system }),
                    ctx: self.ctx(s, Subtype::Production(PSub::Place), false),
                    opts: spots.as_slice().iter().map(|spot| Opt::Production(POpt::Place { spot: *spot })).collect(),
                })
            }
            OStage::Start | OStage::Done => None,
        }
    }

    fn ask_next(&self, sx: &Sx<'_>) -> Next {
        match self.question(sx.state, sx.kernel) {
            Some(c) => Next::Continue(c.ask),
            None => Next::Done,
        }
    }

    /// The unit stands: the Naaz mech announces itself and the fleet supply is judged.
    fn place(&mut self, s: &mut CoreState, k: &Kernel, unit: Unit, spot: Spot) {
        let planet = match spot {
            Spot::Planet(planet) => Some(planet),
            _ => None,
        };
        fx::place_unit(s, self.who, self.system, planet, unit);
        fx::stage_naaz_mech_placed(s, self.who, self.system, unit);
        self.made = Some((unit, spot));
        let c = cx(s, k);
        let half = |u: Unit| k.hooks.combat.fighter_fleet_weight_halves(s, k, self.who, u);
        let standing = fleet::standing(s, c.scope, self.who, self.system, i64::from(self.facts.fleet_limit), None, &half);
        self.unsupported = standing.fleet_excess() > 0 || standing.consumed > standing.transport;
        self.finish(s);
    }
}

impl Window for ProduceOne {
    type Stage = OStage;

    fn stage(&self) -> OStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        loop {
            match self.stage {
                OStage::Start => {
                    let sorted = sort(&cx(sx.state, sx.kernel), self.who, self.system, &self.facts.buildable, self.facts.fleet_limit);
                    if sorted.candidates.as_slice().is_empty() {
                        self.finish(sx.state);
                    } else {
                        self.stage = OStage::Choosing;
                    }
                }
                OStage::Choosing => return self.ask_next(sx),
                OStage::Paying { unit, cost, paid } => {
                    if paid >= cost {
                        self.stage = OStage::Placing { unit };
                        continue;
                    }
                    let (_, opts) = super::pay_opts(&cx(sx.state, sx.kernel), self.who, Spend::Resources, i64::from(paid), i64::from(cost), &Held::of(&cx(sx.state, sx.kernel), self.who).extras());
                    match opts.as_slice() {
                        [] => self.finish(sx.state),
                        [only] => {
                            let Some(worth) = self.pay_step(cost, paid).apply(sx.state, sx.kernel, *only) else {
                                self.finish(sx.state);
                                continue;
                            };
                            self.stage = OStage::Paying { unit, cost, paid: paid + small(worth) };
                        }
                        _ => return self.ask_next(sx),
                    }
                }
                OStage::Placing { unit } => {
                    let spots = placements(&cx(sx.state, sx.kernel), self.who, self.system, unit);
                    match spots.as_slice() {
                        // Paid for, and nowhere to put it.
                        [] => self.finish(sx.state),
                        [only] => {
                            let only = *only;
                            self.place(sx.state, sx.kernel, unit, only);
                        }
                        _ => return self.ask_next(sx),
                    }
                }
                OStage::Done => return Next::Done,
            }
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        self.question(s, k)
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        match (self.stage, pick) {
            (OStage::Choosing, Pick::Opt(Opt::Decline)) => {
                self.finish(sx.state);
                Ok(())
            }
            (OStage::Choosing, Pick::Opt(Opt::Production(POpt::One { unit, .. }))) => {
                let c = cx(sx.state, sx.kernel);
                let sorted = sort(&c, self.who, self.system, &self.facts.buildable, self.facts.fleet_limit);
                let cost = sorted.candidates.as_slice().iter().find(|(u, _)| *u == unit).map(|(_, cost)| *cost).ok_or(Illegal)?;
                let held = Held::of(&c, self.who);
                if cost > 0 && super::available(&c, self.who, Spend::Resources, &held.extras()) < cost {
                    self.finish(sx.state);
                    return Ok(());
                }
                self.stage = OStage::Paying { unit, cost: small(cost), paid: 0 };
                Ok(())
            }
            (OStage::Paying { unit, cost, paid }, Pick::Opt(Opt::Production(pay))) => {
                let Some(worth) = self.pay_step(cost, paid).apply(sx.state, sx.kernel, pay) else {
                    self.finish(sx.state);
                    return Ok(());
                };
                self.stage = OStage::Paying { unit, cost, paid: paid + small(worth) };
                Ok(())
            }
            (OStage::Placing { unit }, Pick::Opt(Opt::Production(POpt::Place { spot }))) => {
                let spots = placements(&cx(sx.state, sx.kernel), self.who, self.system, unit);
                if !spots.as_slice().contains(&spot) {
                    return Err(Illegal);
                }
                self.place(sx.state, sx.kernel, unit, spot);
                Ok(())
            }
            _ => Err(Illegal),
        }
    }
}
