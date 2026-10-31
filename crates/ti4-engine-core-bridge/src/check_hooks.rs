//! The positions and variants that make the hook slots `commander_unlocked` and `leader_action` comparable (package F2).
//!
//! `check_leaders::variants` perturbs statuses, goods and technologies, but no variant reaches "12 ground forces", "units in three gravity
//! rifts", "a ship and a system to fly to" and the like, so the agreement of the hook slots there would prove nothing: the answer of both
//! sides is `false` everywhere. Here three tables seat every faction that owns one of the 29 string functions, and the variants satisfy and
//! miss each condition. [`EnvReport::outcomes`](crate::check_kernel_env::EnvReport) counts what the string side answered, so a test can
//! demand that both answers occurred for every leader (a hook that is never `true` is not compared on the interesting side).

use ti4_content::ContentStore;
use ti4_engine::generated::IdStr;
use ti4_engine_core::generated::{Leader, LeaderType, Planet, System, Technology};
use ti4_engine_core::tables::{Flags, SystemFlags};
use ti4_model::id::{LeaderId, ObjectiveId, PlanetId, PlayerId, SystemId, TechnologyId, UnitTypeId};
use ti4_model::state::{GameState, LeaderStatus};
use ti4_model::units::Unit;
use ti4_selfplay::spec::GameSpec;

use crate::workload::Position;

/// The factions of the three tables (6 seats each). Table 2 seats the Muaat with the Yssaril (the Ssruu route of Umbat).
const TABLES: [[&str; 6]; 3] = [
    ["arborec", "argent", "cabal", "empyrean", "ghost", "mahact"],
    ["mentak", "muaat", "nekro", "nomad", "saar", "yssaril"],
    ["winnu", "naaz", "titans", "yin", "sol", "hacan"],
];

/// Three tables on the first six-player template of the host.
#[must_use]
pub fn hook_positions() -> Vec<Position> {
    let Ok(loader) = ti4_host::maps::template_loader::TemplateLoader::load() else { return Vec::new() };
    let Some(summary) = loader.list_summaries().into_iter().find(|s| s.player_count == 6) else { return Vec::new() };
    let mut out = Vec::new();
    for (n, factions) in TABLES.iter().enumerate() {
        let spec = GameSpec {
            setup: "templates".to_owned(),
            seed: 1 + n as u64,
            players: 6,
            map: summary.alias.clone(),
            factions: factions.iter().map(|f| (*f).to_owned()).collect(),
            speaker: "seat".to_owned(),
            card_set: None,
            rotation: None,
            starting_tech: true,
            thunders_edge: false,
            rules_version: None,
        };
        if let Ok(game) = spec.build() {
            if let Some(galaxy) = game.galaxy() {
                out.push(Position { label: format!("hooks table {}", n + 1), state: game.state.clone(), galaxy: galaxy.clone(), sources: game.sources() });
            }
        }
    }
    out
}

/// Six colour technologies no faction owns or replaces (the commander unlocks count technologies).
fn plain_techs() -> Vec<TechnologyId> {
    use ti4_engine_core::query::tech;
    Technology::ALL
        .iter()
        .copied()
        .filter(|t| tech::colour_type(*t).is_some() && tech::faction_of(*t).is_none() && !tech::is_unit_upgrade(*t))
        .take(6)
        .map(|t| TechnologyId::new(t.as_str()))
        .collect()
}

/// The system Elysium is printed on.
fn elysium() -> SystemId {
    SystemId::new(Planet::Elysium.data().system.map_or("24", |s| s.as_str()))
}

pub(crate) fn ids(g: &GameState) -> Vec<PlayerId> {
    g.players.iter().map(|p| p.id.clone()).collect()
}

pub(crate) fn unit(kind: &str, owner: &PlayerId) -> Unit {
    Unit::new(UnitTypeId::new(kind), owner.clone())
}

