//! From `technology_core.rs` of the legacy crate: the printed-data queries of `ti4_engine_core::query::tech` by alias, and the unit roster
//! of a source scope (copies; the research position queries stay with the string rules).

use std::collections::BTreeMap;

use ti4_engine_core::generated::{Faction, FactionTag, TechType, Technology};
use ti4_engine_core::query::tech::{self as q, Colours};
use ti4_engine_core::state::TechSet;
use ti4_engine_core::tables::source_bit;
use ti4_model::content_types::SourceSet;
use ti4_model::id::TechnologyId;

use crate::generated::IdStr;

/// The typed technology of an alias.
#[inline]
#[must_use]
pub fn tech(alias: &str) -> Option<Technology> {
    <Technology as IdStr>::from_id(alias)
}

pub use ti4_engine_core_setup::roster::scope;

/// The corpus spelling of a research track (`technology::COLOURS`).
#[must_use]
pub fn colour_name(t: TechType) -> &'static str {
    <TechType as IdStr>::as_str(t)
}

/// The colour map of the string engine (`BTreeMap<&str, usize>`, zeros absent).
#[must_use]
pub fn colour_map(counts: Colours) -> BTreeMap<&'static str, usize> {
    q::COLOURS
        .into_iter()
        .zip(counts)
        .filter(|(_, n)| *n > 0)
        .map(|(c, n)| (colour_name(c), usize::from(n)))
        .collect()
}

/// A colour name of the string engine (`BIOTIC` ...) as a track.
#[must_use]
pub fn track(name: &str) -> Option<usize> {
    <TechType as IdStr>::from_id(name).and_then(q::colour_index)
}

/// Ids of a typed set, in id order (the order of the string engine's sorted `Vec<TechnologyId>`).
#[must_use]
pub fn ids(set: &TechSet) -> Vec<TechnologyId> {
    set.iter().map(|t| TechnologyId::new(<Technology as IdStr>::as_str(t))).collect()
}

// --- printed data ------------------------------------------------------------------------------

#[must_use]
pub fn colour_type(alias: &TechnologyId) -> Option<Option<&'static str>> {
    Some(q::colour_type(tech(alias.as_str())?).map(colour_name))
}

#[must_use]
pub fn is_unit_upgrade(alias: &TechnologyId) -> Option<bool> {
    Some(q::is_unit_upgrade(tech(alias.as_str())?))
}

#[must_use]
pub fn faction_of(alias: &TechnologyId) -> Option<Option<&'static str>> {
    Some(q::faction_of(tech(alias.as_str())?).map(<FactionTag as IdStr>::as_str))
}

#[must_use]
pub fn replaced_for_faction(faction: &str, alias: &TechnologyId) -> Option<bool> {
    let faction = <Faction as IdStr>::from_id(faction)?;
    Some(q::replaced_for_faction(faction, tech(alias.as_str())?))
}

#[must_use]
pub fn prerequisites(alias: &TechnologyId) -> Option<BTreeMap<&'static str, usize>> {
    Some(colour_map(q::prerequisites(tech(alias.as_str())?)))
}

#[must_use]
pub fn active_aliases(sources: SourceSet) -> std::collections::BTreeSet<TechnologyId> {
    q::active(scope(sources)).iter().map(|t| TechnologyId::new(<Technology as IdStr>::as_str(t))).collect()
}

// --- the string state as a research context ----------------------------------------------------


pub use ti4_engine_core_setup::roster::roster_for;

// --- the string state at setup as a research context (D2-9b: the thin crate has no string engine to ask) ----------------------------

/// The faction hooks of the research questions at a seat of a position that is being made (`starting_tech`): no purge (the Deepwrought
/// are refused), the Analytical ability's waiver, and nothing else a faction effect could add. The Prophet's Tears and the Research Team laws are read by
/// [`setup_researchable_by_prerequisites`]; the Publicize Weapon Schematics law, the Yin commander and the card-bound waiver offers need a played
/// game (a law passed, a leader unlocked, a card gained), which a position that is being made has not.
struct SetupHooks {
    analytical: bool,
}

