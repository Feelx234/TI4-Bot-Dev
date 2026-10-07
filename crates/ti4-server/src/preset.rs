//! Start presets: prepared opening states for random smoke playthroughs.
//!
//! A random click policy rarely reaches combat, casualties, Mecatol Rex or the agenda phase from
//! the standard opening. A preset only changes the *starting state* (extra fleets, a little trade
//! goods); it never changes a rule. The result is stored in the game's initial state, so crash
//! recovery restores it from the saved state rather than rebuilding it.

use std::collections::{BTreeMap, BTreeSet};

use ti4_content::ContentStore;
use ti4_content::factions;
use ti4_content::galaxy::{self, Galaxy};
use ti4_content::units;
use ti4_engine::seating::{self, MECATOL};
use ti4_engine::{fleet, invasion, leaders, production};
use ti4_model::content_types::{ContentType, POK};
use ti4_model::id::{
    ActionCardId, FactionId, PlanetId, PlayerId, RelicId, SystemId, TechnologyId, UnitTypeId,
};
use ti4_model::state::{GameState, LeaderStatus};
use ti4_model::units::Unit;

/// Fleets beside opponents' home systems, one raider already in Mecatol Rex and the others beside
/// it, with influence for the custodians.
pub const COMBAT: &str = "combat";

/// Every seat holds a hand of action cards that random play has almost never produced.
pub const CARDS: &str = "cards";

/// The agenda phase from round one, with a hand-ordered deck and agenda-window action cards.
pub const AGENDA: &str = "agenda";

/// Every seat holds relics, one of them with a cultural fragment set for crossing.
pub const RELICS: &str = "relics";

/// Each seat holds a defended colony (infantry, mech, PDS on a planet near its home) and the
/// previous seat's invasion force, with a dreadnought, waits in its space area: space cannon,
/// bombardment and ground combat with casualties are the first thing a tactical action reaches.
pub const INVASION: &str = "invasion";

/// The invasion setup, with every seat owning the technologies that open their own prompts.
pub const TECHS: &str = "techs";

/// Every seat holds all its leaders (heroes and commanders unlocked, agents readied), the factions
/// are rotated so Jol-Nar and L1Z1X sit at small tables, and legendary planets on the map are
/// handed out.
pub const LEADERS: &str = "leaders";

/// Suffix on any preset name that also rotates the factions (see [`rotation`]).
pub const ROTATE: &str = "+rot";

/// Every preset name the server accepts. Keep in step with `KNOWN_PRESETS` in
/// `web/e2e/smokePreset.ts` (a test below compares the two).
pub const KNOWN: &[&str] = &[COMBAT, CARDS, AGENDA, RELICS, INVASION, TECHS, LEADERS];

/// Action cards no nightly game ever played, found by diffing 72 final states' discard piles
/// against the corpus. The first ten are in the standard deck; the rest are Thunder's Edge cards,
/// which a standard game never deals, so a hand is the only way their windows and prompts get
/// exercised at all. Cards the corpus lacks are skipped.
const CARD_POOL: &[&str] = &[
    "confusing", "dh2", "mjets1", "parley", "upgrade", "ghost_squad", "rally", "war_machine2",
    "decoy", "reparations", "crashlanding", "exchangeprogram", "lieinwait", "puppetsonastring",
    "extremeduress", "rescue", "strategize1", "piratecontract1", "blackmarketdealing", "overrule",
    "mercenarycontract", "crisis",
];

/// The top of the agenda deck, two per round: round 1 an elect-player law and a for/against law
/// (so a law is in play), round 2 an elect-law agenda (it is discarded when no law is in play,
/// hence the laws first), an elect-planet-style directive, then more elections and laws. Aliases
/// the corpus lacks are skipped.
const AGENDA_ORDER: &[&str] = &[
    "committee", "defense_act", "abolishment", "redistribution", "miscount", "classified",
    "rep_govt", "crisis", "disarmament", "arbiter", "covert", "execution", "checks",
];

/// Action cards with agenda windows (two riders, a veto, hacking, bribery, confusing text), dealt
/// so that every revealed agenda gives some seat something to play. Only two riders: a player who
/// predicted an outcome may not vote (vote.rs), so a rider in every hand, played at the first
/// reveal, leaves nobody to vote and the agenda phase never asks `cast_vote` (seen in the first
/// smoke run of this preset).
const AGENDA_CARD_POOL: &[&str] = &[
    "lead_rider", "imp_rider", "bribery", "hack", "sanction", "insider", "veto", "assassin",
    "distinguished", "confounding", "deadly_plot", "confusing", "abs", "dp1",
];

/// Trade goods each seat gets, enough to matter for votes and riders without a purchase spree.
const AGENDA_TRADE_GOODS: i32 = 3;

/// Relics, dealt round-robin in this order so the interesting ones always land: a standard game
/// reaches relics only through exploration and fragments, so almost none was ever held. Thunder's
/// Edge's Heart of Ixth is included for its die-adjust window; the rest are the standard deck's.
/// Relics the corpus lacks are skipped.
const RELIC_POOL: &[&str] = &[
    "neuraloop", "stellarconverter", "dominusorb", "emphidia", "titanprototype", "heartofixth",
    "thalnos", "codex", "bookoflatvinia", "prophetstears", "mawofworlds", "enigmaticdevice",
    "dynamiscore", "emelpar", "nanoforge", "circletofthevoid",
];

/// Relics per seat at most.
const RELICS_PER_SEAT: usize = 3;

/// Technologies owned from the start in the `techs` preset: the ones with their own prompts or
/// exhaust windows (Quantum Datahub Node, Spatial Conduit Cylinders, Nullification Field, Chaos
/// Mapping, Bio-Stims, Transit Diodes, AI Development Algorithm, Psychoarchaeology, Sling Relay,
/// Magen Defense Grid, Scanlink Drone Network, Supercharge, Vortex), plus the production and
/// bombardment ones that change what an invasion costs and does. Prerequisites are not checked.
const TECH_POOL: &[&str] = &[
    "qdn", "scc", "nf", "cm", "bs", "td", "aida", "pa", "sr", "md", "sdn", "sc", "vtx", "st",
    "mc", "l4", "x89c4",
];

