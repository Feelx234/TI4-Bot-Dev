//! The objectives family of the surface (lane B, 8d item 1): the progress and affordability reads of `objectives.rs` of the legacy crate, the
//! `Observed` accessors built on them (`scoreable_public/_secret`, `revealed_objective_progress*`, `objectives_implicating_system`, the held
//! secrets) and the types they speak (`Position`, `Imagined`, `CardProgress`, `Cost`, ...), answered from the TYPED CORE.
//!
//! The adapter over the core's `query::objectives` and `seen`:
//! * [`ObjView`] is one loaded position (`SeenOwned`: the slices of `seen::READS`, the kernel of the source scope) with the per-seat scans
//!   ([`SeatFacts`]) memoised. The hook answers the scans need (the "extras": Firmament planets, what a trade good pays, Keleres commodities,
//!   Deepwrought oceans, the Helios bonus, the production capacity of Produce en Masse) come from the core kernel's hook table, as
//!   `ti4_engine_core::seen` computes them; no faction text is read.
//! * [`Position`] is the legacy `objectives::Position` (public fields, builders): the view is loaded on the first question.
//! * The accessors of `Observed` use the core's own `Seen` over the same view.
//!
//! **A position the core cannot hold** (an id the tables do not know, a table with no faction seated, more than 16 owners, content that is not
//! the embedded corpus) answers the neutral value (`None` / 0 / empty), where the legacy answers from its string rules. The same neutral value
//! answers a priced objective the core cannot decide (a seat with ocean cards) and an id of an imagined set the tables do not know (dropped).
//! Keleres' IIHQ term is empty, as in the core's `Seen` (no Keleres seat can be seated in the lobby).

use std::cell::{OnceCell, RefCell};
use std::rc::Rc;

use ti4_content::ContentStore;
use ti4_content::galaxy::Galaxy;
use ti4_engine_core::generated::{Planet, PlanetTrait, PublicObjective, System, Technology, UnitBaseType};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::query::board;
use ti4_engine_core::query::economy::Spend as CoreSpend;
use ti4_engine_core::query::objectives::{self as q, Pos, SeatFacts, Terms};
use ti4_engine_core::seen::{ProgressRow, RowFamily, Seen};
use ti4_engine_core::state::ObjectiveRef;
use ti4_model::content_types::SourceSet;
use ti4_model::id::{ObjectiveId, PlanetId, PlayerId, SystemId};
use ti4_model::state::GameState;

use super::production::Spend;
use crate::generated::IdStr;
use crate::seen_load::SeenOwned;

pub use crate::kinds::objectives::{SCORE_KIND, VICTORY_TARGET};

// ---- the types (copies of the legacy ones) --------------------------------------------------------------------------------------------

/// What an option would change, for the counterfactual progress a per-option feature differences. Every field is additive over the real
/// board and every one is optional, so a caller names only what its option kind actually does.
#[derive(Debug, Clone, Copy, Default)]
pub struct Imagined<'i> {
    /// Planets the option would bring under control.
    pub planets: &'i [PlanetId],
    /// Systems the option would put a unit in.
    pub systems: &'i [String],
    /// Technology aliases the option would research.
    pub technologies: &'i [String],
    /// Structures the option would place, as (system, planet).
    pub structures: &'i [(String, String)],
}

impl Imagined<'_> {
    /// Nothing imagined: the real board.
    pub const NONE: Imagined<'static> = Imagined { planets: &[], systems: &[], technologies: &[], structures: &[] };

    /// Whether this changes nothing, in which case the imagined position is the real one.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.planets.is_empty() && self.systems.is_empty() && self.technologies.is_empty() && self.structures.is_empty()
    }
}

/// Stable identity for an exact counting requirement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CountFamily {
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
    PlanetsOfTrait { trait_name: &'static str },
    SameColourTechnologies,
    ShipsInSystems,
    Units { base_type: &'static str },
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
    CombinedValue { kind: Spend },
}

/// Exact raw progress toward one counting requirement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequirementProgress {
    pub family: CountFamily,
    pub have: usize,
    pub threshold: usize,
}

impl RequirementProgress {
    #[must_use]
    pub const fn satisfied(&self) -> bool {
        self.have >= self.threshold
    }
}

