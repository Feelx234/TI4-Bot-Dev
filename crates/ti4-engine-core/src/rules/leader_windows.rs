//! The leader abilities that ask (package B7; string `leaders::dispatch_leader`), as [`Window`]s: Ggrocuto Rinn
//! (`xxchaagent`), Carth of Golden Sands (`hacanagent`), The Helmsman, Rin and Xxekir Grom (Thunder's Edge hero).
//!
//! Each is the effect of one arm of `dispatch_leader` split at its questions: `step` runs to the next question,
//! `answer` is the only place an answer changes the position, and the effect itself (readying, the commodity
//! change) happens after the last answer, atomically, as in the string arm. Whether the ability resolved at all
//! (the string arm's `bool`) is the window's `used` field; the caller exhausts the agent.
//!
//! What stays with the string callers (bails of the adapter, `leaders_core`): the events the commodity changes
//! and the replenish cause on other sides (Firmament siphon, the Cabal replenish event, Trade Agreement), the
//! diplomacy promise a favour keeps, and a question asked inside a battle or an invasion (its context carries
//! the battle).

use crate::choice::generic::Content;
use crate::choice::agenda::Shown;
use crate::choice::leaders::{Ask as LAsk, HeroUnit, Opt as LOpt, Subtype as LSub};
use crate::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use crate::containers::SmallList;
use crate::generated::{Leader, Planet, System, TechType, Technology, Unit, UnitBaseType};
use crate::query::tech;
use crate::ids::Seat;
use crate::kernel::fx;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::query::board;
use crate::state::{Code, CoreState, PlanetFlags};
use crate::tables::Scope;

fn scope(k: &Kernel) -> Scope {
    Scope(u32::from(k.sources.0))
}

fn ctx(leader: Leader, subtype: LSub) -> Ctx {
    Ctx {
        origin: Origin::FactionAbility(Content::Leader(leader)),
        subtype: Subtype::Leaders(subtype),
        optional: false,
        target: None,
        outstanding: SmallList::new(),
        space_battle: false,
        invasion_seq: None,
    }
}

/// Every exhausted planet, in planet (id text) order: the string `exhausted_planets` set.
#[must_use]
pub fn exhausted_planets(core: &CoreState) -> Vec<Planet> {
    core.board().planets().filter(|(_, c)| c.flags.has(PlanetFlags::EXHAUSTED)).map(|(p, _)| p).collect()
}

// -- Ggrocuto Rinn ----------------------------------------------------------------------------------

/// The suspension points of [`XxchaAgent`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XStage {
    Start,
    /// Waiting on "ready which planet" (more than one is exhausted).
    Planet,
    /// The planet is chosen: does the infantry offer apply?
    Offer,
    /// Waiting on "remove 1 infantry from that planet?".
    Infantry,
    Done,
}

/// "Ready any planet. If that planet is in a system adjacent to one you control, you may remove 1 infantry
/// from there."
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XxchaAgent {
    pub who: Seat,
    /// The string arm had a map (`context.galaxy`): without one the removal is never offered.
    pub has_map: bool,
    stage: XStage,
    planet: Option<Planet>,
    /// The ability resolved (the string arm returned `true`).
    pub used: bool,
}

impl XxchaAgent {
    #[must_use]
    pub const fn new(who: Seat, has_map: bool) -> Self {
        Self { who, has_map, stage: XStage::Start, planet: None, used: false }
    }

    const PLANET: Ask = Ask::Leaders(LAsk::XxchaPlanet);
    const INFANTRY: Ask = Ask::Leaders(LAsk::XxchaInfantry);

    /// The infantry of `who` on the planet that the offer may remove, when the offer applies.
    fn offers_removal(&self, core: &CoreState, k: &Kernel, planet: Planet) -> bool {
        if !self.has_map {
            return false;
        }
        let cell = core.board().planet(planet);
        if !cell.flags.has(PlanetFlags::UNITS_ENTRY) {
            return false;
        }
        let Some(home) = board::home_system(core, planet) else { return false };
        let Some(near) = board::neighbours(core, home) else { return false };
        let touches_mine = core
            .board()
            .planets()
            .filter(|(_, c)| c.control == Some(self.who))
            .filter_map(|(p, _)| board::home_system(core, p))
            .any(|system| near.contains(system.index()));
        if !touches_mine {
            return false;
        }
        let scope = scope(k);
        cell.units.as_slice().iter().any(|u| {
            u.owner() == self.who && scope.unit(u.kind()).is_some_and(|d| d.base_type == UnitBaseType::Infantry)
        })
    }

    fn finish(&mut self, sx: &mut Sx<'_>, remove: bool) {
        let planet = self.planet.expect("a chosen planet");
        fx::ready_planet(sx.state, planet);
        if remove {
            let scope = scope(sx.kernel);
            let who = self.who;
            let list = &mut sx.state.board_mut().planet_mut(planet).units;
            if let Some(at) = list
                .as_slice()
                .iter()
                .position(|u| u.owner() == who && scope.unit(u.kind()).is_some_and(|d| d.base_type == UnitBaseType::Infantry))
            {
                list.remove(at);
            }
        }
        self.used = true;
        self.stage = XStage::Done;
    }
}