/// Cards per seat: well under the hand limit of 7, so nothing is discarded at the status phase.
const HAND_SIZE: usize = 5;

/// Placed one jump from an opponent's home. The dreadnought (with the faction's mech) makes sure
/// sustain damage happens; capacity 5: two fighters, two infantry and the mech fit. Three
/// non-fighter ships fill the opening fleet supply of three.
const STRIKE_FLEET: &[(&str, usize)] = &[
    ("carrier", 1),
    ("destroyer", 1),
    ("dreadnought", 1),
    ("fighter", 2),
    ("infantry", 2),
];

/// Placed in or beside Mecatol Rex with ground forces, so the custodians can be lifted. Ground
/// forces in a system's space area are landable (see `invasion::landable`), so the one already in
/// Mecatol can lift the custodians the first time it activates the system.
const RAIDING_PARTY: &[(&str, usize)] = &[
    ("carrier", 1),
    ("cruiser", 1),
    ("dreadnought", 1),
    ("infantry", 2),
];

/// Placed in a defended colony's space area. The carrier and the dreadnought's own slot carry four
/// infantry and the faction's mech (capacity 5); the dreadnought bombards, and with the mech and
/// the defenders' casualties there is a choice of unit to assign hits to on both sides.
const INVASION_FLEET: &[(&str, usize)] = &[
    ("carrier", 1),
    ("dreadnought", 1),
    ("cruiser", 1),
    ("infantry", 4),
];

/// What a colony's planet holds: ground forces to fight and a PDS (space cannon) to shoot first.
const COLONY_DEFENDERS: &[(&str, usize)] = &[("infantry", 3), ("pds", 2)];

/// Reinforcement pool sizes (LRR 76.1): a preset never puts more of a type on the board.
const POOL: &[(&str, usize)] = &[
    ("carrier", 4),
    ("cruiser", 8),
    ("destroyer", 8),
    ("dreadnought", 5),
    ("fighter", 10),
    ("infantry", 12),
    ("mech", 4),
    ("pds", 6),
];

#[must_use]
pub fn is_known(name: &str) -> bool {
    KNOWN.contains(&base(name))
}

/// The preset without its `+rot` suffix.
fn base(name: &str) -> &str {
    name.strip_suffix(ROTATE).unwrap_or(name)
}

/// How far the factions are rotated along `IN_SCOPE_FACTIONS` for this preset and seed: 0 unless
/// the preset is `leaders` or has the `+rot` suffix, else 2 to 5, which always seats Jol-Nar or
/// L1Z1X even at three seats (the fixed order reaches them only at five). Seeded, so the same
/// request builds the same table. The game creator and the preset both ask here, so the home
/// systems the preset looks up are those of the factions actually seated.
#[must_use]
pub fn rotation(name: &str, seed: u64) -> usize {
    if name == LEADERS || name.ends_with(ROTATE) {
        2 + (mix(seed, 900) % 4) as usize
    } else {
        0
    }
}

/// Applies the named preset to a freshly seated game.
///
/// # Errors
/// An unknown preset name, or a placement that would break fleet supply, capacity or a unit pool.
pub fn apply(
    content: &ContentStore,
    state: &mut GameState,
    galaxy: &Galaxy,
    players: &[PlayerId],
    seed: u64,
    preset: &str,
) -> Result<(), String> {
    match base(preset) {
        COMBAT => combat(content, state, galaxy, players, seed),
        LEADERS => {
            unlock_leaders(content, state, galaxy, players);
            Ok(())
        }
        CARDS => {
            deal_cards(content, state, players, seed, CARD_POOL, HAND_SIZE);
            Ok(())
        }
        AGENDA => {
            agenda(content, state, players, seed);
            Ok(())
        }
        RELICS => {
            deal_relics(content, state, players, seed);
            Ok(())
        }
        INVASION => invasion_preset(content, state, galaxy, players, seed),
        TECHS => {
            invasion_preset(content, state, galaxy, players, seed)?;
            grant_techs(content, state, players);
            Ok(())
        }
        other => Err(format!("unknown start_preset {other:?}")),
    }
}

/// The factions actually seated, read from the state (whatever rotation created the game).
fn seated(state: &GameState, players: &[PlayerId]) -> BTreeMap<PlayerId, FactionId> {
    players
        .iter()
        .filter_map(|player| {
            state
                .player(player)
                .map(|seat| (player.clone(), seat.faction.clone()))
        })
        .collect()
}

