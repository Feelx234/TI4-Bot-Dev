//! Production: who can produce, how much, where units may go and what a unit costs (package B3;
//! LRR 67, 68, 79). The reads of `production.rs`: `price_of_under`, `producers`, `mobile_docks`,
//! `capacity`, `module_production`, `barred_capacity`, `structures_on`, `structure_allowed`,
//! `placements`, `war_machine_bonus`.
//!
//! Everything the core cannot see yet is asked of an [`Env`]: faction hooks and the few rules whose
//! owners have not landed (Ral Nel's silenced docks, Quietus, entropic scars, space stations). The
//! string engine implements the trait with its own modules, so the sums stay exact; the hook table
//! replaces it when the factions are ported.

use crate::containers::SmallList;
use crate::choice::production::{ChooseAsk, Currency, Icon, Opt as PayOpt, PayAsk, PlaceAsk, Shown};
use crate::query::fleet::{self, Arrival, Standing};
use crate::generated::{Breakthrough, Leader, Planet, PlanetTrait, Relic, System, Unit, UnitBaseType};
use crate::state::LeaderStatus;
use crate::ids::Seat;
use crate::query::economy::{self, Spend};
use crate::query::{board, laws};
use crate::kernel::seam::{Clamp, DestSite, ExchangeSite, ProdSite, ProduceSite, Q, Seams, SilenceCtx, SilenceSite, SpendSite};
use crate::kernel::Kernel;
use crate::state::{CoreState, Piece, RoundKind};
use crate::tables::Scope;

/// What the rules read besides the state: the position and the kernel (hook table, whose seams carry
/// every faction, law and card term), the source scope and the blanked-laws flag.
#[derive(Clone, Copy)]
pub struct Cx<'a> {
    pub q: Q<'a>,
    pub scope: Scope,
    pub blanked: bool,
}

impl<'a> Cx<'a> {
    #[must_use]
    pub const fn new(core: &'a CoreState, kernel: &'a Kernel, scope: Scope, blanked: bool) -> Self {
        Self { q: Q::new(core, kernel), scope, blanked }
    }

    #[must_use]
    pub const fn core(&self) -> &'a CoreState {
        self.q.s
    }

    fn seams(&self) -> &'a Seams {
        &self.q.k.hooks.seam
    }
}

/// A producing unit and the planet it sits on (`None`: the space area).
pub type Producers = SmallList<(Piece, Option<Planet>), 12>;

pub use crate::query::laws::structure_limit;

/// `production::price_of_under`: `(cost, units made per purchase)`. `cost_halves` is the printed
/// cost doubled (`None`: no cost printed, which the string reads as 0); `single` is Regulated
/// Conscription (`laws::single_unit_production`).
#[must_use]
pub fn price(cost_halves: Option<u8>, single: bool) -> (i64, usize) {
    let halves = i64::from(cost_halves.unwrap_or(0));
    if halves == 1 {
        return (1, if single { 1 } else { 2 });
    }
    // ceil(halves / 2), capped like the string loop.
    (((halves + 1) / 2).clamp(0, 64), 1)
}

/// `price_of_under` for a unit type, with the law read from the state.
#[must_use]
pub fn price_of(cx: &Cx<'_>, unit: Unit) -> (i64, usize) {
    price(unit.data().cost_halves, laws::single_unit_production(cx.core(), cx.blanked))
}

/// `production::producers`: the seat's units with PRODUCTION in `system`: the space area first
/// (list order), then each planet in planet order.
#[must_use]
pub fn producers(cx: &Cx<'_>, who: Seat, system: System) -> Producers {
    let mut found = Producers::new();
    let produces = |u: Unit| u.data().production.is_some();
    for p in board::space_units(cx.core(), system) {
        let k = p.kind();
        if p.owner() == who && produces(k) && !silenced(cx, who, system, Some(k), SilenceCtx::Space) {
            found.push((*p, None));
        }
    }
    for planet in sorted_planets(cx.core(), system).as_slice().iter().copied() {
        for p in board::planet_units(cx.core(), planet) {
            if p.owner() == who && produces(p.kind()) {
                found.push((*p, Some(planet)));
            }
        }
    }
    found
}

/// The planets whose entries live in `system`, in id order (the order of the string `BTreeMap`s).
fn sorted_planets(core: &CoreState, system: System) -> SmallList<Planet, 8> {
    let mut v = SmallList::new();
    for p in board::planets_of(core, system) {
        v.push(p);
    }
    v.as_mut_slice().sort_unstable();
    v
}