/// One revealed or held card's exact progress toward its requirement, in the shape the policy feature path consumes.
#[derive(Debug, Clone, PartialEq)]
pub struct CardProgress {
    /// The objective or secret alias the progress belongs to.
    pub alias: String,
    /// Canonical family token from [`family_token`] or [`cost_family_token`].
    pub family_token: String,
    /// Raw count, or greatest exactly affordable amount for bought cards.
    pub have: f64,
    /// Required count or amount; always > 0.
    pub threshold: f64,
    /// Whether the card is satisfied right now (`have >= threshold`).
    pub satisfied: bool,
    /// Printed stage for public objectives (one or two); secrets have none.
    pub stage: Option<u8>,
}

/// What an objective scored by spending costs (61.10).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cost {
    /// Exhaust planets and spend trade goods for resources or influence.
    Spend { amount: i64, kind: Spend },
    /// Spend trade goods alone.
    TradeGoods(i32),
    /// Spend command tokens from any pools.
    Tokens(i32),
    /// Spend this much influence, this many resources and this many trade goods (a planet is exhausted once).
    AllThree(i64),
}

/// Stable identity for a bought-objective affordability family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CostFamily {
    Spend(Spend),
    TradeGoods,
    Tokens,
    AllThree,
}

/// Greatest exactly affordable scaled cost toward a bought objective.
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

/// One neighbour of a seat with both planet counts (Push Boundaries).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NeighbourPlanets {
    pub neighbour: PlayerId,
    /// Planets this player controls, counted as the objective counts them.
    pub mine: usize,
    /// Planets the neighbour controls, counted the same way.
    pub theirs: usize,
}

impl NeighbourPlanets {
    /// Strictly more planets than this neighbour; a tie is not ahead.
    #[must_use]
    pub const fn ahead(&self) -> bool {
        self.mine > self.theirs
    }
}

/// The canonical feature-name token for a counting family. Keying on the family rather than the alias lets learning transfer between cards that
/// share machinery. Stable lowercase words; parameterised variants fold their parameter in (`colours_2`, `planets_of_trait_cultural`).
#[must_use]
pub fn family_token(family: &CountFamily) -> String {
    match family {
        CountFamily::NonHome => "non_home".to_owned(),
        CountFamily::OnTheRim => "on_the_rim".to_owned(),
        CountFamily::SameTrait => "same_trait".to_owned(),
        CountFamily::TechSpecialties => "tech_specialties".to_owned(),
        CountFamily::UnitUpgrades => "unit_upgrades".to_owned(),
        CountFamily::Colours { per_colour } => format!("colours_{per_colour}"),
        CountFamily::Structures => "structures".to_owned(),
        CountFamily::StructuresAway => "structures_away".to_owned(),
        CountFamily::FleetInOneSystem => "fleet_in_one_system".to_owned(),
        CountFamily::PlanetlessSystems => "planetless_systems".to_owned(),
        CountFamily::AttachedPlanets => "attached_planets".to_owned(),
        CountFamily::NotableSystems => "notable_systems".to_owned(),
        CountFamily::GroundForcesOnOnePlanet => "ground_forces_on_one_planet".to_owned(),
        CountFamily::MechsOnDistinctPlanets => "mechs_on_distinct_planets".to_owned(),
        CountFamily::PlanetsOfTrait { trait_name } => format!("planets_of_trait_{trait_name}"),
        CountFamily::SameColourTechnologies => "same_colour_technologies".to_owned(),
        CountFamily::ShipsInSystems => "ships_in_systems".to_owned(),
        CountFamily::Units { base_type } => format!("units_{base_type}"),
        CountFamily::RivalHomePlanets => "rival_home_planets".to_owned(),
        CountFamily::CapitalShipSystems => "capital_ship_systems".to_owned(),
        CountFamily::CapitalShipsInRivalHomeOrMecatol => "capital_ships_rival_or_mecatol".to_owned(),
        CountFamily::ShipsAdjacentToMecatol => "ships_adjacent_to_mecatol".to_owned(),
        CountFamily::WeakerNeighbours => "weaker_neighbours".to_owned(),
        CountFamily::DistinctRivalHomeReaches => "distinct_rival_home_reaches".to_owned(),
        CountFamily::RivalDockSystemsWithShips => "rival_dock_systems_with_ships".to_owned(),
        CountFamily::RelicFragments => "relic_fragments".to_owned(),
        CountFamily::ActionCards => "action_cards".to_owned(),
        CountFamily::RivalNoteIssuers => "rival_note_issuers".to_owned(),
        CountFamily::ShipSystemsBesideAnomaly => "ship_systems_beside_anomaly".to_owned(),
        CountFamily::ShipSystemsBesideRivalHome => "ship_systems_beside_rival_home".to_owned(),
        CountFamily::Neighbours => "neighbours".to_owned(),
        CountFamily::UnitsInNexus => "units_in_nexus".to_owned(),
        CountFamily::WormholeKinds => "wormhole_kinds".to_owned(),
        CountFamily::FactionTechnologies => "faction_technologies".to_owned(),
        CountFamily::Laws => "laws".to_owned(),
        CountFamily::SharedPlanetSystems => "shared_planet_systems".to_owned(),
        CountFamily::ProductionInOneSystem => "production_in_one_system".to_owned(),
        CountFamily::LegendaryPlanets => "legendary_planets".to_owned(),
        CountFamily::MecatolShipsWhileControlling => "mecatol_ships_while_controlling".to_owned(),
        CountFamily::CombinedValue { kind } => match kind {
            Spend::Resources => "combined_value_resources".to_owned(),
            Spend::Influence => "combined_value_influence".to_owned(),
        },
    }
}