/// splitmix64 over `seed` and a salt: a stable choice that depends on nothing but its inputs.
fn mix(seed: u64, salt: u64) -> u64 {
    let mut z = seed
        .wrapping_add(salt.wrapping_mul(0x9E37_79B9_7F4A_7C15))
        .wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Lifting the custodians is not needed: 27.4 makes every round's agenda phase happen once they
/// are gone, so the flag is set and the deck's top is replaced by [`AGENDA_ORDER`].
fn agenda(content: &ContentStore, state: &mut GameState, players: &[PlayerId], seed: u64) {
    state.custodians_removed = true;
    let top: Vec<String> = AGENDA_ORDER
        .iter()
        .filter(|alias| state.agenda_deck.iter().any(|a| a == *alias))
        .map(|alias| (*alias).to_owned())
        .collect();
    state.agenda_deck.retain(|alias| !top.contains(alias));
    state.agenda_deck.splice(0..0, top);
    deal_cards(content, state, players, seed, AGENDA_CARD_POOL, 4);
    for player in players {
        if let Some(seat) = state.player_mut(player) {
            seat.trade_goods += AGENDA_TRADE_GOODS;
        }
    }
}

/// One defended colony per seat, and the previous seat's invasion force waiting above it.
fn invasion_preset(
    content: &ContentStore,
    state: &mut GameState,
    galaxy: &Galaxy,
    players: &[PlayerId],
    seed: u64,
) -> Result<(), String> {
    let assignments = seated(state, players);
    let homes: Vec<SystemId> = players
        .iter()
        .map(|player| {
            let one = BTreeMap::from([(player.clone(), assignments[player].clone())]);
            seating::home_systems(content, &one).map(|mut homes| homes.remove(0))
        })
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    let home_set: BTreeSet<&str> = homes.iter().map(SystemId::as_str).collect();
    let count = players.len();
    let mut used: BTreeSet<String> = BTreeSet::new();

    // Colonies first, so every fleet can be put beside one.
    let mut colonies: Vec<Option<(SystemId, PlanetId)>> = Vec::new();
    for (j, player) in players.iter().enumerate() {
        let colony = pick_site_of(
            content,
            state,
            galaxy,
            homes[j].as_str(),
            &home_set,
            &used,
            mix(seed, 600 + j as u64),
            2,
            Site::Colony,
        )
        .and_then(|site| colony_planet(content, site.as_str()).map(|planet| (site, planet)));
        if let Some((site, planet)) = &colony {
            used.insert(site.as_str().to_owned());
            let mut defenders = fleet_for(content, &assignments[player], COLONY_DEFENDERS);
            // fleet_for adds a dreadnought-less mech; keep it, the planet is a ground area.
            let board = state.system_mut(site);
            board.set_control(planet.clone(), player.clone());
            for (kind, number) in defenders.drain(..) {
                for _ in 0..number {
                    board
                        .planet_units
                        .entry(planet.clone())
                        .or_default()
                        .push(Unit::new(kind.clone(), player.clone()));
                }
            }
            check_pools(state, player)?;
        }
        colonies.push(colony);
    }

    // The attacker is the previous seat. Its fleet waits in the colony's own space area: ground
    // forces there are landable without moving (see `RAIDING_PARTY`), so the first tactical action
    // that activates the colony meets the PDS's space cannon, the dreadnought's bombardment and a
    // ground combat, instead of a random move that strands the infantry (a carrier that leaves
    // without them drops them over capacity).
    for (i, player) in players.iter().enumerate() {
        let Some((colony, _)) = &colonies[(i + 1) % count] else {
            continue;
        };
        let fleet = fleet_for(content, &assignments[player], INVASION_FLEET);
        place(content, state, player, colony, &fleet)?;
    }
    Ok(())
}

/// Every seat owns every technology of [`TECH_POOL`] the corpus has.
fn grant_techs(content: &ContentStore, state: &mut GameState, players: &[PlayerId]) {
    for player in players {
        if let Some(seat) = state.player_mut(player) {
            for id in TECH_POOL {
                if content.get(ContentType::Technologies, id).is_some() {
                    seat.technologies.insert(TechnologyId::new(*id));
                }
            }
        }
    }
}

/// Every leader of every seat is usable at once: heroes and commanders unlocked, agents readied.
/// Legendary planets on the map that nobody controls go to the seats in turn, so their abilities
/// have an owner.
fn unlock_leaders(
    content: &ContentStore,
    state: &mut GameState,
    galaxy: &Galaxy,
    players: &[PlayerId],
) {
    for player in players {
        let Some(seat) = state.player_mut(player) else {
            continue;
        };
        let leaders: Vec<_> = seat.leaders.keys().cloned().collect();
        for leader in leaders {
            let status = if leaders::kind_of(content, &leader).as_deref() == Some(leaders::AGENT) {
                LeaderStatus::Readied
            } else {
                LeaderStatus::Unlocked
            };
            seat.leaders.insert(leader, status);
        }
    }
    let on_map: BTreeSet<&str> = galaxy.system_ids().into_iter().collect();
    let mut legendary: Vec<(SystemId, PlanetId)> = galaxy::all_planets(content, POK)
        .values()
        .filter(|planet| planet.is_legendary())
        .filter_map(|planet| {
            let system = planet.system_id()?;
            on_map
                .contains(system)
                .then(|| (SystemId::new(system), PlanetId::new(planet.id())))
        })
        .collect();
    legendary.sort();
    for (index, (system, planet)) in legendary.into_iter().enumerate() {
        let taken = state
            .board
            .get(&system)
            .is_some_and(|board| board.planet_control.contains_key(&planet));
        if !taken {
            let owner = players[index % players.len()].clone();
            state.system_mut(&system).set_control(planet, owner);
        }
    }
}

/// Hands the relics of [`RELIC_POOL`] out round-robin from a seeded first seat and takes them out
/// of the relic deck. The seat after the first also gets three cultural fragments (crossing).
fn deal_relics(content: &ContentStore, state: &mut GameState, players: &[PlayerId], seed: u64) {
    let count = players.len();
    let first = (mix(seed, 500) % count as u64) as usize;
    let pool: Vec<&str> = RELIC_POOL
        .iter()
        .copied()
        .filter(|id| content.get(ContentType::Relics, id).is_some())
        .take(count * RELICS_PER_SEAT)
        .collect();
    for (index, id) in pool.iter().enumerate() {
        state.relic_deck.retain(|relic| relic.as_str() != *id);
        if let Some(seat) = state.player_mut(&players[(first + index) % count]) {
            seat.relics.push(RelicId::new(*id));
        }
    }
    if let Some(seat) = state.player_mut(&players[(first + 1) % count]) {
        seat.relic_fragments.insert("CULTURAL".to_owned(), 3);
    }
}

/// Deals each seat a different seeded slice of `pool_ids` (at most `hand` cards each) and takes
/// those cards out of the deck.
fn deal_cards(
    content: &ContentStore,
    state: &mut GameState,
    players: &[PlayerId],
    seed: u64,
    pool_ids: &[&str],
    hand: usize,
) {
    let mut pool: Vec<&str> = pool_ids
        .iter()
        .copied()
        .filter(|id| content.get(ContentType::ActionCards, id).is_some())
        .collect();
    pool.sort_by_key(|id| mix(seed, id.bytes().fold(7_u64, |h, b| h.wrapping_mul(31).wrapping_add(u64::from(b)))));
    let per_seat = hand.min(pool.len() / players.len().max(1));
    for (player, hand) in players.iter().zip(pool.chunks(per_seat.max(1))) {
        for id in hand {
            state.action_card_deck.retain(|card| card.as_str() != *id);
            if let Some(seat) = state.player_mut(player) {
                seat.action_cards.push(ActionCardId::new(*id));
            }
        }
    }
}

fn combat(
    content: &ContentStore,
    state: &mut GameState,
    galaxy: &Galaxy,
    players: &[PlayerId],
    seed: u64,
) -> Result<(), String> {
    let assignments = seated(state, players);
    let homes: Vec<SystemId> = players
        .iter()
        .map(|player| {
            let one = BTreeMap::from([(player.clone(), assignments[player].clone())]);
            seating::home_systems(content, &one).map(|mut homes| homes.remove(0))
        })
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    let home_set: BTreeSet<&str> = homes.iter().map(SystemId::as_str).collect();
    let count = players.len();

    // Half the seats (rounded up) raid Mecatol, chosen by a seeded ranking; the first of them
    // starts inside Mecatol, the rest beside it.
    let mut ranking: Vec<usize> = (0..count).collect();
    ranking.sort_by_key(|i| mix(seed, 100 + *i as u64));
    let holder = ranking[0];
    let mut raiders = vec![false; count];
    for i in ranking.into_iter().take(count.div_ceil(2)) {
        raiders[i] = true;
    }

    let mut used: BTreeSet<String> = BTreeSet::new();
    for (i, player) in players.iter().enumerate() {
        if count > 1 {
            let other = (i + 1 + (mix(seed, 200 + i as u64) as usize) % (count - 1)) % count;
            if let Some(site) = pick_site(
                content,
                state,
                galaxy,
                homes[other].as_str(),
                &home_set,
                &used,
                mix(seed, 300 + i as u64),
                2,
            ) {
                let fleet = fleet_for(content, &assignments[player], STRIKE_FLEET);
                place(content, state, player, &site, &fleet)?;
                used.insert(site.as_str().to_owned());
            }
        }
        if raiders[i] {
            let mecatol = SystemId::new(MECATOL);
            let site = if i == holder && eligible_in_mecatol(state) {
                Some(mecatol)
            } else {
                pick_site(
                    content,
                    state,
                    galaxy,
                    MECATOL,
                    &home_set,
                    &used,
                    mix(seed, 400 + i as u64),
                    1,
                )
            };
            if let Some(site) = site {
                let fleet = fleet_for(content, &assignments[player], RAIDING_PARTY);
                place(content, state, player, &site, &fleet)?;
                used.insert(site.as_str().to_owned());
            }
            top_up_influence(content, state, player);
        }
    }
    Ok(())
}

/// What kind of system a preset wants to put something in.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Site {
    /// Any empty ordinary system.
    Empty,
    /// An empty ordinary system with a planet that is not legendary, to hold a colony.
    Colony,
}

