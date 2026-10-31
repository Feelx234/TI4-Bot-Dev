//! The production window as a [`Window`] (package B3; the string `ProductionWindow` of `production.rs`).
//!
//! Ported: the loop of a use of PRODUCTION for positions with no faction hook in play: choose what to
//! build (`Stage::Choosing`), pay for it (`Paying`), place it (`Placing`), choose again, until the limit is
//! spent, nothing can be built or the player stops. The end-of-use effects of the string `resolve`
//! (`on_production_finished`, the Prophecy of Ixth discard, `UNITS_PRODUCED`, Self-Assembly Routines) and
//! the agent offers stay with the caller, which declines this window for positions where they could fire.
//!
//! The window carries the facts the string window holds in fields; the adapter fills them from it
//! (`production_window_dual.rs`).

use crate::choice::generic::Rule;
use crate::choice::production::{Ask as PAsk, ChooseAsk, Currency, Icon, Opt as POpt, PlaceAsk, Subtype as PSub};
use crate::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Owed, OwedKind, Pick, Subtype, Target};
use crate::containers::SmallList;
use crate::generated::{Planet, System, Unit, UnitBaseType};
use crate::ids::Seat;
use crate::kernel::children::{Made, ProduceFacts};
use crate::kernel::fx;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::query::economy::Spend;
use crate::query::{fleet, reach};
use crate::rules::production as rules;
use super::{Cx, Held, Spot};
use crate::generated::Leader;
use crate::state::{CoreState, EventKind, LeaderStatus, Payload};
use crate::state::{Key, Val};
use crate::tables::Scope;

/// The suspension points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Choosing,
    Paying { unit: Unit, owed: i32, made: u8, cost: i32, paid: i32 },
    Placing { unit: Unit, made: u8 },
    Done,
}

/// One use of PRODUCTION.
#[derive(Debug, Clone)]
pub struct ProductionWindow {
    pub who: Seat,
    pub system: System,
    pub stage: Stage,
    pub limit: i32,
    pub remaining: i32,
    pub free_capacity: i32,
    pub small_exemptions: i32,
    pub credit: i32,
    pub discount_remaining: i32,
    pub fleet_limit: i32,
    pub armada: i32,
    pub free_this_use: bool,
    pub flagship_free: bool,
    pub only_unit: Option<Unit>,
    pub max_unit_cost: Option<i32>,
    pub buildable: [Option<Unit>; 9],
    /// Units produced so far in this use and where they went.
    pub produced: SmallList<(Unit, Spot), 12>,
    pub purchased: bool,
    pub placement_timing_done: bool,
    /// The rules version offers Berekar Berekon inside the production question.
    pub agent_rules: bool,
    /// Berekar Berekon was exhausted in this use.
    pub agent_spent: bool,
    /// The caller has a timing handle: the ground-force production window is announced.
    pub emit_timing: bool,
    /// The units come from an ability, not from the PRODUCTION of a unit (`ProductionWindow::for_ability`, package W5).
    pub ability: bool,
}

fn small(n: i64) -> i32 {
    i32::try_from(n).unwrap_or(if n < 0 { i32::MIN } else { i32::MAX })
}

impl ProductionWindow {
    /// A production by an ability (`ProductionWindow::for_ability` of the string engine): `facts.limit` units, ground forces on any planet of the
    /// system the seat controls, no Winnu offer, no announcement of timing. The window opens at `Choosing` when it may produce anything.
    #[must_use]
    pub fn ability(facts: &ProduceFacts) -> Self {
        let limit = facts.limit.max(0);
        Self {
            who: facts.who,
            system: facts.system,
            stage: if limit > 0 { Stage::Choosing } else { Stage::Done },
            limit,
            remaining: limit,
            free_capacity: 0,
            small_exemptions: facts.small_exemptions,
            credit: 0,
            discount_remaining: facts.discount,
            fleet_limit: facts.fleet_limit,
            armada: facts.armada,
            free_this_use: false,
            flagship_free: facts.flagship_free,
            only_unit: None,
            max_unit_cost: facts.max_unit_cost,
            buildable: facts.buildable,
            produced: SmallList::new(),
            purchased: false,
            placement_timing_done: false,
            agent_rules: false,
            agent_spent: false,
            emit_timing: false,
            ability: true,
        }
    }