/// The canonical feature-name token for a bought-objective cost family: affordability, not accumulation, so distinct tokens.
#[must_use]
pub fn cost_family_token(family: &CostFamily) -> String {
    match family {
        CostFamily::Spend(kind) => match kind {
            Spend::Resources => "cost_spend_resources".to_owned(),
            Spend::Influence => "cost_spend_influence".to_owned(),
        },
        CostFamily::TradeGoods => "cost_trade_goods".to_owned(),
        CostFamily::Tokens => "cost_tokens".to_owned(),
        CostFamily::AllThree => "cost_all_three".to_owned(),
    }
}

// ---- core <-> surface conversions -----------------------------------------------------------------------------------------------------

const fn spend_of(s: CoreSpend) -> Spend {
    match s {
        CoreSpend::Resources => Spend::Resources,
        CoreSpend::Influence => Spend::Influence,
    }
}

const fn base_name(b: UnitBaseType) -> &'static str {
    match b {
        UnitBaseType::Carrier => "carrier",
        UnitBaseType::Cruiser => "cruiser",
        UnitBaseType::Destroyer => "destroyer",
        UnitBaseType::Dreadnought => "dreadnought",
        UnitBaseType::Fighter => "fighter",
        UnitBaseType::Flagship => "flagship",
        UnitBaseType::Infantry => "infantry",
        UnitBaseType::Mech => "mech",
        UnitBaseType::Pds => "pds",
        UnitBaseType::Spacedock => "spacedock",
        UnitBaseType::Warsun => "warsun",
    }
}

const fn trait_name(t: PlanetTrait) -> &'static str {
    match t {
        PlanetTrait::Cultural => "cultural",
        PlanetTrait::Hazardous => "hazardous",
        PlanetTrait::Industrial => "industrial",
        PlanetTrait::Faction => "faction",
        PlanetTrait::Fake => "fake",
        PlanetTrait::Lightning => "lightning",
        PlanetTrait::Mr => "mr",
        PlanetTrait::Spacestation => "spacestation",
    }
}

/// The core's counting family in the surface's type.
#[must_use]
pub fn family(f: q::Family) -> CountFamily {
    use q::Family as F;
    match f {
        F::NonHome => CountFamily::NonHome,
        F::OnTheRim => CountFamily::OnTheRim,
        F::SameTrait => CountFamily::SameTrait,
        F::TechSpecialties => CountFamily::TechSpecialties,
        F::UnitUpgrades => CountFamily::UnitUpgrades,
        F::Colours { per_colour } => CountFamily::Colours { per_colour },
        F::Structures => CountFamily::Structures,
        F::StructuresAway => CountFamily::StructuresAway,
        F::FleetInOneSystem => CountFamily::FleetInOneSystem,
        F::PlanetlessSystems => CountFamily::PlanetlessSystems,
        F::AttachedPlanets => CountFamily::AttachedPlanets,
        F::NotableSystems => CountFamily::NotableSystems,
        F::GroundForcesOnOnePlanet => CountFamily::GroundForcesOnOnePlanet,
        F::MechsOnDistinctPlanets => CountFamily::MechsOnDistinctPlanets,
        F::PlanetsOfTrait(t) => CountFamily::PlanetsOfTrait { trait_name: trait_name(t) },
        F::SameColourTechnologies => CountFamily::SameColourTechnologies,
        F::ShipsInSystems => CountFamily::ShipsInSystems,
        F::Units(b) => CountFamily::Units { base_type: base_name(b) },
        F::RivalHomePlanets => CountFamily::RivalHomePlanets,
        F::CapitalShipSystems => CountFamily::CapitalShipSystems,
        F::CapitalShipsInRivalHomeOrMecatol => CountFamily::CapitalShipsInRivalHomeOrMecatol,
        F::ShipsAdjacentToMecatol => CountFamily::ShipsAdjacentToMecatol,
        F::WeakerNeighbours => CountFamily::WeakerNeighbours,
        F::DistinctRivalHomeReaches => CountFamily::DistinctRivalHomeReaches,
        F::RivalDockSystemsWithShips => CountFamily::RivalDockSystemsWithShips,
        F::RelicFragments => CountFamily::RelicFragments,
        F::ActionCards => CountFamily::ActionCards,
        F::RivalNoteIssuers => CountFamily::RivalNoteIssuers,
        F::ShipSystemsBesideAnomaly => CountFamily::ShipSystemsBesideAnomaly,
        F::ShipSystemsBesideRivalHome => CountFamily::ShipSystemsBesideRivalHome,
        F::Neighbours => CountFamily::Neighbours,
        F::UnitsInNexus => CountFamily::UnitsInNexus,
        F::WormholeKinds => CountFamily::WormholeKinds,
        F::FactionTechnologies => CountFamily::FactionTechnologies,
        F::Laws => CountFamily::Laws,
        F::SharedPlanetSystems => CountFamily::SharedPlanetSystems,
        F::ProductionInOneSystem => CountFamily::ProductionInOneSystem,
        F::LegendaryPlanets => CountFamily::LegendaryPlanets,
        F::MecatolShipsWhileControlling => CountFamily::MecatolShipsWhileControlling,
        F::CombinedValue(s) => CountFamily::CombinedValue { kind: spend_of(s) },
    }
}

