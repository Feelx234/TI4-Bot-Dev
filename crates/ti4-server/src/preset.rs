//! Start presets: prepared opening states for random smoke playthroughs.
//!
//! A random click policy rarely reaches combat, casualties, Mecatol Rex or the agenda phase from
//! the standard opening. A preset only changes the *starting state* (extra fleets, a little trade
//! goods); it never changes a rule. The result is stored in the game's initial state, so crash
//! recovery restores it from the saved state rather than rebuilding it.

use std::collections::{BTreeMap, BTreeSet};

use ti4_content::ContentStore;
use ti4_content::galaxy::{self, Galaxy};
use ti4_engine::seating::{self, MECATOL};
use ti4_engine::{fleet, invasion, production};
use ti4_model::content_types::POK;
use ti4_model::id::{PlayerId, SystemId, UnitTypeId};
use ti4_model::state::GameState;
use ti4_model::units::Unit;

/// Fleets beside opponents' home systems and beside Mecatol Rex, with influence for the custodians.
pub const COMBAT: &str = "combat";

/// Every preset name the server accepts.
pub const KNOWN: &[&str] = &[COMBAT];

/// Placed one jump from an opponent's home. Capacity 4: two fighters and two infantry fit.
const STRIKE_FLEET: &[(&str, usize)] = &[
    ("carrier", 1),
    ("destroyer", 1),
    ("fighter", 2),
    ("infantry", 2),
];

/// Placed beside Mecatol Rex with ground forces, so the custodians can be lifted.
const RAIDING_PARTY: &[(&str, usize)] = &[("carrier", 1), ("cruiser", 1), ("infantry", 2)];

/// Reinforcement pool sizes (LRR 76.1): a preset never puts more of a type on the board.
const POOL: &[(&str, usize)] = &[
    ("carrier", 4),
    ("cruiser", 8),
    ("destroyer", 8),
    ("fighter", 10),
    ("infantry", 12),
];

#[must_use]
pub fn is_known(name: &str) -> bool {
    KNOWN.contains(&name)
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
    match preset {
        COMBAT => combat(content, state, galaxy, players, seed),
        other => Err(format!("unknown start_preset {other:?}")),
    }
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

fn combat(
    content: &ContentStore,
    state: &mut GameState,
    galaxy: &Galaxy,
    players: &[PlayerId],
    seed: u64,
) -> Result<(), String> {
    let assignments = seating::seat_in_scope(players);
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

    // Half the seats (rounded up) raid Mecatol, chosen by a seeded ranking.
    let mut ranking: Vec<usize> = (0..count).collect();
    ranking.sort_by_key(|i| mix(seed, 100 + *i as u64));
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
                place(content, state, player, &site, STRIKE_FLEET)?;
                used.insert(site.as_str().to_owned());
            }
        }
        if raiders[i] {
            if let Some(site) = pick_site(
                content,
                state,
                galaxy,
                MECATOL,
                &home_set,
                &used,
                mix(seed, 400 + i as u64),
                1,
            ) {
                place(content, state, player, &site, RAIDING_PARTY)?;
                used.insert(site.as_str().to_owned());
            }
            top_up_influence(content, state, player);
        }
    }
    Ok(())
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
            .collect();
        if !eligible.is_empty() {
            let index = usize::try_from(choice % eligible.len() as u64).unwrap_or(0);
            return Some(SystemId::new(eligible[index]));
        }
        layer = next;
    }
    None
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

fn place(
    content: &ContentStore,
    state: &mut GameState,
    player: &PlayerId,
    site: &SystemId,
    fleet_spec: &[(&str, usize)],
) -> Result<(), String> {
    let board = state.system_mut(site);
    for (kind, count) in fleet_spec {
        for _ in 0..*count {
            board
                .units
                .push(Unit::new(UnitTypeId::new(*kind), player.clone()));
        }
    }
    let over_supply = fleet::over_supply(state, content, POK, player, site);
    let over_capacity = fleet::over_capacity(state, content, POK, player, site);
    if over_supply > 0 || over_capacity > 0 {
        return Err(format!(
            "preset fleet for {player} in {site} is over supply ({over_supply}) or capacity ({over_capacity})"
        ));
    }
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
            .filter(|unit| &unit.owner == player && unit.type_id.as_str() == *kind)
            .count();
        if on_board > *limit {
            return Err(format!(
                "preset puts {on_board} {kind} on the board for {player}, over the pool of {limit}"
            ));
        }
    }
    Ok(())
}

/// Trade goods so a raider can pay the custodians' six influence (27.2) without any rule change.
fn top_up_influence(content: &ContentStore, state: &mut GameState, player: &PlayerId) {
    let available =
        production::available(state, content, POK, player, production::Spend::Influence);
    if available < invasion::CUSTODIANS_COST {
        if let Some(seat) = state.player_mut(player) {
            seat.trade_goods += i32::try_from(invasion::CUSTODIANS_COST - available).unwrap_or(0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::{create_game_with_preset, create_game_with_template};

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
    fn raiders_sit_beside_mecatol_with_ground_forces_and_six_influence() {
        for n in 3..=6 {
            for seed in 0..8 {
                let list = players(n);
                let (state, galaxy) =
                    create_game_with_preset(content(), &list, seed, None, Some(COMBAT)).unwrap();
                let raiders: Vec<&PlayerId> =
                    list.iter()
                        .filter(|player| {
                            state.board.iter().any(|(system, board)| {
                                galaxy.are_adjacent(system.as_str(), MECATOL)
                                    && board.units.iter().any(|u| {
                                        &u.owner == *player && u.type_id.as_str() == "cruiser"
                                    })
                            })
                        })
                        .collect();
                assert_eq!(
                    raiders.len(),
                    n.div_ceil(2),
                    "{n}p seed {seed}: half the seats should raid Mecatol"
                );
                for player in raiders {
                    assert!(
                        production::available(
                            &state,
                            content(),
                            POK,
                            player,
                            production::Spend::Influence
                        ) >= invasion::CUSTODIANS_COST,
                        "{n}p seed {seed}: {player} cannot pay the custodians"
                    );
                }
                assert!(
                    state
                        .board
                        .get(&SystemId::new(MECATOL))
                        .is_none_or(|b| b.units.is_empty()),
                    "Mecatol must stay empty while the custodians token sits there"
                );
            }
        }
    }
}