    /// A use of PRODUCTION outside a tactical action (the Warfare secondary; the string `ProductionWindow::new`): the same fields as
    /// [`Self::ability`] but the units come from the PRODUCTION of a unit, so Berekar Berekon is offered from the rules version that
    /// has him (`agent_rules`) and the ground forces go only on planets that hold a producer. No announcement of timing.
    #[must_use]
    pub fn use_of(facts: &ProduceFacts, agent_rules: bool) -> Self {
        let mut window = Self::ability(facts);
        window.ability = false;
        window.agent_rules = agent_rules;
        window
    }

    /// What the use made, in order, with the planet each unit stands on (`None`: the space area; a ship placed away from the producing
    /// system or on the Crimson card has no planet of this system either).
    #[must_use]
    pub fn made(&self) -> Made {
        self.produced
            .as_slice()
            .iter()
            .map(|(unit, spot)| (*unit, if let Spot::Planet(planet) = spot { Some(*planet) } else { None }))
            .collect()
    }

    fn cx<'a>(&self, s: &'a CoreState, k: &'a Kernel) -> Cx<'a> {
        Cx::new(s, k, Scope(u32::from(k.sources.0)), reach::blanked(s))
    }

    fn choose_ask(&self) -> ChooseAsk {
        let mut produced = [0_u8; 11];
        for (unit, _) in self.produced.as_slice() {
            let base = unit.data().base_type as usize;
            produced[base] = produced[base].saturating_add(1);
        }
        ChooseAsk {
            who: self.who,
            system: self.system,
            limit: self.limit,
            remaining: self.remaining,
            free_capacity: self.free_capacity,
            small_exemptions: self.small_exemptions,
            credit: self.credit,
            discount_remaining: self.discount_remaining,
            fleet_limit: self.fleet_limit,
            free_this_use: self.free_this_use,
            flagship_free: self.flagship_free,
            only_unit: self.only_unit,
            max_unit_cost: self.max_unit_cost,
            produced,
            armada: self.armada,
            buildable: self.buildable,
            icon: self.icon(),
            ability: self.ability,
        }
    }

    fn place_ask(&self, unit: Unit, made: u8) -> PlaceAsk {
        PlaceAsk { who: self.who, system: self.system, unit, made, fleet_limit: self.fleet_limit, icon: self.icon(), ability: self.ability }
    }

    /// What the ships of this use have done so far (`bastion::adjust_ship_spots`): the first ship placed away from the
    /// producing system fixes the spot of the rest; a ship placed in the producing system closes the Icon.
    #[must_use]
    pub fn icon(&self) -> Icon {
        let mut placed = false;
        for (unit, spot) in self.produced.as_slice() {
            if !unit.data().ship {
                continue;
            }
            if matches!(spot, Spot::Remote(..) | Spot::Card) {
                return Icon::Remote(*spot);
            }
            placed = true;
        }
        if placed { Icon::Local } else { Icon::Fresh }
    }

    fn ctx(&self, s: &CoreState, rule: Rule, subtype: PSub, outstanding: SmallList<Owed, 4>) -> Ctx {
        let mut ctx = Ctx {
            origin: Origin::Rule(rule),
            subtype: Subtype::Production(subtype),
            optional: false,
            target: Some(Target::System(self.system)),
            outstanding,
            space_battle: false,
            invasion_seq: None,
        };
        rules::enrich_ctx(s, &mut ctx);
        ctx
    }

    /// Berekar Berekon (Winnu agent) may be offered inside the production question.
    fn agent_offerable(&self, s: &CoreState) -> bool {
        self.agent_rules && !self.purchased && !self.agent_spent && !self.free_this_use && Self::agent_ready(s, self.who)
    }

    fn agent_ready(s: &CoreState, who: Seat) -> bool {
        s.players().by_seat(who).is_some_and(|p| p.leaders.status.get(&Leader::BerekarBerekon) == Some(&LeaderStatus::Readied))
    }

