//! `Area::Production` (package B3): the typed production reads of `ti4_engine_core::rules::production`
//! behind the string interface. One [`ProdView`] is the typed view of one position plus the string
//! engine's answers to what the core cannot see yet (faction hooks, Ral Nel, Quietus, entropic scars,
//! space stations): it implements the core's `Env`. A text id the tables do not know makes a method
//! answer `None` and the caller keeps the string answer.

use ti4_content::ContentStore;
use ti4_content::galaxy::Galaxy;
use ti4_engine_core::generated::{Planet, System, Unit, UnitBaseType};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::query::Scope;
use ti4_engine_core::query::economy::Spend as CoreSpend;
use ti4_engine_core::rules::production::{self as rules, Cx, Env, Spot};
use ti4_engine_core::state::{Slice, SliceSet};
use ti4_model::content_types::SourceSet;
use ti4_model::id::{PlanetId, PlayerId, SystemId, UnitTypeId};
use ti4_model::state::GameState;
use ti4_model::units::Unit as StrUnit;

use crate::core_sync::{CoreView, Ctx, Sources};
use crate::generated::IdStr;
use crate::production::Spend;

/// What the production reads need.
pub const READS: SliceSet = SliceSet::of(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Agenda]);

/// The typed view of one position.
pub struct ProdView<'a> {
    view: CoreView,
    cx: Ctx,
    scope: Scope,
    blanked: bool,
    state: &'a GameState,
    content: &'a ContentStore,
    sources: SourceSet,
}

fn typed<T: IdStr>(id: &str) -> Option<T> {
    T::from_id(id)
}

fn spend(s: CoreSpend) -> Spend {
    match s {
        CoreSpend::Resources => Spend::Resources,
        CoreSpend::Influence => Spend::Influence,
    }
}

impl<'a> ProdView<'a> {
    /// The view of a position; `None` when something is not mapped.
    #[must_use]
    pub fn of(state: &'a GameState, content: &'a ContentStore, sources: SourceSet) -> Option<Self> {
        let mut cx = Ctx::of(state).ok()?;
        let view = CoreView::of(state, &mut cx, Sources { galaxy: None::<&Galaxy>, rng: None }, READS).ok()?;
        Some(Self {
            view,
            cx,
            scope: crate::core_adapter::scope(sources),
            blanked: crate::laws::blanked(state),
            state,
            content,
            sources,
        })
    }