/// Statuses for every leader of every seat: the agents, the heroes and the commanders each get their own.
fn statuses(g: &mut GameState, agent: LeaderStatus, hero: LeaderStatus, commander: LeaderStatus) {
    for seat in &mut g.players {
        for (id, status) in &mut seat.leaders {
            let kind = Leader::ALL.iter().find(|l| l.as_str() == id.as_str()).map(|l| ti4_engine_core::rules::leaders::kind(*l));
            *status = match kind {
                Some(LeaderType::Agent) => agent,
                Some(LeaderType::Hero) => hero,
                Some(LeaderType::Commander) => commander,
                None => *status,
            };
        }
    }
}

fn mixed(g: &mut GameState) {
    statuses(g, LeaderStatus::Readied, LeaderStatus::Unlocked, LeaderStatus::Locked);
}

pub(crate) fn home(g: &GameState, i: usize) -> Option<SystemId> {
    g.players.get(i).and_then(|p| p.home_system.clone())
}

fn first_planet(content: &ContentStore, g: &GameState, system: &SystemId) -> Option<PlanetId> {
    ti4_content::galaxy::planets_in(content, system.as_str(), ti4_model::content_types::DEFAULT).first().map(|p| PlanetId::new(p.id()))
        .or_else(|| g.board.get(system).and_then(|b| b.planet_control.keys().next().cloned()))
}

/// The first `n` systems in `ok`: those on the map first, then the others in id order (a unit may stand in a system the map does not hold).
fn systems_where(p: &Position, n: usize, ok: impl Fn(System) -> bool) -> Vec<SystemId> {
    // Out of the game's sources a system holds no planet for the core (the string reads the whole corpus in a few hooks): not a position the game reaches.
    let scope = ti4_engine_core::query::reach::scope_of(ti4_engine::live::kernel::kernel_for(p.sources));
    let all: Vec<System> = System::ALL.iter().copied().filter(|s| scope.system(*s).is_some() && ok(*s)).collect();
    let (on, off): (Vec<System>, Vec<System>) = all.into_iter().partition(|s| p.galaxy.coord_of(s.as_str()).is_some());
    on.into_iter().chain(off).take(n).map(|s| SystemId::new(s.as_str())).collect()
}

/// The conditions of the commanders, in numbers (the unlock is `>=`): `met` satisfies each, `missed` is one short of each.
#[derive(Clone, Copy)]
struct Knobs {
    infantry: usize,
    pds: usize,
    docks: usize,
    cruisers: usize,
    rifts: usize,
    wormholes: usize,
    techs: usize,
    secret: bool,
    mecatol: bool,
    pool: usize,
    everywhere: bool,
}

const MET: Knobs = Knobs { infantry: 12, pds: 6, docks: 3, cruisers: 4, rifts: 3, wormholes: 3, techs: 3, secret: true, mecatol: true, pool: 2, everywhere: true };
const MISSED: Knobs = Knobs { infantry: 11, pds: 5, docks: 2, cruisers: 3, rifts: 2, wormholes: 2, techs: 2, secret: false, mecatol: false, pool: 1, everywhere: false };

