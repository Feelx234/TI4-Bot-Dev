//! Objective progress and qualification on the typed state (package A4): the public objectives of
//! `objectives.rs` (`counting_progress`, `remaining_position_progress`, `bought_progress_at`) and the
//! position secrets of `secrets.rs`, each with the imagined variants the bot differences per option.
//!
//! Every function names the string function it answers and returns the same value. Thresholds are
//! in code exactly as the string engine has them (a `match` per objective, not data). What the core
//! cannot see is passed in as [`Terms`], computed by the caller with the string engine, to move to
//! the A5 `HookTable` later:
//!
//! * `iihq`: seats holding I.I.H.Q. Modernization (`keleres::has_iihq`): neighbours of everyone near Mecatol;
//! * `per_good`, `commodities`: what a trade good pays (`hooks_economy::trade_good_worth`) and the
//!   commodities a Keleres agent lets a payment spend (`keleres::spendable_commodities`);
//! * `oceans`: the seat holds Deepwrought ocean cards (not planets of the table: the bought
//!   progress is then answered by the string engine, `None`);
//! * `helios`: the Last Bastion dock bonus per planet (`bastion_units::resource_bonus`);
//! * `blanked`: Law's Order (`laws::blanked`);
//! * `firmament`: planets the Firmament commander lets the seat treat as controlled
//!   (`borrowed_commanders_b::firmament_planets`), for secrets;
//! * `capacity`: `production::capacity` per system, for Produce en Masse.
//!
//! The scan of a seat's board happens once per position and seat ([`SeatFacts`]); a call then reads
//! those facts plus the imagined sets of the option ([`Imagined`]), which makes the per-option
//! variants cheap. Behaviours of the string engine that look like slips are kept (this is a pure
//! refactor): the planet dedupe of `Position::imagining` compares a display name with an id and so
//! never fires; the imagined planets of Discover Lost Outposts and Rule Distant Lands read display
//! names as ids and so add nothing; the bought-progress bonus counts coexisting planets.

use crate::bits::SysSet;
use crate::containers::{SeatSet, Set, SmallList};
use crate::generated::{
    Faction, FactionTag, Planet, PlanetTrait, PublicObjective, SecretObjective, System, TechType, Technology, Unit,
    UnitBaseType,
};
use crate::ids::Seat;
use crate::state::{CoreState, ObjectiveRef, PlanetFlags, SysFlags};
use crate::tables::{Scope, source_bit};

use super::board;
use super::economy::{self, Spend};

// --- families and results ------------------------------------------------------------------------

/// `objectives::CountFamily`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    NonHome,
    OnTheRim,
    SameTrait,
    TechSpecialties,
    UnitUpgrades,
    Colours { per_colour: usize },
    Structures,
    StructuresAway,
    FleetInOneSystem,
    PlanetlessSystems,
    AttachedPlanets,
    NotableSystems,
    GroundForcesOnOnePlanet,
    MechsOnDistinctPlanets,
    PlanetsOfTrait(PlanetTrait),
    SameColourTechnologies,
    ShipsInSystems,
    Units(UnitBaseType),
    RivalHomePlanets,
    CapitalShipSystems,
    CapitalShipsInRivalHomeOrMecatol,
    ShipsAdjacentToMecatol,
    WeakerNeighbours,
    DistinctRivalHomeReaches,
    RivalDockSystemsWithShips,
    RelicFragments,
    ActionCards,
    RivalNoteIssuers,
    ShipSystemsBesideAnomaly,
    ShipSystemsBesideRivalHome,
    Neighbours,
    UnitsInNexus,
    WormholeKinds,
    FactionTechnologies,
    Laws,
    SharedPlanetSystems,
    ProductionInOneSystem,
    LegendaryPlanets,
    MecatolShipsWhileControlling,
    CombinedValue(Spend),
}

/// `objectives::RequirementProgress`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress {
    pub family: Family,
    pub have: usize,
    pub threshold: usize,
}

impl Progress {
    #[must_use]
    pub const fn satisfied(&self) -> bool {
        self.have >= self.threshold
    }
}

/// `objectives::CostFamily`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CostFamily {
    Spend(Spend),
    TradeGoods,
    Tokens,
    AllThree,
}

/// `objectives::CostProgress`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CostProgress {
    pub family: CostFamily,
    pub have: i64,
    pub target: i64,
}

impl CostProgress {
    #[must_use]
    pub const fn satisfied(self) -> bool {
        self.have >= self.target
    }
}

/// `objectives::Cost`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cost {
    Spend { amount: i64, kind: Spend },
    TradeGoods(i32),
    Tokens(i32),
    AllThree(i64),
}

// --- what the option would change ------------------------------------------------------------------

/// `objectives::Imagined` in typed form. The caller maps ids; an id the tables do not know makes the
/// caller answer with the string engine.
#[derive(Debug, Clone, Copy, Default)]
pub struct Imagined<'a> {
    pub planets: &'a [Planet],
    pub systems: &'a [System],
    pub technologies: &'a [Technology],
    pub structures: &'a [(System, Planet)],
}

impl Imagined<'_> {
    pub const NONE: Imagined<'static> = Imagined { planets: &[], systems: &[], technologies: &[], structures: &[] };
}

// --- hook terms --------------------------------------------------------------------------------------

/// The answers of faction and card hooks the core does not hold (see the module docs).
#[derive(Debug, Clone, Default)]
pub struct Terms {
    pub iihq: SeatSet,
    pub blanked: bool,
    /// What one trade good pays; at least 1.
    pub per_good: i64,
    pub commodities: i64,
    pub oceans: bool,
    pub helios: SmallList<(Planet, i64), 4>,
    pub firmament: Set<Planet, 3>,
    /// `production::capacity` of the seat per system (secrets: Produce en Masse); `None` entries are
    /// zero. Filled only when the caller asks for it.
    pub capacity: SmallList<(System, i64), 8>,
    pub capacity_known: bool,
    /// The tag of the seat's faction in the homeworld table (`Planet::homeworld_of`), if it has one.
    pub faction_tag: Option<FactionTag>,
    /// A hook answered with something the core has no id for: secrets are the string engine's.
    pub unmapped: bool,
}

// --- the position of one seat -----------------------------------------------------------------------

/// The scan of one seat's board, once per position (`objectives::Position::new` and the helpers that
/// every requirement re-derived).
#[derive(Debug, Clone)]
pub struct SeatFacts {
    pub seat: Seat,
    pub scope: Scope,
    /// The scoring view: controlled planets, then coexisting ones, in scope, space stations out
    /// (`Position::controlled`).
    pub controlled: SmallList<Planet, 32>,
    /// `GameState::controlled_planets`: `(system, planet)` sorted.
    pub raw: board::Controlled,
    /// `Position::home_system`.
    pub home: Option<System>,
    /// Systems with a `board` entry.
    pub touched: SysSet,
    /// Systems where the seat has any unit, in space or on a planet.
    pub unit_systems: SysSet,
    /// Systems where the seat has a ship in space.
    pub ship_systems: SysSet,
    /// Systems where the seat has a flagship or a war sun in space.
    pub capital_systems: SysSet,
    /// Non-fighter ships per system (entries above zero).
    pub fleet: SmallList<(System, u16), 24>,
    /// One entry per structure of the seat on a planet: `(system, planet)`.
    pub structures: SmallList<(System, Planet), 16>,
    /// Owned technologies per [`TechType`] (a technology counts for each of its types).
    pub tech_types: [u16; 5],
    pub owned: crate::state::TechSet,
    /// Units of the seat in scope, by base type, in space and on planets.
    pub by_base: [u16; 11],
    /// Owned technologies by colour in the order biotic, cybernetic, propulsion, warfare (a
    /// technology counts for the first colour it carries: `technology::colour_type`).
    pub colours: [u16; 4],
    /// Planets holding a mech of the seat.
    pub mech_planets: Set<Planet, 3>,
    /// Most ground forces of the seat on one planet.
    pub ground_max: u16,
    pub terms: Terms,
}

