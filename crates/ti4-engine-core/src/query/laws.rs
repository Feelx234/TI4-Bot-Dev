//! Laws in play and the standing rules they impose (`laws.rs`).
//!
//! `blanked` is Law's Order: "all laws are blank until the end of this turn" (`laws::blanked`, a
//! Keleres faction mark; the typed marks do not carry it yet), so the caller computes it and passes
//! it in. While it is true no law is in play and none is elected.

use crate::containers::SmallList;
use crate::generated::{Agenda, Attachment, Planet, TechType, UnitBaseType};
use crate::ids::Seat;
use crate::state::{CoreState, Outcome};

use super::board;
use super::economy::Spend;

/// `ti4_model::state::RULES_DMZ_EXPLORATION`.
pub const RULES_DMZ_EXPLORATION: u32 = 34;

// The aliases of `state.laws` that the standing rules read.
pub const FLEET_REGULATIONS: Agenda = Agenda::FleetRegulations;
pub const SANCTIONS: Agenda = Agenda::ExecutiveSanctions;
pub const CENSURE: Agenda = Agenda::PoliticalCensure;
pub const SHARED_RESEARCH: Agenda = Agenda::SharedResearch;
pub const DEFENSE_ACT: Agenda = Agenda::HomelandDefenseAct;
pub const CONVENTIONS: Agenda = Agenda::ConventionsOfWar;
pub const DEMILITARIZED_ZONE: Agenda = Agenda::DemilitarizedZone;
pub const HOLY_PLANET_OF_IXTH: Agenda = Agenda::HolyPlanetOfIxth;
pub const CORE_MINING: Agenda = Agenda::CoreMining;
pub const SENATE_SANCTUARY: Agenda = Agenda::SenateSanctuary;
pub const TERRAFORMING_INITIATIVE: Agenda = Agenda::TerraformingInitiative;
pub const CONSCRIPTION: Agenda = Agenda::RegulatedConscription;
pub const REP_GOVT: Agenda = Agenda::RepresentativeGovernment;
pub const REPRESENTATIVE_GOVERNMENT: Agenda = Agenda::RepresentativeGovernmentBaseGame;
pub const TRAVEL_BAN: Agenda = Agenda::EnforcedTravelBan;
pub const WORMHOLE_RECON: Agenda = Agenda::WormholeReconstruction;
pub const MINISTER_EXPLORATION: Agenda = Agenda::MinisterOfExploration;
pub const MINISTER_POLICY: Agenda = Agenda::MinisterOfPolicy;

/// `laws::in_play` as agendas (key order of `state.laws`). Empty while blanked.
#[must_use]
pub fn in_play(core: &CoreState, blanked: bool) -> SmallList<Agenda, 16> {
    let mut out = SmallList::new();
    if !blanked {
        for law in core.agenda().laws.keys() {
            out.push(law);
        }
    }
    out
}

/// `laws::active`: the law is in play and laws are not blank.
#[must_use]
pub fn active(core: &CoreState, blanked: bool, law: Agenda) -> bool {
    !blanked && core.agenda().laws.contains_key(&law)
}

/// `laws::elected`: what the law was elected onto. `None` while blanked.
#[must_use]
pub fn elected(core: &CoreState, blanked: bool, law: Agenda) -> Option<Outcome> {
    if blanked { None } else { core.agenda().laws.get(&law).copied() }
}

/// `laws::elected(..) == planet`: the law was elected onto this planet.
#[must_use]
pub fn elected_planet(core: &CoreState, blanked: bool, law: Agenda, planet: Planet) -> bool {
    elected(core, blanked, law) == Some(Outcome::Planet(planet))
}

/// `laws::holds_ministry`.
#[must_use]
pub fn holds_ministry(core: &CoreState, blanked: bool, seat: Seat, law: Agenda) -> bool {
    elected(core, blanked, law) == Some(Outcome::Player(seat))
}

/// `laws::fleet_pool_cap`: Fleet Regulations holds the pool at four.
#[must_use]
pub fn fleet_pool_cap(core: &CoreState, blanked: bool, base: i32) -> i32 {
    if active(core, blanked, FLEET_REGULATIONS) { base.min(4) } else { base }
}

/// `laws::action_card_limit`: Sanctions caps the hand limit at three.
#[must_use]
pub fn action_card_limit(core: &CoreState, blanked: bool, base: usize) -> usize {
    if active(core, blanked, SANCTIONS) { base.min(3) } else { base }
}

/// The law half of `laws::action_cards_forbidden`: Political Censure's elected owner.
#[must_use]
pub fn censure_bars(core: &CoreState, blanked: bool, seat: Seat) -> bool {
    active(core, blanked, CENSURE) && elected(core, blanked, CENSURE) == Some(Outcome::Player(seat))
}

/// `laws::nebulae_passable`.
#[must_use]
pub fn nebulae_passable(core: &CoreState, blanked: bool) -> bool {
    active(core, blanked, SHARED_RESEARCH)
}