impl Window for XxchaAgent {
    type Stage = XStage;

    fn stage(&self) -> XStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        loop {
            match self.stage {
                XStage::Start => {
                    let planets = exhausted_planets(sx.state);
                    match planets.as_slice() {
                        [] => self.stage = XStage::Done,
                        [only] => {
                            self.planet = Some(*only);
                            self.stage = XStage::Offer;
                        }
                        _ => self.stage = XStage::Planet,
                    }
                }
                XStage::Offer => {
                    let planet = self.planet.expect("a chosen planet");
                    if self.offers_removal(sx.state, sx.kernel, planet) {
                        self.stage = XStage::Infantry;
                    } else {
                        self.finish(sx, false);
                    }
                }
                XStage::Planet => return Next::Continue(Self::PLANET),
                XStage::Infantry => return Next::Continue(Self::INFANTRY),
                XStage::Done => return Next::Done,
            }
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        let leader = Leader::GgrocutoRinn;
        match self.stage {
            XStage::Planet => Some(Choice {
                actor: self.who,
                ask: Self::PLANET,
                ctx: ctx(leader, LSub::XxchaReadyPlanet),
                opts: exhausted_planets(s)
                    .into_iter()
                    .map(|p| Opt::Leaders(LOpt::Planet(p, Shown { votes: 0, system: board::system_of(s, scope(k), p) })))
                    .collect::<Opts>(),
            }),
            XStage::Infantry => Some(Choice {
                actor: self.who,
                ask: Self::INFANTRY,
                ctx: ctx(leader, LSub::XxchaRemoveInfantry),
                opts: [Opt::Leaders(LOpt::Remove), Opt::Leaders(LOpt::Keep)].into_iter().collect::<Opts>(),
            }),
            _ => None,
        }
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        match (self.stage, pick) {
            (XStage::Planet, Pick::Opt(Opt::Leaders(LOpt::Planet(p, _)))) if exhausted_planets(sx.state).contains(&p) => {
                self.planet = Some(p);
                self.stage = XStage::Offer;
                Ok(())
            }
            (XStage::Infantry, Pick::Opt(Opt::Leaders(LOpt::Remove))) => {
                self.finish(sx, true);
                Ok(())
            }
            (XStage::Infantry, Pick::Opt(Opt::Leaders(LOpt::Keep))) => {
                self.finish(sx, false);
                Ok(())
            }
            _ => Err(Illegal),
        }
    }
}

// -- Carth of Golden Sands --------------------------------------------------------------------------

/// The suspension points of [`HacanAgent`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HStage {
    Start,
    Branch,
    Done,
}

/// "Gain 2 commodities or replenish another player's commodities."
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HacanAgent {
    pub who: Seat,
    /// The interned `"trade_agreement"`: the source of the staged `TRADE_GOODS_GAINED` of a Trade Agreement the replenish triggers
    /// (`None`: the replenish announces nothing).
    pub agreement: Option<Code>,
    stage: HStage,
    pub used: bool,
}

/// The other players, in `GameState.players` order.
#[must_use]
pub fn others(core: &CoreState, who: Seat) -> SmallList<Seat, 8> {
    core.players().iter().map(|p| p.seat).filter(|s| *s != who).collect()
}

impl HacanAgent {
    #[must_use]
    pub const fn new(who: Seat) -> Self {
        Self { who, agreement: None, stage: HStage::Start, used: false }
    }

    /// [`Self::new`] with the interned text of the Trade Agreement's source.
    #[must_use]
    pub const fn with_source(who: Seat, agreement: Code) -> Self {
        Self { who, agreement: Some(agreement), stage: HStage::Start, used: false }
    }

    const ASK: Ask = Ask::Leaders(LAsk::HacanBranch);
}

impl Window for HacanAgent {
    type Stage = HStage;