fn progress_of(p: q::Progress) -> RequirementProgress {
    RequirementProgress { family: family(p.family), have: p.have, threshold: p.threshold }
}

const fn cost_family(f: q::CostFamily) -> CostFamily {
    match f {
        q::CostFamily::Spend(s) => CostFamily::Spend(spend_of(s)),
        q::CostFamily::TradeGoods => CostFamily::TradeGoods,
        q::CostFamily::Tokens => CostFamily::Tokens,
        q::CostFamily::AllThree => CostFamily::AllThree,
    }
}

pub fn cost_progress_of(p: q::CostProgress) -> CostProgress {
    CostProgress { family: cost_family(p.family), have: p.have, target: p.target }
}

/// The typed variant of a public objective alias; `None` for any other id.
fn public(alias: &ObjectiveId) -> Option<PublicObjective> {
    <PublicObjective as IdStr>::from_id(alias.as_str())
}

/// The price of an objective, if it is bought rather than achieved.
#[must_use]
pub fn cost_of(alias: &ObjectiveId) -> Option<Cost> {
    Some(match q::cost_of(public(alias)?)? {
        q::Cost::Spend { amount, kind } => Cost::Spend { amount, kind: spend_of(kind) },
        q::Cost::TradeGoods(n) => Cost::TradeGoods(n),
        q::Cost::Tokens(n) => Cost::Tokens(n),
        q::Cost::AllThree(n) => Cost::AllThree(n),
    })
}

/// The row of a core progress, as the policy feature path consumes it.
fn card_of(row: &ProgressRow) -> CardProgress {
    let alias = match row.objective {
        ObjectiveRef::Public(o) => o.as_str(),
        ObjectiveRef::Secret(o) => o.as_str(),
    };
    let family_token = match row.family {
        RowFamily::Count(f) => family_token(&family(f)),
        RowFamily::Cost(c) => cost_family_token(&cost_family(c)),
    };
    // Counts are small integers; the cast is exact.
    #[expect(clippy::cast_precision_loss, reason = "small integer counts")]
    let (have, threshold) = (row.have as f64, row.threshold as f64);
    CardProgress { alias: alias.to_owned(), family_token, have, threshold, satisfied: row.satisfied, stage: row.stage }
}

/// The imagined sets of an option in typed form (an id the tables do not know is dropped).
#[derive(Debug, Clone, Default)]
struct TypedImagined {
    planets: Vec<Planet>,
    systems: Vec<System>,
    technologies: Vec<Technology>,
    structures: Vec<(System, Planet)>,
}

impl TypedImagined {
    fn of(im: &Imagined<'_>) -> Self {
        Self {
            planets: im.planets.iter().filter_map(|p| <Planet as IdStr>::from_id(p.as_str())).collect(),
            systems: im.systems.iter().filter_map(|s| <System as IdStr>::from_id(s)).collect(),
            technologies: im.technologies.iter().filter_map(|t| <Technology as IdStr>::from_id(t)).collect(),
            structures: im
                .structures
                .iter()
                .filter_map(|(s, p)| Some((<System as IdStr>::from_id(s)?, <Planet as IdStr>::from_id(p)?)))
                .collect(),
        }
    }