fn apply(p: &Position, g: &mut GameState, k: Knobs) {
    let content = ContentStore::embedded();
    let players = ids(g);
    let rifts = systems_where(p, k.rifts, |s| s.data().has(SystemFlags::GRAVITY_RIFT));
    let worms = systems_where(p, k.wormholes, |s| s.data().wormholes.iter().any(|w| matches!(w, ti4_engine_core::generated::Wormhole::Alpha | ti4_engine_core::generated::Wormhole::Beta)));
    let techs = plain_techs();
    for (i, id) in players.iter().enumerate() {
        let Some(h) = home(g, i) else { continue };
        let planet = first_planet(content, g, &h);
        // Ground forces, structures and ships at home, on a planet the seat controls.
        if let Some(planet) = planet {
            let sys = g.system_mut(&h);
            sys.planet_control.insert(planet.clone(), id.clone());
            let on = sys.planet_units.entry(planet).or_default();
            on.retain(|u| &u.owner != id || !matches!(u.type_id.as_str(), "infantry" | "pds" | "spacedock"));
            for _ in 0..k.infantry {
                on.push(unit("infantry", id));
            }
            for _ in 0..k.pds {
                on.push(unit("pds", id));
            }
            for _ in 0..k.docks {
                on.push(unit("spacedock", id));
            }
        }
        let sys = g.system_mut(&h);
        sys.units.retain(|u| &u.owner != id || u.type_id.as_str() != "cruiser");
        for _ in 0..k.cruisers {
            sys.units.push(unit("cruiser", id));
        }
        for s in rifts.iter().chain(worms.iter()) {
            g.system_mut(s).units.push(unit("fighter", id));
        }
        if let Some(seat) = g.player_mut(id) {
            seat.technologies.retain(|t| !techs.contains(t));
            for t in techs.iter().take(k.techs) {
                seat.technologies.insert(t.clone());
            }
        }
        if k.secret {
            let alias = ti4_engine_core::generated::SecretObjective::ALL[i % 4].as_str();
            g.scored_objectives.entry(id.clone()).or_default().insert(ObjectiveId::new(alias));
        } else {
            g.scored_objectives.remove(id);
        }
    }
    // Neighbours with everybody: a fighter of every seat in every home system.
    if k.everywhere {
        let homes: Vec<SystemId> = (0..players.len()).filter_map(|i| home(g, i)).collect();
        for h in homes {
            for id in &players {
                g.system_mut(&h).units.push(unit("fighter", id));
            }
        }
    }
    // Mecatol Rex.
    let mecatol = SystemId::new("18");
    if k.mecatol {
        if let Some(first) = players.first() {
            g.system_mut(&mecatol).planet_control.insert(PlanetId::new(Planet::MecatolRex.as_str()), first.clone());
        }
    } else {
        g.system_mut(&mecatol).planet_control.remove(&PlanetId::new(Planet::MecatolRex.as_str()));
    }
    // The Mahact fleet pools hold `pool` tokens of the other seats.
    for (i, id) in players.iter().enumerate() {
        if g.player(id).is_some_and(|s| s.faction.as_str() == "mahact") {
            let others: Vec<String> = players.iter().enumerate().filter(|(j, _)| *j != i).map(|(_, o)| o.to_string()).take(k.pool).collect();
            g.faction_marks.insert(format!("mahact:fleet:{id}"), others.join(","));
        }
    }
}

/// Every unit of the seat that is a ship leaves the board.
fn no_ships(g: &mut GameState) {
    let ships = ["fighter", "carrier", "cruiser", "destroyer", "dreadnought", "warsun"];
    for sys in g.board.values_mut() {
        sys.units.retain(|u| !ships.contains(&u.type_id.as_str()) && !u.type_id.as_str().ends_with("_flagship") && !u.type_id.as_str().starts_with("nomad_flagship"));
    }
}

pub(crate) fn add_space(g: &mut GameState, system: &SystemId, kind: &str, owner: &PlayerId, n: usize) {
    for _ in 0..n {
        g.system_mut(system).units.push(unit(kind, owner));
    }
}

/// Every table seat gets a war sun at home.
fn war_suns(g: &mut GameState) {
    for (i, id) in ids(g).iter().enumerate() {
        if let Some(h) = home(g, i) {
            add_space(g, &h, "warsun", id, 1);
        }
    }
}

/// A command token of every seat on Mecatol Rex (a place to fly to).
fn tokens_on_mecatol(g: &mut GameState) {
    for id in ids(g) {
        g.system_mut(&SystemId::new("18")).command_tokens.insert(id);
    }
}

fn broke(g: &mut GameState) {
    g.exhausted_planets.extend(g.board.values().flat_map(|s| s.planet_control.keys().cloned()).collect::<Vec<_>>());
    for seat in &mut g.players {
        seat.trade_goods = 0;
        seat.commodities = 0;
    }
}