fn in_scope_planet(scope: Scope, planet: Planet) -> bool {
    scope.0 & source_bit(planet.data().source) != 0
}

/// `Planet::is_space_station`.
#[must_use]
pub fn is_space_station(planet: Planet) -> bool {
    planet.data().traits.contains(&PlanetTrait::Spacestation)
}

/// `Position::new`'s controlled list for any seat (the scoring view).
#[must_use]
pub fn scoring_planets(core: &CoreState, scope: Scope, seat: Seat) -> SmallList<Planet, 32> {
    let mut out = SmallList::new();
    for (system, planet) in board::controlled_planets(core, seat).as_slice() {
        let _ = system;
        if in_scope_planet(scope, *planet) && !is_space_station(*planet) {
            out.push(*planet);
        }
    }
    // Coexisting planets (rule 13): board order, planet order.
    for (planet, cell) in core.board().planets() {
        if cell.coexist.contains(seat)
            && cell.flags.has(PlanetFlags::COEXIST_ENTRY)
            && in_scope_planet(scope, planet)
            && !is_space_station(planet)
        {
            out.push(planet);
        }
    }
    out
}

/// `UnitType::is_ground_force`: infantry and mechs, and the Titans' PDS.
fn is_ground_force(unit: Unit) -> bool {
    matches!(unit.data().base_type, UnitBaseType::Infantry | UnitBaseType::Mech)
        || matches!(unit, Unit::HelTitanI | Unit::HelTitanII)
}

impl SeatFacts {
    /// Scan the board for `seat`. `terms` are the hook answers for this position and seat.
    #[must_use]
    pub fn of(core: &CoreState, scope: Scope, seat: Seat, terms: Terms) -> Self {
        let mut f = Self {
            seat,
            scope,
            controlled: scoring_planets(core, scope, seat),
            raw: board::controlled_planets(core, seat),
            home: None,
            touched: SysSet::EMPTY,
            unit_systems: SysSet::EMPTY,
            ship_systems: SysSet::EMPTY,
            capital_systems: SysSet::EMPTY,
            fleet: SmallList::new(),
            structures: SmallList::new(),
            tech_types: [0; 5],
            owned: crate::state::TechSet::EMPTY,
            by_base: [0; 11],
            colours: [0; 4],
            mech_planets: Set::new(),
            ground_max: 0,
            terms,
        };
        f.home = home_system_of(core, seat);
        for (system, cell) in core.board().systems() {
            if cell.flags.has(SysFlags::TOUCHED) {
                f.touched.insert(system.index());
            }
            let mut ships = 0u16;
            for piece in cell.units.as_slice() {
                if piece.owner() != seat {
                    continue;
                }
                // Any unit of the seat counts for the "holds units" systems, whatever its type.
                f.unit_systems.insert(system.index());
                let Some(data) = scope.unit(piece.kind()) else { continue };
                if data.ship {
                    f.ship_systems.insert(system.index());
                    if data.base_type != UnitBaseType::Fighter {
                        ships += 1;
                    }
                }
                if matches!(data.base_type, UnitBaseType::Flagship | UnitBaseType::Warsun) {
                    f.capital_systems.insert(system.index());
                }
                f.by_base[data.base_type.index()] += 1;
            }
            if ships > 0 {
                f.fleet.push((system, ships));
            }
        }
        for (planet, cell) in core.board().planets() {
            let Some(system) = board::home_system(core, planet) else { continue };
            let mut ground = 0u16;
            for piece in cell.units.as_slice() {
                if piece.owner() != seat {
                    continue;
                }
                f.unit_systems.insert(system.index());
                let Some(data) = scope.unit(piece.kind()) else { continue };
                f.by_base[data.base_type.index()] += 1;
                if data.structure {
                    f.structures.push((system, planet));
                }
                if data.base_type == UnitBaseType::Mech {
                    f.mech_planets.insert(planet);
                }
                if is_ground_force(piece.kind()) {
                    ground += 1;
                }
            }
            f.ground_max = f.ground_max.max(ground);
        }
        if let Some(player) = core.players().by_seat(seat) {
            f.owned = player.tech.owned;
            for tech in player.tech.owned.iter() {
                let types = tech.data().types;
                for kind in types {
                    f.tech_types[*kind as usize] += 1;
                }
                let first = [TechType::Biotic, TechType::Cybernetic, TechType::Propulsion, TechType::Warfare]
                    .iter()
                    .position(|c| types.contains(c));
                if let Some(i) = first {
                    f.colours[i] += 1;
                }
            }
        }
        f
    }
}

/// `Position::home_system`: the seat's own record, else its faction's.
#[must_use]
pub fn home_system_of(core: &CoreState, seat: Seat) -> Option<System> {
    let player = core.players().by_seat(seat)?;
    player.home_system.or_else(|| player.faction.data().home_system)
}

/// What the requirements read besides the facts: the state, the map flag, the imagined sets.
#[derive(Debug, Clone, Copy)]
pub struct Pos<'a> {
    pub core: &'a CoreState,
    pub facts: &'a SeatFacts,
    /// The string `Position.galaxy.is_some()`.
    pub map: bool,
    pub imagined: Imagined<'a>,
}

impl<'a> Pos<'a> {
    #[must_use]
    pub const fn new(core: &'a CoreState, facts: &'a SeatFacts, map: bool, imagined: Imagined<'a>) -> Self {
        Self { core, facts, map, imagined }
    }

    fn seat(&self) -> Seat {
        self.facts.seat
    }

    fn scope(&self) -> Scope {
        self.facts.scope
    }

    /// `Position::imagining`'s planets: appended without a dedupe (the string dedupe compares a display
    /// name with an id and never fires), in scope, no stations.
    fn imagined_planets(&self) -> impl Iterator<Item = Planet> + '_ {
        let scope = self.scope();
        self.imagined.planets.iter().copied().filter(move |p| in_scope_planet(scope, *p) && !is_space_station(*p))
    }

    /// The scoring view with the imagined planets.
    fn controlled_len(&self) -> usize {
        self.facts.controlled.len() + self.imagined_planets().count()
    }

    fn controlled_each(&self, mut f: impl FnMut(Planet)) {
        for p in self.facts.controlled.as_slice() {
            f(*p);
        }
        for p in self.imagined_planets() {
            f(p);
        }
    }

    /// `Position::systems_holding_units`.
    fn systems_holding_units(&self) -> SysSet {
        let mut s = self.facts.unit_systems;
        for sys in self.imagined.systems {
            s.insert(sys.index());
        }
        s
    }

    /// `Position::systems_with_ships`.
    fn systems_with_ships(&self) -> SysSet {
        let mut s = self.facts.ship_systems;
        for sys in self.imagined.systems {
            s.insert(sys.index());
        }
        s
    }

    fn imagined_in(&self, system: System) -> bool {
        self.imagined.systems.contains(&system)
    }

    /// `Position::technology_types`.
    fn technology_types(&self, wanted: TechType) -> usize {
        let mut n = usize::from(self.facts.tech_types[wanted as usize]);
        // Already-researched technologies are ignored (a clean delta); a duplicate in the list counts twice.
        for tech in self.imagined.technologies {
            if !self.facts.owned.contains(*tech) && tech.data().types.contains(&wanted) {
                n += 1;
            }
        }
        n
    }

    /// `Position::structures`: the seat's structures, then the imagined ones on planets without one.
    fn structure_entries(&self) -> SmallList<(System, Planet), 24> {
        let mut found: SmallList<(System, Planet), 24> = SmallList::new();
        for e in self.facts.structures.as_slice() {
            found.push(*e);
        }
        let held: SmallList<Planet, 16> = self.facts.structures.as_slice().iter().map(|(_, p)| *p).collect();
        for (system, planet) in self.imagined.structures {
            if !held.as_slice().contains(planet) {
                found.push((*system, *planet));
            }
        }
        found
    }
}

