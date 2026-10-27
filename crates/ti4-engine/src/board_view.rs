//! `Area::BoardQueries` (package A1): the typed board queries of `ti4_engine_core::query` behind a
//! string interface. One [`BoardView`] is built per position (the codecs of the Meta, Seats,
//! Players, Board, Map, Agenda and Strategy slices run once); every method answers a question of
//! `planets.rs`, `coexistence.rs`, `supply.rs`, `fleet.rs`, `laws.rs`, `tokens.rs` or
//! `GameState` with the same value in the same order. A text id the tables do not know makes the
//! method answer `None`: the caller keeps the string answer (the strangler's escape hatch).
//! Nothing here decides a rule.

use std::collections::BTreeSet;

use ti4_content::galaxy::Galaxy;
use ti4_engine_core::generated::{Agenda, Planet, System, TechType, Unit};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::query::economy::Spend as CoreSpend;
use ti4_engine_core::query::fleet::{Arrival, Standing as CoreStanding};
use ti4_engine_core::query::{self, Scope};
use ti4_engine_core::state::{Slice, SliceSet};
use ti4_model::content_types::SourceSet;
use ti4_model::id::{PlanetId, PlayerId, SystemId, UnitTypeId};
use ti4_model::state::GameState;

use crate::core_sync::{CoreView, Ctx, Sources};
use crate::fleet::Standing;
use crate::generated::IdStr;
use crate::production::Spend;

/// What the queries read.
pub const READS: SliceSet =
    SliceSet::of(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Map, Slice::Agenda, Slice::Strategy]);
/// The same without the map (no neighbours).
pub const READS_NO_MAP: SliceSet =
    SliceSet::of(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Agenda, Slice::Strategy]);

/// The typed view of one position.
pub struct BoardView {
    view: CoreView,
    cx: Ctx,
    scope: Scope,
    blanked: bool,
}

fn typed<T: IdStr>(id: &str) -> Option<T> {
    T::from_id(id)
}

fn spend(s: Spend) -> CoreSpend {
    match s {
        Spend::Resources => CoreSpend::Resources,
        Spend::Influence => CoreSpend::Influence,
    }
}

impl BoardView {
    /// The view of a position; `None` when something is not mapped (an unknown id, more than 16
    /// owners). With no galaxy the map slice is left out and [`Self::neighbours`] answers `None`.
    #[must_use]
    pub fn of(gs: &GameState, galaxy: Option<&Galaxy>, sources: SourceSet) -> Option<Self> {
        let mut cx = Ctx::of(gs).ok()?;
        let reads = if galaxy.is_some() { READS } else { READS_NO_MAP };
        let view = CoreView::of(gs, &mut cx, Sources { galaxy, rng: None }, reads).ok()?;
        Some(Self { view, cx, scope: crate::core_adapter::scope(sources), blanked: crate::laws::blanked(gs) })
    }

    /// A view for a string call site that has no source scope or galaxy at hand (laws,
    /// coexistence, planet control): the full source scope, no map. Dual and `Core` backends only.
    #[must_use]
    pub fn light(gs: &GameState) -> Option<Self> {
        Self::of(gs, None, ti4_model::content_types::FULL)
    }

    /// The core state, for callers that ask `ti4_engine_core::query` directly.
    #[must_use]
    pub fn core(&self) -> &ti4_engine_core::state::CoreState {
        &self.view
    }

    #[must_use]
    pub const fn scope(&self) -> Scope {
        self.scope
    }

    #[must_use]
    pub const fn blanked(&self) -> bool {
        self.blanked
    }

    fn seat(&self, p: &PlayerId) -> Option<Seat> {
        self.cx.owners.seat(p.as_str())
    }

    fn player(&self, s: Seat) -> Option<PlayerId> {
        self.cx.owners.id(s).map(PlayerId::new)
    }

    fn players(&self, set: impl IntoIterator<Item = Seat>) -> Option<Vec<PlayerId>> {
        set.into_iter().map(|s| self.player(s)).collect()
    }

    // -- board ---------------------------------------------------------------------------------

    /// `GameState::controlled_planets`, owned.
    #[must_use]
    pub fn controlled_planets(&self, player: &PlayerId) -> Option<Vec<(SystemId, PlanetId)>> {
        let seat = self.seat(player)?;
        Some(
            query::board::controlled_planets(&self.view, seat)
                .as_slice()
                .iter()
                .map(|(s, p)| (SystemId::new(s.as_str()), PlanetId::new(p.as_str())))
                .collect(),
        )
    }