    /// The question of the current stage, if it asks one.
    fn question(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        let cx = self.cx(s, k);
        match self.stage {
            Stage::Done => None,
            Stage::Choosing => {
                if self.remaining <= 0 && self.small_exemptions <= 0 {
                    return None;
                }
                let ask = self.choose_ask();
                let mut opts: Opts = rules::build_opts(&cx, self.who, &ask, &Held::of(&cx, self.who).extras()).as_slice().iter().map(|o| Opt::Production(*o)).collect();
                if self.agent_offerable(s) && (self.remaining > 0 || self.small_exemptions > 0) {
                    let reaches = opts.len() > 0 || {
                        let mut probe = ask;
                        probe.discount_remaining += 2;
                        !rules::build_opts(&cx, self.who, &probe, &Held::of(&cx, self.who).extras()).as_slice().is_empty()
                    };
                    if reaches {
                        opts.push(Opt::Production(POpt::Agent));
                    }
                }
                if opts.len() == 0 {
                    return None;
                }
                opts.push(Opt::Production(POpt::Stop));
                let mut owed = SmallList::new();
                owed.push(Owed { kind: OwedKind::ProductionCapacity, amount: i64::from(self.limit), paid: i64::from((self.limit - self.remaining).max(0)) });
                Some(Choice { actor: self.who, ask: Ask::Production(PAsk::Choose(ask)), ctx: self.ctx(s, Rule::R68, PSub::Choose, owed), opts })
            }
            Stage::Paying { owed, cost, paid, .. } => {
                let (ask, opts) = rules::pay_opts(&cx, self.who, Spend::Resources, 0, i64::from(owed), &Held::of(&cx, self.who).extras());
                if opts.as_slice().is_empty() {
                    return None;
                }
                let mut out = SmallList::new();
                out.push(Owed { kind: OwedKind::Resources, amount: i64::from(cost), paid: i64::from(paid) });
                Some(Choice {
                    actor: self.who,
                    ask: Ask::Production(PAsk::Pay(ask)),
                    ctx: self.ctx(s, Rule::R34_3_75_2_75_3, PSub::Pay(Currency::Resources), out),
                    opts: opts.as_slice().iter().map(|o| Opt::Production(*o)).collect(),
                })
            }
            Stage::Placing { unit, made } => {
                let ask = self.place_ask(unit, made);
                let opts = rules::place_opts(&cx, &ask);
                if opts.as_slice().is_empty() {
                    return None;
                }
                let home = rules::place_home_standing(&cx, &ask);
                let mut out = SmallList::new();
                out.push(Owed { kind: OwedKind::FleetSupply, amount: home.fleet_limit, paid: home.fleet_charged });
                out.push(Owed { kind: OwedKind::TransportCapacity, amount: home.transport, paid: home.consumed });
                Some(Choice {
                    actor: self.who,
                    ask: Ask::Production(PAsk::Place(ask)),
                    ctx: self.ctx(s, Rule::R68, PSub::Place, out),
                    opts: opts.as_slice().iter().map(|o| Opt::Production(*o)).collect(),
                })
            }
        }
    }

    /// Spend `cost` of the credit; what is still owed.
    fn spend_credit(&mut self, cost: i32) -> i32 {
        let used = self.credit.min(cost);
        self.credit -= used;
        cost - used
    }

    /// Apply a payment option; its worth, `None` for one that cannot be paid.
    fn apply_payment(&self, s: &mut CoreState, k: &Kernel, picked: POpt) -> Option<i32> {
        let cx = self.cx(s, k);
        let (_, offered) = rules::pay_opts(&cx, self.who, Spend::Resources, 0, 0, &Held::of(&cx, self.who).extras());
        // The offered option equal to the pick carries the worth (the pick, parsed from an id, does not).
        let chosen = *offered.as_slice().iter().find(|o| **o == picked)?;
        match chosen {
            POpt::Exhaust { planet, shown, .. } => {
                fx::exhaust_planet(s, planet);
                Some(shown.worth)
            }
            POpt::TradeGood { shown } => {
                let p = s.players_mut().by_seat_mut(self.who)?;
                if p.econ.trade_goods <= 0 {
                    return None;
                }
                p.econ.trade_goods -= 1;
                Some(shown.worth)
            }
            POpt::Commodity { shown } => {
                // Keleres' agent: a commodity spent as if it were a trade good.
                let p = s.players_mut().by_seat_mut(self.who)?;
                p.econ.commodities -= 1;
                Some(shown.worth)
            }
            _ => None,
        }
    }