// --- map shape -----------------------------------------------------------------------------------------

/// `objectives::edge_systems`: systems with a neighbouring hex that holds no tile.
#[must_use]
pub fn edge_systems(core: &CoreState) -> SysSet {
    let mut out = SysSet::EMPTY;
    let Some(g) = core.galaxy() else { return out };
    for system in g.in_play().iter() {
        let system = crate::ids::system_at(system);
        if let Some(here) = g.coord_of(system)
            && here.neighbours().into_iter().any(|next| g.system_at(next).is_none())
        {
            out.insert(system.index());
        }
    }
    out
}

/// `seating::mecatol_in_galaxy`.
#[must_use]
pub fn mecatol_in_galaxy(core: &CoreState) -> System {
    match core.galaxy() {
        Some(g) if g.coord_of(System::MecatolRex112).is_some() => System::MecatolRex112,
        _ => System::MecatolRex18,
    }
}

/// `seating::mecatol_on`: the Thunder's Edge tile when the board holds it.
#[must_use]
pub fn mecatol_on(core: &CoreState) -> System {
    if core.board().sys(System::MecatolRex112).flags.has(SysFlags::TOUCHED) {
        System::MecatolRex112
    } else {
        System::MecatolRex18
    }
}

/// `seating::is_mecatol`.
#[must_use]
pub fn is_mecatol(system: System) -> bool {
    matches!(system, System::MecatolRex18 | System::MecatolRex112)
}

/// `transactions::presence`: systems where the seat has a unit (space or planet) or controls a planet.
#[must_use]
pub fn presence(core: &CoreState, seat: Seat) -> SysSet {
    let mut out = board::systems_with_units_of(core, seat);
    for (planet, cell) in core.board().planets() {
        let Some(system) = board::home_system(core, planet) else { continue };
        if cell.control == Some(seat) || cell.units.as_slice().iter().any(|u| u.owner() == seat) {
            out.insert(system.index());
        }
    }
    out
}

fn near_mecatol(core: &CoreState, presence: &SysSet) -> bool {
    let mecatol = mecatol_in_galaxy(core);
    if presence.contains(mecatol.index()) {
        return true;
    }
    core.galaxy().is_some_and(|g| g.adjacent(mecatol).iter().any(|s| presence.contains(s)))
}

fn present_together(core: &CoreState, here: &SysSet, there: &SysSet) -> bool {
    if here.intersects(there) {
        return true;
    }
    let Some(g) = core.galaxy() else { return false };
    here.iter().any(|s| g.adjacent(crate::ids::system_at(s)).intersects(there))
}

/// `transactions::neighbours`: the other seats of the seating order that are neighbours of `seat`.
#[must_use]
pub fn neighbours(core: &CoreState, terms: &Terms, seat: Seat) -> SmallList<Seat, 8> {
    let mut out = SmallList::new();
    let mut here: Option<SysSet> = None;
    for other in core.seats().seating_order() {
        let other = *other;
        if other == seat {
            continue;
        }
        let theirs = presence(core, other);
        let mine = *here.get_or_insert_with(|| presence(core, seat));
        let mecatol = (terms.iihq.contains(seat) && near_mecatol(core, &theirs))
            || (terms.iihq.contains(other) && near_mecatol(core, &mine));
        if mecatol || present_together(core, &mine, &theirs) {
            out.push(other);
        }
    }
    out
}

// --- the public objectives -----------------------------------------------------------------------------

/// The threshold and family of a counting objective (`objectives::counting_progress`'s table).
#[must_use]
pub const fn counting_row(obj: PublicObjective) -> Option<(Family, usize)> {
    use PublicObjective as O;
    Some(match obj {
        O::ExpandBorders => (Family::NonHome, 6),
        O::SubdueTheGalaxy => (Family::NonHome, 11),
        O::PopulateTheOuterRim => (Family::OnTheRim, 3),
        O::ControlTheBorderlands => (Family::OnTheRim, 5),
        O::CornerTheMarket => (Family::SameTrait, 4),
        O::UnifyTheColonies => (Family::SameTrait, 6),
        O::FoundResearchOutposts => (Family::TechSpecialties, 3),
        O::FormGalacticBrainTrust => (Family::TechSpecialties, 5),
        O::DevelopWeaponry => (Family::UnitUpgrades, 2),
        O::RevolutionizeWarfare => (Family::UnitUpgrades, 3),
        O::DiversifyResearch => (Family::Colours { per_colour: 2 }, 2),
        O::MasterTheSciences => (Family::Colours { per_colour: 2 }, 4),
        O::BuildDefenses => (Family::Structures, 4),
        O::ConstructMassiveCities => (Family::Structures, 7),
        O::ImproveInfrastructure => (Family::StructuresAway, 3),
        O::ProtectTheBorder => (Family::StructuresAway, 5),
        O::RaiseAFleet => (Family::FleetInOneSystem, 5),
        O::CommandAnArmada => (Family::FleetInOneSystem, 8),
        O::ExploreDeepSpace => (Family::PlanetlessSystems, 3),
        O::PatrolVastTerritories => (Family::PlanetlessSystems, 5),
        O::ReclaimAncientMonuments => (Family::AttachedPlanets, 3),
        O::DiscoverLostOutposts => (Family::AttachedPlanets, 2),
        O::MakeHistory => (Family::NotableSystems, 2),
        O::BecomeALegend => (Family::NotableSystems, 4),
        _ => return None,
    })
}

/// The row of a "remaining position" objective (`objectives::remaining_position_progress`).
#[must_use]
pub const fn remaining_row(obj: PublicObjective) -> Option<(Family, usize)> {
    use PublicObjective as O;
    Some(match obj {
        O::ConquerTheWeak => (Family::RivalHomePlanets, 1),
        O::EngineerAMarvel => (Family::CapitalShipSystems, 1),
        O::AchieveSupremacy => (Family::CapitalShipsInRivalHomeOrMecatol, 1),
        O::IntimidateCouncil => (Family::ShipsAdjacentToMecatol, 2),
        O::PushBoundaries => (Family::WeakerNeighbours, 2),
        O::RuleDistantLands => (Family::DistinctRivalHomeReaches, 2),
        _ => return None,
    })
}

/// `objectives::cost_of`.
#[must_use]
pub const fn cost_of(obj: PublicObjective) -> Option<Cost> {
    use PublicObjective as O;
    Some(match obj {
        O::ErectAMonument => Cost::Spend { amount: 8, kind: Spend::Resources },
        O::FoundAGoldenAge => Cost::Spend { amount: 16, kind: Spend::Resources },
        O::SwayTheCouncil => Cost::Spend { amount: 8, kind: Spend::Influence },
        O::ManipulateGalacticLaw => Cost::Spend { amount: 16, kind: Spend::Influence },
        O::NegotiateTradeRoutes => Cost::TradeGoods(5),
        O::CentralizeGalacticTrade => Cost::TradeGoods(10),
        O::LeadFromTheFront => Cost::Tokens(3),
        O::GalvanizeThePeople => Cost::Tokens(6),
        O::AmassWealth => Cost::AllThree(3),
        O::HoldVastReserves => Cost::AllThree(6),
        _ => return None,
    })
}