    /// Who controls the planet (`SystemState::planet_control`).
    #[must_use]
    pub fn control(&self, planet: &PlanetId) -> Option<Option<PlayerId>> {
        let p: Planet = typed(planet.as_str())?;
        match query::board::control(&self.view, p) {
            None => Some(None),
            Some(s) => Some(Some(self.player(s)?)),
        }
    }

    /// `planets::in_system`.
    #[must_use]
    pub fn planets_in(&self, system: &SystemId) -> Option<Vec<PlanetId>> {
        let s: System = typed(system.as_str())?;
        Some(query::board::planets_in(&self.view, self.scope, s).as_slice().iter().map(|p| PlanetId::new(p.as_str())).collect())
    }

    /// `planets::system_of`.
    #[must_use]
    pub fn system_of(&self, planet: &str) -> Option<Option<SystemId>> {
        let p: Planet = typed(planet)?;
        Some(query::board::system_of(&self.view, self.scope, p).map(|s| SystemId::new(s.as_str())))
    }

    /// `coexistence::coexisters`.
    #[must_use]
    pub fn coexisters(&self, system: &SystemId, planet: &PlanetId) -> Option<BTreeSet<PlayerId>> {
        let (s, p): (System, Planet) = (typed(system.as_str())?, typed(planet.as_str())?);
        Some(self.players(query::board::coexisters(&self.view, s, p).iter())?.into_iter().collect())
    }

    /// `coexistence::controls_for_scoring`.
    #[must_use]
    pub fn controls_for_scoring(&self, system: &SystemId, planet: &PlanetId, player: &PlayerId) -> Option<bool> {
        let (s, p): (System, Planet) = (typed(system.as_str())?, typed(planet.as_str())?);
        Some(query::board::controls_for_scoring(&self.view, s, p, self.seat(player)?))
    }

    /// `coexistence::in_coexistence`.
    #[must_use]
    pub fn in_coexistence(&self, system: &SystemId, planet: &PlanetId) -> Option<bool> {
        let (s, p): (System, Planet) = (typed(system.as_str())?, typed(planet.as_str())?);
        Some(query::board::in_coexistence(&self.view, s, p))
    }

    /// `GameState::systems_with_token`.
    #[must_use]
    pub fn systems_with_token(&self, player: &PlayerId) -> Option<BTreeSet<SystemId>> {
        let seat = self.seat(player)?;
        Some(
            query::board::systems_with_token(&self.view, seat)
                .iter()
                .map(|i| SystemId::new(ti4_engine_core::ids::system_at(i).as_str()))
                .collect(),
        )
    }

    /// `GameState::systems_with_units_of`.
    #[must_use]
    pub fn systems_with_units_of(&self, player: &PlayerId) -> Option<BTreeSet<SystemId>> {
        let seat = self.seat(player)?;
        Some(
            query::board::systems_with_units_of(&self.view, seat)
                .iter()
                .map(|i| SystemId::new(ti4_engine_core::ids::system_at(i).as_str()))
                .collect(),
        )
    }

    /// `Galaxy::adjacent`, as the map slice has it.
    #[must_use]
    pub fn neighbours(&self, system: &SystemId) -> Option<BTreeSet<SystemId>> {
        let s: System = typed(system.as_str())?;
        let set = query::board::neighbours(&self.view, s)?;
        Some(set.iter().map(|i| SystemId::new(ti4_engine_core::ids::system_at(i).as_str())).collect())
    }

    // -- economy -------------------------------------------------------------------------------

    /// `production::planet_value_now` with the hook term `extra` (the Helios dock bonus).
    #[must_use]
    pub fn planet_value_now(&self, planet: &PlanetId, kind: Spend, extra: i64) -> Option<i64> {
        let p: Planet = typed(planet.as_str())?;
        Some(query::economy::value_now(&self.view, self.scope, self.blanked, p, spend(kind), extra))
    }

    /// `production::spendable_planets` without the Deepwrought oceans.
    #[must_use]
    pub fn spendable_planets(&self, player: &PlayerId) -> Option<Vec<PlanetId>> {
        let seat = self.seat(player)?;
        Some(
            query::economy::spendable_planets(&self.view, seat)
                .as_slice()
                .iter()
                .map(|(_, p)| PlanetId::new(p.as_str()))
                .collect(),
        )
    }

    /// `GameState::tokens_in_reinforcements` (`mahact_held` = `tokens_held_by_mahact`).
    #[must_use]
    pub fn tokens_in_reinforcements(&self, player: &PlayerId, mahact_held: i32) -> Option<i32> {
        Some(query::economy::tokens_in_reinforcements(&self.view, self.seat(player)?, mahact_held))
    }