    /// `place`: put the units on the board and update the limits.
    fn place(&mut self, s: &mut CoreState, k: &Kernel, unit: Unit, spot: Spot, made: usize) {
        let cx = self.cx(s, k);
        let made = rules::placed_of(&cx, self.who, unit, made);
        let (system, planet) = match spot {
            Spot::Space => (self.system, None),
            Spot::Planet(p) => (self.system, Some(p)),
            Spot::Remote(sys, at) => (sys, at),
            Spot::Card => (self.system, None),
        };
        for _ in 0..made {
            if spot == Spot::Card {
                // The Crimson hero card is not a place on the board.
                k.hooks.direct.crimson_cards_put_on_card(s, self.who, unit);
                self.produced.push((unit, spot));
                continue;
            }
            fx::place_unit(s, self.who, system, planet, unit);
            self.produced.push((unit, spot));
            // Absolute Synergy resolves before the next action.
            fx::stage_naaz_mech_placed(s, self.who, system, unit);
        }
        // The Last Bastion's Icon is exhausted by placing the ships in one of its systems.
        if made > 0 && spot != Spot::Card && system != self.system {
            k.hooks.direct.bastion_ship_placed(s, k, self.who, self.system, system, unit);
        }
        let d = unit.data();
        let count = made as i64;
        let held_sol = s.players().by_seat(self.who).is_some_and(|p| p.cards.breakthrough == Some(crate::generated::Breakthrough::BellumGloriosum));
        let granted = if held_sol && d.ship { fleet::capacity(unit) } else { 0 } * i64::from(made > 0);
        self.free_capacity = small(i64::from(self.free_capacity) + granted);
        let spends = d.base_type == UnitBaseType::Fighter || fleet::is_ground_force(unit);
        let free = if spends { count.min(i64::from(self.free_capacity)) } else { 0 };
        let exempt = if matches!(d.base_type, UnitBaseType::Fighter | UnitBaseType::Infantry) { (count - free).min(i64::from(self.small_exemptions)) } else { 0 };
        self.small_exemptions = small(i64::from(self.small_exemptions) - exempt);
        self.free_capacity = small(i64::from(self.free_capacity) - free);
        self.remaining = small(i64::from(self.remaining) - (count - free - exempt));
    }
}

impl Window for ProductionWindow {
    type Stage = Stage;

    fn stage(&self) -> Stage {
        self.stage
    }

    /// The string `settle`.
    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        loop {
            match self.stage {
                Stage::Paying { unit, owed, made, cost, paid } => {
                    let cx = self.cx(sx.state, sx.kernel);
                    let (_, opts) = rules::pay_opts(&cx, self.who, Spend::Resources, 0, i64::from(owed), &Held::of(&cx, self.who).extras());
                    match opts.as_slice() {
                        [] => {
                            self.stage = Stage::Done;
                            return Next::Done;
                        }
                        [only] => {
                            let only = *only;
                            let Some(worth) = self.apply_payment(sx.state, sx.kernel, only) else {
                                self.stage = Stage::Done;
                                return Next::Done;
                            };
                            if owed > worth {
                                self.stage = Stage::Paying { unit, owed: owed - worth, made, cost, paid: paid + worth };
                                continue;
                            }
                            self.credit += worth - owed;
                            self.stage = Stage::Placing { unit, made };
                        }
                        _ => return self.ask_next(sx),
                    }
                }
                Stage::Placing { unit, made } => {
                    if !self.placement_timing_done {
                        if self.emit_timing && fleet::is_ground_force(unit) && !unit.data().structure {
                            let payload = Payload::new()
                                .with(Key::Player, Val::Seat(self.who))
                                .with(Key::System, Val::System(self.system))
                                .with(Key::Count, Val::Int(i64::from(made)));
                            if sx.emit(fx::event(EventKind::GroundForcesBeingProduced, payload)).is_err() {
                                self.stage = Stage::Done;
                                return Next::Done;
                            }
                        }
                        self.placement_timing_done = true;
                    }
                    let spots = rules::spots_for(&self.cx(sx.state, sx.kernel), self.who, self.system, unit, self.icon(), self.ability);
                    match spots.as_slice() {
                        [only] => {
                            let only = *only;
                            self.place(sx.state, sx.kernel, unit, only, usize::from(made));
                            self.agent_destinations_spent(sx);
                            self.placement_timing_done = false;
                            self.stage = Stage::Choosing;
                        }
                        [] => {
                            self.agent_destinations_spent(sx);
                            self.stage = Stage::Done;
                            return Next::Done;
                        }
                        _ => return self.ask_next(sx),
                    }
                }
                Stage::Choosing => {
                    if (self.remaining <= 0 && self.small_exemptions <= 0) || self.question(sx.state, sx.kernel).is_none() {
                        self.stage = Stage::Done;
                        return Next::Done;
                    }
                    return self.ask_next(sx);
                }
                Stage::Done => return Next::Done,
            }
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        self.question(s, k)
    }

