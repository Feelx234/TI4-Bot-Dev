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

// -- Gurno Aggero -----------------------------------------------------------------------------------

/// The suspension points of [`ArmageddonRelay`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SStage {
    Start,
    Target,
    Done,
}

/// Gurno Aggero (Saar hero): "ACTION: Choose 1 system that is adjacent to 1 of your space docks. Destroy all other players'
/// infantry and fighters in that system." The system is chosen before anything is destroyed; the units are removed directly
/// (a leader effect announces no destruction). Never offered without a map or a dock with a neighbour. The neighbours are the
/// map's plain adjacency: a seat whose adjacency a faction rule changes is the adapter's to decline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArmageddonRelay {
    pub who: Seat,
    /// The string arm had a map (`context.galaxy`).
    pub has_map: bool,
    stage: SStage,
    pub used: bool,
}

impl ArmageddonRelay {
    #[must_use]
    pub const fn new(who: Seat, has_map: bool) -> Self {
        Self { who, has_map, stage: SStage::Start, used: false }
    }

    const ASK: Ask = Ask::Leaders(LAsk::SaarHero);

    /// The systems next to a space dock of the seat, in system order (the string sorts them by id).
    fn candidates(&self, core: &CoreState, k: &Kernel) -> Vec<System> {
        if !self.has_map {
            return Vec::new();
        }
        let scope = scope(k);
        let is_dock = |p: &crate::state::Piece| p.owner() == self.who && scope.unit(p.kind()).is_some_and(|d| d.base_type == UnitBaseType::Spacedock);
        let mut docks: Vec<System> = core.board().systems().filter(|(_, c)| c.units.as_slice().iter().any(is_dock)).map(|(s, _)| s).collect();
        docks.extend(
            core.board()
                .planets()
                .filter(|(_, c)| c.units.as_slice().iter().any(is_dock))
                .filter_map(|(planet, c)| c.placed_in.or(planet.data().system)),
        );
        let mut found: Vec<System> = Vec::new();
        for dock in docks {
            let Some(near) = board::neighbours(core, dock) else { continue };
            for index in near.iter() {
                if let Some(next) = System::ALL.get(index)
                    && !found.contains(next)
                {
                    found.push(*next);
                }
            }
        }
        found.sort_unstable();
        found
    }

    fn strike(&mut self, sx: &mut Sx<'_>, system: System) {
        let scope = scope(sx.kernel);
        let who = self.who;
        let doomed = |p: &crate::state::Piece| {
            p.owner() != who && scope.unit(p.kind()).is_some_and(|d| matches!(d.base_type, UnitBaseType::Fighter | UnitBaseType::Infantry))
        };
        sx.state.board_mut().sys_mut(system).units.retain(|p| !doomed(p));
        let planets: Vec<Planet> = board::planets_of(sx.state, system).collect();
        for planet in planets {
            sx.state.board_mut().planet_mut(planet).units.retain(|p| !doomed(p));
        }
        self.used = true;
        self.stage = SStage::Done;
    }
}

impl Window for ArmageddonRelay {
    type Stage = SStage;

    fn stage(&self) -> SStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == SStage::Start {
            self.stage = if self.candidates(sx.state, sx.kernel).is_empty() { SStage::Done } else { SStage::Target };
        }
        if self.stage == SStage::Target { Next::Continue(Self::ASK) } else { Next::Done }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        (self.stage == SStage::Target).then(|| Choice {
            actor: self.who,
            ask: Self::ASK,
            ctx: ctx(Leader::GurnoAggero, LSub::SaarHero),
            opts: self.candidates(s, k).into_iter().map(|system| Opt::Leaders(LOpt::System(system))).collect::<Opts>(),
        })
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        match (self.stage, pick) {
            (SStage::Target, Pick::Opt(Opt::Leaders(LOpt::System(system)))) if self.candidates(sx.state, sx.kernel).contains(&system) => {
                self.strike(sx, system);
                Ok(())
            }
            _ => Err(Illegal),
        }
    }
}

// -- Riftwalker Meian ------------------------------------------------------------------------------

/// What the string position vouches for the Ghost hero: the systems the card may name at all and those holding a wormhole now
/// (`ghost::hero_excluded`, `hooks_movement::wormholes_at`: the printed wormholes, the Creuss tokens, the ion storm, the Nexus and the
/// module wormholes are string reads the core does not hold).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SwapFacts {
    /// Every system the hero may name (not the Creuss systems, the Nexus or a Fracture system).
    pub allowed: crate::bits::SysSet,
    /// Every system that holds a wormhole.
    pub wormholes: crate::bits::SysSet,
}

/// The suspension points of [`RiftwalkerMeian`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GStage {
    Start,
    First,
    Second(System),
    Done,
}