    // -- supply and fleet ----------------------------------------------------------------------

    /// `supply::held` minus the Crimson hero card.
    #[must_use]
    pub fn held_on_board(&self, player: &PlayerId, base_type: &str) -> Option<i64> {
        let base = typed(base_type)?;
        Some(query::fleet::held_on_board(&self.view, self.scope, self.seat(player)?, base))
    }

    /// `supply::remaining` with `on_card` ships on the Crimson hero card.
    #[must_use]
    pub fn remaining(&self, player: &PlayerId, unit: &UnitTypeId, on_card: i64) -> Option<i64> {
        let unit: Unit = typed(unit.as_str())?;
        Some(query::fleet::remaining(&self.view, self.scope, self.seat(player)?, unit, on_card))
    }

    /// `fleet::standing_with` for the limit and the half-weight answer the caller worked out.
    #[must_use]
    pub fn standing(
        &self,
        player: &PlayerId,
        system: &SystemId,
        fleet_limit: i64,
        arriving: Option<(&str, i64, bool)>,
        half_weight: &dyn Fn(&str) -> bool,
    ) -> Option<Standing> {
        let sys: System = typed(system.as_str())?;
        let arrival = match arriving {
            None => None,
            Some((kind, count, in_space)) => Some(Arrival { kind: typed(kind)?, count, in_space }),
        };
        let half = |u: Unit| half_weight(u.as_str());
        let s: CoreStanding = query::fleet::standing(&self.view, self.scope, self.seat(player)?, sys, fleet_limit, arrival, &half);
        Some(Standing {
            fleet_limit: s.fleet_limit,
            fleet_charged: s.fleet_charged,
            transport: s.transport,
            consumed: s.consumed,
            fighters_charged: s.fighters_charged,
            capacity_excess: s.capacity_excess,
            ships: s.ships,
            carried: s.carried,
            fighters: s.fighters,
            fighter_support: s.fighter_support,
            upgraded: s.upgraded,
            half_weight: s.half_weight,
        })
    }

    // -- seats ---------------------------------------------------------------------------------

    /// `GameState::clockwise_from`.
    #[must_use]
    pub fn clockwise_from(&self, player: &PlayerId) -> Option<Vec<PlayerId>> {
        // A player who does not sit gets the unrotated order; one the owner table does not know
        // cannot be asked.
        let seat = self.seat(player).unwrap_or(Seat(u8::MAX));
        self.players(query::seats::clockwise_from(&self.view, seat).as_slice().iter().copied())
    }

    /// `GameState::initiative_order`.
    #[must_use]
    pub fn initiative_order(&self) -> Option<Vec<PlayerId>> {
        self.players(query::seats::initiative_order(&self.view).as_slice().iter().copied())
    }

    // -- laws ----------------------------------------------------------------------------------

    fn law(alias: &str) -> Option<Agenda> {
        typed(alias)
    }

    /// `laws::in_play`.
    #[must_use]
    pub fn laws_in_play(&self) -> Vec<String> {
        query::laws::in_play(&self.view, self.blanked).as_slice().iter().map(|a| a.as_str().to_owned()).collect()
    }

    /// `laws::active`.
    #[must_use]
    pub fn law_active(&self, alias: &str) -> Option<bool> {
        Some(query::laws::active(&self.view, self.blanked, Self::law(alias)?))
    }

    /// `laws::holds_ministry`.
    #[must_use]
    pub fn holds_ministry(&self, player: &PlayerId, alias: &str) -> Option<bool> {
        Some(query::laws::holds_ministry(&self.view, self.blanked, self.seat(player)?, Self::law(alias)?))
    }

    /// `laws::fleet_pool_cap`.
    #[must_use]
    pub fn fleet_pool_cap(&self, base: i32) -> i32 {
        query::laws::fleet_pool_cap(&self.view, self.blanked, base)
    }

    /// `laws::action_card_limit`.
    #[must_use]
    pub fn action_card_limit(&self, base: usize) -> usize {
        query::laws::action_card_limit(&self.view, self.blanked, base)
    }

    /// `laws::planet_value_bonus_sources`.
    #[must_use]
    pub fn planet_value_bonus_sources(&self, planet: &PlanetId) -> Option<Vec<(&'static str, i64, i64)>> {
        use query::laws::ValueLaw;
        let p: Planet = typed(planet.as_str())?;
        Some(
            query::laws::planet_value_bonus_sources(&self.view, self.blanked, p)
                .as_slice()
                .iter()
                .map(|(l, r, i)| {
                    let name = match l {
                        ValueLaw::CoreMining => "Core Mining",
                        ValueLaw::SenateSanctuary => "Senate Sanctuary",
                        ValueLaw::TerraformingInitiative => "Terraforming Initiative",
                    };
                    (name, *r, *i)
                })
                .collect(),
        )
    }