/// `production::mobile_docks`: space-only structures with PRODUCTION in the space area.
#[must_use]
pub fn mobile_docks(cx: &Cx<'_>, who: Seat, system: System) -> SmallList<Piece, 4> {
    let mut out = SmallList::new();
    for p in board::space_units(cx.core(), system) {
        let d = p.kind().data();
        if p.owner() == who && d.space_only && d.structure && d.production.is_some() {
            out.push(*p);
        }
    }
    out
}

/// `production::war_machine_bonus`.
#[must_use]
pub fn war_machine_bonus(core: &CoreState, who: Seat) -> i64 {
    let seq = core.meta().activation_seq;
    core.players().by_seat(who).map_or(0, |p| 5 * p.round_marks.of(RoundKind::WarMachine).filter(|n| *n == seq).count() as i64)
}

/// What the seat's units read a planet as worth when spent: the printed value and the attachments, then every
/// term of the `planet_spend` seam (laws, the Helios dock, the Winnu swap, ...).
#[must_use]
pub fn planet_value(cx: &Cx<'_>, who: Seat, planet: Planet, spend: Spend) -> i64 {
    let base = economy::printed_value(cx.scope, planet, spend) + economy::attachment_bonus(cx.core(), planet, spend);
    cx.seams().planet_spend.eval(cx.q, &SpendSite { who, planet, spend: hook_spend(spend) }, base, Clamp::None).value
}

const fn hook_spend(s: Spend) -> crate::kernel::hooks::Spend {
    match s {
        Spend::Resources => crate::kernel::hooks::Spend::Resources,
        Spend::Influence => crate::kernel::hooks::Spend::Influence,
    }
}

/// One `silenced` decision (a piece whose abilities are off, or with `unit` `None` the whole system's).
fn silenced(cx: &Cx<'_>, owner: Seat, system: System, unit: Option<Unit>, ctx: SilenceCtx) -> bool {
    cx.seams().silenced.decide(cx.q, &SilenceSite { owner, system, unit, ctx }, false)
}

/// Production values granted by a module to the space area (`planet` `None`) or one planet.
fn extra(cx: &Cx<'_>, who: Seat, system: System, planet: Option<Planet>) -> i64 {
    cx.seams().extra_production.eval(cx.q, &ProdSite { who, system, planet }, 0, Clamp::None).value
}

fn cannot(cx: &Cx<'_>, who: Seat, unit: UnitBaseType, producer: UnitBaseType) -> bool {
    cx.seams().cannot_produce.decide(cx.q, &ProduceSite { who, unit, producer }, false)
}

fn unit_production(unit: Unit, resources: i64) -> i64 {
    unit.data().production.map_or(0, |p| i64::from(p.value) + if p.plus_planet_resources { resources } else { 0 })
}

fn dock_resources(cx: &Cx<'_>, who: Seat, planet: Option<Planet>) -> i64 {
    planet.map_or(0, |pl| planet_value(cx, who, pl, Spend::Resources))
}

/// `production::capacity` (68.1a).
#[must_use]
pub fn capacity(cx: &Cx<'_>, who: Seat, system: System) -> i64 {
    producers(cx, who, system)
        .as_slice()
        .iter()
        .map(|(u, pl)| unit_production(u.kind(), dock_resources(cx, who, *pl)))
        .sum::<i64>()
        + war_machine_bonus(cx.core(), who)
        + module_production(cx, who, system)
}

/// `production::module_production`.
#[must_use]
pub fn module_production(cx: &Cx<'_>, who: Seat, system: System) -> i64 {
    extra(cx, who, system, None).max(0)
        + module_planet_producers(cx, who, system).as_slice().iter().map(|(_, v)| *v).sum::<i64>()
}

/// `production::module_planet_producers`: planets holding units or controlled, with their PRODUCTION.
#[must_use]
pub fn module_planet_producers(cx: &Cx<'_>, who: Seat, system: System) -> SmallList<(Planet, i64), 4> {
    let mut out = SmallList::new();
    for planet in sorted_planets(cx.core(), system).as_slice() {
        let cell = cx.core().board().planet(*planet);
        if cell.units.is_empty() && cell.control.is_none() {
            continue;
        }
        // A planet site evaluates the system term too; the planet's own share is the difference.
        let v = extra(cx, who, system, Some(*planet)) - extra(cx, who, system, None);
        if v > 0 {
            out.push((*planet, v));
        }
    }
    out
}