    fn get(&self) -> q::Imagined<'_> {
        q::Imagined { planets: &self.planets, systems: &self.systems, technologies: &self.technologies, structures: &self.structures }
    }
}

/// A way to pay a bill of resources: [`ObjView::first_resource_plan`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourcePlan {
    pub planets: Vec<String>,
    pub trade_goods: i32,
    pub worth: i64,
}

// ---- the loaded position -------------------------------------------------------------------------------------------------------------

/// One position loaded into the typed core, with the per-seat scans memoised. Belongs to one `Observed` or one [`Position`].
pub struct ObjView {
    owned: SeenOwned,
    sources: SourceSet,
    has_map: bool,
    edge: ti4_engine_core::bits::SysSet,
    facts: RefCell<[Option<Rc<SeatFacts>>; 16]>,
}

impl ObjView {
    /// Load a position; `None` when the typed state cannot hold it (an id the tables do not know, more than 16 owners) or when a map was
    /// given and did not load.
    #[must_use]
    pub fn build(state: &GameState, sources: SourceSet, galaxy: Option<&Galaxy>) -> Option<Rc<Self>> {
        let owned = SeenOwned::from_parts(state, sources, galaxy).ok()?;
        // The map half of a requirement needs the core's galaxy when the position has one.
        if galaxy.is_some() && owned.core().galaxy().is_none() {
            return None;
        }
        let has_map = galaxy.is_some();
        let edge = if has_map { q::edge_systems(owned.core()) } else { ti4_engine_core::bits::SysSet::EMPTY };
        Some(Rc::new(Self { owned, sources, has_map, edge, facts: RefCell::new(std::array::from_fn(|_| None)) }))
    }

    fn seat_of(&self, player: &PlayerId) -> Option<Seat> {
        self.owned.seat(player)
    }

    /// Whether the position was loaded with its map.
    pub(crate) const fn has_map(&self) -> bool {
        self.has_map
    }

    /// The loaded position.
    pub(crate) const fn owned(&self) -> &SeenOwned {
        &self.owned
    }

    fn scope(&self) -> ti4_engine_core::tables::Scope {
        crate::ties::core_adapter::scope(self.sources)
    }

    /// The hook answers of one seat: the core view's own (`Seen::terms`, public since D3 lane V0; this type kept a copy).
    fn terms(&self, seat: Seat, force_capacity: bool) -> Terms {
        Seen::new(self.owned.core(), self.owned.kernel(), seat).with_blanked(self.owned.blanked()).terms(seat, force_capacity)
    }

    fn facts(&self, seat: Seat) -> Rc<SeatFacts> {
        let slot = usize::from(seat.0);
        if let Some(f) = &self.facts.borrow()[slot] {
            return Rc::clone(f);
        }
        let f = Rc::new(SeatFacts::of(self.owned.core(), self.scope(), seat, self.terms(seat, false)));
        self.facts.borrow_mut()[slot] = Some(Rc::clone(&f));
        f
    }

    /// The plan that pays `cost` resources first (`payment::plans(..)[0]` of the string): the planets to exhaust in the order it lists them, the
    /// trade goods, and what the plan is worth. `None` when nothing pays it or the seat holds cards the core has no planets for.
    pub fn first_resource_plan(&self, player: &PlayerId, cost: i64) -> Option<ResourcePlan> {
        let seat = self.seat_of(player)?;
        let facts = self.facts(seat);
        let core = self.owned.core();
        let useful = q::useful_planets(core, &facts, ti4_engine_core::query::economy::Spend::Resources)?;
        let per_good = facts.terms.per_good;
        let goods = (core.players().by_seat(seat).map_or(0, |p| i64::from(p.econ.trade_goods)) + facts.terms.commodities) * per_good;
        let plans = q::plans(useful.as_slice(), cost, goods, per_good);
        let plan = plans.as_slice().first()?;
        let worth_of = |planet: &ti4_engine_core::generated::Planet| useful.as_slice().iter().find(|(p, _)| p == planet).map_or(0, |(_, worth)| *worth);
        Some(ResourcePlan {
            planets: plan.planets().iter().map(|planet| ti4_engine_core::generated::Planet::as_str(*planet).to_owned()).collect(),
            trade_goods: plan.trade_goods,
            worth: plan.planets().iter().map(worth_of).sum::<i64>() + i64::from(plan.trade_goods) * per_good,
        })
    }