/// `laws::structure_cap_lifted`.
#[must_use]
pub fn structure_cap_lifted(core: &CoreState, blanked: bool, base: UnitBaseType) -> bool {
    base == UnitBaseType::Pds && active(core, blanked, DEFENSE_ACT)
}

/// `laws::bombardment_forbidden`: Conventions of War, on a planet in scope with the CULTURAL trait.
#[must_use]
pub fn bombardment_forbidden(core: &CoreState, blanked: bool, scope: super::Scope, planet: Planet) -> bool {
    use crate::generated::PlanetTrait;
    if !active(core, blanked, CONVENTIONS) {
        return false;
    }
    let data = planet.data();
    scope.0 & crate::tables::source_bit(data.source) != 0 && data.traits.contains(&PlanetTrait::Cultural)
}

/// `laws::planet_is_demilitarized`: the elected planet, or the exploration card's attachment
/// (rules version 34 and later).
#[must_use]
pub fn planet_is_demilitarized(core: &CoreState, blanked: bool, planet: Planet) -> bool {
    elected_planet(core, blanked, DEMILITARIZED_ZONE, planet)
        || (core.meta().rules_at_least(RULES_DMZ_EXPLORATION)
            && board::attachments(core, planet).as_slice().contains(&Attachment::DemilitarizedZone))
}

/// `laws::production_forbidden_on`: Holy Planet of Ixth.
#[must_use]
pub fn production_forbidden_on(core: &CoreState, blanked: bool, planet: Planet) -> bool {
    elected_planet(core, blanked, HOLY_PLANET_OF_IXTH, planet)
}

/// The laws that change a planet's value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueLaw {
    CoreMining,
    SenateSanctuary,
    TerraformingInitiative,
}

/// `laws::planet_value_bonus_sources`: `(law, resources, influence)`, in the string order.
#[must_use]
pub fn planet_value_bonus_sources(core: &CoreState, blanked: bool, planet: Planet) -> SmallList<(ValueLaw, i64, i64), 3> {
    let mut out = SmallList::new();
    if elected_planet(core, blanked, CORE_MINING, planet) {
        out.push((ValueLaw::CoreMining, 2, 0));
    }
    if elected_planet(core, blanked, SENATE_SANCTUARY, planet) {
        out.push((ValueLaw::SenateSanctuary, 0, 2));
    }
    if elected_planet(core, blanked, TERRAFORMING_INITIATIVE, planet) {
        out.push((ValueLaw::TerraformingInitiative, 1, 1));
    }
    out
}

/// `laws::planet_value_bonus`.
#[must_use]
pub fn planet_value_bonus(core: &CoreState, blanked: bool, planet: Planet, spend: Spend) -> i64 {
    planet_value_bonus_sources(core, blanked, planet)
        .as_slice()
        .iter()
        .map(|(_, r, i)| match spend {
            Spend::Resources => *r,
            Spend::Influence => *i,
        })
        .sum()
}

/// `laws::single_unit_production`.
#[must_use]
pub fn single_unit_production(core: &CoreState, blanked: bool) -> bool {
    active(core, blanked, CONSCRIPTION)
}

/// `laws::research_team_waivers`: prerequisites of `colour` the seat may waive (0 or 1).
#[must_use]
pub fn research_team_waivers(core: &CoreState, blanked: bool, seat: Seat, colour: TechType) -> usize {
    let law = match colour {
        TechType::Biotic => Agenda::ResearchTeamBiotic,
        TechType::Cybernetic => Agenda::ResearchTeamCybernetic,
        TechType::Propulsion => Agenda::ResearchTeamPropulsion,
        TechType::Warfare => Agenda::ResearchTeamWarfare,
        TechType::Unitupgrade => return 0,
    };
    let Some(Outcome::Planet(planet)) = elected(core, blanked, law) else {
        return 0;
    };
    if board::exhausted(core, planet) {
        return 0;
    }
    usize::from(board::controlled_planets(core, seat).as_slice().iter().any(|(_, p)| *p == planet))
}

/// `laws::flat_votes`: Representative Government (both printings).
#[must_use]
pub fn flat_votes(core: &CoreState, blanked: bool) -> bool {
    active(core, blanked, REP_GOVT) || active(core, blanked, REPRESENTATIVE_GOVERNMENT)
}

/// `laws::wormholes_suppressed`: Enforced Travel Ban.
#[must_use]
pub fn wormholes_suppressed(core: &CoreState, blanked: bool) -> bool {
    active(core, blanked, TRAVEL_BAN)
}

/// `laws::wormholes_all_connected`: Wormhole Reconstruction.
#[must_use]
pub fn wormholes_all_connected(core: &CoreState, blanked: bool) -> bool {
    active(core, blanked, WORMHOLE_RECON)
}

/// `laws::draws_at_status_end`: Minister of Policy.
#[must_use]
pub fn draws_at_status_end(core: &CoreState, blanked: bool, seat: Seat) -> bool {
    holds_ministry(core, blanked, seat, MINISTER_POLICY)
}