/// The nearest eligible system to `from`: one jump away if any qualifies, else up to `max_jumps`.
#[allow(clippy::too_many_arguments)]
fn pick_site(
    content: &ContentStore,
    state: &GameState,
    galaxy: &Galaxy,
    from: &str,
    home_set: &BTreeSet<&str>,
    used: &BTreeSet<String>,
    choice: u64,
    max_jumps: usize,
) -> Option<SystemId> {
    pick_site_of(content, state, galaxy, from, home_set, used, choice, max_jumps, Site::Empty)
}

#[allow(clippy::too_many_arguments)]
fn pick_site_of(
    content: &ContentStore,
    state: &GameState,
    galaxy: &Galaxy,
    from: &str,
    home_set: &BTreeSet<&str>,
    used: &BTreeSet<String>,
    choice: u64,
    max_jumps: usize,
    kind: Site,
) -> Option<SystemId> {
    let mut seen: BTreeSet<&str> = BTreeSet::from([from]);
    let mut layer: BTreeSet<&str> = BTreeSet::from([from]);
    for _ in 0..max_jumps {
        let mut next: BTreeSet<&str> = BTreeSet::new();
        for system in &layer {
            for neighbour in galaxy.adjacent(system) {
                if seen.insert(neighbour) {
                    next.insert(neighbour);
                }
            }
        }
        let eligible: Vec<&str> = next
            .iter()
            .copied()
            .filter(|id| eligible(content, state, id, home_set, used))
            .filter(|id| kind == Site::Empty || colony_planet(content, id).is_some())
            .collect();
        if !eligible.is_empty() {
            let index = usize::try_from(choice % eligible.len() as u64).unwrap_or(0);
            return Some(SystemId::new(eligible[index]));
        }
        layer = next;
    }
    None
}

/// The planet a colony would sit on: the first non-legendary planet printed on the tile.
fn colony_planet(content: &ContentStore, system: &str) -> Option<PlanetId> {
    let mut planets: Vec<_> = galaxy::planets_in(content, system, POK)
        .into_iter()
        .filter(|planet| !planet.is_legendary() && !planet.is_space_station())
        .map(|planet| planet.id().to_owned())
        .collect();
    planets.sort();
    planets.into_iter().next().map(PlanetId::new)
}

/// Mecatol Rex's space area is free for the first raider.
fn eligible_in_mecatol(state: &GameState) -> bool {
    state
        .board
        .get(&SystemId::new(MECATOL))
        .is_none_or(|board| {
            board.units.is_empty() && board.planet_units.values().all(Vec::is_empty)
        })
}