/// Riftwalker Meian (Ghost hero): "ACTION: Swap the positions of any 2 non-Fracture systems that contain wormholes or your units,
/// other than the Creuss system, Ahk Creuxx system, or the Wormhole Nexus." Both systems are chosen (each question may be declined,
/// which resolves nothing) before the edit is recorded as the next `MapEdit` mark; the game replays it onto its own map at the end of
/// the step. The systems are offered in the order of the map (hex order). Never offered without a map or two systems to choose from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RiftwalkerMeian {
    pub who: Seat,
    pub has_map: bool,
    facts: SwapFacts,
    stage: GStage,
    pub used: bool,
}

impl RiftwalkerMeian {
    #[must_use]
    pub const fn new(who: Seat, has_map: bool, facts: SwapFacts) -> Self {
        Self { who, has_map, facts, stage: GStage::Start, used: false }
    }

    fn holds_units(&self, core: &CoreState, system: System) -> bool {
        core.board().sys(system).units.as_slice().iter().any(|u| u.owner() == self.who)
            || board::planets_of(core, system).any(|p| core.board().planet(p).units.as_slice().iter().any(|u| u.owner() == self.who))
    }

    /// The systems the card may name, in the order of the map.
    fn candidates(&self, core: &CoreState) -> Vec<System> {
        if !self.has_map {
            return Vec::new();
        }
        let Some(galaxy) = core.galaxy() else { return Vec::new() };
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
            .filter(|system| self.facts.allowed.contains(system.index()) && (self.facts.wormholes.contains(system.index()) || self.holds_units(core, *system)))
            .collect()
    }

    fn options(&self, core: &CoreState, skip: Option<System>) -> Opts {
        self.candidates(core).into_iter().filter(|s| Some(*s) != skip).map(|s| Opt::Leaders(LOpt::System(s))).chain(std::iter::once(Opt::Decline)).collect()
    }

    fn record(&mut self, sx: &mut Sx<'_>, a: System, b: System) {
        use crate::keys::{Atom, MarkKey, MarkVal, Word};
        let seq = sx.state.marks().tail.iter().filter(|(k, _)| matches!(k, MarkKey::MapEdit(_))).count();
        let value = MarkVal::Rec(vec![MarkVal::Atom(Atom::Word(Word::Swap)), MarkVal::Atom(Atom::System(a)), MarkVal::Atom(Atom::System(b))]);
        sx.state.marks_mut().tail.edit().insert(MarkKey::MapEdit(u32::try_from(seq).unwrap_or(u32::MAX)), value);
        self.used = true;
        self.stage = GStage::Done;
    }
}

impl Window for RiftwalkerMeian {
    type Stage = GStage;

    fn stage(&self) -> GStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == GStage::Start {
            self.stage = if self.candidates(sx.state).len() < 2 { GStage::Done } else { GStage::First };
        }
        match self.stage {
            GStage::First => Next::Continue(Ask::Leaders(LAsk::GhostSwapFirst)),
            GStage::Second(_) => Next::Continue(Ask::Leaders(LAsk::GhostSwapSecond)),
            GStage::Start | GStage::Done => Next::Done,
        }
    }

    fn ask_of(&self, s: &CoreState, _: &Kernel) -> Option<Choice> {
        match self.stage {
            GStage::First => Some(Choice { actor: self.who, ask: Ask::Leaders(LAsk::GhostSwapFirst), ctx: ctx(Leader::RiftwalkerMeian, LSub::GhostSwapFirst), opts: self.options(s, None) }),
            GStage::Second(first) => {
                Some(Choice { actor: self.who, ask: Ask::Leaders(LAsk::GhostSwapSecond), ctx: ctx(Leader::RiftwalkerMeian, LSub::GhostSwapSecond), opts: self.options(s, Some(first)) })
            }
            GStage::Start | GStage::Done => None,
        }
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        match (self.stage, pick) {
            (GStage::First | GStage::Second(_), Pick::Opt(Opt::Decline)) => {
                self.stage = GStage::Done;
                Ok(())
            }
            (GStage::First, Pick::Opt(Opt::Leaders(LOpt::System(first)))) if self.candidates(sx.state).contains(&first) => {
                self.stage = GStage::Second(first);
                Ok(())
            }
            (GStage::Second(first), Pick::Opt(Opt::Leaders(LOpt::System(second)))) if second != first && self.candidates(sx.state).contains(&second) => {
                self.record(sx, first, second);
                Ok(())
            }
            _ => Err(Illegal),
        }
    }
}

// -- Nekro Malleon ----------------------------------------------------------------------------------

/// The suspension points of [`NekroMalleon`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MStage {
    Start,
    Player,
    Payment(Seat),
    Card(Seat),
    Done,
}