/// Points of a public objective (`objectives::points_for`; 1 = stage I, 2 = stage II).
#[must_use]
pub fn points_of(obj: PublicObjective) -> u8 {
    obj.data().points
}

/// `objectives::stage_of`: stage one or two from the printed points.
#[must_use]
pub fn stage_of(obj: PublicObjective) -> Option<u8> {
    match points_of(obj) {
        1 => Some(1),
        2 => Some(2),
        _ => None,
    }
}

fn non_home(pos: &Pos<'_>) -> usize {
    let mut n = 0;
    pos.controlled_each(|p| n += usize::from(p.data().homeworld_of.is_none()));
    n
}

fn on_the_rim(pos: &Pos<'_>, edge: &SysSet) -> Option<usize> {
    if !pos.map {
        return None;
    }
    let held = pos.systems_holding_units();
    Some(held.iter().filter(|s| edge.contains(*s) && Some(crate::ids::system_at(*s)) != pos.facts.home).count())
}

fn same_trait(pos: &Pos<'_>) -> usize {
    let mut counts = [0usize; 3];
    pos.controlled_each(|p| {
        // A dual-trait planet counts toward both of its traits.
        for t in p.data().traits {
            match t {
                PlanetTrait::Cultural => counts[0] += 1,
                PlanetTrait::Hazardous => counts[1] += 1,
                PlanetTrait::Industrial => counts[2] += 1,
                _ => {}
            }
        }
    });
    counts.into_iter().max().unwrap_or(0)
}

fn tech_specialties(pos: &Pos<'_>) -> usize {
    let mut n = 0;
    pos.controlled_each(|p| {
        // A research facility gives a planet without a specialty its specialty.
        let printed = !p.data().specialties.is_empty();
        if printed || board::attachments(pos.core, p).as_slice().iter().any(|a| !a.data().specialties.is_empty()) {
            n += 1;
        }
    });
    n
}

fn colours(pos: &Pos<'_>, per_colour: usize) -> usize {
    [TechType::Biotic, TechType::Cybernetic, TechType::Propulsion, TechType::Warfare]
        .into_iter()
        .filter(|c| pos.technology_types(*c) >= per_colour)
        .count()
}

fn structures_away(pos: &Pos<'_>) -> usize {
    let mut planets: Set<Planet, 3> = Set::new();
    for (system, planet) in pos.structure_entries().as_slice() {
        if Some(*system) != pos.facts.home {
            planets.insert(*planet);
        }
    }
    planets.len()
}

fn fleet_in_one_system(pos: &Pos<'_>) -> usize {
    // The max over the board's entries of the real ships plus one for an imagined presence.
    let mut best = 0usize;
    for (system, n) in pos.facts.fleet.as_slice() {
        best = best.max(usize::from(*n) + usize::from(pos.imagined_in(*system)));
    }
    if pos.imagined.systems.iter().any(|s| pos.facts.touched.contains(s.index())) {
        best = best.max(1);
    }
    best
}

fn planetless_systems(pos: &Pos<'_>) -> usize {
    let scope = pos.scope();
    pos.systems_holding_units()
        .iter()
        .filter(|s| scope.system(crate::ids::system_at(*s)).is_some_and(|d| d.planets.is_empty()))
        .count()
}

fn attached_planets(pos: &Pos<'_>) -> usize {
    // Real control from the board. The imagined half of the string function reads display names as
    // ids and adds nothing.
    pos.facts.raw.as_slice().iter().filter(|(_, p)| !board::attachments(pos.core, *p).is_empty()).count()
}

fn in_notable_systems(pos: &Pos<'_>) -> usize {
    let scope = pos.scope();
    pos.systems_holding_units()
        .iter()
        .filter(|s| {
            let system = crate::ids::system_at(*s);
            if is_mecatol(system) {
                return true;
            }
            let Some(d) = scope.system(system) else { return false };
            let a = &d.anomalies;
            a.nebula
                || a.supernova
                || a.scar
                || a.asteroid_field
                || a.gravity_rift
                || d.planets.iter().any(|p| in_scope_planet(scope, *p) && board::is_legendary(pos.core, *p))
        })
        .count()
}

/// `rival_home_planets`: the home planets of everyone but the seat.
fn rival_home_planets(pos: &Pos<'_>) -> Set<Planet, 3> {
    let mut out = Set::new();
    for player in pos.core.players().iter() {
        if player.seat == pos.seat() {
            continue;
        }
        if !player.home_planets.is_empty() {
            for p in player.home_planets.as_slice() {
                out.insert(*p);
            }
            continue;
        }
        for p in player.faction.data().home_planets {
            out.insert(*p);
        }
    }
    out
}

/// `rival_home_systems`.
fn rival_home_systems(pos: &Pos<'_>) -> SysSet {
    let mut out = SysSet::EMPTY;
    for player in pos.core.players().iter() {
        if player.seat == pos.seat() {
            continue;
        }
        if let Some(home) = player.home_system.or_else(|| player.faction.data().home_system) {
            out.insert(home.index());
        }
    }
    out
}

fn rival_home_planets_count(pos: &Pos<'_>) -> usize {
    let rivals = rival_home_planets(pos);
    let mut n = 0;
    pos.controlled_each(|p| n += usize::from(rivals.contains(p)));
    n
}

fn capital_ships_in_rival_home_or_mecatol(pos: &Pos<'_>) -> usize {
    let mut theirs = rival_home_systems(pos);
    theirs.insert(mecatol_on(pos.core).index());
    pos.facts.capital_systems.iter().filter(|s| theirs.contains(*s)).count()
}

fn ships_adjacent_to_mecatol(pos: &Pos<'_>) -> Option<usize> {
    if !pos.map {
        return None;
    }
    let g = pos.core.galaxy()?;
    let beside = g.adjacent(mecatol_in_galaxy(pos.core));
    Some(pos.systems_with_ships().iter().filter(|s| beside.contains(*s)).count())
}

/// `objectives::neighbour_planet_counts`, reduced to what Push Boundaries counts: rows ahead.
fn weaker_neighbours(pos: &Pos<'_>) -> Option<usize> {
    if !pos.map {
        return None;
    }
    let mine = pos.controlled_len();
    let rows = neighbours(pos.core, &pos.facts.terms, pos.seat());
    Some(
        rows.as_slice()
            .iter()
            .filter(|other| mine > scoring_planets(pos.core, pos.scope(), **other).len())
            .count(),
    )
}

fn distinct_rival_home_reaches(pos: &Pos<'_>) -> Option<usize> {
    if !pos.map {
        return None;
    }
    let g = pos.core.galaxy()?;
    let mut held = SysSet::EMPTY;
    for (system, _) in pos.facts.raw.as_slice() {
        held.insert(system.index());
    }
    let mut n = 0;
    for player in pos.core.players().iter() {
        if player.seat == pos.seat() {
            continue;
        }
        let Some(home) = player.home_system.or_else(|| player.faction.data().home_system) else { continue };
        let mut reach = g.adjacent(home);
        reach.insert(home.index());
        if held.intersects(&reach) {
            n += 1;
        }
    }
    Some(n)
}