    /// Run `f` on the typed position of `player` with an imagined option; `None` when `player` is not a seat of the position.
    fn with_pos<T>(&self, player: &PlayerId, imagined: &Imagined<'_>, f: impl FnOnce(&Pos<'_>, &Self) -> T) -> Option<T> {
        let seat = self.seat_of(player)?;
        let facts = self.facts(seat);
        let im = TypedImagined::of(imagined);
        let pos = Pos::new(self.owned.core(), &facts, self.has_map, im.get());
        Some(f(&pos, self))
    }

    fn seen(&self, player: &PlayerId) -> Option<Seen<'_>> {
        self.owned.seen(player)
    }

    /// How many revealed objectives `player` could score now in the status window (`Observed::scoreable_public`).
    #[must_use]
    pub fn scoreable_public(&self, player: &PlayerId) -> usize {
        let Some(seen) = self.seen(player) else { return 0 };
        seen.scoreable_public(seen.me()).unwrap_or(0)
    }

    /// How many of the secrets `player` holds could be scored now in the status window (`Observed::scoreable_secret`).
    #[must_use]
    pub fn scoreable_secret(&self, player: &PlayerId) -> usize {
        let Some(seen) = self.seen(player) else { return 0 };
        if self.owned.secrets_masked(player) {
            return 0;
        }
        seen.scoreable_secrets(seen.me(), ti4_engine_core::generated::ObjectivePhase::Status, None).map_or(0, |l| l.len())
    }

    /// Exact progress of every revealed public objective for `player`, after `imagined` happened
    /// (`Observed::revealed_objective_progress_imagining`).
    #[must_use]
    pub fn revealed_progress(&self, player: &PlayerId, imagined: &Imagined<'_>) -> Vec<CardProgress> {
        let Some(seen) = self.seen(player) else { return Vec::new() };
        let im = TypedImagined::of(imagined);
        seen.revealed_objective_progress(seen.me(), im.get()).map(|rows| rows.iter().map(card_of).collect()).unwrap_or_default()
    }

    /// Progress of the secrets `player` holds, after `imagined` happened (`SeatObservation::held_secret_progress_imagining`).
    #[must_use]
    pub fn held_secrets(&self, player: &PlayerId, imagined: &Imagined<'_>) -> Vec<CardProgress> {
        let Some(seen) = self.seen(player) else { return Vec::new() };
        if self.owned.secrets_masked(player) {
            return Vec::new();
        }
        let im = TypedImagined::of(imagined);
        seen.my_secret_progress(im.get()).map(|rows| rows.iter().map(card_of).collect()).unwrap_or_default()
    }

    /// Revealed public objectives whose fixed map-shaped requirement includes `system` (`Observed::objectives_implicating_system`).
    #[must_use]
    pub fn implicating(&self, player: &PlayerId, system: &SystemId) -> Vec<ObjectiveId> {
        let (Some(seen), Some(system)) = (self.seen(player), <System as IdStr>::from_id(system.as_str())) else { return Vec::new() };
        seen.objectives_implicating_system(seen.me(), system).as_slice().iter().map(|o| ObjectiveId::new(o.as_str())).collect()
    }
}

// ---- the legacy `Position` ------------------------------------------------------------------------------------------------------------

/// What the engine needs to evaluate a requirement: a player's position, ready for any number of requirement checks. The typed view of the
/// position is loaded on the first question (the fields are read then; set `galaxy` before asking).
pub struct Position<'a> {
    pub state: &'a GameState,
    pub content: &'a ContentStore,
    pub sources: SourceSet,
    pub player: &'a PlayerId,
    /// The map, when the caller has one. Requirements about the shape of the board report unmet without it.
    pub galaxy: Option<&'a Galaxy>,
    planets: Vec<PlanetId>,
    systems: Vec<String>,
    technologies: Vec<String>,
    structures: Vec<(String, String)>,
    view: OnceCell<Option<Rc<ObjView>>>,
}

impl<'a> Position<'a> {
    /// Resolve a player's position once, ready for any number of requirement checks.
    #[must_use]
    pub fn new(state: &'a GameState, content: &'a ContentStore, sources: SourceSet, player: &'a PlayerId) -> Self {
        Self {
            state,
            content,
            sources,
            player,
            galaxy: None,
            planets: Vec::new(),
            systems: Vec::new(),
            technologies: Vec::new(),
            structures: Vec::new(),
            view: OnceCell::new(),
        }
    }

