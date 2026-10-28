//! The leader abilities that ask (package B7; string `leaders::dispatch_leader`), as [`Window`]s: Ggrocuto Rinn
//! (`xxchaagent`) and Carth of Golden Sands (`hacanagent`).
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
use crate::choice::leaders::{Ask as LAsk, Opt as LOpt, Subtype as LSub};
use crate::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use crate::containers::SmallList;
use crate::generated::{Leader, Planet, System, UnitBaseType};
use crate::ids::Seat;
use crate::kernel::fx;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::query::board;
use crate::state::{CoreState, PlanetFlags};
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
        Self { who, stage: HStage::Start, used: false }
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
                if let Some(p) = sx.state.players_mut().by_seat_mut(target) {
                    p.econ.commodities = i16::from(p.faction.data().commodities);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::driver::{Driver, DriverError, Status};
    use crate::generated::Faction;
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
}