/// Nekro Malleon (Nekro agent): "ACTION: Exhaust this card and choose a player who could pay: that player may discard 1 action card or
/// spend 1 command token from their command sheet to gain 2 trade goods" (the token returns to the supply). The owner chooses the player
/// (no question when one player could pay), that player answers a card, a pool or neither (always asked), and a card is asked again among
/// the distinct printed cards of the hand (no question when there is one). The agent was used whatever the answer; nothing changes before
/// the last answer. The discard is staged for the game step (a `CardsStaged` mark) and the gain is announced. `source` is the interned
/// `nekroagent`, the source of the trade goods.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NekroMalleon {
    pub who: Seat,
    source: Code,
    stage: MStage,
    /// The printed-name group of each action card, read from the table's facts when the hand is first looked at.
    groups: Vec<u8>,
    pub used: bool,
}

impl NekroMalleon {
    #[must_use]
    pub fn new(who: Seat, source: Code) -> Self {
        Self { who, source, stage: MStage::Start, groups: Vec::new(), used: false }
    }

    fn pools(core: &CoreState, seat: Seat) -> Vec<crate::choice::strategy::Pool> {
        use crate::choice::strategy::Pool;
        let Some(p) = core.players().by_seat(seat) else { return Vec::new() };
        [(Pool::Tactic, p.econ.tactic_tokens), (Pool::Fleet, p.econ.fleet_tokens), (Pool::Strategic, p.econ.strategic_tokens)]
            .into_iter()
            .filter(|(_, tokens)| *tokens > 0)
            .map(|(pool, _)| pool)
            .collect()
    }

    fn can_pay(core: &CoreState, seat: Seat) -> bool {
        core.players().by_seat(seat).is_some_and(|p| !p.cards.hand.is_empty()) || !Self::pools(core, seat).is_empty()
    }

    fn payers(core: &CoreState) -> Vec<Seat> {
        core.players().iter().map(|p| p.seat).filter(|seat| Self::can_pay(core, *seat)).collect()
    }

    /// One option per printed card of the hand, the first copy, in the order of the printed names (`first_of_each`).
    fn cards(core: &CoreState, groups: &[u8], seat: Seat) -> Vec<crate::generated::ActionCard> {
        let Some(p) = core.players().by_seat(seat) else { return Vec::new() };
        let mut seen: Vec<u8> = Vec::new();
        let mut firsts: Vec<crate::generated::ActionCard> = Vec::new();
        for card in p.cards.hand.as_slice() {
            let Some(group) = groups.get(*card as usize).copied() else { return Vec::new() };
            if !seen.contains(&group) {
                seen.push(group);
                firsts.push(*card);
            }
        }
        firsts.sort_by_key(|card| card.name_rank());
        firsts
    }

    fn gain(&mut self, s: &mut CoreState, target: Seat) {
        fx::gain_trade_goods_staged(s, target, 2, self.source);
    }

    fn discard(&mut self, s: &mut CoreState, target: Seat, card: crate::generated::ActionCard) {
        if fx::discard_action_card_staged(s, target, card) {
            self.gain(s, target);
        }
    }

    fn finish(&mut self) {
        self.used = true;
        self.stage = MStage::Done;
    }

    fn card_ctx(&self) -> Ctx {
        use crate::choice::cards::Reason;
        let mut c = ctx(Leader::NekroMalleon, LSub::NekroPlayer);
        c.subtype = Subtype::Cards(Reason::MalleonDiscard.subtype());
        c
    }
}

impl Window for NekroMalleon {
    type Stage = MStage;