    fn stage(&self) -> HStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == HStage::Start {
            // Nobody to replenish and no self-gain branch to offer is a broken table: never offered.
            self.stage = if others(sx.state, self.who).is_empty() { HStage::Done } else { HStage::Branch };
        }
        match self.stage {
            HStage::Branch => Next::Continue(Self::ASK),
            HStage::Start | HStage::Done => Next::Done,
        }
    }

    fn ask_of(&self, s: &CoreState, _: &Kernel) -> Option<Choice> {
        (self.stage == HStage::Branch).then(|| Choice {
            actor: self.who,
            ask: Self::ASK,
            ctx: ctx(Leader::CarthOfGoldenSands, LSub::HacanBranch),
            opts: std::iter::once(Opt::Leaders(LOpt::GainCommodities))
                .chain(others(s, self.who).as_slice().iter().map(|seat| Opt::Leaders(LOpt::Replenish(*seat))))
                .collect::<Opts>(),
        })
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        if self.stage != HStage::Branch {
            return Err(Illegal);
        }
        match pick {
            Pick::Opt(Opt::Leaders(LOpt::GainCommodities)) => {
                if let Some(p) = sx.state.players_mut().by_seat_mut(self.who) {
                    let limit = i16::from(p.faction.data().commodities);
                    p.econ.commodities = p.econ.commodities.saturating_add(2).min(limit);
                }
            }
            Pick::Opt(Opt::Leaders(LOpt::Replenish(target))) if others(sx.state, self.who).as_slice().contains(&target) => {
                match self.agreement {
                    Some(source) => {
                        fx::replenish_announced(sx.state, target, source);
                    }
                    None => {
                        fx::replenish_commodities(sx.state, target);
                    }
                }
            }
            _ => return Err(Illegal),
        }
        self.used = true;
        self.stage = HStage::Done;
        Ok(())
    }
}

// -- The Helmsman -----------------------------------------------------------------------------------

/// The suspension points of [`Helmsman`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LStage {
    Start,
    Destination,
    Done,
}

/// "Choose 1 system that contains no ships of other players; place your flagship and dreadnoughts there." The
/// ships are *placed*, not moved: range and anomalies do not apply. Never offered without a map or a ship to
/// gather.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Helmsman {
    pub who: Seat,
    /// The string arm had a map (`context.galaxy`).
    pub has_map: bool,
    stage: LStage,
    pub used: bool,
}

impl Helmsman {
    #[must_use]
    pub const fn new(who: Seat, has_map: bool) -> Self {
        Self { who, has_map, stage: LStage::Start, used: false }
    }

    const ASK: Ask = Ask::Leaders(LAsk::L1z1xDestination);

    fn named(&self, scope: Scope, p: crate::state::Piece) -> bool {
        p.owner() == self.who
            && scope.unit(p.kind()).is_some_and(|d| matches!(d.base_type, UnitBaseType::Flagship | UnitBaseType::Dreadnought))
    }

    /// The systems that may receive the ships, in the string galaxy's order (hex order): on the map, not a
    /// hyperlane tile, holding no ship of another seat.
    fn safe(&self, core: &CoreState, k: &Kernel) -> Vec<System> {
        let Some(galaxy) = core.galaxy() else { return Vec::new() };
        let scope = scope(k);
        let mut at: Vec<(crate::hex::Hex, System)> = galaxy
            .in_play()
            .iter()
            .filter(|s| !galaxy.is_hyperlane(crate::ids::system_at(*s)))
            .filter_map(|s| {
                let system = crate::ids::system_at(s);
                galaxy.coord_of(system).map(|hex| (hex, system))
            })
            .collect();
        at.sort_unstable_by_key(|(hex, _)| *hex);
        at.into_iter()
            .map(|(_, system)| system)
            .filter(|system| {
                !core
                    .board()
                    .sys(*system)
                    .units
                    .as_slice()
                    .iter()
                    .any(|u| u.owner() != self.who && scope.unit(u.kind()).is_some_and(|d| d.ship))
            })
            .collect()
    }

    fn gather(&mut self, sx: &mut Sx<'_>, destination: System) {
        let scope = scope(sx.kernel);
        let mut gathered: Vec<crate::state::Piece> = Vec::new();
        for system in System::ALL {
            if system == destination {
                continue;
            }
            let has = sx.state.board().sys(system).units.as_slice().iter().any(|u| self.named(scope, *u));
            if !has {
                continue;
            }
            let list = &mut sx.state.board_mut().sys_mut(system).units;
            gathered.extend(list.as_slice().iter().copied().filter(|u| self.named(scope, *u)));
            list.retain(|u| !self.named(scope, *u));
        }
        if !gathered.is_empty() {
            let cell = sx.state.board_mut().sys_mut(destination);
            cell.flags.set(crate::state::SysFlags::TOUCHED, true);
            for piece in &gathered {
                cell.units.push(*piece);
            }
        }
        self.used = !gathered.is_empty();
        self.stage = LStage::Done;
    }
}

impl Window for Helmsman {
    type Stage = LStage;