/// `production::barred_capacity`.
#[must_use]
pub fn barred_capacity(cx: &Cx<'_>, who: Seat, system: System, unit: UnitBaseType) -> i64 {
    producers(cx, who, system)
        .as_slice()
        .iter()
        .filter(|(u, _)| cannot(cx, who, unit, u.kind().data().base_type))
        .map(|(u, pl)| unit_production(u.kind(), dock_resources(cx, who, *pl)))
        .sum()
}

/// `production::structures_on`: the seat's units of this base type on the planet.
#[must_use]
pub fn structures_on(cx: &Cx<'_>, who: Seat, planet: Planet, base: UnitBaseType) -> usize {
    board::planet_units(cx.core(), planet).iter().filter(|p| p.owner() == who && p.kind().data().base_type == base).count()
}

/// `production::structure_allowed` (79.2).
#[must_use]
pub fn structure_allowed(cx: &Cx<'_>, who: Seat, planet: Planet, base: UnitBaseType) -> bool {
    laws::structure_allowed(cx.core(), cx.blanked, who, planet, base)
}

pub mod home;
pub mod offers;
pub mod one;
pub mod pay;
pub mod probe;
pub mod strategy_cards;
pub mod supply;
pub mod triggered;

/// What the table adds to a context when it asks (`DecisionContext::about_invasion`): the invasion of the system
/// the question is about (production contexts never name a battle).
pub fn enrich_ctx(s: &CoreState, ctx: &mut crate::choice::Ctx) {
    use crate::choice::Target;
    if !ctx.space_battle {
        if let Some(active) = s.transient().invasion.active.as_ref() {
            let in_system = match ctx.target {
                Some(Target::System(x) | Target::Planet { system: x, .. } | Target::Unit { system: x, .. }) => x == active.system,
                _ => true,
            };
            if in_system {
                ctx.invasion_seq = Some(active.seq);
            }
        }
    }
}
pub mod window;

pub use crate::choice::production::Spot;

/// `production::placements` (68.2 to 68.4).
#[must_use]
pub fn placements(cx: &Cx<'_>, who: Seat, system: System, unit: Unit) -> SmallList<Spot, 8> {
    let mut spots = SmallList::new();
    if silenced(cx, who, system, None, SilenceCtx::Production) {
        return spots;
    }
    let d = unit.data();
    if d.ship {
        let blockaded = board::space_units(cx.core(), system).iter().any(|p| p.owner() != who && p.kind().data().ship);
        if !blockaded {
            spots.push(Spot::Space);
        }
        return spots;
    }
    if system.data().anomalies.scar {
        return spots;
    }
    let made: Producers = {
        let mut m = Producers::new();
        for (u, pl) in producers(cx, who, system).as_slice() {
            if !cannot(cx, who, d.base_type, u.kind().data().base_type) {
                m.push((*u, *pl));
            }
        }
        m
    };
    let mut candidates: SmallList<Planet, 12> = SmallList::new();
    for (_, pl) in made.as_slice() {
        if let Some(p) = pl {
            candidates.push(*p);
        }
    }
    for (p, _) in module_planet_producers(cx, who, system).as_slice() {
        candidates.push(*p);
    }
    for planet in candidates.as_slice() {
        let planet = *planet;
        if laws::production_forbidden_on(cx.core(), cx.blanked, planet)
            || laws::planet_is_demilitarized(cx.core(), cx.blanked, planet)
            || planet.data().traits.contains(&PlanetTrait::Spacestation)
            || !structure_allowed(cx, who, planet, d.base_type)
        {
            continue;
        }
        let s = Spot::Planet(planet);
        if !spots.as_slice().contains(&s) {
            spots.push(s);
        }
    }
    if made.as_slice().iter().any(|(_, pl)| pl.is_none()) || extra(cx, who, system, None) > 0 {
        if !spots.as_slice().contains(&Spot::Space) {
            spots.push(Spot::Space);
        }
    }
    spots
}

/// What the payment reads need that lives in a faction package above this layer: Keleres' spendable
/// commodities and the Deepwrought's readied ocean cards.
#[derive(Debug, Clone, Copy, Default)]
pub struct Extras<'a> {
    pub commodities: i64,
    pub oceans: &'a [Planet],
}

/// The payment extras of a seat as the hook table answers them: Keleres' commodities (open only inside a payment window
/// the agent was used for) and the Deepwrought's readied ocean cards.
#[derive(Debug, Clone)]
pub struct Held {
    commodities: i64,
    oceans: SmallList<Planet, 8>,
}