    fn rules_cx(&self) -> Cx<'_> {
        Cx { core: &self.view, scope: self.scope, blanked: self.blanked, env: self }
    }

    fn seat(&self, p: &PlayerId) -> Option<Seat> {
        self.cx.owners.seat(p.as_str())
    }

    fn pid(&self, s: Seat) -> PlayerId {
        PlayerId::new(self.cx.owners.id(s).unwrap_or_default())
    }

    fn str_unit(&self, piece: ti4_engine_core::state::Piece) -> StrUnit {
        let mut u = StrUnit::new(UnitTypeId::new(piece.kind().as_str()), self.pid(piece.owner()));
        u.sustained_damage = piece.sustained();
        u.galvanized = piece.galvanized();
        u
    }

    /// `production::producers`.
    #[must_use]
    pub fn producers(&self, player: &PlayerId, system: &SystemId) -> Option<Vec<(StrUnit, Option<PlanetId>)>> {
        let (who, sys) = (self.seat(player)?, typed::<System>(system.as_str())?);
        Some(
            rules::producers(&self.rules_cx(), who, sys)
                .as_slice()
                .iter()
                .map(|(p, pl)| (self.str_unit(*p), pl.map(|x| PlanetId::new(x.as_str()))))
                .collect(),
        )
    }

    /// `production::mobile_docks`.
    #[must_use]
    pub fn mobile_docks(&self, player: &PlayerId, system: &SystemId) -> Option<Vec<StrUnit>> {
        let (who, sys) = (self.seat(player)?, typed::<System>(system.as_str())?);
        Some(rules::mobile_docks(&self.rules_cx(), who, sys).as_slice().iter().map(|p| self.str_unit(*p)).collect())
    }

    /// `production::capacity`.
    #[must_use]
    pub fn capacity(&self, player: &PlayerId, system: &SystemId) -> Option<i64> {
        Some(rules::capacity(&self.rules_cx(), self.seat(player)?, typed(system.as_str())?))
    }

    /// `production::module_production`.
    #[must_use]
    pub fn module_production(&self, player: &PlayerId, system: &SystemId) -> Option<i64> {
        Some(rules::module_production(&self.rules_cx(), self.seat(player)?, typed(system.as_str())?))
    }

    /// `production::barred_capacity` for a unit base type.
    #[must_use]
    pub fn barred_capacity(&self, player: &PlayerId, system: &SystemId, base: &str) -> Option<i64> {
        Some(rules::barred_capacity(&self.rules_cx(), self.seat(player)?, typed(system.as_str())?, typed::<UnitBaseType>(base)?))
    }

    /// `production::structures_on`.
    #[must_use]
    pub fn structures_on(&self, player: &PlayerId, planet: &PlanetId, base: &str) -> Option<usize> {
        Some(rules::structures_on(&self.rules_cx(), self.seat(player)?, typed(planet.as_str())?, typed::<UnitBaseType>(base)?))
    }

    /// `production::structure_allowed`.
    #[must_use]
    pub fn structure_allowed(&self, player: &PlayerId, planet: &PlanetId, base: &str) -> Option<bool> {
        Some(rules::structure_allowed(&self.rules_cx(), self.seat(player)?, typed(planet.as_str())?, typed::<UnitBaseType>(base)?))
    }

    /// `production::placements`, as the string spots (`space` or the planet id).
    #[must_use]
    pub fn placements(&self, player: &PlayerId, system: &SystemId, unit_id: &str) -> Option<Vec<String>> {
        let spots = rules::placements(&self.rules_cx(), self.seat(player)?, typed(system.as_str())?, typed::<Unit>(unit_id)?);
        Some(
            spots
                .as_slice()
                .iter()
                .map(|s| match s {
                    Spot::Space => crate::production::SPACE.to_owned(),
                    Spot::Planet(p) => p.as_str().to_owned(),
                })
                .collect(),
        )
    }

    /// `production::war_machine_bonus`.
    #[must_use]
    pub fn war_machine_bonus(&self, player: &PlayerId) -> Option<i64> {
        Some(rules::war_machine_bonus(&self.view, self.seat(player)?))
    }

    /// `production::price_of_under` with the law read from the position.
    #[must_use]
    pub fn price_of(&self, unit_id: &str) -> Option<(i64, usize)> {
        Some(rules::price_of(&self.rules_cx(), typed::<Unit>(unit_id)?))
    }
}

impl Env for ProdView<'_> {
    fn planet_spend_value(&self, who: Seat, planet: Planet, kind: CoreSpend, value: i64) -> i64 {
        crate::factions::hooks_economy::planet_spend_value(
            self.state,
            self.content,
            &self.pid(who),
            &PlanetId::new(planet.as_str()),
            spend(kind),
            value,
        )
    }

    fn extra_production(&self, who: Seat, system: System) -> i64 {
        crate::factions::hooks_economy::extra_production(
            self.state,
            self.content,
            self.sources,
            &self.pid(who),
            &SystemId::new(system.as_str()),
        )
    }

    fn extra_production_planet(&self, who: Seat, system: System, planet: Planet) -> i64 {
        crate::factions::hooks_economy::extra_production_planet(
            self.state,
            self.content,
            self.sources,
            &self.pid(who),
            &SystemId::new(system.as_str()),
            &PlanetId::new(planet.as_str()),
        )
    }

    fn has_cannot_produce(&self) -> bool {
        crate::factions::hooks_economy::has_cannot_produce()
    }

    fn cannot_produce(&self, who: Seat, unit: UnitBaseType, producer: UnitBaseType) -> bool {
        crate::factions::hooks_economy::cannot_produce(self.state, self.content, &self.pid(who), unit.as_str(), producer.as_str())
    }

    fn silenced_in_space(&self, who: Seat, unit: Unit) -> bool {
        let u = StrUnit::new(UnitTypeId::new(unit.as_str()), self.pid(who));
        crate::factions::ralnel::silenced_in_space(self.state, self.content, self.sources, &u)
    }

    fn abilities_lost(&self, who: Seat, system: System) -> bool {
        crate::factions::crimson::abilities_lost(self.state, &self.pid(who), &SystemId::new(system.as_str()))
    }

    fn abilities_usable(&self, system: System) -> bool {
        crate::entropic_scars::abilities_usable(self.content, self.sources, &SystemId::new(system.as_str()), None)
    }

    fn is_space_station(&self, planet: Planet) -> bool {
        ti4_content::galaxy::is_space_station(self.content, planet.as_str(), self.sources)
    }

    fn resource_bonus(&self, planet: Planet) -> i64 {
        crate::factions::bastion_units::resource_bonus(self.state, &PlanetId::new(planet.as_str()))
    }
}