    fn stage(&self) -> LStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == LStage::Start {
            let scope = scope(sx.kernel);
            // Nothing anywhere to gather: the use could do nothing, so it was never offered.
            let ships = self.has_map && sx.state.board().systems().any(|(_, c)| c.units.as_slice().iter().any(|u| self.named(scope, *u)));
            let safe = if ships { self.safe(sx.state, sx.kernel) } else { Vec::new() };
            match safe.as_slice() {
                [] => self.stage = LStage::Done,
                [only] => {
                    let only = *only;
                    self.gather(sx, only);
                }
                _ => self.stage = LStage::Destination,
            }
        }
        match self.stage {
            LStage::Destination => Next::Continue(Self::ASK),
            LStage::Start | LStage::Done => Next::Done,
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        (self.stage == LStage::Destination).then(|| Choice {
            actor: self.who,
            ask: Self::ASK,
            ctx: ctx(Leader::TheHelmsman, LSub::L1z1xDestination),
            opts: self.safe(s, k).into_iter().map(|system| Opt::Leaders(LOpt::System(system))).collect::<Opts>(),
        })
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        match (self.stage, pick) {
            (LStage::Destination, Pick::Opt(Opt::Leaders(LOpt::System(system)))) if self.safe(sx.state, sx.kernel).contains(&system) => {
                self.gather(sx, system);
                Ok(())
            }
            _ => Err(Illegal),
        }
    }
}

// -- Rin, the Masters' Legacy -----------------------------------------------------------------------

/// The suspension points of [`RinHero`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RStage {
    Start,
    /// Waiting on the swap of `held` (the technology at `next - 1` of the eligible list).
    Swap,
    Done,
}

/// "For each non-unit upgrade technology you own, you may replace that technology with any technology of the
/// same colour from the deck." One question per technology that has a replacement on offer; declining all is a
/// legal resolution, but a use with nothing on offer at all is not one (`used` stays false).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RinHero {
    pub who: Seat,
    /// The technologies in the order of the corpus file (the string lists replacements in record order, which
    /// is not the generated enum order; the generator emits no such table yet, the adapter supplies it).
    order: Vec<Technology>,
    /// The technologies sorted by alias text (the string's `BTreeSet<TechnologyId>` order, which also is not
    /// the enum order); supplied by the adapter, which owns the alias texts.
    by_alias: Vec<Technology>,
    stage: RStage,
    /// The eligible technologies, in the order of their alias text (the string set's order).
    eligible: Vec<Technology>,
    /// The eligible technology the window is at.
    at: Option<Technology>,
    opportunity: bool,
    pub used: bool,
}

impl RinHero {
    #[must_use]
    pub fn new(who: Seat, order: Vec<Technology>, by_alias: Vec<Technology>) -> Self {
        Self { who, order, by_alias, stage: RStage::Start, eligible: Vec::new(), at: None, opportunity: false, used: false }
    }

    /// The technologies that can stand in for `held` now: same first type, not a unit upgrade, in the game's
    /// scope, open to the seat's faction, not owned; in table (record) order.
    fn replacements(&self, core: &CoreState, k: &Kernel, held: Technology) -> Vec<Technology> {
        let who = self.who;
        let (Some(colour), Some(p)) = (held.data().types.first().copied(), core.players().by_seat(who)) else { return Vec::new() };
        let scope = scope(k);
        self.order
            .iter()
            .copied()
            .filter(|t| {
                let d = t.data();
                crate::tables::source_bit(d.source) & scope.0 != 0
                    && d.types.first() == Some(&colour)
                    && !d.types.contains(&TechType::Unitupgrade)
                    && d.faction.is_none_or(|tag| tech::tag_is(tag, p.faction))
                    && !p.tech.owned.contains(*t)
            })
            .collect()
    }

    fn eligible_of(&self, core: &CoreState) -> Vec<Technology> {
        let Some(p) = core.players().by_seat(self.who) else { return Vec::new() };
        self.by_alias
            .iter()
            .copied()
            .filter(|t| p.tech.owned.contains(*t) && !tech::is_unit_upgrade(*t) && !t.data().types.is_empty())
            .collect()
    }

    /// The next eligible technology after `after` (or the first) that has a replacement; sets `at`.
    fn advance(&mut self, core: &CoreState, k: &Kernel, after: Option<Technology>) -> bool {
        let from = after.map_or(0, |a| self.eligible.iter().position(|t| *t == a).map_or(self.eligible.len(), |i| i + 1));
        for t in self.eligible[from..].iter().copied() {
            if !self.replacements(core, k, t).is_empty() {
                self.at = Some(t);
                self.opportunity = true;
                return true;
            }
        }
        false
    }

    const ASK: fn(Technology) -> Ask = |held| Ask::Leaders(LAsk::JolnarSwap { held });
}

impl Window for RinHero {
    type Stage = RStage;