impl Held {
    #[must_use]
    pub fn of(cx: &Cx<'_>, who: Seat) -> Self {
        let (s, k) = (cx.core(), cx.q.k);
        let mut oceans = SmallList::new();
        for p in k.hooks.direct.deepwrought_spendable_oceans(s, who).as_slice() {
            oceans.push(*p);
        }
        Self { commodities: k.hooks.direct.keleres_spendable_commodities(s, who), oceans }
    }

    #[must_use]
    pub fn extras(&self) -> Extras<'_> {
        Extras { commodities: self.commodities, oceans: self.oceans.as_slice() }
    }
}

/// `production::trade_good_worth`: one, folded through the `trade_good_worth` hook (Mentak).
#[must_use]
pub fn trade_good_worth(cx: &Cx<'_>, who: Seat) -> i64 {
    cx.q.k.hooks.economy.trade_good_worth(cx.core(), who, 1)
}

/// `production::spendable_planets` with the off-map cards appended.
#[must_use]
pub fn spendable_planets(cx: &Cx<'_>, who: Seat, ex: &Extras<'_>) -> SmallList<Planet, 24> {
    let mut out = SmallList::new();
    for (_, p) in economy::spendable_planets(cx.core(), who).as_slice() {
        out.push(*p);
    }
    for p in ex.oceans {
        out.push(*p);
    }
    out
}

/// A face: which currency of the planet pays, and for how much.
pub type Face = (Spend, i64);

/// `production::payment_faces`: the faces by which a planet can pay one bill of `kind`.
#[must_use]
pub fn payment_faces(cx: &Cx<'_>, who: Seat, planet: Planet, kind: Spend) -> SmallList<Face, 2> {
    let face = |k: Spend| planet_value(cx, who, planet, k);
    let combines = cx
        .core()
        .players()
        .by_seat(who)
        .is_some_and(|p| p.leaders.status.get(&Leader::XxekirGrom) == Some(&LeaderStatus::Unlocked));
    let ordinary = if combines { face(Spend::Resources) + face(Spend::Influence) } else { face(kind) };
    let mut faces = SmallList::new();
    if ordinary > 0 {
        faces.push((kind, ordinary));
    }
    let Some(p) = cx.core().players().by_seat(who) else {
        return faces;
    };
    let freelancers = kind == Spend::Resources && cx.core().meta().influence_pays_for_units.contains(who);
    let archons_gift = p.cards.breakthrough == Some(Breakthrough::ArchonsGift);
    if !freelancers && !archons_gift {
        return faces;
    }
    let other = if kind == Spend::Resources { Spend::Influence } else { Spend::Resources };
    let alternate = face(other);
    if alternate > 0 && alternate != ordinary {
        faces.push((other, alternate));
    }
    faces
}

/// `production::max_face_value`.
#[must_use]
pub fn max_face_value(cx: &Cx<'_>, who: Seat, planet: Planet, kind: Spend) -> i64 {
    payment_faces(cx, who, planet, kind).as_slice().iter().map(|(_, w)| *w).max().unwrap_or(0)
}

/// `relics::triad_value`: the Triad is spent as a planet card.
fn triad_value(cx: &Cx<'_>, who: Seat) -> i64 {
    cx.core().players().by_seat(who).filter(|p| p.cards.relics.as_slice().contains(&Relic::TheTriad)).map_or(0, |p| {
        let kinds = (0..4).filter(|i| p.cards.fragments.present & (1 << i) != 0 && p.cards.fragments.counts[*i] > 0).count();
        3 + kinds as i64
    })
}

/// `production::available`: spendable resources or influence, counting trade goods (LRR 75.3, 47.3).
#[must_use]
pub fn available(cx: &Cx<'_>, who: Seat, kind: Spend, ex: &Extras<'_>) -> i64 {
    let from_planets: i64 = spendable_planets(cx, who, ex).as_slice().iter().map(|p| max_face_value(cx, who, *p, kind)).sum();
    let worth = trade_good_worth(cx, who);
    let goods = cx.core().players().by_seat(who).map_or(0, |p| i64::from(p.econ.trade_goods)) * worth + ex.commodities * worth;
    let war_machine = if kind == Spend::Resources { war_machine_bonus(cx.core(), who) } else { 0 };
    from_planets + triad_value(cx, who) + goods + war_machine
}

