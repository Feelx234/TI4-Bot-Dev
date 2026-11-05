//! Objectives a particular map can never satisfy (the pure half of `ti4_engine::map_feasibility`: the census of a galaxy and the
//! lists of objectives it rules out; the writes are the setup's and the string oracle's).
//!
//! **Project setup rule (not a printed rule).** Under the printed rules a revealed or dealt objective that nobody can score simply
//! stays in the game. This project decides otherwise: an unwinnable draw spoils a game, and the ordinary board generators make that
//! likely. See `ti4_engine::map_feasibility` for the long form.

use std::collections::BTreeSet;

use ti4_content::ContentStore;
use ti4_content::galaxy::{Galaxy, System, all_planets, all_systems};
use ti4_model::content_types::SourceSet;
use ti4_model::id::SystemId;

use crate::mecatol::{is_mecatol, mecatol_in_galaxy};

/// What a built map contains, as far as objectives care.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MapCensus {
    /// Players seated.
    pub players: usize,
    /// Systems that can hold units (on the grid, not hyperlane tiles, plus off-map tiles).
    pub holdable: usize,
    /// Of those, systems with no planet at all.
    pub planetless: usize,
    /// Systems with Mecatol Rex, a legendary planet, or an anomaly.
    pub notable: usize,
    /// Systems that hold units and have an anomaly next to them.
    pub beside_anomaly: usize,
    /// Systems with an anomaly (units may be there, so supernovae count: the bound is generous).
    pub anomalies: usize,
    /// Systems with a printed or token alpha wormhole / beta wormhole.
    pub alpha: usize,
    pub beta: usize,
    /// Planets on the map (stations excluded), and those outside every homeworld.
    pub planets: usize,
    pub non_home_planets: usize,
    /// The largest number of planets sharing one trait.
    pub most_common_trait: usize,
    pub cultural: usize,
    pub hazardous: usize,
    pub industrial: usize,
    pub legendary: usize,
    pub mecatol: bool,
    /// Total resource and influence value of all planets.
    pub resources: i64,
    pub influence: i64,
    /// Systems with two or more planets.
    pub multi_planet_systems: usize,
    /// The best any single seat can do on the rim (edge systems that hold units, minus its own
    /// home if that is on the edge).
    pub best_rim: usize,
    /// The best any single seat can do for structures on planets away from its own home.
    pub best_planets_away: usize,
    /// Systems that hold units and touch Mecatol Rex.
    pub beside_mecatol: usize,
    /// Systems that hold units and touch a home system.
    pub beside_a_home: usize,
}

/// Count what `galaxy` holds. `homes` are the seated players' home systems, in seat order.
#[must_use]
pub fn census(
    content: &ContentStore,
    sources: SourceSet,
    galaxy: &Galaxy,
    homes: &[SystemId],
) -> MapCensus {
    let systems = all_systems(content, sources);
    let planets = all_planets(content, sources);
    let mut holdable: Vec<&str> = galaxy.system_ids_holding_things();
    holdable.extend(galaxy.off_map_system_ids());
    let holdable_set: BTreeSet<&str> = holdable.iter().copied().collect();
    let mecatol = mecatol_in_galaxy(galaxy);

    let is_anomaly = |id: &str| systems.get(id).is_some_and(System::is_anomaly);
    let planets_of = |id: &str| -> Vec<&str> {
        systems.get(id).map_or_else(Vec::new, |s| {
            s.planets()
                .into_iter()
                .filter(|p| planets.get(p).is_some_and(|p| !p.is_space_station()))
                .collect()
        })
    };
    let is_legendary_system = |id: &str| {
        planets_of(id).iter().any(|p| {
            planets
                .get(p)
                .is_some_and(ti4_content::galaxy::Planet::is_legendary)
        })
    };

    let mut c = MapCensus {
        players: homes.len(),
        holdable: holdable.len(),
        mecatol: galaxy.coord_of(mecatol).is_some(),
        ..MapCensus::default()
    };
    let mut trait_counts = std::collections::BTreeMap::<String, usize>::new();
    for id in &holdable {
        let own = planets_of(id);
        if systems.get(id).is_some_and(|s| s.planets().is_empty()) {
            c.planetless += 1;
        }
        if own.len() >= 2 {
            c.multi_planet_systems += 1;
        }
        if is_anomaly(id) {
            c.anomalies += 1;
        }
        if is_mecatol(id) || is_anomaly(id) || is_legendary_system(id) {
            c.notable += 1;
        }
        if galaxy.adjacent(id).into_iter().any(|n| is_anomaly(n)) {
            c.beside_anomaly += 1;
        }
        let kinds = galaxy.wormhole_kinds(id);
        c.alpha += usize::from(kinds.contains("ALPHA"));
        c.beta += usize::from(kinds.contains("BETA"));
        for name in own {
            let Some(planet) = planets.get(name) else {
                continue;
            };
            c.planets += 1;
            c.non_home_planets += usize::from(planet.homeworld_of().is_none());
            c.resources += planet.resources();
            c.influence += planet.influence();
            c.legendary += usize::from(planet.is_legendary());
            for kind in planet.traits() {
                *trait_counts.entry(kind.to_ascii_uppercase()).or_default() += 1;
            }
        }
    }
    c.cultural = trait_counts.get("CULTURAL").copied().unwrap_or(0);
    c.hazardous = trait_counts.get("HAZARDOUS").copied().unwrap_or(0);
    c.industrial = trait_counts.get("INDUSTRIAL").copied().unwrap_or(0);
    c.most_common_trait = trait_counts.values().copied().max().unwrap_or(0);

    let edge: BTreeSet<&str> = galaxy
        .system_ids()
        .into_iter()
        .filter(|id| {
            galaxy.coord_of(id).is_some_and(|here| {
                here.neighbours()
                    .into_iter()
                    .any(|next| galaxy.system_at(next).is_none())
            })
        })
        .filter(|id| holdable_set.contains(id))
        .collect();
    let home_ids: BTreeSet<&str> = homes.iter().map(SystemId::as_str).collect();
    c.best_rim = homes
        .iter()
        .map(|home| edge.iter().filter(|id| **id != home.as_str()).count())
        .max()
        .unwrap_or(edge.len());
    c.best_planets_away = homes
        .iter()
        .map(|home| {
            holdable
                .iter()
                .filter(|id| **id != home.as_str())
                .map(|id| planets_of(id).len())
                .sum()
        })
        .max()
        .unwrap_or(c.planets);
    c.beside_mecatol = galaxy
        .adjacent(mecatol)
        .into_iter()
        .filter(|id| holdable_set.contains(id))
        .count();
    c.beside_a_home = holdable
        .iter()
        .filter(|id| {
            galaxy
                .adjacent(id)
                .into_iter()
                .any(|n| home_ids.contains(n))
        })
        .count();
    c
}