    fn stage(&self) -> RStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == RStage::Start {
            self.eligible = self.eligible_of(sx.state);
            // Nothing swappable: the hero is not spent.
            self.stage = if self.eligible.is_empty() {
                RStage::Done
            } else if self.advance(sx.state, sx.kernel, None) {
                RStage::Swap
            } else {
                RStage::Done
            };
        }
        match (self.stage, self.at) {
            (RStage::Swap, Some(held)) => Next::Continue(Self::ASK(held)),
            _ => {
                self.used = self.opportunity;
                Next::Done
            }
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        let held = self.at.filter(|_| self.stage == RStage::Swap)?;
        Some(Choice {
            actor: self.who,
            ask: Self::ASK(held),
            ctx: ctx(Leader::RinTheMastersLegacy, LSub::JolnarSwap),
            // The decline comes last: a decider that takes the first option trades.
            opts: self.replacements(s, k, held)
                .into_iter()
                .map(|t| Opt::Leaders(LOpt::Trade(t)))
                .chain(std::iter::once(Opt::Leaders(LOpt::Keep)))
                .collect::<Opts>(),
        })
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        let (RStage::Swap, Some(held)) = (self.stage, self.at) else { return Err(Illegal) };
        match pick {
            Pick::Opt(Opt::Leaders(LOpt::Keep)) => {}
            Pick::Opt(Opt::Leaders(LOpt::Trade(t))) if self.replacements(sx.state, sx.kernel, held).contains(&t) => {
                let Some(p) = sx.state.players_mut().by_seat_mut(self.who) else { return Err(Illegal) };
                p.tech.owned.remove(held);
                p.tech.owned.insert(t);
            }
            _ => return Err(Illegal),
        }
        self.stage = if self.advance(sx.state, sx.kernel, Some(held)) { RStage::Swap } else { RStage::Done };
        Ok(())
    }
}

// -- Xxekir Grom, Thunder's Edge ------------------------------------------------------------------------

/// The suspension points of [`XxchaHeroTe`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TStage {
    Start,
    /// Between two placements: another one, or the end.
    Loop,
    /// Waiting on "place another PDS / mech, or stop".
    Place,
    /// Waiting on "which planet gets the unit".
    Planet,
    Done,
}

/// "Place any combination of up to 4 PDS or mechs onto planets you control; ready each planet that you place a
/// unit on. Then, purge this card." Every placement is decided before anything is placed (a refused answer leaves
/// the position as it was); stopping early is legal. The planets in the string's order, the unit forms and the
/// supply left (`supply::allowed`) are the adapter's; the purge is the caller's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XxchaHeroTe {
    pub who: Seat,
    planets: Vec<(System, Planet)>,
    pds: Option<Unit>,
    mech: Option<Unit>,
    pds_left: usize,
    mech_left: usize,
    placed: u8,
    kind: Option<Unit>,
    plan: Vec<(Unit, System, Planet)>,
    stage: TStage,
    /// The ability resolved (the string arm returned `true`).
    pub used: bool,
}

impl XxchaHeroTe {
    /// The most placements one use makes.
    pub const MOST: u8 = 4;

    #[must_use]
    pub fn new(who: Seat, planets: &[(System, Planet)], pds: Option<(Unit, usize)>, mech: Option<(Unit, usize)>) -> Self {
        Self {
            who,
            planets: planets.to_vec(),
            pds: pds.map(|p| p.0),
            mech: mech.map(|m| m.0),
            pds_left: pds.map_or(0, |p| p.1),
            mech_left: mech.map_or(0, |m| m.1),
            placed: 0,
            kind: None,
            plan: Vec::new(),
            stage: TStage::Start,
            used: false,
        }
    }

    const PLACE: Ask = Ask::Leaders(LAsk::XxchaHeroPlace);
    const PLANET: Ask = Ask::Leaders(LAsk::XxchaHeroPlanet);

    /// The units go down and their planets are readied.
    fn finish(&mut self, sx: &mut Sx<'_>) {
        for (unit, system, planet) in &self.plan {
            fx::place_unit(sx.state, self.who, *system, Some(*planet), *unit);
            fx::stage_naaz_mech_placed(sx.state, self.who, *system, *unit);
            fx::ready_planet(sx.state, *planet);
        }
        self.used = true;
        self.stage = TStage::Done;
    }

    fn plan_unit(&mut self, unit: Unit, system: System, planet: Planet) {
        self.plan.push((unit, system, planet));
        self.placed += 1;
        self.kind = None;
        self.stage = TStage::Loop;
    }
}

impl Window for XxchaHeroTe {
    type Stage = TStage;