/// What `payment::affordable` (the string's research, Technology-follower and Inheritance Systems gate) counts: the value of each spendable planet
/// by the face of the bill (`payment::plans` reads `planet_value_now`, one face; not the alternate face Freelancers / Archon's Gift offer, not Xxekir
/// Grom's combined face), and the trade goods at their worth. NOT [`available`], which adds what a production step may spend: the Triad, War
/// Machine's five (the mark outlives the tactical action that played it until the next activation: a seat with nothing to spend but a stale
/// War Machine mark is not able to pay a research) and the larger face of a planet. ONE gate for the research window, the follower's eligibility and
/// Inheritance Systems (integration 15 lane Q; the single face is lane R's finding, integration 18).
#[must_use]
pub fn payable(cx: &Cx<'_>, who: Seat, kind: Spend, ex: &Extras<'_>) -> i64 {
    let from_planets: i64 = spendable_planets(cx, who, ex).as_slice().iter().map(|p| planet_value(cx, who, *p, kind)).sum();
    let worth = trade_good_worth(cx, who);
    let goods = cx.core().players().by_seat(who).map_or(0, |p| i64::from(p.econ.trade_goods)) * worth + ex.commodities * worth; // mutants:skip (a seat that is not seated has no goods: the default is unreachable)
    from_planets + goods
}

/// `production::payment_options` as typed options: the planets (each face that would not strand the rest of the
/// bill), then trade goods, then commodities. `paid` and `cost` describe the whole bill.
#[must_use]
pub fn pay_opts(cx: &Cx<'_>, who: Seat, kind: Spend, paid: i64, cost: i64, ex: &Extras<'_>) -> (PayAsk, SmallList<PayOpt, 24>) {
    let spendable = spendable_planets(cx, who, ex);
    let pool = available(cx, who, kind, ex);
    let goods = cx.core().players().by_seat(who).map_or(0, |p| i64::from(p.econ.trade_goods));
    let worth = trade_good_worth(cx, who);
    let goods_capacity = goods * worth + ex.commodities * worth;
    let mut opts = SmallList::new();
    for planet in spendable.as_slice() {
        let remaining_after: i64 = spendable.as_slice().iter().filter(|o| *o != planet).map(|o| max_face_value(cx, who, *o, kind)).sum::<i64>() + goods_capacity;
        for (source, face) in payment_faces(cx, who, *planet, kind).as_slice() {
            if paid + face + remaining_after < cost {
                continue;
            }
            let removed = max_face_value(cx, who, *planet, kind);
            opts.push(PayOpt::Exhaust { planet: *planet, source: currency(*source), shown: Shown { worth: small(*face), pool_after: small(pool - removed) } });
        }
    }
    if goods > 0 {
        opts.push(PayOpt::TradeGood { shown: Shown { worth: small(worth), pool_after: small(pool - worth) } });
    }
    if ex.commodities > 0 {
        opts.push(PayOpt::Commodity { shown: Shown { worth: small(worth), pool_after: small(pool - worth) } });
    }
    (PayAsk { kind: currency(kind), owed: cost - paid, pool, goods, commodities: ex.commodities }, opts)
}

fn small(n: i64) -> i32 {
    i32::try_from(n).unwrap_or(if n < 0 { i32::MIN } else { i32::MAX })
}

const fn currency(s: Spend) -> Currency {
    match s {
        Spend::Resources => Currency::Resources,
        Spend::Influence => Currency::Influence,
    }
}

/// Where the unit may go: the system's own spots, then the places modules add (`production_destinations`),
/// the latter only when the unit has a spot here at all.
#[must_use]
pub fn spots_of(cx: &Cx<'_>, who: Seat, system: System, unit: Unit, icon: Icon) -> SmallList<Spot, 12> {
    spots_for(cx, who, system, unit, icon, false)
}

/// The spots of a unit an ABILITY produces (`ProductionWindow::local_spots`): ships as for PRODUCTION, ground forces and structures on
/// any planet of the system the seat controls that is not demilitarized, no space station, and takes the structure (79.2); none where
/// the abilities are off (entropic scars).
#[must_use]
pub fn ability_placements(cx: &Cx<'_>, who: Seat, system: System, unit: Unit) -> SmallList<Spot, 8> {
    let d = unit.data();
    if d.ship {
        return placements(cx, who, system, unit);
    }
    let mut spots = SmallList::new();
    if system.data().anomalies.scar {
        return spots;
    }
    for (at, planet) in board::controlled_planets(cx.core(), who).as_slice() {
        let planet = *planet;
        if *at != system
            || laws::planet_is_demilitarized(cx.core(), cx.blanked, planet)
            || planet.data().traits.contains(&PlanetTrait::Spacestation)
            || !structure_allowed(cx, who, planet, d.base_type)
        {
            continue;
        }
        let s = Spot::Planet(planet);
        if !spots.as_slice().contains(&s) {
            spots.push(s);
        }
    }
    spots
}