    /// `laws::planet_is_demilitarized`.
    #[must_use]
    pub fn planet_is_demilitarized(&self, planet: &PlanetId) -> Option<bool> {
        Some(query::laws::planet_is_demilitarized(&self.view, self.blanked, typed(planet.as_str())?))
    }

    /// `laws::production_forbidden_on`.
    #[must_use]
    pub fn production_forbidden_on(&self, planet: &PlanetId) -> Option<bool> {
        Some(query::laws::production_forbidden_on(&self.view, self.blanked, typed(planet.as_str())?))
    }

    /// `laws::research_team_waivers`.
    #[must_use]
    pub fn research_team_waivers(&self, player: &PlayerId, colour: &str) -> Option<usize> {
        let c = match colour {
            "BIOTIC" => TechType::Biotic,
            "CYBERNETIC" => TechType::Cybernetic,
            "PROPULSION" => TechType::Propulsion,
            "WARFARE" => TechType::Warfare,
            _ => return Some(0),
        };
        Some(query::laws::research_team_waivers(&self.view, self.blanked, self.seat(player)?, c))
    }

    /// `laws::bombardment_forbidden`.
    #[must_use]
    pub fn bombardment_forbidden(&self, planet: &PlanetId) -> Option<bool> {
        Some(query::laws::bombardment_forbidden(&self.view, self.blanked, self.scope, typed(planet.as_str())?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ti4_engine_core::generated::Unit;

    #[test]
    fn constants_and_tables_match_the_string_engine() {
        assert_eq!(query::laws::RULES_DMZ_EXPLORATION, ti4_model::state::RULES_DMZ_EXPLORATION);
        assert_eq!(query::economy::TOKENS_PER_FACTION, ti4_model::state::TOKENS_PER_FACTION);
        assert_eq!(query::fleet::UNLIMITED_FLEET_BILL, crate::fleet::UNLIMITED_FLEET_BILL);
        let content = ti4_content::ContentStore::embedded();
        let types = ti4_content::units::catalogue(content, ti4_model::content_types::FULL);
        for u in Unit::ALL {
            let Some(t) = types.get(u.as_str()) else { continue };
            assert_eq!(query::fleet::fighter_support(u), t.fighter_support(), "{u:?}");
            assert_eq!(query::fleet::capacity(u), t.capacity(), "{u:?}");
            assert_eq!(query::fleet::capacity_cost(u), t.capacity_cost(), "{u:?}");
            assert_eq!(query::fleet::counts_against_supply(u), crate::fleet::counts_against_supply(t), "{u:?}");
            assert_eq!(query::fleet::consumes_capacity(u), t.consumes_capacity(), "{u:?}");
        }
        for base in ti4_engine_core::generated::UnitBaseType::ALL {
            assert_eq!(query::fleet::plastic(base), crate::supply::plastic(base.as_str()), "{base:?}");
        }
    }

    #[test]
    fn law_aliases_name_the_agendas_the_string_engine_reads() {
        use query::laws::*;
        for (law, alias) in [
            (FLEET_REGULATIONS, "regulations"), (SANCTIONS, "sanctions"), (CENSURE, "censure"),
            (SHARED_RESEARCH, "shared_research"), (DEFENSE_ACT, "defense_act"), (CONVENTIONS, "conventions"),
            (DEMILITARIZED_ZONE, "demilitarized_zone"), (HOLY_PLANET_OF_IXTH, "holy_planet_of_ixth"),
            (CORE_MINING, "core_mining"), (SENATE_SANCTUARY, "senate_sanctuary"),
            (TERRAFORMING_INITIATIVE, "terraforming_initiative"), (CONSCRIPTION, "conscription"),
            (REP_GOVT, "rep_govt"), (REPRESENTATIVE_GOVERNMENT, "representative_government"),
            (TRAVEL_BAN, "travel_ban"), (WORMHOLE_RECON, "wormhole_recon"),
            (MINISTER_EXPLORATION, "minister_exploration"), (MINISTER_POLICY, "minister_policy"),
        ] {
            assert_eq!(law.as_str(), alias);
        }
        assert_eq!(ti4_engine_core::generated::Attachment::DemilitarizedZone.as_str(), crate::laws::DMZ_ATTACHMENT);
    }
}