    /// The same position with the map.
    #[must_use]
    pub fn with_galaxy(mut self, galaxy: &'a Galaxy) -> Self {
        self.galaxy = Some(galaxy);
        self
    }

    /// The position as it would stand if the player also controlled `extra`.
    #[must_use]
    pub fn imagining(state: &'a GameState, content: &'a ContentStore, sources: SourceSet, player: &'a PlayerId, extra: &[PlanetId]) -> Self {
        let mut position = Self::new(state, content, sources, player);
        position.planets = extra.to_vec();
        position
    }

    /// The position as it would stand after everything in `imagined` happened.
    #[must_use]
    pub fn imagining_all(state: &'a GameState, content: &'a ContentStore, sources: SourceSet, player: &'a PlayerId, imagined: &Imagined<'_>) -> Self {
        let mut position = Self::imagining(state, content, sources, player, imagined.planets);
        position.systems.extend(imagined.systems.iter().cloned());
        position.technologies.extend(imagined.technologies.iter().cloned());
        position.structures.extend(imagined.structures.iter().cloned());
        position
    }

    /// The same position over an already loaded view (the view of an `Observed`).
    #[must_use]
    pub fn with_view(self, view: Option<Rc<ObjView>>) -> Self {
        let _ = self.view.set(view);
        self
    }

    fn view(&self) -> Option<&Rc<ObjView>> {
        if !self.content.is_embedded() {
            return None;
        }
        self.view.get_or_init(|| ObjView::build(self.state, self.sources, self.galaxy)).as_ref()
    }

    fn imagined(&self) -> Imagined<'_> {
        Imagined { planets: &self.planets, systems: &self.systems, technologies: &self.technologies, structures: &self.structures }
    }

    fn typed<T>(&self, f: impl FnOnce(&Pos<'_>, &ObjView) -> T) -> Option<T> {
        self.view()?.with_pos(self.player, &self.imagined(), f)
    }
}

/// Exact progress for public objectives backed by counting families. `None` for an objective that is not one (or a position the core cannot hold).
#[must_use]
pub fn counting_progress(alias: &ObjectiveId, position: &Position<'_>) -> Option<RequirementProgress> {
    let obj = public(alias)?;
    position.typed(|pos, view| q::counting_progress(pos, &view.edge, obj).map(progress_of))?
}

/// Exact progress for public objectives whose requirement is a position the player must hold (not a running count).
#[must_use]
pub fn remaining_position_progress(alias: &ObjectiveId, position: &Position<'_>) -> Option<RequirementProgress> {
    let obj = public(alias)?;
    position.typed(|pos, _| q::remaining_position_progress(pos, obj).map(progress_of))?
}

/// The greatest exactly affordable amount toward a bought objective, with the planets the option would bring under control counted.
#[must_use]
pub fn bought_progress_at(position: &Position<'_>, alias: &ObjectiveId) -> Option<CostProgress> {
    let obj = public(alias)?;
    q::cost_of(obj)?;
    position.typed(|pos, _| q::bought_progress(pos, obj).map(cost_progress_of))?
}

/// The home planets whose loss bars this player from scoring public objectives (61.16). Empty when nothing bars them: every home planet is
/// theirs, the faction scores without its home (Saar Nomadic), there is no faction home, or the core cannot hold the position.
#[must_use]
pub fn home_planets_not_controlled(position: &Position<'_>) -> Vec<PlanetId> {
    let Some(view) = position.view() else { return Vec::new() };
    let Some(seat) = view.seat_of(position.player) else { return Vec::new() };
    let (core, k) = (view.owned.core(), view.owned.kernel());
    if k.hooks.strategy.scores_without_home(core, seat) {
        return Vec::new(); // Saar Nomadic
    }
    let Some(player) = core.players().by_seat(seat) else { return Vec::new() };
    player.home_planets.as_slice().iter().filter(|planet| !board::controls(core, seat, **planet)).map(|planet| PlanetId::new(planet.as_str())).collect()
}

/// Every neighbour of `position.player` with both planet counts, in seating order (Push Boundaries). `None` without a map (adjacency needs
/// one) or when the core cannot hold the position. The player's own count does not include planets the position imagines.
#[must_use]
pub fn neighbour_planet_counts(position: &Position<'_>) -> Option<Vec<NeighbourPlanets>> {
    position.galaxy?;
    let view = position.view()?;
    let seat = view.seat_of(position.player)?;
    let core = view.owned.core();
    let scope = view.scope();
    let mine = q::scoring_planets(core, scope, seat).len();
    let facts = view.facts(seat);
    let rows = q::neighbours(core, &facts.terms, seat);
    Some(
        rows.as_slice()
            .iter()
            .filter_map(|other| {
                Some(NeighbourPlanets { neighbour: view.owned.player(*other)?, mine, theirs: q::scoring_planets(core, scope, *other).len() })
            })
            .collect(),
    )
}