/// [`spots_of`] for a window that produces by an ability (`ability`) or by PRODUCTION.
#[must_use]
pub fn spots_for(cx: &Cx<'_>, who: Seat, system: System, unit: Unit, icon: Icon, ability: bool) -> SmallList<Spot, 12> {
    let mut spots: SmallList<Spot, 12> = SmallList::new();
    let local = if ability { ability_placements(cx, who, system, unit) } else { placements(cx, who, system, unit) };
    for s in local.as_slice() {
        spots.push(*s);
    }
    if !spots.as_slice().is_empty() {
        let dests = cx.seams().production_destinations.collect(cx.q, &DestSite { who, system, base: unit.data().base_type });
        // The producing system's own spots are `placements` (the string `production_destinations` drops it).
        for (sys, planet) in dests.as_slice().iter().filter(|(sys, _)| *sys != system) {
            spots.push(Spot::Remote(*sys, *planet));
        }
        if unit.data().ship {
            // The Last Bastion's Icon shapes the spots of the whole use, then the Crimson hero offers its card.
            let k = cx.q.k;
            k.hooks.direct.bastion_adjust_ship_spots(cx.core(), k, who, system, icon, &mut spots);
            k.hooks.direct.crimson_cards_offer_card_spot(cx.core(), who, &mut spots);
        }
    }
    spots
}

/// `will_place`: the units that will actually arrive of `made` bought (31.4 and the placement bans).
#[must_use]
pub fn placed_of(cx: &Cx<'_>, who: Seat, unit: Unit, made: usize) -> usize {
    if cx.q.k.hooks.economy.effect_placement_forbidden(cx.core(), cx.q.k, who, unit) {
        0
    } else {
        made.min(usize::try_from(fleet::remaining_all(cx.core(), cx.scope, who, unit)).unwrap_or(0))
    }
}

/// What a placement question says about one spot.
#[derive(Debug, Clone, Copy)]
pub struct PlaceInfo {
    pub placed: usize,
    /// The standing the spot is measured against (the spot's own system) and the one after the unit arrives.
    pub before: Standing,
    pub after: Standing,
}

fn place_ask_standing(cx: &Cx<'_>, ask: &PlaceAsk, system: Option<System>, arriving: Option<Arrival>) -> Standing {
    let half = |u: Unit| cx.q.k.hooks.combat.fighter_fleet_weight_halves(cx.core(), cx.q.k, ask.who, u);
    fleet::standing_in(cx.core(), cx.scope, ask.who, system, i64::from(ask.fleet_limit), arriving, &half)
}

/// The facts of the placement option for `spot`.
#[must_use]
pub fn place_info(cx: &Cx<'_>, ask: &PlaceAsk, spot: Spot) -> PlaceInfo {
    let placed = placed_of(cx, ask.who, ask.unit, usize::from(ask.made));
    let target = match spot {
        Spot::Remote(system, _) => Some(system),
        Spot::Card => None,
        _ => Some(ask.system),
    };
    let after = place_ask_standing(cx, ask, target, Some(Arrival { kind: ask.unit, count: placed as i64, in_space: spot_in_space(spot) }));
    PlaceInfo { placed, before: place_ask_standing(cx, ask, target, None), after }
}

/// The standing of the producing system, for the question's context.
#[must_use]
pub fn place_home_standing(cx: &Cx<'_>, ask: &PlaceAsk) -> Standing {
    place_ask_standing(cx, ask, Some(ask.system), None)
}

/// The placement question's options: one per spot, only when there are two or more.
#[must_use]
pub fn place_opts(cx: &Cx<'_>, ask: &PlaceAsk) -> SmallList<PayOpt, 18> {
    let spots = spots_for(cx, ask.who, ask.system, ask.unit, ask.icon, ask.ability);
    let mut out = SmallList::new();
    if spots.as_slice().len() >= 2 {
        for s in spots.as_slice() {
            out.push(PayOpt::Place { spot: *s });
        }
    }
    out
}