/// `objectives::counting_progress`. `edge` is [`edge_systems`] of the position (the caller caches it).
#[must_use]
pub fn counting_progress(pos: &Pos<'_>, edge: &SysSet, obj: PublicObjective) -> Option<Progress> {
    let (family, threshold) = counting_row(obj)?;
    let have = match family {
        Family::NonHome => non_home(pos),
        Family::OnTheRim => on_the_rim(pos, edge)?,
        Family::SameTrait => same_trait(pos),
        Family::TechSpecialties => tech_specialties(pos),
        Family::UnitUpgrades => pos.technology_types(TechType::Unitupgrade),
        Family::Colours { per_colour } => colours(pos, per_colour),
        Family::Structures => pos.structure_entries().len(),
        Family::StructuresAway => structures_away(pos),
        Family::FleetInOneSystem => fleet_in_one_system(pos),
        Family::PlanetlessSystems => planetless_systems(pos),
        Family::AttachedPlanets => attached_planets(pos),
        Family::NotableSystems => in_notable_systems(pos),
        _ => return None,
    };
    Some(Progress { family, have, threshold })
}

/// `objectives::remaining_position_progress`.
#[must_use]
pub fn remaining_position_progress(pos: &Pos<'_>, obj: PublicObjective) -> Option<Progress> {
    let (family, threshold) = remaining_row(obj)?;
    let have = match family {
        Family::RivalHomePlanets => rival_home_planets_count(pos),
        Family::CapitalShipSystems => pos.facts.capital_systems.len(),
        Family::CapitalShipsInRivalHomeOrMecatol => capital_ships_in_rival_home_or_mecatol(pos),
        Family::ShipsAdjacentToMecatol => ships_adjacent_to_mecatol(pos)?,
        Family::WeakerNeighbours => weaker_neighbours(pos)?,
        Family::DistinctRivalHomeReaches => distinct_rival_home_reaches(pos)?,
        _ => return None,
    };
    Some(Progress { family, have, threshold })
}

// --- bought objectives ----------------------------------------------------------------------------------

/// `payment::MAX_PLANETS_PER_PLAN`.
const MAX_PLANETS_PER_PLAN: usize = 6;

/// `payment::Plan`: the planets to exhaust (in the order `payment::plans` lists them) and the trade goods to
/// spend (commodities first, see `kernel::fx_pay`).
#[derive(Debug, Clone, Copy)]
pub struct PlanC {
    pub trade_goods: i32,
    n: u8,
    planets: [Planet; MAX_PLANETS_PER_PLAN],
}

impl PlanC {
    /// The planets to exhaust.
    #[must_use]
    pub fn planets(&self) -> &[Planet] {
        &self.planets[..usize::from(self.n)]
    }
}

/// The planets that can pay `kind` with their worth, `payment::plans`' `useful` list; `None` while
/// the seat holds cards the core has no planets for (ocean cards).
pub fn useful_planets(core: &CoreState, f: &SeatFacts, kind: Spend) -> Option<SmallList<(Planet, i64), 8>> {
    if f.terms.oceans {
        return None;
    }
    let mut useful: SmallList<(Planet, i64), 8> = SmallList::new();
    for (_, planet) in economy::spendable_planets(core, f.seat).as_slice() {
        let helios = match kind {
            Spend::Resources => f.terms.helios.as_slice().iter().find(|(p, _)| p == planet).map_or(0, |(_, v)| *v),
            Spend::Influence => 0,
        };
        let worth = economy::value_now(core, f.scope, f.terms.blanked, *planet, kind, helios);
        if worth > 0 {
            useful.push((*planet, worth));
        }
    }
    useful.as_mut_slice().sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    useful.truncate(MAX_PLANETS_PER_PLAN);
    Some(useful)
}

/// `payment::plans`, sorted as the string engine sorts them.
pub fn plans(useful: &[(Planet, i64)], cost: i64, goods: i64, per_good: i64) -> SmallList<PlanC, 64> {
    let mut found: SmallList<PlanC, 64> = SmallList::new();
    let count = useful.len();
    for mask in 0u32..(1u32 << count) {
        let mut plan = PlanC { trade_goods: 0, n: 0, planets: [Planet::from_index(0).expect("a planet exists"); MAX_PLANETS_PER_PLAN] };
        let mut from_planets = 0i64;
        let mut last = 0i64;
        for (index, (planet, worth)) in useful.iter().enumerate() {
            if mask & (1 << index) != 0 {
                plan.planets[usize::from(plan.n)] = *planet;
                plan.n += 1;
                from_planets += worth;
                last = *worth;
            }
        }
        if plan.n > 0 && from_planets >= cost + last {
            continue; // the last planet was not needed
        }
        let shortfall = (cost - from_planets).max(0);
        if shortfall > goods {
            continue;
        }
        plan.trade_goods = i32::try_from((shortfall + per_good - 1) / per_good).unwrap_or(i32::MAX);
        found.push(plan);
    }
    found.as_mut_slice().sort_by(|a, b| {
        a.trade_goods
            .cmp(&b.trade_goods)
            .then_with(|| a.n.cmp(&b.n))
            .then_with(|| a.planets[..usize::from(a.n)].cmp(&b.planets[..usize::from(b.n)]))
    });
    found
}

/// The eligible token pool of a payment: strategy then tactic tokens, never fleet (`TOKEN_COST_POOLS`).
#[must_use]
pub fn token_pool(core: &CoreState, seat: Seat) -> i32 {
    core.players().by_seat(seat).map_or(0, |p| i32::from(p.econ.strategic_tokens) + i32::from(p.econ.tactic_tokens))
}

/// `objectives::can_afford` for one scaled cost. `useful_*` are the spendable lists of the two kinds
/// (built once per progress call, they do not depend on the amount).
fn can_afford(
    core: &CoreState,
    f: &SeatFacts,
    cost: Cost,
    useful: &Useful,
) -> Option<bool> {
    let player = core.players().by_seat(f.seat);
    Some(match cost {
        Cost::Spend { amount, kind } => {
            if amount <= 0 {
                return Some(true);
            }
            let list = useful.of(kind)?;
            !plans(list.as_slice(), amount, goods_of(core, f, f.terms.per_good), f.terms.per_good).is_empty()
        }
        Cost::TradeGoods(amount) => player.is_some_and(|p| i32::from(p.econ.trade_goods) >= amount),
        Cost::Tokens(amount) => player.is_some() && token_pool(core, f.seat) >= amount,
        Cost::AllThree(amount) => {
            player?;
            let res = useful.of(Spend::Resources)?;
            let inf = useful.of(Spend::Influence)?;
            all_three_from(core, f, res.as_slice(), inf.as_slice(), amount).is_some()
        }
    })
}

/// `objectives::all_three_plan` over the two spendable lists: the first disjoint pair of a resources plan and
/// an influence plan whose trade goods, plus the printed `amount`, the seat holds (plain trade goods: the
/// string compares with `trade_goods` alone, commodities only pay inside the plans).
fn all_three_from(core: &CoreState, f: &SeatFacts, res: &[(Planet, i64)], inf: &[(Planet, i64)], amount: i64) -> Option<(PlanC, PlanC)> {
    let held = i64::from(core.players().by_seat(f.seat)?.econ.trade_goods);
    let g = goods_of(core, f, f.terms.per_good);
    let resources = plans(res, amount, g, f.terms.per_good);
    let influence = plans(inf, amount, g, f.terms.per_good);
    for r in resources.as_slice() {
        for i in influence.as_slice() {
            let disjoint = !r.planets().iter().any(|p| i.planets().contains(p));
            if !disjoint {
                continue;
            }
            let goods_needed = i64::from(r.trade_goods) + i64::from(i.trade_goods) + amount;
            if goods_needed <= held {
                return Some((*r, *i));
            }
        }
    }
    None
}