/// An empty ordinary system: not Mecatol, not a home, not an anomaly or hyperlane, not taken.
fn eligible(
    content: &ContentStore,
    state: &GameState,
    id: &str,
    home_set: &BTreeSet<&str>,
    used: &BTreeSet<String>,
) -> bool {
    if id == MECATOL || home_set.contains(id) || used.contains(id) {
        return false;
    }
    let Some(system) = galaxy::system(content, id, POK) else {
        return false;
    };
    if system.is_anomaly() || system.is_hyperlane() {
        return false;
    }
    state.board.get(&SystemId::new(id)).is_none_or(|board| {
        board.units.is_empty() && board.planet_units.values().all(Vec::is_empty)
    })
}

/// The unit types of a preset fleet for one faction: the dreadnought is the faction's own
/// version (L1Z1X's super-dreadnought, for one), and a faction mech rides with the ground forces
/// where the corpus has one. Both sustain damage.
fn fleet_for(
    content: &ContentStore,
    faction: &FactionId,
    spec: &[(&str, usize)],
) -> Vec<(UnitTypeId, usize)> {
    let resolve = |kind: &str| {
        factions::resolve_unit(content, faction.as_str(), &UnitTypeId::new(kind), POK)
    };
    let mut fleet: Vec<(UnitTypeId, usize)> = spec
        .iter()
        .map(|(kind, count)| {
            let id = if *kind == "dreadnought" {
                resolve(kind)
            } else {
                UnitTypeId::new(*kind)
            };
            (id, *count)
        })
        .collect();
    let mech = resolve("mech");
    if units::unit_type(content, mech.as_str(), POK).is_some() {
        fleet.push((mech, 1));
    }
    fleet
}

/// Whether a unit type id is `kind` or a faction's own version of it (`sol_mech` for `mech`).
fn is_kind(id: &str, kind: &str) -> bool {
    id == kind || id.ends_with(&format!("_{kind}"))
}

fn place(
    content: &ContentStore,
    state: &mut GameState,
    player: &PlayerId,
    site: &SystemId,
    fleet_spec: &[(UnitTypeId, usize)],
) -> Result<(), String> {
    let board = state.system_mut(site);
    for (kind, count) in fleet_spec {
        for _ in 0..*count {
            board.units.push(Unit::new(kind.clone(), player.clone()));
        }
    }
    let over_supply = fleet::over_supply(state, content, POK, player, site);
    let over_capacity = fleet::over_capacity(state, content, POK, player, site);
    if over_supply > 0 || over_capacity > 0 {
        return Err(format!(
            "preset fleet for {player} in {site} is over supply ({over_supply}) or capacity ({over_capacity})"
        ));
    }
    check_pools(state, player)?;
    Ok(())
}

/// No preset puts more of a unit type on the board than its reinforcement pool holds.
fn check_pools(state: &GameState, player: &PlayerId) -> Result<(), String> {
    for (kind, limit) in POOL {
        let on_board = state
            .board
            .values()
            .flat_map(|system| {
                system
                    .units
                    .iter()
                    .chain(system.planet_units.values().flatten())
            })
            .filter(|unit| &unit.owner == player && is_kind(unit.type_id.as_str(), kind))
            .count();
        if on_board > *limit {
            return Err(format!(
                "preset puts {on_board} {kind} on the board for {player}, over the pool of {limit}"
            ));
        }
    }
    Ok(())
}

/// Influence a raider keeps beyond the custodians' six. Random play spends it early: two
/// Leadership token buys, or four Letnev Munitions Reserves rerolls (2 trade goods each) in the
/// strike fleets' first fights, drained raiders before they reached Mecatol. 27.2 checks the cost
/// when the invasion starts, not when the game does.
const RAIDER_SPARE_INFLUENCE: i64 = 10;