/// Everything the Choosing question says about one buildable unit: the numbers the option shows, so the
/// listing (`build_opts`) and the renderer read the same values.
#[derive(Debug, Clone)]
pub struct BuildInfo {
    pub unit: Unit,
    pub printed: i64,
    pub pair: usize,
    pub cost: i64,
    pub discount_used: i64,
    pub exchange: bool,
    pub affordable: bool,
    pub spots: SmallList<Spot, 12>,
    pub made: usize,
    pub placed: usize,
    pub remaining_after: i64,
    pub free_after: i64,
    /// Standing of the producing system before any option is taken.
    pub before: Standing,
    pub spendable_resources: i64,
    /// Standing after the paid build / after the exchange, when the unit has a single destination.
    pub after_paid: Option<Standing>,
    pub after_exchange: Option<Standing>,
    /// `post_build_constraints(unit, 1)` for the exchange.
    pub exchange_after: (i64, i64),
}

/// The `Where` of a spot as the string engine writes it: `space`, a planet id, or `system@at`.
fn spot_in_space(spot: Spot) -> bool {
    matches!(spot, Spot::Space | Spot::Remote(_, None) | Spot::Card)
}

fn standing_at(cx: &Cx<'_>, who: Seat, ask: &ChooseAsk, system: Option<System>, arriving: Option<Arrival>) -> Standing {
    let half = |u: Unit| cx.q.k.hooks.combat.fighter_fleet_weight_halves(cx.core(), cx.q.k, who, u);
    fleet::standing_in(cx.core(), cx.scope, who, system, i64::from(ask.fleet_limit), arriving, &half)
}

fn standing_after(cx: &Cx<'_>, who: Seat, ask: &ChooseAsk, unit: Unit, spot: Spot, count: i64) -> Standing {
    let target = match spot {
        Spot::Remote(system, _) => Some(system),
        Spot::Card => None,
        _ => Some(ask.system),
    };
    standing_at(cx, who, ask, target, Some(Arrival { kind: unit, count, in_space: spot_in_space(spot) }))
}

/// `post_build_constraints`: the production limit and the free-capacity allowance after placing `count`.
fn post_build(cx: &Cx<'_>, who: Seat, ask: &ChooseAsk, unit: Unit, count: i64) -> (i64, i64) {
    let held_sol = cx.core().players().by_seat(who).is_some_and(|p| p.cards.breakthrough == Some(Breakthrough::BellumGloriosum));
    let d = unit.data();
    let granted = if held_sol && d.ship { fleet::capacity(unit) } else { 0 } * i64::from(count > 0);
    let spends = d.base_type == UnitBaseType::Fighter || fleet::is_ground_force(unit);
    let small = i64::from(ask.small_exemptions);
    let free = if spends { count.min(i64::from(ask.free_capacity) + granted) } else { 0 };
    let exempt = if matches!(d.base_type, UnitBaseType::Fighter | UnitBaseType::Infantry) { (count - free).min(small) } else { 0 };
    (i64::from(ask.remaining) - (count - free - exempt), i64::from(ask.free_capacity) + granted - free + small - exempt)
}

/// Standing of the producing system before any option is taken.
#[must_use]
pub fn standing_before(cx: &Cx<'_>, who: Seat, ask: &ChooseAsk) -> Standing {
    standing_at(cx, who, ask, Some(ask.system), None)
}