    fn stage(&self) -> TStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        loop {
            match self.stage {
                // Nothing to place on: the ability was never offered.
                TStage::Start => self.stage = if self.planets.is_empty() { TStage::Done } else { TStage::Loop },
                TStage::Loop => {
                    if self.placed >= Self::MOST || (self.pds_left == 0 && self.mech_left == 0) {
                        self.finish(sx);
                    } else {
                        self.stage = TStage::Place;
                    }
                }
                TStage::Place => return Next::Continue(Self::PLACE),
                TStage::Planet => return Next::Continue(Self::PLANET),
                TStage::Done => return Next::Done,
            }
        }
    }

    fn ask_of(&self, _: &CoreState, k: &Kernel) -> Option<Choice> {
        let leader = Leader::XxekirGromTe;
        match self.stage {
            TStage::Place => {
                let mut opts = Opts::default();
                if self.pds_left > 0 {
                    opts.push(Opt::Leaders(LOpt::HeroPlace(HeroUnit::Pds)));
                }
                if self.mech_left > 0 {
                    opts.push(Opt::Leaders(LOpt::HeroPlace(HeroUnit::Mech)));
                }
                opts.push(Opt::Leaders(LOpt::HeroStop));
                Some(Choice { actor: self.who, ask: Self::PLACE, ctx: ctx(leader, LSub::XxchaHeroPlace), opts })
            }
            TStage::Planet => {
                let _ = k;
                Some(Choice {
                    actor: self.who,
                    ask: Self::PLANET,
                    ctx: ctx(leader, LSub::XxchaHeroPlanet),
                    opts: self
                        .planets
                        .iter()
                        .map(|(system, planet)| Opt::Leaders(LOpt::HeroPlanet(*planet, Shown { votes: 0, system: Some(*system) })))
                        .collect::<Opts>(),
                })
            }
            _ => None,
        }
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        match (self.stage, pick) {
            (TStage::Place, Pick::Opt(Opt::Leaders(LOpt::HeroStop))) => {
                self.finish(sx);
                Ok(())
            }
            (TStage::Place, Pick::Opt(Opt::Leaders(LOpt::HeroPlace(which)))) => {
                let (unit, left) = match which {
                    HeroUnit::Pds => (self.pds, &mut self.pds_left),
                    HeroUnit::Mech => (self.mech, &mut self.mech_left),
                };
                let Some(unit) = unit.filter(|_| *left > 0) else { return Err(Illegal) };
                *left -= 1;
                if let [(system, planet)] = self.planets.as_slice() {
                    let (system, planet) = (*system, *planet);
                    self.plan_unit(unit, system, planet);
                } else {
                    self.kind = Some(unit);
                    self.stage = TStage::Planet;
                }
                Ok(())
            }
            (TStage::Planet, Pick::Opt(Opt::Leaders(LOpt::HeroPlanet(planet, _)))) => {
                let Some(&(system, _)) = self.planets.iter().find(|(_, p)| *p == planet) else { return Err(Illegal) };
                let Some(unit) = self.kind else { return Err(Illegal) };
                self.plan_unit(unit, system, planet);
                Ok(())
            }
            _ => Err(Illegal),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::driver::{Driver, DriverError, Status};
    use crate::generated::Faction;
    use crate::choice::leaders::HeroUnit;
    use crate::kernel::stage::AbilityTable;
    use crate::kernel::Resolver;
    use crate::state::{Player, Players};

    fn kernel() -> Kernel {
        Kernel::plain()
    }

    fn table(seats: u8) -> CoreState {
        let mut s = CoreState::all_loaded();
        let list = (0..seats).map(|i| Player::new(Seat(i), Faction::from_index(usize::from(i)).unwrap())).collect();
        s.players = Players::from_vec(list);
        s
    }

    fn driver<W: Window>(w: W) -> Driver<W> {
        Driver::new(w, Resolver::new(&[Seat(0)], Some(Seat(0))), AbilityTable::default(), 1)
    }

    fn exhaust_two(s: &mut CoreState) -> (Planet, Planet) {
        let (a, b) = (Planet::from_index(3).unwrap(), Planet::from_index(9).unwrap());
        fx::exhaust_planet(s, a);
        fx::exhaust_planet(s, b);
        (a, b)
    }

    #[test]
    fn xxcha_with_nothing_exhausted_is_not_used() {
        let (k, mut s) = (kernel(), table(2));
        let mut d = driver(XxchaAgent::new(Seat(0), false));
        assert_eq!(d.run(&mut s, &k).unwrap(), Status::Done);
        assert!(!d.window.used);
    }

    #[test]
    fn xxcha_with_one_exhausted_planet_readies_it_without_a_question() {
        let (k, mut s) = (kernel(), table(2));
        let a = Planet::from_index(3).unwrap();
        fx::exhaust_planet(&mut s, a);
        let mut d = driver(XxchaAgent::new(Seat(0), false));
        assert_eq!(d.run(&mut s, &k).unwrap(), Status::Done);
        assert!(d.window.used);
        assert!(exhausted_planets(&s).is_empty());
    }

    #[test]
    fn xxcha_with_two_asks_which_and_refuses_a_planet_that_is_not_exhausted() {
        let (k, mut s) = (kernel(), table(2));
        let (a, b) = exhaust_two(&mut s);
        let mut d = driver(XxchaAgent::new(Seat(0), false));
        let Status::Asking(c) = d.run(&mut s, &k).unwrap() else { panic!("asks") };
        assert_eq!(c.opts.len(), 2);
        let other = Planet::from_index(5).unwrap();
        assert_eq!(d.resume(&mut s, &k, Pick::Opt(Opt::Leaders(LOpt::Planet(other, Shown::default())))), Err(DriverError::Illegal));
        assert_eq!(exhausted_planets(&s), vec![a, b], "nothing changes before a legal answer");
        assert_eq!(d.resume(&mut s, &k, Pick::Opt(c.opts.as_slice()[1])).unwrap(), Status::Done);
        assert_eq!(exhausted_planets(&s), vec![a]);
        assert!(d.window.used);
    }

    #[test]
    fn hacan_gains_two_capped_at_the_limit_and_replenishes_another_to_theirs() {
        let k = kernel();
        let mut s = table(3);
        let limit = |s: &CoreState, seat: u8| i16::from(s.players().by_seat(Seat(seat)).unwrap().faction.data().commodities);
        let mut d = driver(HacanAgent::new(Seat(0)));
        let Status::Asking(c) = d.run(&mut s, &k).unwrap() else { panic!("asks") };
        assert_eq!(c.opts.as_slice()[0], Opt::Leaders(LOpt::GainCommodities));
        assert_eq!(c.opts.len(), 3, "self plus two others, in player order");
        assert_eq!(d.resume(&mut s, &k, Pick::Opt(Opt::Leaders(LOpt::Replenish(Seat(2))))).unwrap(), Status::Done);
        assert_eq!(s.players().by_seat(Seat(2)).unwrap().econ.commodities, limit(&s, 2));
        assert!(d.window.used);

        let mut s = table(2);
        s.players_mut().by_seat_mut(Seat(0)).unwrap().econ.commodities = 0;
        let mut d = driver(HacanAgent::new(Seat(0)));
        d.run(&mut s, &k).unwrap();
        d.resume(&mut s, &k, Pick::Opt(Opt::Leaders(LOpt::GainCommodities))).unwrap();
        assert_eq!(s.players().by_seat(Seat(0)).unwrap().econ.commodities, 2.min(limit(&s, 0)));
        assert_eq!(d.resume(&mut s, &k, Pick::Opt(Opt::Decline)), Err(DriverError::NotAsking));
    }

    #[test]
    fn hacan_alone_at_the_table_is_not_offered() {
        let (k, mut s) = (kernel(), table(1));
        let mut d = driver(HacanAgent::new(Seat(0)));
        assert_eq!(d.run(&mut s, &k).unwrap(), Status::Done);
        assert!(!d.window.used);
    }

    // -- Xxekir Grom -------------------------------------------------------------------------------

    fn grom_planets() -> Vec<(System, Planet)> {
        vec![
            (System::from_index(3).unwrap(), Planet::from_index(3).unwrap()),
            (System::from_index(4).unwrap(), Planet::from_index(9).unwrap()),
            (System::from_index(5).unwrap(), Planet::from_index(12).unwrap()),
        ]
    }

    fn pieces(s: &CoreState, planet: Planet, unit: Unit) -> usize {
        s.board().planet(planet).units.as_slice().iter().filter(|u| u.owner() == Seat(0) && u.kind() == unit).count()
    }

    fn grom_units() -> (Unit, Unit) {
        let find = |base| Unit::ALL.iter().copied().find(|u| u.data().base_type == base && u.data().faction.is_none()).expect("a unit of that base type");
        (find(UnitBaseType::Pds), find(UnitBaseType::Mech))
    }

    #[test]
    fn grom_places_four_units_without_a_stop_and_readies_the_planets_it_placed_on() {
        let (k, mut s) = (kernel(), table(2));
        let (pds, mech) = grom_units();
        let planets = grom_planets();
        for (_, p) in &planets {
            fx::exhaust_planet(&mut s, *p);
        }
        let mut d = driver(XxchaHeroTe::new(Seat(0), &planets, Some((pds, 6)), Some((mech, 6))));
        let mut status = d.run(&mut s, &k).unwrap();
        for i in 0..4 {
            let Status::Asking(c) = status else { panic!("question {i}") };
            assert_eq!(c.ask, Ask::Leaders(LAsk::XxchaHeroPlace));
            assert_eq!(c.opts.len(), 3);
            let Status::Asking(c) = d.resume(&mut s, &k, Pick::Opt(Opt::Leaders(LOpt::HeroPlace(if i % 2 == 0 { HeroUnit::Pds } else { HeroUnit::Mech })))).unwrap() else {
                panic!("which planet")
            };
            assert_eq!(c.ask, Ask::Leaders(LAsk::XxchaHeroPlanet));
            assert_eq!(c.opts.len(), 3);
            assert_eq!(pieces(&s, planets[0].1, pds) + pieces(&s, planets[0].1, mech), 0, "nothing is placed before the end");
            status = d.resume(&mut s, &k, Pick::Opt(c.opts.as_slice()[0])).unwrap();
        }
        assert_eq!(status, Status::Done, "the fifth question is never put");
        assert!(d.window.used);
        assert_eq!((pieces(&s, planets[0].1, pds), pieces(&s, planets[0].1, mech)), (2, 2));
        assert!(!exhausted_planets(&s).contains(&planets[0].1), "the planet placed on is readied");
        assert_eq!(exhausted_planets(&s).len(), 2, "the others stay exhausted");
    }

    #[test]
    fn grom_asks_no_planet_for_a_single_planet_and_may_stop_at_once() {
        let (k, mut s) = (kernel(), table(2));
        let (pds, mech) = grom_units();
        let one = &grom_planets()[..1];
        let mut d = driver(XxchaHeroTe::new(Seat(0), one, Some((pds, 1)), Some((mech, 1))));
        assert!(matches!(d.run(&mut s, &k).unwrap(), Status::Asking(_)));
        let Status::Asking(c) = d.resume(&mut s, &k, Pick::Opt(Opt::Leaders(LOpt::HeroPlace(HeroUnit::Mech)))).unwrap() else { panic!("asks again") };
        assert_eq!(c.ask, Ask::Leaders(LAsk::XxchaHeroPlace));
        assert_eq!(c.opts.as_slice(), &[Opt::Leaders(LOpt::HeroPlace(HeroUnit::Pds)), Opt::Leaders(LOpt::HeroStop)], "the mech is used up");
        assert_eq!(d.resume(&mut s, &k, Pick::Opt(Opt::Leaders(LOpt::HeroStop))).unwrap(), Status::Done);
        assert_eq!((pieces(&s, one[0].1, pds), pieces(&s, one[0].1, mech)), (0, 1));
        assert!(d.window.used);

        let mut s = table(2);
        let mut d = driver(XxchaHeroTe::new(Seat(0), one, Some((pds, 1)), Some((mech, 1))));
        d.run(&mut s, &k).unwrap();
        assert_eq!(d.resume(&mut s, &k, Pick::Opt(Opt::Leaders(LOpt::HeroStop))).unwrap(), Status::Done);
        assert!(d.window.used, "stopping early still resolves the use");
        assert_eq!(pieces(&s, one[0].1, pds) + pieces(&s, one[0].1, mech), 0);
    }

    #[test]
    fn grom_refuses_what_is_not_offered_and_changes_nothing_before_the_end() {
        let (k, mut s) = (kernel(), table(2));
        let (pds, mech) = grom_units();
        let planets = grom_planets();
        // Only a PDS is left to place.
        let mut d = driver(XxchaHeroTe::new(Seat(0), &planets, Some((pds, 1)), Some((mech, 0))));
        d.run(&mut s, &k).unwrap();
        assert_eq!(d.resume(&mut s, &k, Pick::Opt(Opt::Leaders(LOpt::HeroPlace(HeroUnit::Mech)))), Err(DriverError::Illegal));
        assert_eq!(d.resume(&mut s, &k, Pick::Opt(Opt::Leaders(LOpt::HeroPlanet(planets[0].1, Shown::default())))), Err(DriverError::Illegal), "a planet before a unit");
        d.resume(&mut s, &k, Pick::Opt(Opt::Leaders(LOpt::HeroPlace(HeroUnit::Pds)))).unwrap();
        let other = Planet::from_index(20).unwrap();
        assert_eq!(d.resume(&mut s, &k, Pick::Opt(Opt::Leaders(LOpt::HeroPlanet(other, Shown::default())))), Err(DriverError::Illegal), "not a controlled planet");
        assert_eq!(d.resume(&mut s, &k, Pick::Opt(Opt::Leaders(LOpt::HeroStop))), Err(DriverError::Illegal), "stop is for the other question");
        assert_eq!(pieces(&s, planets[1].1, pds), 0, "nothing placed yet");
        assert_eq!(d.resume(&mut s, &k, Pick::Opt(Opt::Leaders(LOpt::HeroPlanet(planets[1].1, Shown::default())))).unwrap(), Status::Done, "the PDS was the last unit");
        assert_eq!(pieces(&s, planets[1].1, pds), 1);
        let touched = |system: System| s.board().sys(system).flags.has(crate::state::SysFlags::TOUCHED);
        assert!(touched(planets[1].0), "the system of the planet chosen is where the unit went");
        assert!(!touched(planets[0].0), "not the system of the first planet");
    }

    #[test]
    fn grom_without_a_planet_or_without_supply_asks_nothing() {
        let (k, mut s) = (kernel(), table(2));
        let (pds, mech) = grom_units();
        let mut d = driver(XxchaHeroTe::new(Seat(0), &[], Some((pds, 1)), Some((mech, 1))));
        assert_eq!(d.run(&mut s, &k).unwrap(), Status::Done);
        assert!(!d.window.used, "never offered");
        // Nothing left to place: the string loop breaks at once and the use still resolves.
        let mut d = driver(XxchaHeroTe::new(Seat(0), &grom_planets(), Some((pds, 0)), None));
        assert_eq!(d.run(&mut s, &k).unwrap(), Status::Done);
        assert!(d.window.used);
    }
}