/// The trade goods a payment may count, each worth `per` (commodities a Keleres agent made spendable included).
fn goods_of(core: &CoreState, f: &SeatFacts, per: i64) -> i64 {
    (core.players().by_seat(f.seat).map_or(0, |p| i64::from(p.econ.trade_goods)) + f.terms.commodities) * per
}

/// The first plan `payment::plans` lists for `amount` of `kind` (`objectives::pay_for` pays this one). `None`
/// when no plan pays it, and while the seat holds ocean cards (the string pays with those too).
#[must_use]
pub fn first_plan(core: &CoreState, f: &SeatFacts, kind: Spend, amount: i64) -> Option<PlanC> {
    let list = useful_planets(core, f, kind)?;
    plans(list.as_slice(), amount, goods_of(core, f, f.terms.per_good), f.terms.per_good).as_slice().first().copied()
}

/// `objectives::all_three_plan`. `None` when no pair pays, and while the seat holds ocean cards.
#[must_use]
pub fn all_three_plan(core: &CoreState, f: &SeatFacts, amount: i64) -> Option<(PlanC, PlanC)> {
    let res = useful_planets(core, f, Spend::Resources)?;
    let inf = useful_planets(core, f, Spend::Influence)?;
    all_three_from(core, f, res.as_slice(), inf.as_slice(), amount)
}

struct Useful {
    resources: Option<SmallList<(Planet, i64), 8>>,
    influence: Option<SmallList<(Planet, i64), 8>>,
}

impl Useful {
    fn of(&self, kind: Spend) -> Option<&SmallList<(Planet, i64), 8>> {
        match kind {
            Spend::Resources => self.resources.as_ref(),
            Spend::Influence => self.influence.as_ref(),
        }
    }
}

fn scaled(family: CostFamily, amount: i64) -> Cost {
    match family {
        CostFamily::Spend(kind) => Cost::Spend { amount, kind },
        CostFamily::TradeGoods => Cost::TradeGoods(i32::try_from(amount).unwrap_or(i32::MAX)),
        CostFamily::Tokens => Cost::Tokens(i32::try_from(amount).unwrap_or(i32::MAX)),
        CostFamily::AllThree => Cost::AllThree(amount),
    }
}

/// `objectives::bought_progress` (no imagined planets). `None` for an objective without a price, and
/// while a hook holds cards the core cannot express.
#[must_use]
pub fn bought_progress_base(core: &CoreState, f: &SeatFacts, obj: PublicObjective) -> Option<CostProgress> {
    let cost = cost_of(obj)?;
    let (family, target) = match cost {
        Cost::Spend { amount, kind } => (CostFamily::Spend(kind), amount),
        Cost::TradeGoods(amount) => (CostFamily::TradeGoods, i64::from(amount)),
        Cost::Tokens(amount) => (CostFamily::Tokens, i64::from(amount)),
        Cost::AllThree(amount) => (CostFamily::AllThree, amount),
    };
    if target <= 0 {
        return None;
    }
    let useful = Useful {
        resources: match family {
            CostFamily::Spend(Spend::Resources) | CostFamily::AllThree => useful_planets(core, f, Spend::Resources),
            _ => None,
        },
        influence: match family {
            CostFamily::Spend(Spend::Influence) | CostFamily::AllThree => useful_planets(core, f, Spend::Influence),
            _ => None,
        },
    };
    // The spend families need their list; a seat with ocean cards cannot be answered here.
    match family {
        CostFamily::Spend(kind) if useful.of(kind).is_none() => return None,
        CostFamily::AllThree if useful.resources.is_none() || useful.influence.is_none() => return None,
        _ => {}
    }
    let mut have = 0;
    for amount in (0..=target).rev() {
        if can_afford(core, f, scaled(family, amount), &useful)? {
            have = amount;
            break;
        }
    }
    Some(CostProgress { family, have, target })
}

/// The resources or influence the imagined planets add (`Position::imagined_spending_bonus`): the
/// planets of the scoring view the seat does not really control (coexisting ones included).
fn imagined_spending_bonus(pos: &Pos<'_>, kind: Spend) -> i64 {
    let mut total = 0;
    let real = &pos.facts.raw;
    pos.controlled_each(|p| {
        if !real.as_slice().iter().any(|(_, r)| *r == p) {
            let d = p.data();
            total += i64::from(match kind {
                Spend::Resources => d.resources,
                Spend::Influence => d.influence,
            });
        }
    });
    total
}

/// `objectives::bought_progress_at`.
#[must_use]
pub fn bought_progress(pos: &Pos<'_>, obj: PublicObjective) -> Option<CostProgress> {
    let base = bought_progress_base(pos.core, pos.facts, obj)?;
    let bonus = match base.family {
        CostFamily::Spend(kind) => imagined_spending_bonus(pos, kind),
        CostFamily::AllThree | CostFamily::TradeGoods | CostFamily::Tokens => 0,
    };
    if bonus <= 0 {
        return Some(base);
    }
    Some(CostProgress { family: base.family, have: (base.have + bonus).min(base.target), target: base.target })
}

/// Whether a faction record exists for a seat (used by callers that need the string fallback).
#[must_use]
pub fn faction_of(core: &CoreState, seat: Seat) -> Option<Faction> {
    core.players().by_seat(seat).map(|p| p.faction)
}

// --- the secret objectives ---------------------------------------------------------------------------

/// A secret whose progress the core does not answer (it needs state outside the slices the view
/// loads): the string engine answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unsupported;

/// `secrets::counting_progress` and `remaining_position_progress` rows: family and threshold of the
/// position secrets (`fc` takes its threshold from the table). Strengthen Bonds reads the promissory notes of the position
/// ([`rival_note_issuers`]); Produce en Masse reads [`Terms::capacity`], which the caller fills (`Unsupported` while it did not).
fn secret_row(obj: SecretObjective) -> Result<Option<(Family, usize)>, Unsupported> {
    use SecretObjective as S;
    Ok(Some(match obj {
        S::EstablishAPerimeter => (Family::Units(UnitBaseType::Pds), 4),
        S::FuelTheWarMachine => (Family::Units(UnitBaseType::Spacedock), 3),
        S::GatherAMightyFleet => (Family::Units(UnitBaseType::Dreadnought), 5),
        S::ControlTheRegion => (Family::ShipsInSystems, 6),
        S::ForgeAnAlliance => (Family::PlanetsOfTrait(PlanetTrait::Cultural), 4),
        S::MineRareMetals => (Family::PlanetsOfTrait(PlanetTrait::Hazardous), 4),
        S::MonopolizeProduction => (Family::PlanetsOfTrait(PlanetTrait::Industrial), 4),
        S::MasterTheLawsOfPhysics => (Family::SameColourTechnologies, 4),
        S::MechanizeTheMilitary => (Family::MechsOnDistinctPlanets, 4),
        S::OccupyTheFringe => (Family::GroundForcesOnOnePlanet, 9),
        S::CutSupplyLines => (Family::RivalDockSystemsWithShips, 1),
        S::DestroyHereticalWorks => (Family::RelicFragments, 2),
        S::FormASpyNetwork => (Family::ActionCards, 5),
        S::LearnTheSecretsOfTheCosmos => (Family::ShipSystemsBesideAnomaly, 3),
        S::ThreatenEnemies => (Family::ShipSystemsBesideRivalHome, 1),
        S::FosterCohesion => (Family::Neighbours, 0),
        S::DefySpaceAndTime => (Family::UnitsInNexus, 1),
        S::BecomeTheGatekeeper => (Family::WormholeKinds, 2),
        S::AdaptNewStrategies => (Family::FactionTechnologies, 2),
        S::DictatePolicy => (Family::Laws, 3),
        S::StakeYourClaim => (Family::SharedPlanetSystems, 1),
        S::SeizeAnIcon => (Family::LegendaryPlanets, 1),
        S::OccupyTheSeatOfTheEmpire => (Family::MecatolShipsWhileControlling, 3),
        S::EstablishHegemony => (Family::CombinedValue(Spend::Influence), 12),
        S::HoardRawMaterials => (Family::CombinedValue(Spend::Resources), 12),
        S::StrengthenBonds => (Family::RivalNoteIssuers, 1),
        S::ProduceEnMasse => (Family::ProductionInOneSystem, 8),
        _ => return Ok(None),
    }))
}