/// Public objectives (by alias) that this map can never let anyone score.
#[must_use]
pub fn unsatisfiable_public(c: &MapCensus) -> Vec<&'static str> {
    let mut out = Vec::new();
    let mut when = |alias: &'static str, impossible: bool| {
        if impossible {
            out.push(alias);
        }
    };
    when("corner", c.most_common_trait < 4);
    when("unify_colonies", c.most_common_trait < 6);
    when("expand_borders", c.non_home_planets < 6);
    when("subdue", c.non_home_planets < 11);
    // Attachments come from exploring a planet, which needs a trait (frontier cards aside).
    when("lost_outposts", c.cultural + c.hazardous + c.industrial < 2);
    when(
        "ancient_monuments",
        c.cultural + c.hazardous + c.industrial < 3,
    );
    when("intimidate", c.beside_mecatol < 2);
    when("outer_rim", c.best_rim < 3);
    when("control_borderlands", c.best_rim < 5);
    when("infrastructure", c.best_planets_away < 3);
    when("protect_border", c.best_planets_away < 5);
    when("deep_space", c.planetless < 3);
    when("vast_territories", c.planetless < 5);
    when("make_history", c.notable < 2);
    when("become_legend", c.notable < 4);
    when("push_boundaries", c.players < 3);
    when("distant_lands", c.players < 3);
    when("conquer", c.players < 2);
    when("monument", c.resources < 8);
    when("golden_age", c.resources < 16);
    when("sway_council", c.influence < 8);
    when("manipulate_law", c.influence < 16);
    when("amass_wealth", c.resources < 3 || c.influence < 3);
    when("vast_reserves", c.resources < 6 || c.influence < 6);
    out
}

/// Secret objectives (by alias) that this map can never let anyone score.
#[must_use]
pub fn unsatisfiable_secret(c: &MapCensus) -> Vec<&'static str> {
    let mut out = Vec::new();
    let mut when = |alias: &'static str, impossible: bool| {
        if impossible {
            out.push(alias);
        }
    };
    when("btgk", c.alpha == 0 || c.beta == 0);
    when("lsc", c.beside_anomaly < 3);
    when("btv", c.anomalies == 0);
    // Mirage can only arrive from a frontier token, and those sit in empty systems.
    when("sai", c.legendary == 0 && c.planetless == 0);
    when("faa", c.cultural < 4);
    when("mrm", c.hazardous < 4);
    when("mp", c.industrial < 4);
    when("ose", !c.mecatol);
    when("syc", c.multi_planet_systems == 0);
    when("te", c.beside_a_home == 0);
    when("ctr", c.holdable < 6);
    when("eh", c.influence < 12);
    when("hrm", c.resources < 12);
    when("csl", c.players < 2);
    when("dts", c.players < 2);
    out
}