/// The progress of one secret for `player` in a loaded view (`secrets::progress`).
pub(crate) fn secret_progress_in(view: &ObjView, player: &PlayerId, imagined: &Imagined<'_>, obj: ti4_engine_core::generated::SecretObjective) -> Option<RequirementProgress> {
    let seat = view.seat_of(player)?;
    // Produce en Masse reads the seat's production in every system, which the shared scan only pays for when the seat holds the card or it is
    // revealed; `secrets::progress` may be asked of any secret, so the scan is made for this question alone.
    let one_off = (obj == ti4_engine_core::generated::SecretObjective::ProduceEnMasse && !q::wants_capacity(view.owned.core(), seat))
        .then(|| Rc::new(SeatFacts::of(view.owned.core(), view.scope(), seat, view.terms(seat, true))));
    let facts = one_off.unwrap_or_else(|| view.facts(seat));
    let im = TypedImagined::of(imagined);
    let pos = Pos::new(view.owned.core(), &facts, view.has_map, im.get());
    q::secret_progress(&pos, obj).ok().flatten().map(progress_of)
}

/// The progress of every secret `player` holds in one loaded position, in hand order, with `imagined` happened first
/// (`choice::held_secret_progress_imagining`); empty when the core cannot hold the position.
#[must_use]
pub fn held_secrets_of(state: &GameState, content: &ContentStore, sources: SourceSet, galaxy: Option<&Galaxy>, player: &PlayerId, imagined: &Imagined<'_>) -> Vec<CardProgress> {
    if !content.is_embedded() {
        return Vec::new();
    }
    ObjView::build(state, sources, galaxy).map(|view| view.held_secrets(player, imagined)).unwrap_or_default()
}

/// Revealed objectives this player could score right now (the status window), in reveal order, with the map when the caller has one:
/// none while a home planet is lost (61.16), a bought objective when it can be afforded, a position objective when its requirement is met.
/// Empty when the core cannot hold the position.
#[must_use]
pub fn scoreable_on(state: &GameState, content: &ContentStore, sources: SourceSet, player: &PlayerId, galaxy: Option<&Galaxy>) -> Vec<ObjectiveId> {
    if !content.is_embedded() {
        return Vec::new();
    }
    let Some(view) = ObjView::build(state, sources, galaxy) else { return Vec::new() };
    let Some(seen) = view.seen(player) else { return Vec::new() };
    seen.scoreable_publics(seen.me())
        .map(|refs| {
            refs.as_slice()
                .iter()
                .map(|r| match r {
                    ObjectiveRef::Public(o) => ObjectiveId::new(o.as_str()),
                    ObjectiveRef::Secret(o) => ObjectiveId::new(o.as_str()),
                })
                .collect()
        })
        .unwrap_or_default()
}

// ---- the accessors of `Observed` -------------------------------------------------------------------------------------------------------

/// The view an `Observed` keeps for its objectives questions: loaded on the first one.
pub(crate) fn view_of(cell: &OnceCell<Option<Rc<ObjView>>>, state: &GameState, content: &ContentStore, sources: SourceSet, galaxy: Option<&Galaxy>) -> Option<Rc<ObjView>> {
    if !content.is_embedded() {
        return None;
    }
    cell.get_or_init(|| ObjView::build(state, sources, galaxy)).clone()
}

/// The public objectives whose scoring rule is written (the legacy `objectives::requirement_for` is `Some` for exactly these).
const REGISTERED: [&str; 30] = [
    "conquer", "intimidate", "outer_rim", "control_borderlands", "push_boundaries", "distant_lands", "engineer_marvel", "supremacy", "expand_borders", "subdue", "corner",
    "unify_colonies", "research_outposts", "brain_trust", "develop", "revolutionize", "diversify", "master_science", "build_defenses", "massive_cities", "infrastructure",
    "protect_border", "raise_fleet", "command_armada", "deep_space", "vast_territories", "ancient_monuments", "lost_outposts", "make_history", "become_legend",
];

/// Whether the objective `alias` has a written scoring rule.
#[must_use]
pub fn has_requirement(alias: &ti4_model::id::ObjectiveId) -> bool {
    REGISTERED.contains(&alias.as_str())
}