/// `secrets::Position::controlled`: the planets the seat holds, those the option would take and the
/// Firmament commander's, deduplicated. Stations and planets out of scope are in it (the counts
/// that care filter by scope themselves).
fn secret_controlled(pos: &Pos<'_>) -> Set<Planet, 3> {
    let mut held: Set<Planet, 3> = Set::new();
    for (_, p) in pos.facts.raw.as_slice() {
        held.insert(*p);
    }
    for p in pos.imagined.planets {
        held.insert(*p);
    }
    for p in pos.facts.terms.firmament.iter() {
        held.insert(p);
    }
    held
}

fn ship_systems_real(pos: &Pos<'_>) -> SysSet {
    pos.facts.ship_systems
}

/// `secrets::Position::systems_with_ships`: the real systems and the imagined ones, once each.
fn secret_ships_in_systems(pos: &Pos<'_>) -> usize {
    pos.systems_with_ships().len()
}

fn fragments(core: &CoreState, seat: Seat) -> usize {
    core.players().by_seat(seat).map_or(0, |p| {
        let f = &p.cards.fragments;
        crate::generated::ExploreType::ALL
            .iter()
            .filter_map(|k| f.get(*k))
            .filter(|n| *n > 0)
            .map(|n| usize::try_from(n).unwrap_or(0))
            .sum()
    })
}

fn rival_dock_systems_with_ships(pos: &Pos<'_>) -> usize {
    let scope = pos.scope();
    ship_systems_real(pos)
        .iter()
        .filter(|s| {
            let system = crate::ids::system_at(*s);
            board::planets_of(pos.core, system).any(|planet| {
                board::planet_units(pos.core, planet).iter().any(|u| {
                    u.owner() != pos.seat()
                        && scope.unit(u.kind()).is_some_and(|d| d.base_type == UnitBaseType::Spacedock)
                })
            })
        })
        .count()
}

fn ship_systems_beside_anomaly(pos: &Pos<'_>) -> Option<usize> {
    if !pos.map {
        return None;
    }
    let g = pos.core.galaxy()?;
    let scope = pos.scope();
    Some(
        ship_systems_real(pos)
            .iter()
            .filter(|s| {
                g.adjacent(crate::ids::system_at(*s)).iter().any(|other| {
                    scope.system(crate::ids::system_at(other)).is_some_and(|d| {
                        let a = &d.anomalies;
                        a.nebula || a.supernova || a.scar || a.asteroid_field || a.gravity_rift
                    })
                })
            })
            .count(),
    )
}

fn ship_systems_beside_rival_home(pos: &Pos<'_>) -> Option<usize> {
    if !pos.map {
        return None;
    }
    let g = pos.core.galaxy()?;
    let scope = pos.scope();
    let mine = pos.facts.terms.faction_tag;
    Some(
        ship_systems_real(pos)
            .iter()
            .filter(|s| {
                g.adjacent(crate::ids::system_at(*s)).iter().any(|other| {
                    scope.system(crate::ids::system_at(other)).is_some_and(|d| {
                        d.planets.iter().any(|planet| {
                            in_scope_planet(scope, *planet)
                                && planet.data().homeworld_of.is_some_and(|tag| Some(tag) != mine)
                        })
                    })
                })
            })
            .count(),
    )
}

fn neighbour_progress(pos: &Pos<'_>) -> Option<(usize, usize)> {
    if !pos.map {
        return None;
    }
    let others = pos.core.players().iter().filter(|p| p.seat != pos.seat()).count();
    if others == 0 {
        return None;
    }
    let reached = neighbours(pos.core, &pos.facts.terms, pos.seat());
    let have = pos
        .core
        .players()
        .iter()
        .filter(|p| p.seat != pos.seat() && reached.as_slice().contains(&p.seat))
        .count();
    Some((have, others))
}

fn units_in_nexus(pos: &Pos<'_>) -> usize {
    let scope = pos.scope();
    [System::MalliceLockedNexus, System::MalliceNexus]
        .into_iter()
        .filter(|s| scope.system(*s).is_some())
        .map(|s| board::space_units(pos.core, s).iter().filter(|u| u.owner() == pos.seat()).count())
        .sum()
}

fn wormhole_kinds(pos: &Pos<'_>) -> usize {
    let scope = pos.scope();
    let mut kinds = 0u8;
    for s in ship_systems_real(pos).iter() {
        let system = crate::ids::system_at(s);
        if scope.system(system).is_some() {
            kinds |= crate::tables::printed_wormholes(system);
        }
        if pos.map
            && let Some(g) = pos.core.galaxy()
        {
            kinds |= g.wormhole_kinds(system);
        }
    }
    (kinds & crate::tables::WH_ALPHA_BETA).count_ones() as usize
}

fn shared_planet_systems(pos: &Pos<'_>) -> usize {
    let firm = &pos.facts.terms.firmament;
    let (mut mine, mut theirs) = (SysSet::EMPTY, SysSet::EMPTY);
    for (planet, cell) in pos.core.board().planets() {
        let (Some(owner), Some(system)) = (cell.control, board::home_system(pos.core, planet)) else { continue };
        if owner == pos.seat() || firm.contains(planet) {
            mine.insert(system.index());
        }
        if owner != pos.seat() && !firm.contains(planet) {
            theirs.insert(system.index());
        }
    }
    mine.iter().filter(|s| theirs.contains(*s)).count()
}

fn mecatol_ships_while_controlling(pos: &Pos<'_>) -> usize {
    let mecatol = mecatol_on(pos.core);
    let firm = &pos.facts.terms.firmament;
    let controls = board::controls_a_planet_in(pos.core, mecatol, pos.seat())
        || board::planets_of(pos.core, mecatol).any(|p| board::control(pos.core, p).is_some() && firm.contains(p));
    if !controls {
        return 0;
    }
    let scope = pos.scope();
    board::space_units(pos.core, mecatol)
        .iter()
        .filter(|u| u.owner() == pos.seat() && scope.unit(u.kind()).is_some_and(|d| d.ship))
        .count()
}

fn combined_value(pos: &Pos<'_>, kind: Spend) -> i64 {
    let scope = pos.scope();
    secret_controlled(pos).iter().map(|p| economy::printed_value(scope, p, kind)).sum()
}