/// The variants of one table, each with a label.
#[must_use]
pub fn hook_variants(p: &Position) -> Vec<(&'static str, GameState)> {
    let base = &p.state;
    let content = ContentStore::embedded();
    let mut out: Vec<(&'static str, GameState)> = Vec::new();
    let mut push = |label: &'static str, build: &dyn Fn(&mut GameState)| {
        let mut g = base.clone();
        build(&mut g);
        out.push((label, g));
    };
    push("hooks as is", &|_| {});
    push("hooks statuses", &mixed);
    push("hooks all unlocked", &|g| statuses(g, LeaderStatus::Unlocked, LeaderStatus::Unlocked, LeaderStatus::Unlocked));
    push("hooks met", &|g| {
        mixed(g);
        apply(p, g, MET);
    });
    push("hooks missed", &|g| {
        mixed(g);
        apply(p, g, MISSED);
    });
    push("hooks met, spent", &|g| {
        statuses(g, LeaderStatus::Exhausted, LeaderStatus::Purged, LeaderStatus::Unlocked);
        apply(p, g, MET);
    });
    push("hooks flock and war suns", &|g| {
        mixed(g);
        tokens_on_mecatol(g);
        war_suns(g);
    });
    push("hooks broke", &|g| {
        mixed(g);
        war_suns(g);
        broke(g);
    });
    push("hooks no ships", &|g| {
        mixed(g);
        tokens_on_mecatol(g);
        no_ships(g);
    });
    push("hooks box empty", &|g| {
        mixed(g);
        tokens_on_mecatol(g);
        for (i, id) in ids(g).iter().enumerate() {
            let Some(h) = home(g, i) else { continue };
            for (kind, n) in [("carrier", 4), ("cruiser", 8), ("destroyer", 8), ("dreadnought", 5), ("warsun", 2)] {
                add_space(g, &h, kind, id, n);
            }
            let faction = g.player(id).map(|s| s.faction.to_string()).unwrap_or_default();
            if let Some(flagship) = ti4_content::units::faction_unit(content, &faction, "flagship", ti4_model::content_types::DEFAULT) {
                add_space(g, &h, flagship.id(), id, 1);
            }
        }
    });
    push("hooks no cards", &|g| {
        mixed(g);
        for seat in &mut g.players {
            seat.action_cards.clear();
            seat.tactic_tokens = 0;
            seat.fleet_tokens = 0;
            seat.strategic_tokens = 0;
        }
    });
    push("hooks one hand", &|g| {
        mixed(g);
        let keep = g.players.first().map(|p| p.id.clone());
        for seat in &mut g.players {
            if Some(&seat.id) != keep.as_ref() {
                seat.action_cards.clear();
            } else if seat.action_cards.is_empty() {
                seat.action_cards.push(ti4_model::id::ActionCardId::new(ti4_engine_core::generated::ActionCard::ALL[0].as_str()));
            }
            seat.tactic_tokens = 0;
            seat.fleet_tokens = 0;
            seat.strategic_tokens = 0;
        }
    });
    push("hooks tail", &|g| {
        mixed(g);
        g.relic_deck.clear();
        g.unclaimed_strategy_cards.clear();
        for seat in &mut g.players {
            seat.strategy_cards.clear();
        }
        no_docks_and_full_infantry(g);
    });
    push("hooks specialty", &|g| {
        mixed(g);
        // A system with a technology specialty holds a fighter of every seat; Nomad's flagship; Elysium is in play.
        let special = systems_where(p, 1, |s| {
            Planet::ALL.iter().any(|pl| pl.data().system == Some(s) && !pl.data().specialties.is_empty())
        });
        for id in ids(g) {
            for s in &special {
                add_space(g, s, "fighter", &id, 1);
            }
            let flagship = g.player(&id).is_some_and(|s| s.faction.as_str() == "nomad");
            if flagship {
                if let Some(h) = g.player(&id).and_then(|s| s.home_system.clone()) {
                    add_space(g, &h, "nomad_flagship", &id, 1);
                }
            }
        }
        if let Some(first) = ids(g).first() {
            g.system_mut(&elysium()).planet_control.insert(PlanetId::new(Planet::Elysium.as_str()), first.clone());
        }
        g.relic_deck.clear();
    });
    push("hooks geoform and spent hero", &|g| {
        mixed(g);
        if let Some(first) = ids(g).first() {
            g.system_mut(&elysium()).planet_control.insert(PlanetId::new(Planet::Elysium.as_str()), first.clone());
        }
        g.planet_attachments.entry(PlanetId::new(Planet::Elysium.as_str())).or_default().push("titanshero".to_owned());
        let round = g.round.to_string();
        for id in ids(g) {
            if g.player(&id).is_some_and(|s| s.faction.as_str() == "nomad") {
                g.faction_marks.insert(format!("nomad:hero:{id}"), round.clone());
                if let Some(h) = g.player(&id).and_then(|s| s.home_system.clone()) {
                    add_space(g, &h, "nomad_flagship", &id, 1);
                }
            }
        }
    });
    push("hooks nekro tokens", &|g| {
        mixed(g);
        let others: Vec<PlayerId> = ids(g);
        for (i, id) in others.iter().enumerate() {
            let target = TechnologyId::new(plain_techs()[5].as_str());
            if g.player(id).is_some_and(|s| s.faction.as_str() == "nekro") {
                if let Some(seat) = g.player_mut(id) {
                    seat.technologies.clear();
                    let vax = Technology::ValefarAssimilatorX.as_str();
                    for t in [plain_techs()[0].as_str(), plain_techs()[1].as_str(), vax] {
                        seat.technologies.insert(TechnologyId::new(t));
                    }
                    if i % 2 == 0 {
                        seat.assimilated_technologies.insert(vax.to_owned(), target.clone());
                    }
                }
                // Another seat owns the target technology.
                for other in &others {
                    if other != id {
                        if let Some(seat) = g.player_mut(other) {
                            seat.technologies.insert(target.clone());
                        }
                    }
                }
            }
        }
    });
    // Sling Relay (a technology component action): owned and ready, spent, unaffordable, and every dock blockaded.
    push("hooks sling", &|g| {
        mixed(g);
        sling(g, false);
    });
    push("hooks sling spent", &|g| {
        mixed(g);
        sling(g, true);
    });
    push("hooks sling broke", &|g| {
        mixed(g);
        sling(g, false);
        broke(g);
    });
    push("hooks sling blockaded", &|g| {
        mixed(g);
        sling(g, false);
        apply(p, g, MET);
    });
    push("hooks ssruu", &|g| {
        statuses(g, LeaderStatus::Exhausted, LeaderStatus::Unlocked, LeaderStatus::Locked);
        for seat in &mut g.players {
            if seat.faction.as_str() == "yssaril" {
                seat.leaders.insert(LeaderId::new("yssarilagent"), LeaderStatus::Readied);
            }
        }
        war_suns(g);
    });
    // The Ssruu holder looks at agents that are spent for good: a purged or locked agent is nobody's to copy.
    push("hooks ssruu, gone agents", &|g| {
        statuses(g, LeaderStatus::Purged, LeaderStatus::Unlocked, LeaderStatus::Locked);
        for seat in &mut g.players {
            if seat.faction.as_str() == "yssaril" {
                seat.leaders.insert(LeaderId::new("yssarilagent"), LeaderStatus::Readied);
            }
        }
        war_suns(g);
    });
    out
}

/// Every seat owns Sling Relay, spent or ready.
fn sling(g: &mut GameState, spent: bool) {
    let sr = TechnologyId::new(Technology::SlingRelay.as_str());
    for seat in &mut g.players {
        seat.technologies.insert(sr.clone());
        if spent {
            seat.exhausted_technologies.insert(sr.clone());
        } else {
            seat.exhausted_technologies.remove(&sr);
        }
    }
}

fn no_docks_and_full_infantry(g: &mut GameState) {
    for (i, id) in ids(g).iter().enumerate() {
        let Some(h) = home(g, i) else { continue };
        let sys = g.system_mut(&h);
        sys.units.retain(|u| u.type_id.as_str() != "spacedock");
        for units in sys.planet_units.values_mut() {
            units.retain(|u| u.type_id.as_str() != "spacedock");
        }
        if let Some(planet) = sys.planet_control.keys().next().cloned() {
            let on = sys.planet_units.entry(planet).or_default();
            for _ in 0..22 {
                on.push(unit("infantry", id));
            }
        }
    }
}