/// The Choosing question's facts for one unit; `None` when the unit is not offered.
#[must_use]
pub fn build_for(cx: &Cx<'_>, who: Seat, ask: &ChooseAsk, unit: Unit, ex: &Extras<'_>, before: &Standing, spendable_resources: i64) -> Option<BuildInfo> {
    let d = unit.data();
    if ask.only_unit.is_some_and(|only| only != unit) {
        return None;
    }
    if ask.max_unit_cost.is_some_and(|max| i64::from(d.cost_halves.unwrap_or(0)) > 2 * i64::from(max)) {
        return None;
    }
    let (printed, pair) = price_of(cx, unit);
    let (cost, discount_used) = if d.base_type == UnitBaseType::Flagship && ask.flagship_free {
        (0, 0)
    } else if ask.free_this_use {
        (0, printed)
    } else {
        let used = i64::from(ask.discount_remaining).min(printed);
        (printed - used, used)
    };
    let exchange = cx.seams().unit_exchange.first(cx.q, &ExchangeSite { who, system: ask.system, unit }).is_some();
    let affordable = cost <= available(cx, who, Spend::Resources, ex) + i64::from(ask.credit);
    if !affordable && !exchange {
        return None;
    }
    // A unit an ability produces is barred by a blanket ban only (the producer "ability" is no unit base type: a mech stands for it,
    // which no module's ban names), and no producer bars capacity.
    if ask.ability && cannot(cx, who, d.base_type, UnitBaseType::Mech) {
        return None;
    }
    let spots = spots_for(cx, who, ask.system, unit, ask.icon, ask.ability);
    if spots.as_slice().is_empty() {
        return None;
    }
    let barred = if ask.ability { 0 } else { barred_capacity(cx, who, ask.system, d.base_type) };
    let usable = if barred == 0 {
        i64::from(ask.remaining)
    } else {
        let same_kind = i64::from(ask.produced[d.base_type as usize]);
        i64::from(ask.remaining).min(i64::from(ask.limit) - barred - same_kind)
    };
    let extra = if matches!(d.base_type, UnitBaseType::Fighter | UnitBaseType::Infantry) { i64::from(ask.small_exemptions) } else { 0 };
    let made = pair.min(usize::try_from(usable + extra).unwrap_or(0));
    if made == 0 {
        return None;
    }
    let placed = placed_of(cx, who, unit, made);
    if placed == 0 {
        return None;
    }
    let (remaining_after, free_after) = post_build(cx, who, ask, unit, placed as i64);
    let only = |count: i64| match spots.as_slice() {
        [one] => Some(standing_after(cx, who, ask, unit, *one, count)),
        _ => None,
    };
    let after_paid = if affordable { only(placed as i64) } else { None };
    let (exchange_after, after_exchange) = if exchange { (post_build(cx, who, ask, unit, 1), only(1)) } else { ((0, 0), None) };
    Some(BuildInfo {
        unit,
        printed,
        pair,
        cost,
        discount_used,
        exchange,
        affordable,
        spots,
        made,
        placed,
        remaining_after,
        free_after,
        before: *before,
        spendable_resources,
        after_paid,
        after_exchange,
        exchange_after,
    })
}

/// The Choosing question's options: for each buildable unit the paid build, then the exchange.
#[must_use]
pub fn build_opts(cx: &Cx<'_>, who: Seat, ask: &ChooseAsk, ex: &Extras<'_>) -> SmallList<PayOpt, 18> {
    let before = standing_at(cx, who, ask, Some(ask.system), None);
    let spendable = available(cx, who, Spend::Resources, ex);
    let mut out = SmallList::new();
    for unit in ask.buildable.iter().flatten() {
        let Some(info) = build_for(cx, who, ask, *unit, ex, &before, spendable) else { continue };
        if info.affordable {
            out.push(PayOpt::Build { unit: *unit, made: u8::try_from(info.made).unwrap_or(u8::MAX) });
        }
        if info.exchange {
            out.push(PayOpt::Exchange { unit: *unit });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn price_rounds_up_and_halves_the_yield() {
        assert_eq!(price(Some(1), false), (1, 2));
        assert_eq!(price(Some(1), true), (1, 1));
        assert_eq!(price(Some(2), false), (1, 1));
        assert_eq!(price(Some(5), false), (3, 1));
        assert_eq!(price(None, false), (0, 1));
    }

    /// Found by the bench with `production=core` (seed 11, Saar commander): a module's destination in the
    /// producing system itself is dropped (its own spots are `placements`), as the string's
    /// `hooks_economy::production_destinations` does.
    #[test]
    fn a_destination_in_the_producing_system_is_not_a_remote_spot() {
        use crate::kernel::seam::{Pass, Source};
        let mut k = Kernel::plain();
        let (here, there) = (crate::ids::system_at(30), crate::ids::system_at(31));
        k.hooks.seam.production_destinations.push(Source::Faction(crate::generated::Faction::TheClanOfSaar), Pass::Add, |_: Q<'_>, _: &DestSite, out| {
            out.push((crate::ids::system_at(30), None));
            out.push((crate::ids::system_at(31), None));
        });
        let mut s = CoreState::all_loaded();
        s.players = crate::state::Players::from_vec(vec![crate::state::Player::new(Seat(0), crate::generated::Faction::TheClanOfSaar)]);
        let cx = Cx::new(&s, &k, Scope(u32::from(k.sources.0)), false);
        let spots = spots_of(&cx, Seat(0), here, Unit::CruiserI, Icon::Fresh);
        assert_eq!(spots.as_slice(), &[Spot::Space, Spot::Remote(there, None)]);
    }

    #[test]
    fn structure_limits() {
        assert_eq!(structure_limit(UnitBaseType::Pds), Some(2));
        assert_eq!(structure_limit(UnitBaseType::Spacedock), Some(1));
        assert_eq!(structure_limit(UnitBaseType::Mech), None);
    }
}