/// `secrets::progress`: `counting_progress` or `remaining_position_progress` of a held secret.
///
/// # Errors
/// [`Unsupported`] for a secret the core does not answer, and while a hook term could not be mapped.
pub fn secret_progress(pos: &Pos<'_>, obj: SecretObjective) -> Result<Option<Progress>, Unsupported> {
    let Some((family, mut threshold)) = secret_row(obj)? else { return Ok(None) };
    if pos.facts.terms.unmapped {
        return Err(Unsupported);
    }
    let scope = pos.scope();
    let have = match family {
        Family::Units(base) => usize::from(pos.facts.by_base[base.index()]),
        Family::ShipsInSystems => secret_ships_in_systems(pos),
        Family::PlanetsOfTrait(t) => secret_controlled(pos)
            .iter()
            .filter(|p| in_scope_planet(scope, *p) && p.data().traits.contains(&t))
            .count(),
        Family::SameColourTechnologies => usize::from(pos.facts.colours.into_iter().max().unwrap_or(0)),
        Family::MechsOnDistinctPlanets => pos.facts.mech_planets.len(),
        Family::GroundForcesOnOnePlanet => usize::from(pos.facts.ground_max),
        Family::RivalDockSystemsWithShips => rival_dock_systems_with_ships(pos),
        Family::RelicFragments => fragments(pos.core, pos.seat()),
        Family::ActionCards => pos.core.players().by_seat(pos.seat()).map_or(0, |p| p.cards.hand.len()),
        Family::ShipSystemsBesideAnomaly => match ship_systems_beside_anomaly(pos) {
            Some(n) => n,
            None => return Ok(None),
        },
        Family::ShipSystemsBesideRivalHome => match ship_systems_beside_rival_home(pos) {
            Some(n) => n,
            None => return Ok(None),
        },
        Family::Neighbours => match neighbour_progress(pos) {
            Some((have, need)) => {
                threshold = need;
                have
            }
            None => return Ok(None),
        },
        Family::UnitsInNexus => units_in_nexus(pos),
        Family::WormholeKinds => wormhole_kinds(pos),
        Family::FactionTechnologies => pos.facts.owned.iter().filter(|t| t.data().faction.is_some()).count(),
        Family::Laws => pos.core.agenda().laws.len(),
        Family::SharedPlanetSystems => shared_planet_systems(pos),
        Family::LegendaryPlanets => {
            secret_controlled(pos).iter().filter(|p| in_scope_planet(scope, *p) && board::is_legendary(pos.core, *p)).count()
        }
        Family::MecatolShipsWhileControlling => mecatol_ships_while_controlling(pos),
        Family::CombinedValue(kind) => usize::try_from(combined_value(pos, kind)).unwrap_or(0),
        Family::RivalNoteIssuers => rival_note_issuers(pos.core, pos.seat()),
        // `production_in_one_system_count`: the most PRODUCTION the seat has in any one system (the string's board entries; a system with none has 0).
        Family::ProductionInOneSystem if pos.facts.terms.capacity_known => {
            usize::try_from(pos.facts.terms.capacity.as_slice().iter().map(|(_, c)| *c).max().unwrap_or(0)).unwrap_or(0) // mutants:skip: a capacity is never negative
        }
        Family::ProductionInOneSystem => return Err(Unsupported),
        _ => return Ok(None),
    };
    Ok(Some(Progress { family, have, threshold }))
}

/// Whether the seat's progress on a secret reads [`Terms::capacity`]: Produce en Masse in its hand, or revealed (a leaked secret is scored by whoever meets it).
#[must_use]
pub fn wants_capacity(core: &CoreState, seat: Seat) -> bool {
    let mine = core.players().by_seat(seat).is_some_and(|p| p.cards.secrets.contains(&SecretObjective::ProduceEnMasse));
    mine || core.objectives().revealed.iter().any(|r| matches!(r, ObjectiveRef::Secret(SecretObjective::ProduceEnMasse)))
}

/// `secrets::rival_note_issuers_count` (Strengthen Bonds): how many other issuers have a promissory note in the seat's play area. The Support for the
/// Throne lent to the seat counts for its owner; a note placed faceup counts for the seat playing the faction that issued it (the first one, when
/// two play it; the faction itself when nobody does), never for the seat's own faction. Each issuer counts once.
#[must_use]
pub fn rival_note_issuers(core: &CoreState, seat: Seat) -> usize {
    #[derive(PartialEq, Eq, PartialOrd, Ord)]
    enum Issuer {
        Seat(Seat),
        Faction(Faction),
    }
    let mut issuers: Vec<Issuer> = Vec::new();
    for (owner, holder) in core.trade().support_holders.iter() {
        if *holder == seat && *owner != seat {
            issuers.push(Issuer::Seat(*owner));
        }
    }
    let own = core.players().by_seat(seat).map(|p| p.faction);
    for (key, holder) in core.cards().promissory.iter() {
        if *holder == seat && core.cards().promissory_faceup.contains_key(key) && own != Some(key.faction) {
            issuers.push(core.players().iter().find(|p| p.faction == key.faction).map_or(Issuer::Faction(key.faction), |p| Issuer::Seat(p.seat)));
        }
    }
    issuers.sort_unstable();
    issuers.dedup();
    issuers.len()
}

/// The rules version from which a secret made public keeps its printed timing (`ti4_model::state::RULES_LEAKED_EVENT_SECRET`).
pub const RULES_LEAKED_EVENT_SECRET: u32 = 24;

/// `secrets::feat_for`: the feat that satisfies an event secret (recorded for the occurrence of its window).
#[must_use]
pub const fn feat_of(obj: SecretObjective) -> Option<crate::state::Feat> {
    use crate::state::Feat as F;
    use SecretObjective as S;
    Some(match obj {
        S::DestroyTheirGreatestShip => F::DestroyedACapitalShip,
        S::MakeAnExampleOfTheirWorld => F::BombardedOutTheLastGroundForces,
        S::TurnTheirFleetsToDust => F::SpaceCannonTookTheLastNonFighters,
        S::FightWithPrecision => F::BarrageTookTheLastFighters,
        S::SparkARebellion => F::WonAgainstThePointsLeader,
        S::UnveilFlagship => F::WonBesideASurvivingFlagship,
        S::BetrayAFriend => F::WonAgainstANoteHolder,
        S::BraveTheVoid => F::WonInAnAnomaly,
        S::DarkenTheSkies => F::WonInARivalHome,
        S::DemonstrateYourPower => F::HeldThreeShipsAfterASpaceCombat,
        S::DriveTheDebate => F::ElectedByAnAgenda,
        S::ProveEndurance => F::LastToPass,
        S::BecomeAMartyr => F::LostAHomePlanet,
        _ => return None,
    })
}

/// The printed timing of a secret (`secrets::timing`): when it may be scored.
#[must_use]
pub fn secret_points(obj: SecretObjective) -> u8 {
    obj.data().points
}

/// `secrets::HAND_LIMIT` (45.4): three, scored ones counting.
pub const HAND_LIMIT: usize = 3;

/// What `secrets::enforce_hand_limit` does next for a seat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HandStep {
    /// Within the limit, or every secret in hand is scored and there is nothing to hand back.
    Within,
    /// Over the limit with one unscored secret: it goes back without a question.
    Return(SecretObjective),
    /// Over the limit: which one goes back. `held` is secrets held plus scored.
    Ask { held: u8, hand: SmallList<SecretObjective, 4> },
}

/// `secrets::held_count`: secrets in hand plus the ones already scored.
#[must_use]
pub fn secrets_held(core: &CoreState, seat: Seat) -> usize {
    core.players().by_seat(seat).map_or(0, |p| p.cards.secrets.len()) + core.objectives().scored[usize::from(seat.0)].secret.len()
}

/// The next step of `secrets::enforce_hand_limit` (the Obsidian lifts the limit by one).
#[must_use]
pub fn hand_step(core: &CoreState, seat: Seat) -> HandStep {
    let held = secrets_held(core, seat);
    let hand = core.players().by_seat(seat).map_or_else(SmallList::new, |p| p.cards.secrets.clone());
    if held <= HAND_LIMIT + crate::query::relic::secret_objective_bonus(core, seat) || hand.is_empty() {
        return HandStep::Within;
    }
    match hand.as_slice() {
        [only] => HandStep::Return(*only),
        _ => HandStep::Ask { held: u8::try_from(held).unwrap_or(u8::MAX), hand },
    }
}