/// Trade goods so a raider can pay the custodians' six influence (27.2) without any rule change.
fn top_up_influence(content: &ContentStore, state: &mut GameState, player: &PlayerId) {
    let available =
        production::available(state, content, POK, player, production::Spend::Influence);
    let wanted = invasion::CUSTODIANS_COST + RAIDER_SPARE_INFLUENCE;
    if available < wanted {
        if let Some(seat) = state.player_mut(player) {
            seat.trade_goods += i32::try_from(wanted - available).unwrap_or(0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::{create_game_with_preset, create_game_with_template};
    use ti4_model::state::LeaderStatus;

    fn players(n: usize) -> Vec<PlayerId> {
        (1..=n).map(|i| PlayerId::new(format!("p{i}"))).collect()
    }

    fn content() -> &'static ContentStore {
        ContentStore::embedded()
    }

    fn homes_of(state_players: &[PlayerId]) -> Vec<SystemId> {
        let assignments = seating::seat_in_scope(state_players);
        state_players
            .iter()
            .map(|p| {
                let one = BTreeMap::from([(p.clone(), assignments[p].clone())]);
                seating::home_systems(content(), &one).unwrap().remove(0)
            })
            .collect()
    }

    #[test]
    fn an_unknown_preset_is_an_error() {
        let err =
            create_game_with_preset(content(), &players(3), 5, None, Some("nope")).unwrap_err();
        assert!(err.contains("unknown start_preset"), "{err}");
        assert!(!is_known("nope") && is_known(COMBAT));
    }

    #[test]
    fn no_preset_leaves_the_opening_state_untouched() {
        for n in 3..=6 {
            let (plain, _) = create_game_with_template(content(), &players(n), 11, None).unwrap();
            let (same, _) =
                create_game_with_preset(content(), &players(n), 11, None, None).unwrap();
            assert_eq!(
                serde_json::to_string(&plain).unwrap(),
                serde_json::to_string(&same).unwrap(),
                "{n} players"
            );
        }
    }

    #[test]
    fn the_preset_is_deterministic_per_seed_and_varies_between_seeds() {
        let build = |seed| {
            let (state, _) =
                create_game_with_preset(content(), &players(4), seed, None, Some(COMBAT)).unwrap();
            serde_json::to_string(&state).unwrap()
        };
        assert_eq!(build(7), build(7));
        assert_ne!(build(7), build(8));
    }

    #[test]
    fn every_player_count_gets_legal_fleets_within_supply_capacity_and_pools() {
        for n in 3..=6 {
            for seed in 0..8 {
                let (state, _) =
                    create_game_with_preset(content(), &players(n), seed, None, Some(COMBAT))
                        .unwrap_or_else(|e| panic!("{n} players seed {seed}: {e}"));
                for player in &players(n) {
                    for (system, board) in &state.board {
                        assert_eq!(
                            fleet::over_supply(&state, content(), POK, player, system),
                            0,
                            "{n}p seed {seed}: {player} over fleet supply in {system}"
                        );
                        assert_eq!(
                            fleet::over_capacity(&state, content(), POK, player, system),
                            0,
                            "{n}p seed {seed}: {player} over capacity in {system}"
                        );
                        // No two players share a space area.
                        let owners: BTreeSet<_> = board.units.iter().map(|u| &u.owner).collect();
                        assert!(owners.len() <= 1, "{n}p seed {seed}: {system} is shared");
                    }
                }
            }
        }
    }

    #[test]
    fn every_preset_fleet_can_sustain_damage_with_a_ship_and_a_mech() {
        for n in 3..=6 {
            for seed in 0..8 {
                let list = players(n);
                let (plain, _) = create_game_with_template(content(), &list, seed, None).unwrap();
                let (state, _) =
                    create_game_with_preset(content(), &list, seed, None, Some(COMBAT)).unwrap();
                let mut fleets = 0;
                for (system, board) in &state.board {
                    let before = plain.board.get(system).map_or(0, |b| b.units.len());
                    if board.units.len() <= before {
                        continue;
                    }
                    fleets += 1;
                    let kinds: Vec<_> = board
                        .units
                        .iter()
                        .map(|u| units::unit_type(content(), u.type_id.as_str(), POK).unwrap())
                        .collect();
                    assert!(
                        kinds.iter().any(|k| k.is_ship() && k.sustain_damage()),
                        "{n}p seed {seed}: the fleet in {system} has no sustaining ship"
                    );
                    assert!(
                        kinds.iter().any(|k| k.is_ground_force() && k.sustain_damage()),
                        "{n}p seed {seed}: the fleet in {system} has no sustaining mech"
                    );
                }
                assert!(fleets >= n, "{n}p seed {seed}: only {fleets} fleets placed");
            }
        }
    }

    #[test]
    fn a_preset_dreadnought_is_the_factions_own_version() {
        let fleet = fleet_for(content(), &FactionId::new("l1z1x"), STRIKE_FLEET);
        let dreadnought = fleet
            .iter()
            .find(|(id, _)| id.as_str().contains("dread"))
            .unwrap_or_else(|| panic!("no dreadnought in {fleet:?}"));
        assert_ne!(dreadnought.0.as_str(), "dreadnought", "L1Z1X has a super-dreadnought");
        assert!(fleet.iter().any(|(id, _)| id.as_str().ends_with("mech")));
    }

    #[test]
    fn a_strike_fleet_sits_beside_an_opponents_home_system() {
        for n in 3..=6 {
            for seed in 0..8 {
                let list = players(n);
                let (plain, _) = create_game_with_template(content(), &list, seed, None).unwrap();
                let (state, galaxy) =
                    create_game_with_preset(content(), &list, seed, None, Some(COMBAT)).unwrap();
                let homes = homes_of(&list);
                let mut beside_a_home = 0;
                for (i, player) in list.iter().enumerate() {
                    let strike: Vec<&SystemId> = state
                        .board
                        .iter()
                        .filter(|(system, board)| {
                            let before = plain.board.get(*system).map_or(0, |b| b.units.len());
                            board.units.len() > before
                                && board.units.iter().any(|u| {
                                    &u.owner == player && u.type_id.as_str() == "destroyer"
                                })
                        })
                        .map(|(system, _)| system)
                        .collect();
                    assert!(
                        !strike.is_empty(),
                        "{n}p seed {seed}: {player} has no strike fleet"
                    );
                    for site in strike {
                        let within_two = homes.iter().enumerate().any(|(j, home)| {
                            j != i
                                && (galaxy.are_adjacent(site.as_str(), home.as_str())
                                    || galaxy
                                        .adjacent(home.as_str())
                                        .iter()
                                        .any(|mid| galaxy.are_adjacent(site.as_str(), mid)))
                        });
                        assert!(
                            within_two,
                            "{n}p seed {seed}: {site} is not near an opponent home"
                        );
                        if homes.iter().enumerate().any(|(j, home)| {
                            j != i && galaxy.are_adjacent(site.as_str(), home.as_str())
                        }) {
                            beside_a_home += 1;
                        }
                    }
                }
                assert!(
                    beside_a_home > 0,
                    "{n}p seed {seed}: no fleet is adjacent to a home"
                );
            }
        }
    }

    #[test]
    fn raiders_start_in_or_beside_mecatol_and_one_can_lift_the_custodians_at_once() {
        let mecatol = SystemId::new(MECATOL);
        for n in 3..=6 {
            for seed in 0..8 {
                let list = players(n);
                let (state, galaxy) =
                    create_game_with_preset(content(), &list, seed, None, Some(COMBAT)).unwrap();
                let raiders: Vec<&PlayerId> = list
                    .iter()
                    .filter(|player| {
                        state.board.iter().any(|(system, board)| {
                            (system == &mecatol || galaxy.are_adjacent(system.as_str(), MECATOL))
                                && board
                                    .units
                                    .iter()
                                    .any(|u| &u.owner == *player && u.type_id.as_str() == "cruiser")
                        })
                    })
                    .collect();
                assert_eq!(
                    raiders.len(),
                    n.div_ceil(2),
                    "{n}p seed {seed}: half the seats should raid Mecatol"
                );
                let in_mecatol: Vec<&PlayerId> = raiders
                    .iter()
                    .copied()
                    .filter(|player| {
                        state
                            .board
                            .get(&mecatol)
                            .is_some_and(|b| b.units.iter().any(|u| &u.owner == *player))
                    })
                    .collect();
                assert_eq!(
                    in_mecatol.len(),
                    1,
                    "{n}p seed {seed}: one raider starts in Mecatol"
                );
                for player in &raiders {
                    assert!(
                        production::available(
                            &state,
                            content(),
                            POK,
                            player,
                            production::Spend::Influence
                        ) >= invasion::CUSTODIANS_COST + RAIDER_SPARE_INFLUENCE,
                        "{n}p seed {seed}: {player} has no influence to spare beyond the custodians"
                    );
                }
                // The engine's own gate: the raider inside Mecatol has landable ground forces and
                // the influence, so the custodians can be lifted the first time it invades.
                assert!(
                    invasion::custodians_removable(&state, content(), POK, in_mecatol[0], &mecatol),
                    "{n}p seed {seed}: the raider in Mecatol cannot lift the custodians"
                );
                // Nobody else is in Mecatol.
                assert!(
                    state
                        .board
                        .get(&mecatol)
                        .is_none_or(|b| b.units.iter().all(|u| &u.owner == in_mecatol[0])),
                    "{n}p seed {seed}: Mecatol is shared"
                );
            }
        }
    }

    #[test]
    fn the_cards_preset_deals_distinct_never_played_cards_and_leaves_them_out_of_the_deck() {
        for n in 3..=6 {
            let list = players(n);
            let (plain, _) = create_game_with_template(content(), &list, 3, None).unwrap();
            let (state, _) =
                create_game_with_preset(content(), &list, 3, None, Some(CARDS)).unwrap();
            let mut dealt: Vec<&ActionCardId> = Vec::new();
            for player in &list {
                let hand = &state.player(player).unwrap().action_cards;
                let before = plain.player(player).unwrap().action_cards.len();
                assert!(hand.len() > before, "{n}p: {player} got no cards");
                assert!(hand.len() <= before + HAND_SIZE, "{n}p: hand over {HAND_SIZE} dealt");
                assert!(hand.len() < 7, "{n}p: the hand limit would trigger at once");
                dealt.extend(hand.iter().skip(before));
            }
            let unique: BTreeSet<_> = dealt.iter().collect();
            assert_eq!(unique.len(), dealt.len(), "{n}p: a card was dealt twice");
            for card in &dealt {
                assert!(CARD_POOL.contains(&card.as_str()));
                assert!(!state.action_card_deck.contains(card), "{card} is still in the deck");
            }
        }
    }

    #[test]
    fn the_agenda_preset_puts_the_ordered_agendas_on_top_and_removes_the_custodians() {
        for n in 3..=6 {
            let list = players(n);
            let (plain, _) = create_game_with_template(content(), &list, 5, None).unwrap();
            assert!(!plain.custodians_removed);
            let (state, _) =
                create_game_with_preset(content(), &list, 5, None, Some(AGENDA)).unwrap();
            assert!(state.custodians_removed);
            assert_eq!(state.agenda_deck.len(), plain.agenda_deck.len(), "{n}p: deck size changed");
            let mut a = state.agenda_deck.clone();
            let mut b = plain.agenda_deck.clone();
            a.sort();
            b.sort();
            assert_eq!(a, b, "{n}p: the deck must hold the same cards");
            assert_eq!(&state.agenda_deck[..4], ["committee", "defense_act", "abolishment", "redistribution"]);
            for player in &list {
                let seat = state.player(player).unwrap();
                assert!(seat.trade_goods >= AGENDA_TRADE_GOODS);
                assert!(seat.action_cards.len() < 7);
            }
        }
    }

    #[test]
    fn every_agenda_preset_alias_exists() {
        let aliases: Vec<String> = create_game_with_template(content(), &players(3), 1, None)
            .unwrap()
            .0
            .agenda_deck;
        for alias in AGENDA_ORDER {
            assert!(aliases.iter().any(|a| a == alias), "{alias} is not in the agenda deck");
        }
        for id in AGENDA_CARD_POOL {
            assert!(content().get(ContentType::ActionCards, id).is_some(), "{id}");
        }
    }

    #[test]
    fn the_relics_preset_deals_distinct_relics_with_neuraloop_and_something_to_purge() {
        for n in 3..=6 {
            for seed in 0..4 {
                let list = players(n);
                let (state, _) =
                    create_game_with_preset(content(), &list, seed, None, Some(RELICS)).unwrap();
                let held: Vec<&RelicId> = list
                    .iter()
                    .flat_map(|p| state.player(p).unwrap().relics.iter())
                    .collect();
                let unique: BTreeSet<_> = held.iter().collect();
                assert_eq!(unique.len(), held.len(), "{n}p seed {seed}: a relic dealt twice");
                assert!(held.iter().all(|r| !state.relic_deck.contains(r)));
                let owner = list
                    .iter()
                    .find(|p| {
                        state
                            .player(p)
                            .unwrap()
                            .relics
                            .iter()
                            .any(|r| r.as_str() == "neuraloop")
                    })
                    .expect("neuraloop is dealt");
                assert!(state.player(owner).unwrap().relics.len() >= 2, "{n}p: nothing to purge");
                assert!(list.iter().any(
                    |p| state.player(p).unwrap().relic_fragments.get("CULTURAL") == Some(&3)
                ));
            }
        }
    }

    #[test]
    fn the_invasion_preset_defends_a_colony_per_seat_with_an_attacker_above_it() {
        for n in 3..=6 {
            for seed in 0..8 {
                let list = players(n);
                let (state, _) =
                    create_game_with_preset(content(), &list, seed, None, Some(INVASION))
                        .unwrap_or_else(|e| panic!("{n}p seed {seed}: {e}"));
                let pds_systems: BTreeSet<(&SystemId, &PlayerId)> = state
                    .board
                    .iter()
                    .flat_map(|(system, board)| {
                        board
                            .planet_units
                            .values()
                            .flatten()
                            .filter(|u| u.type_id.as_str() == "pds")
                            .map(move |u| (system, &u.owner))
                    })
                    .collect();
                // Home systems start with PDS or not depending on faction; the colony ones are
                // the ones with a planet whose controller matches and that are not homes.
                let homes = homes_of(&list);
                let colonies: Vec<_> = pds_systems
                    .iter()
                    .filter(|(system, _)| !homes.contains(system))
                    .collect();
                assert_eq!(colonies.len(), n, "{n}p seed {seed}: one colony per seat");
                for (i, player) in list.iter().enumerate() {
                    let (colony, owner) = colonies
                        .iter()
                        .map(|c| **c)
                        .find(|(_, owner)| *owner == &list[(i + 1) % n])
                        .expect("the next seat's colony");
                    assert_eq!(owner, &list[(i + 1) % n]);
                    let board = &state.board[colony];
                    assert!(board.planet_control.values().any(|c| c == owner));
                    assert!(board.planet_units.values().flatten().any(|u| {
                        u.type_id.as_str() == "infantry" && &u.owner == owner
                    }));
                    // The attacker waits in the colony's space area with a dreadnought and four
                    // infantry, and nobody else is there.
                    assert!(
                        board.units.iter().all(|u| &u.owner == player),
                        "{n}p seed {seed}: the colony's space is shared"
                    );
                    assert!(board.units.iter().any(|u| u.type_id.as_str().contains("dread")));
                    let infantry = board
                        .units
                        .iter()
                        .filter(|u| &u.owner == player && u.type_id.as_str() == "infantry")
                        .count();
                    assert_eq!(infantry, 4);
                }
            }
        }
    }

    #[test]
    fn the_techs_preset_is_the_invasion_setup_plus_every_pool_technology() {
        let list = players(4);
        let (plain, _) = create_game_with_template(content(), &list, 2, None).unwrap();
        let (invasion, _) =
            create_game_with_preset(content(), &list, 2, None, Some(INVASION)).unwrap();
        let (techs, _) = create_game_with_preset(content(), &list, 2, None, Some(TECHS)).unwrap();
        for player in &list {
            let owned = &techs.player(player).unwrap().technologies;
            for id in TECH_POOL {
                assert!(owned.contains(&TechnologyId::new(*id)), "{player} lacks {id}");
            }
            assert!(owned.len() > plain.player(player).unwrap().technologies.len());
        }
        assert_eq!(
            serde_json::to_string(&techs.board).unwrap(),
            serde_json::to_string(&invasion.board).unwrap(),
            "the board is the invasion preset's"
        );
    }

    #[test]
    fn the_leaders_preset_rotates_the_factions_and_unlocks_every_leader() {
        for n in 3..=6 {
            for seed in 0..8 {
                let list = players(n);
                let (state, _) =
                    create_game_with_preset(content(), &list, seed, None, Some(LEADERS))
                        .unwrap_or_else(|e| panic!("{n}p seed {seed}: {e}"));
                let rotated = rotation(LEADERS, seed);
                assert!((2..=5).contains(&rotated));
                let factions: Vec<&str> = list
                    .iter()
                    .map(|p| state.player(p).unwrap().faction.as_str())
                    .collect();
                if n <= 4 {
                    assert!(
                        factions.contains(&"jolnar") || factions.contains(&"l1z1x"),
                        "{n}p seed {seed}: {factions:?}"
                    );
                }
                let distinct: BTreeSet<_> = factions.iter().collect();
                assert_eq!(distinct.len(), n.min(6), "{factions:?}");
                for player in &list {
                    let seat = state.player(player).unwrap();
                    assert!(!seat.leaders.is_empty());
                    for (leader, status) in &seat.leaders {
                        let kind = leaders::kind_of(content(), leader);
                        let expected = if kind.as_deref() == Some(leaders::AGENT) {
                            LeaderStatus::Readied
                        } else {
                            LeaderStatus::Unlocked
                        };
                        assert_eq!(*status, expected, "{n}p seed {seed}: {leader}");
                    }
                }
            }
        }
    }

    #[test]
    fn rotation_is_zero_without_the_leaders_preset_or_suffix_and_the_suffix_is_known() {
        assert_eq!(rotation(COMBAT, 5), 0);
        assert_eq!(rotation(INVASION, 5), 0);
        assert!(rotation("invasion+rot", 5) >= 2);
        assert!(is_known("invasion+rot") && is_known(LEADERS) && !is_known("nope+rot"));
        let list = players(4);
        let (plain, _) = create_game_with_template(content(), &list, 9, None).unwrap();
        let (rotated, _) =
            create_game_with_preset(content(), &list, 9, None, Some("combat+rot")).unwrap();
        let faction = |s: &GameState, i: usize| s.player(&list[i]).unwrap().faction.clone();
        assert_ne!(faction(&plain, 0), faction(&rotated, 0));
        // The combat fleets were built for the factions that actually sit there.
        let (_, _) = create_game_with_preset(content(), &players(5), 3, None, Some("combat+rot"))
            .unwrap();
    }

    #[test]
    fn every_tech_pool_entry_exists() {
        for id in TECH_POOL {
            assert!(content().get(ContentType::Technologies, id).is_some(), "{id}");
        }
    }

    #[test]
    fn every_relic_pool_entry_exists() {
        for id in RELIC_POOL {
            assert!(content().get(ContentType::Relics, id).is_some(), "{id}");
        }
    }

    #[test]
    fn every_pool_card_exists_in_the_corpus() {
        for id in CARD_POOL {
            assert!(content().get(ContentType::ActionCards, id).is_some(), "{id}");
        }
    }

    #[test]
    fn the_known_presets_match_the_harness_list() {
        let ts = include_str!("../../../web/e2e/smokePreset.ts");
        let line = ts
            .lines()
            .find(|l| l.contains("KNOWN_PRESETS ="))
            .expect("KNOWN_PRESETS in smokePreset.ts");
        let listed: Vec<&str> = line.split('"').skip(1).step_by(2).collect();
        assert_eq!(listed, KNOWN);
    }
}