    /// The string `resolve` (without its end-of-use effects).
    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        let Pick::Opt(Opt::Production(opt)) = pick else { return Err(Illegal) };
        match self.stage {
            Stage::Done => Err(Illegal),
            Stage::Choosing => match opt {
                POpt::Stop => {
                    self.stage = Stage::Done;
                    Ok(())
                }
                POpt::Build { unit, made } => {
                    let ask = self.choose_ask();
                    let cx = self.cx(sx.state, sx.kernel);
                    let (printed, _) = rules::price_of(&cx, unit);
                    let d = unit.data();
                    let (cost, used) = if d.base_type == UnitBaseType::Flagship && self.flagship_free {
                        (0, 0)
                    } else if self.free_this_use {
                        (0, printed)
                    } else {
                        let used = i64::from(ask.discount_remaining).min(printed);
                        (printed - used, used)
                    };
                    if !self.free_this_use {
                        self.discount_remaining = small(i64::from(self.discount_remaining) - used);
                    }
                    self.purchased = true;
                    let owed = self.spend_credit(small(cost));
                    self.stage = if owed > 0 {
                        Stage::Paying { unit, owed, made, cost: small(cost), paid: small(cost) - owed }
                    } else {
                        Stage::Placing { unit, made }
                    };
                    Ok(())
                }
                POpt::Agent => {
                    if Self::agent_ready(sx.state, self.who) {
                        if let Some(p) = sx.state.players_mut().by_seat_mut(self.who) {
                            p.leaders.status.insert(Leader::BerekarBerekon, LeaderStatus::Exhausted);
                        }
                        // `leaders::exhaust`: a Nomad that hears every agent exhausted (Temporal Command Suite) is told, by a staged event.
                        if sx.kernel.hooks.direct.nomad_watches_agent_exhaustion(sx.state) {
                            let payload = Payload::new().with(Key::Player, Val::Seat(self.who)).with(Key::Leader, Val::Leader(Leader::BerekarBerekon));
                            fx::stage(sx.state, fx::event(EventKind::AgentExhausted, payload));
                        }
                        sx.state.transient_mut().production.discount_remaining += 2;
                        self.discount_remaining += 2;
                        self.agent_spent = true;
                    }
                    Ok(())
                }
                POpt::Exchange { unit } => {
                    let (who, system, kernel) = (self.who, self.system, sx.kernel);
                    let done = {
                        let mut cx = sx.cx();
                        kernel.hooks.economy.production_unit_exchange_performed(&mut cx, who, system, unit)
                    };
                    if !done {
                        self.stage = Stage::Done;
                        return Ok(());
                    }
                    self.purchased = true;
                    self.stage = Stage::Placing { unit, made: 1 };
                    Ok(())
                }
                _ => Err(Illegal),
            },
            Stage::Paying { unit, owed, made, cost, paid } => {
                let Some(worth) = self.apply_payment(sx.state, sx.kernel, opt) else {
                    self.stage = Stage::Done;
                    return Ok(());
                };
                let owed = owed - worth;
                let paid = paid + worth;
                self.stage = if owed > 0 {
                    Stage::Paying { unit, owed, made, cost, paid }
                } else {
                    self.credit += -owed;
                    Stage::Placing { unit, made }
                };
                Ok(())
            }
            Stage::Placing { unit, made } => {
                let POpt::Place { spot } = opt else { return Err(Illegal) };
                self.place(sx.state, sx.kernel, unit, spot, usize::from(made));
                self.agent_destinations_spent(sx);
                self.placement_timing_done = false;
                self.stage = Stage::Choosing;
                Ok(())
            }
        }
    }
}

impl ProductionWindow {
    /// `Obs::AgentDestinationsSpent` of the string window: a placement spends the grant of Argent's agent
    /// (the closing observers clear it).
    fn agent_destinations_spent(&self, sx: &mut Sx<'_>) {
        sx.observe(&crate::kernel::seam::observe::Obs::ProductionWindowClosed { who: self.who, system: self.system }, crate::kernel::seam::observe::Phase::After);
    }

    fn ask_next(&self, sx: &Sx<'_>) -> Next {
        match self.question(sx.state, sx.kernel) {
            Some(c) => Next::Continue(c.ask),
            None => Next::Done,
        }
    }
}

#[allow(dead_code)]
const fn _unused(_: Planet) {}