impl q::ResearchHooks for SetupHooks {
    fn purged(&self, _: Technology) -> bool {
        false
    }
    fn waived_prerequisites(&self, t: Technology) -> usize {
        usize::from(self.analytical && !q::is_unit_upgrade(t))
    }
    fn extra_colours(&self) -> Colours {
        [0; 4]
    }
    fn inheritance_affordable(&self) -> bool {
        false
    }
    fn waiver_offer(&self, _: Technology) -> bool {
        false
    }
}

/// The technologies `player` could research with the prerequisites met, in a position that is being made (`starting_tech::options`,
/// `StartingTechSource::Researchable`; `technology::researchable_by_prerequisites` of the legacy crate for a seat that has played nothing).
/// `None` when the seat holds something the typed tables do not know.
#[must_use]
pub fn setup_researchable_by_prerequisites(state: &ti4_model::state::GameState, sources: SourceSet, player: &ti4_model::id::PlayerId) -> Option<Vec<TechnologyId>> {
    let seat = state.player(player)?;
    let sc = scope(sources);
    let faction = <Faction as IdStr>::from_id(seat.faction.as_str())?;
    let mut owned = TechSet::new();
    for t in &seat.technologies {
        owned.insert(tech(t.as_str())?);
    }
    let held = q::colours_of(&owned);
    let free = state.rules_version >= q::RULES_TECH_EFFECTS && seat.technologies.contains(ti4_model::id::text(&"pa"));
    let mut spec = [0u16; 4];
    for (_, planet) in state.controlled_planets(player) {
        if !free && state.exhausted_planets.contains(planet) {
            continue;
        }
        let typed = <ti4_engine_core::generated::Planet as IdStr>::from_id(planet.as_str())?;
        let mut attached = Vec::new();
        for id in state.planet_attachments.get(planet).into_iter().flatten() {
            attached.push(<ti4_engine_core::generated::Attachment as IdStr>::from_id(id)?);
        }
        for s in q::specialties_of(sc, typed, attached).as_slice() {
            if let Some(i) = q::colour_index(*s) {
                spec[i] += 1;
            }
        }
    }
    let mut holdings = [0u16; 4];
    for i in 0..4 {
        holdings[i] = held[i] + spec[i];
    }
    let mut joined = [false; 4];
    if seat.relics.iter().any(|r| r.as_str() == "quantumcore") {
        joined = [true; 4];
    } else if let Some(b) = &seat.breakthrough {
        let d = <ti4_engine_core::generated::Breakthrough as IdStr>::from_id(b.as_str())?.data();
        if sc.0 & source_bit(d.source) != 0
            && let [first, second] = d.synergy
        {
            for c in [first, second] {
                if let Some(i) = q::colour_index(*c) {
                    joined[i] = true;
                }
            }
        }
    }
    // The waivers that do not depend on the technology: the Prophet's Tears, and the Research Team laws (a controlled, readied planet).
    let mut waivable = usize::from(seat.relics.iter().any(|r| r.as_str() == "prophetstears"));
    for colour in q::COLOURS {
        let alias = match colour_name(colour) {
            "BIOTIC" => "rt_biotic",
            "CYBERNETIC" => "rt_cybernetic",
            "PROPULSION" => "rt_propulsion",
            _ => "rt_warfare",
        };
        if let Some(planet) = crate::queries::laws::elected(state, alias) {
            let planet = ti4_model::id::PlanetId::new(planet.clone());
            waivable += usize::from(!state.exhausted_planets.contains(&planet) && state.controlled_planets(player).into_iter().any(|(_, held)| *held == planet));
        }
    }
    let ready = |t: Technology| owned.contains(t) && !seat.exhausted_technologies.contains(ti4_model::id::text(<Technology as IdStr>::as_str(t)));
    let ctx = q::ResearchCtx {
        faction,
        owned,
        holdings,
        waivable,
        joined,
        aida_ready: state.rules_version >= q::RULES_TECH_EFFECTS && ready(Technology::AIDevelopmentAlgorithm),
        inheritance_card: ready(Technology::InheritanceSystems),
        war_sun_waived: false,
    };
    let hooks = SetupHooks { analytical: faction.data().abilities.contains(&ti4_engine_core::generated::Ability::Analytical) };
    Some(ids(&ctx.researchable_by_prerequisites(sc, &hooks)))
}