    fn stage(&self) -> MStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == MStage::Start {
            let payers = Self::payers(sx.state);
            self.stage = match payers.as_slice() {
                [] => MStage::Done,
                [only] => MStage::Payment(*only),
                _ => MStage::Player,
            };
        }
        match self.stage {
            MStage::Player => Next::Continue(Ask::Leaders(LAsk::NekroPlayer)),
            MStage::Payment(_) => Next::Continue(Ask::Leaders(LAsk::NekroPayment)),
            MStage::Card(_) => Next::Continue(Ask::Cards(crate::choice::cards::Ask::OwnHand { reason: crate::choice::cards::Reason::MalleonDiscard })),
            MStage::Start | MStage::Done => Next::Done,
        }
    }

    fn ask_of(&self, s: &CoreState, _: &Kernel) -> Option<Choice> {
        use crate::choice::cards::{Ask as CAsk, Opt as COpt, Reason};
        match self.stage {
            MStage::Player => Some(Choice {
                actor: self.who,
                ask: Ask::Leaders(LAsk::NekroPlayer),
                ctx: ctx(Leader::NekroMalleon, LSub::NekroPlayer),
                opts: Self::payers(s).into_iter().map(|seat| Opt::Leaders(LOpt::Player(seat))).collect::<Opts>(),
            }),
            MStage::Payment(target) => {
                let mut opts = Opts::default();
                if s.players().by_seat(target).is_some_and(|p| !p.cards.hand.is_empty()) {
                    opts.push(Opt::Leaders(LOpt::Discard));
                }
                for pool in Self::pools(s, target) {
                    opts.push(Opt::Leaders(LOpt::Token(pool)));
                }
                opts.push(Opt::Decline);
                Some(Choice { actor: target, ask: Ask::Leaders(LAsk::NekroPayment), ctx: ctx(Leader::NekroMalleon, LSub::NekroPayment), opts })
            }
            MStage::Card(target) => {
                let groups = self.groups.as_slice();
                Some(Choice {
                    actor: target,
                    ask: Ask::Cards(CAsk::OwnHand { reason: Reason::MalleonDiscard }),
                    ctx: self.card_ctx(),
                    opts: Self::cards(s, groups, target).into_iter().map(|card| Opt::Cards(COpt::Card(card))).collect::<Opts>(),
                })
            }
            MStage::Start | MStage::Done => None,
        }
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        use crate::choice::cards::Opt as COpt;
        match (self.stage, pick) {
            (MStage::Player, Pick::Opt(Opt::Leaders(LOpt::Player(seat)))) if Self::payers(sx.state).contains(&seat) => {
                self.stage = MStage::Payment(seat);
                Ok(())
            }
            (MStage::Payment(_), Pick::Opt(Opt::Decline)) => {
                self.finish();
                Ok(())
            }
            (MStage::Payment(target), Pick::Opt(Opt::Leaders(LOpt::Discard))) => {
                let groups = sx.table.facts.card_group_table().map(<[u8]>::to_vec).unwrap_or_default();
                let cards = Self::cards(sx.state, &groups, target);
                match cards.as_slice() {
                    [] => return Err(Illegal),
                    [only] => {
                        self.discard(sx.state, target, *only);
                        self.finish();
                    }
                    _ => {
                        self.groups = groups;
                        self.stage = MStage::Card(target);
                    }
                }
                Ok(())
            }
            (MStage::Payment(target), Pick::Opt(Opt::Leaders(LOpt::Token(pool)))) if Self::pools(sx.state, target).contains(&pool) => {
                if let Some(p) = sx.state.players_mut().by_seat_mut(target) {
                    let slot = match pool {
                        crate::choice::strategy::Pool::Tactic => &mut p.econ.tactic_tokens,
                        crate::choice::strategy::Pool::Fleet => &mut p.econ.fleet_tokens,
                        crate::choice::strategy::Pool::Strategic => &mut p.econ.strategic_tokens,
                    };
                    *slot -= 1;
                }
                self.gain(sx.state, target);
                self.finish();
                Ok(())
            }
            (MStage::Card(target), Pick::Opt(Opt::Cards(COpt::Card(card)))) if Self::cards(sx.state, self.groups.as_slice(), target).contains(&card) => {
                self.discard(sx.state, target, card);
                self.finish();
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

    // -- Gurno Aggero and Riftwalker Meian ---------------------------------------------------------

    /// Real systems with a planet, in system order: `(system, its first planet)`.
    fn with_planet(n: usize) -> Vec<(System, Planet)> {
        let mut found: Vec<(System, Planet)> = Vec::new();
        for planet in Planet::ALL {
            if let Some(system) = planet.data().system
                && !found.iter().any(|(s, _)| *s == system)
            {
                found.push((system, planet));
            }
        }
        found.sort_unstable();
        found.truncate(n);
        found
    }

    /// A map with `systems` in a row, one hex apart, in the order given.
    fn row(s: &mut CoreState, systems: &[System]) {
        let mut galaxy = crate::galaxy::CoreGalaxy::new(&[]);
        for (i, system) in systems.iter().enumerate() {
            galaxy.place(crate::ids::system_at(*system as usize), crate::hex::Hex::new(i as i32, 0)).expect("on the grid");
        }
        s.map_mut().set_galaxy(galaxy);
    }

    fn unit_of(base: UnitBaseType, seat: Seat) -> crate::state::Piece {
        let unit = Unit::ALL.iter().copied().find(|u| u.data().base_type == base && u.data().faction.is_none()).expect("a plain unit");
        crate::state::Piece::new(unit, seat)
    }

    fn systems_offered(c: &Choice) -> Vec<System> {
        c.opts.as_slice().iter().filter_map(|o| if let Opt::Leaders(LOpt::System(s)) = o { Some(*s) } else { None }).collect()
    }

    #[test]
    fn the_saar_hero_offers_the_systems_next_to_a_dock_and_destroys_only_other_seats_fighters_and_infantry_there() {
        let (k, mut s) = (kernel(), table(2));
        let planets = with_planet(4);
        let [a, b, c, far] = [planets[0], planets[1], planets[2], planets[3]];
        // b - a - c in a row, the dock on a planet of `a`; `far` two hexes beyond `c`.
        row(&mut s, &[b.0, a.0, c.0, planets[3].0]);
        s.board_mut().planet_mut(a.1).units.push(unit_of(UnitBaseType::Spacedock, Seat(0)));
        // The other seat's fighters, infantry and a cruiser; the hero's own infantry.
        for (system, planet) in [b, c, far] {
            s.board_mut().sys_mut(system).units.push(unit_of(UnitBaseType::Fighter, Seat(1)));
            s.board_mut().sys_mut(system).units.push(unit_of(UnitBaseType::Cruiser, Seat(1)));
            s.board_mut().planet_mut(planet).units.push(unit_of(UnitBaseType::Infantry, Seat(1)));
            s.board_mut().planet_mut(planet).units.push(unit_of(UnitBaseType::Mech, Seat(1)));
            s.board_mut().planet_mut(planet).units.push(unit_of(UnitBaseType::Infantry, Seat(0)));
        }
        s.board_mut().sys_mut(b.0).units.push(unit_of(UnitBaseType::Fighter, Seat(0)));
        let mut d = driver(ArmageddonRelay::new(Seat(0), true));
        let Status::Asking(q) = d.run(&mut s, &k).unwrap() else { panic!("asks") };
        let mut expected = vec![b.0, c.0];
        expected.sort_unstable();
        assert_eq!(systems_offered(&q), expected, "the dock's own system and a far one are not offered");
        assert_eq!(d.resume(&mut s, &k, Pick::Opt(Opt::Leaders(LOpt::System(far.0)))), Err(DriverError::Illegal));
        assert_eq!(d.resume(&mut s, &k, Pick::Opt(Opt::Decline)), Err(DriverError::Illegal), "the question is not a may");
        assert_eq!(d.resume(&mut s, &k, Pick::Opt(Opt::Leaders(LOpt::System(b.0)))).unwrap(), Status::Done);
        assert!(d.window.used);
        let kinds = |system: System, planet: Planet, seat: Seat| -> Vec<UnitBaseType> {
            let mut all: Vec<UnitBaseType> = s.board().sys(system).units.as_slice().iter().chain(s.board().planet(planet).units.as_slice()).filter(|p| p.owner() == seat).map(|p| p.kind().data().base_type).collect();
            all.sort_unstable_by_key(|b| *b as usize);
            all
        };
        assert_eq!(kinds(b.0, b.1, Seat(1)), vec![UnitBaseType::Cruiser, UnitBaseType::Mech], "the other seat keeps what is neither fighter nor infantry");
        assert_eq!(kinds(b.0, b.1, Seat(0)), vec![UnitBaseType::Fighter, UnitBaseType::Infantry], "the hero's own units stay");
        assert_eq!(kinds(c.0, c.1, Seat(1)).len(), 4, "a system not chosen is untouched");
        assert_eq!(kinds(far.0, far.1, Seat(1)).len(), 4);
    }

    #[test]
    fn the_saar_hero_offers_the_systems_of_all_its_docks_in_system_order() {
        let (k, mut s) = (kernel(), table(2));
        let p = with_planet(5);
        let [s0, s1, s2, s3, s4] = [p[0], p[1], p[2], p[3], p[4]];
        // s0 - s1 - s2 - s4 - s3 in a row: a space dock on s4 (listed first) next to s2 and s3, a dock on a planet of s1 next to s0 and s2.
        row(&mut s, &[s0.0, s1.0, s2.0, s4.0, s3.0]);
        s.board_mut().sys_mut(s4.0).units.push(unit_of(UnitBaseType::Spacedock, Seat(0)));
        s.board_mut().planet_mut(s1.1).units.push(unit_of(UnitBaseType::Spacedock, Seat(0)));
        let mut d = driver(ArmageddonRelay::new(Seat(0), true));
        let Status::Asking(q) = d.run(&mut s, &k).unwrap() else { panic!("asks") };
        let mut expected = vec![s0.0, s2.0, s3.0];
        expected.sort_unstable();
        assert_eq!(systems_offered(&q), expected);
    }

    #[test]
    fn the_saar_hero_is_not_offered_without_a_map_or_a_dock_with_a_neighbour() {
        let (k, planets) = (kernel(), with_planet(2));
        let mut s = table(2);
        row(&mut s, &[planets[0].0, planets[1].0]);
        let mut d = driver(ArmageddonRelay::new(Seat(0), true));
        assert_eq!(d.run(&mut s, &k).unwrap(), Status::Done);
        assert!(!d.window.used, "no dock");
        s.board_mut().sys_mut(planets[0].0).units.push(unit_of(UnitBaseType::Spacedock, Seat(0)));
        let mut d = driver(ArmageddonRelay::new(Seat(0), false));
        assert_eq!(d.run(&mut s, &k).unwrap(), Status::Done);
        assert!(!d.window.used, "the string arm had no map");
        let mut d = driver(ArmageddonRelay::new(Seat(0), true));
        let Status::Asking(q) = d.run(&mut s, &k).unwrap() else { panic!("a dock in space counts") };
        assert_eq!(systems_offered(&q), vec![planets[1].0]);
    }

    fn swap_facts(allowed: &[System], wormholes: &[System]) -> SwapFacts {
        let set = |list: &[System]| {
            let mut out = crate::bits::SysSet::EMPTY;
            for system in list {
                out.insert(system.index());
            }
            out
        };
        SwapFacts { allowed: set(allowed), wormholes: set(wormholes) }
    }

    fn map_edits(s: &CoreState) -> Vec<(u32, crate::keys::MarkVal)> {
        s.marks().tail.iter().filter_map(|(k, v)| if let crate::keys::MarkKey::MapEdit(n) = k { Some((*n, v.clone())) } else { None }).collect()
    }

    #[test]
    fn the_ghost_hero_offers_wormhole_systems_and_those_with_its_units_in_the_order_of_the_map_and_records_the_swap() {
        use crate::keys::{Atom, MarkVal, Word};
        let (k, mut s) = (kernel(), table(2));
        let p = with_planet(6);
        let [s0, s1, s2, s3, s4, s5] = [p[0].0, p[1].0, p[2].0, p[3].0, p[4].0, p[5].0];
        // The map order is not the system order: s3 first, then s1, s5, s0, s4, s2.
        row(&mut s, &[s3, s1, s5, s0, s4, s2]);
        s.board_mut().sys_mut(s1).units.push(unit_of(UnitBaseType::Cruiser, Seat(0)));
        s.board_mut().planet_mut(p[2].1).units.push(unit_of(UnitBaseType::Infantry, Seat(0)));
        s.board_mut().sys_mut(s4).units.push(unit_of(UnitBaseType::Cruiser, Seat(1)));
        // s0 holds the seat's own ship and a wormhole but the card may not name it (the Creuss systems, the Nexus, a Fracture system).
        s.board_mut().sys_mut(s0).units.push(unit_of(UnitBaseType::Cruiser, Seat(0)));
        let facts = swap_facts(&[s1, s2, s3, s5], &[s0, s5]);
        let mut d = driver(RiftwalkerMeian::new(Seat(0), true, facts));
        let Status::Asking(q) = d.run(&mut s, &k).unwrap() else { panic!("asks") };
        assert_eq!(q.ask, Ask::Leaders(LAsk::GhostSwapFirst));
        // s1 (its ship), s5 (a wormhole), s2 (its infantry on a planet) in the order of the map; s4 holds only the other seat's ship
        // and s3 holds nothing; s0 is not allowed.
        assert_eq!(systems_offered(&q), vec![s1, s5, s2]);
        assert_eq!(q.opts.as_slice().last(), Some(&Opt::Decline), "each question may be declined");
        assert_eq!(d.resume(&mut s, &k, Pick::Opt(Opt::Leaders(LOpt::System(s4)))), Err(DriverError::Illegal));
        let Status::Asking(q2) = d.resume(&mut s, &k, Pick::Opt(Opt::Leaders(LOpt::System(s5)))).unwrap() else { panic!("asks again") };
        assert_eq!(q2.ask, Ask::Leaders(LAsk::GhostSwapSecond));
        assert_eq!(systems_offered(&q2), vec![s1, s2], "not the first system again");
        assert!(map_edits(&s).is_empty(), "nothing is written before the second answer");
        assert_eq!(d.resume(&mut s, &k, Pick::Opt(Opt::Leaders(LOpt::System(s5)))), Err(DriverError::Illegal));
        assert_eq!(d.resume(&mut s, &k, Pick::Opt(Opt::Leaders(LOpt::System(s2)))).unwrap(), Status::Done);
        assert!(d.window.used);
        let rec = |a: System, b: System| MarkVal::Rec(vec![MarkVal::Atom(Atom::Word(Word::Swap)), MarkVal::Atom(Atom::System(a)), MarkVal::Atom(Atom::System(b))]);
        assert_eq!(map_edits(&s), vec![(0, rec(s5, s2))]);
        // The next edit is the next mark.
        let mut d = driver(RiftwalkerMeian::new(Seat(0), true, facts));
        d.run(&mut s, &k).unwrap();
        d.resume(&mut s, &k, Pick::Opt(Opt::Leaders(LOpt::System(s1)))).unwrap();
        d.resume(&mut s, &k, Pick::Opt(Opt::Leaders(LOpt::System(s2)))).unwrap();
        assert_eq!(map_edits(&s), vec![(0, rec(s5, s2)), (1, rec(s1, s2))]);
    }

    #[test]
    fn the_ghost_hero_resolves_nothing_when_declined_without_a_map_or_with_fewer_than_two_systems() {
        let (k, p) = (kernel(), with_planet(3));
        let facts = swap_facts(&[p[0].0, p[1].0, p[2].0], &[p[0].0, p[1].0]);
        for declines in [0, 1] {
            let mut s = table(2);
            row(&mut s, &[p[0].0, p[1].0, p[2].0]);
            let mut d = driver(RiftwalkerMeian::new(Seat(0), true, facts));
            d.run(&mut s, &k).unwrap();
            if declines == 1 {
                d.resume(&mut s, &k, Pick::Opt(Opt::Leaders(LOpt::System(p[0].0)))).unwrap();
            }
            assert_eq!(d.resume(&mut s, &k, Pick::Opt(Opt::Decline)).unwrap(), Status::Done);
            assert!(!d.window.used);
            assert!(map_edits(&s).is_empty());
        }
        let mut s = table(2);
        row(&mut s, &[p[0].0, p[1].0, p[2].0]);
        let mut d = driver(RiftwalkerMeian::new(Seat(0), false, facts));
        assert_eq!(d.run(&mut s, &k).unwrap(), Status::Done, "the string arm had no map");
        let mut d = driver(RiftwalkerMeian::new(Seat(0), true, swap_facts(&[p[0].0, p[1].0, p[2].0], &[p[0].0])));
        assert_eq!(d.run(&mut s, &k).unwrap(), Status::Done, "one system only");
        assert!(!d.window.used);
    }

    // -- Nekro Malleon -----------------------------------------------------------------------------

    fn malleon_driver(seats: u8) -> Driver<NekroMalleon> {
        let mut t = AbilityTable::default();
        t.facts.card_groups = (0..crate::generated::ActionCard::COUNT).map(|i| u8::try_from(i).unwrap_or(u8::MAX)).collect();
        let seat_list: Vec<Seat> = (0..seats).map(Seat).collect();
        Driver::new(NekroMalleon::new(Seat(0), Code(5)), Resolver::new(&seat_list, Some(Seat(0))), t, 1)
    }

    fn broke(s: &mut CoreState, seat: u8) {
        let econ = &mut s.players_mut().by_seat_mut(Seat(seat)).unwrap().econ;
        (econ.tactic_tokens, econ.fleet_tokens, econ.strategic_tokens) = (0, 0, 0);
    }

    fn give(s: &mut CoreState, seat: u8, cards: &[crate::generated::ActionCard]) {
        let hand = &mut s.players_mut().by_seat_mut(Seat(seat)).unwrap().cards.hand;
        for card in cards {
            hand.push(*card);
        }
    }

    fn goods(s: &CoreState, seat: u8) -> i16 {
        s.players().by_seat(Seat(seat)).unwrap().econ.trade_goods
    }

    #[test]
    fn malleon_asks_a_lone_payer_only_for_the_payment_and_a_token_returns_to_the_supply_for_two_trade_goods() {
        use crate::choice::strategy::Pool;
        let (k, mut s) = (kernel(), table(3));
        for seat in 0..3 {
            broke(&mut s, seat);
        }
        s.players_mut().by_seat_mut(Seat(2)).unwrap().econ.fleet_tokens = 2;
        s.players_mut().by_seat_mut(Seat(2)).unwrap().econ.strategic_tokens = 1;
        let mut d = malleon_driver(3);
        let Status::Asking(q) = d.run(&mut s, &k).unwrap() else { panic!("asks") };
        assert_eq!((q.actor, q.ask), (Seat(2), Ask::Leaders(LAsk::NekroPayment)), "one payer: no choice of player, and the payer answers");
        assert_eq!(q.opts.as_slice(), &[Opt::Leaders(LOpt::Token(Pool::Fleet)), Opt::Leaders(LOpt::Token(Pool::Strategic)), Opt::Decline], "no card, no empty pool");
        assert_eq!(d.resume(&mut s, &k, Pick::Opt(Opt::Leaders(LOpt::Token(Pool::Tactic)))), Err(DriverError::Illegal));
        assert_eq!(goods(&s, 2), 0, "nothing before a legal answer");
        assert_eq!(d.resume(&mut s, &k, Pick::Opt(Opt::Leaders(LOpt::Token(Pool::Fleet)))).unwrap(), Status::Done);
        assert!(d.window.used);
        let p = s.players().by_seat(Seat(2)).unwrap();
        assert_eq!((p.econ.fleet_tokens, p.econ.strategic_tokens, p.econ.trade_goods), (1, 1, 2));
        assert_eq!(s.pending().staged.len(), 1, "the gain is announced");
    }

    #[test]
    fn malleon_lets_the_owner_pick_among_several_payers_and_the_payer_may_decline_which_still_uses_the_agent() {
        let (k, mut s) = (kernel(), table(3));
        for seat in [0, 2] {
            s.players_mut().by_seat_mut(Seat(seat)).unwrap().econ.tactic_tokens = 3;
        }
        let mut d = malleon_driver(3);
        let Status::Asking(q) = d.run(&mut s, &k).unwrap() else { panic!("asks") };
        assert_eq!((q.actor, q.ask), (Seat(0), Ask::Leaders(LAsk::NekroPlayer)));
        let paying: Vec<Opt> = [0u8, 2].into_iter().map(|i| Opt::Leaders(LOpt::Player(Seat(i)))).collect();
        assert_eq!(q.opts.as_slice(), paying.as_slice(), "the seats that could pay, in seat order");
        assert_eq!(d.resume(&mut s, &k, Pick::Opt(Opt::Leaders(LOpt::Player(Seat(1))))), Err(DriverError::Illegal));
        let Status::Asking(q) = d.resume(&mut s, &k, Pick::Opt(Opt::Leaders(LOpt::Player(Seat(2))))).unwrap() else { panic!("asks the payer") };
        assert_eq!((q.actor, q.ask), (Seat(2), Ask::Leaders(LAsk::NekroPayment)));
        assert_eq!(d.resume(&mut s, &k, Pick::Opt(Opt::Decline)).unwrap(), Status::Done);
        assert!(d.window.used, "the agent was used whether or not the payer took the offer");
        assert_eq!(goods(&s, 2), 0);
        assert!(s.pending().staged.is_empty());
    }

    #[test]
    fn malleon_discards_a_card_staged_for_the_game_step_and_asks_which_only_among_distinct_printed_cards() {
        use crate::generated::ActionCard as C;
        use crate::keys::{Atom, MarkKey, MarkVal, Word};
        let (k, mut s) = (kernel(), table(2));
        broke(&mut s, 0);
        broke(&mut s, 1);
        give(&mut s, 1, &[C::ALL[7], C::ALL[3], C::ALL[3]]);
        let mut d = malleon_driver(2);
        let Status::Asking(q) = d.run(&mut s, &k).unwrap() else { panic!("asks") };
        assert_eq!(q.opts.as_slice(), &[Opt::Leaders(LOpt::Discard), Opt::Decline]);
        let Status::Asking(q) = d.resume(&mut s, &k, Pick::Opt(Opt::Leaders(LOpt::Discard))).unwrap() else { panic!("which card") };
        assert_eq!((q.actor, q.ask), (Seat(1), Ask::Cards(crate::choice::cards::Ask::OwnHand { reason: crate::choice::cards::Reason::MalleonDiscard })));
        let mut ranked = [C::ALL[7], C::ALL[3]];
        ranked.sort_by_key(|c| c.name_rank());
        let expected: Vec<Opt> = ranked.iter().map(|c| Opt::Cards(crate::choice::cards::Opt::Card(*c))).collect();
        assert_eq!(q.opts.as_slice(), expected.as_slice(), "one per printed card, in the order of the names");
        assert_eq!(d.resume(&mut s, &k, Pick::Opt(Opt::Cards(crate::choice::cards::Opt::Card(C::ALL[9])))), Err(DriverError::Illegal));
        assert_eq!(d.resume(&mut s, &k, Pick::Opt(expected[0])).unwrap(), Status::Done);
        let chosen = ranked[0];
        let hand = s.players().by_seat(Seat(1)).unwrap().cards.hand.as_slice().to_vec();
        assert_eq!(hand.len(), 2);
        assert_eq!(hand.iter().filter(|c| **c == chosen).count(), if chosen == C::ALL[3] { 1 } else { 0 });
        assert_eq!(goods(&s, 1), 2);
        let row = MarkVal::Rec(vec![MarkVal::Atom(Atom::Word(Word::Discarded)), MarkVal::Atom(Atom::Seat(Seat(1))), MarkVal::Atom(Atom::Absent), MarkVal::Atom(Atom::Card(chosen))]);
        assert_eq!(s.marks().tail.get(&MarkKey::CardsStaged(0)), Some(&row), "the discard waits for the game step");
        // One distinct card (two copies of it): taken without a question.
        let mut s = table(2);
        broke(&mut s, 0);
        broke(&mut s, 1);
        give(&mut s, 1, &[C::ALL[3], C::ALL[3]]);
        s.marks_mut().tail.edit().insert(MarkKey::CardsStaged(4), MarkVal::Atom(Atom::Num(0)));
        let mut d = malleon_driver(2);
        d.run(&mut s, &k).unwrap();
        assert_eq!(d.resume(&mut s, &k, Pick::Opt(Opt::Leaders(LOpt::Discard))).unwrap(), Status::Done);
        assert_eq!(s.players().by_seat(Seat(1)).unwrap().cards.hand.len(), 1);
        assert!(s.marks().tail.get(&MarkKey::CardsStaged(5)).is_some(), "the next staged number");
    }

    #[test]
    fn malleon_is_not_offered_when_nobody_could_pay() {
        let (k, mut s) = (kernel(), table(2));
        broke(&mut s, 0);
        broke(&mut s, 1);
        let mut d = malleon_driver(2);
        assert_eq!(d.run(&mut s, &k).unwrap(), Status::Done);
        assert!(!d.window.used);
    }
}
