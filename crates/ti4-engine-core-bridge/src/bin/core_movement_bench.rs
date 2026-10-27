//! Micro-benchmark of the movement slice: the string engine vs the typed core on the same
//! questions (docs/ENGINE_CORE.md, "Measurements").
//!
//! ```text
//! core_movement_bench MODE CONFIG GAMES STRIDE [REPEAT]
//! ```
//! Plays the first GAMES games of CONFIG with the rule bot (as `selfplay run`) and keeps every
//! STRIDE-th position; then, timed, asks what the bot's `movable_into` asks: for every seat, every
//! destination that can hold things and every ship of the seat, can it reach (at its move value,
//! else one more). REPEAT times over all positions. Only that phase is timed, in CPU time of the
//! thread (`/proc/thread-self/schedstat`). MODE:
//!
//! - `legacy`: `Board::for_player` + `MovementRules::with_laws` + `apply_movement_effects` per
//!   (seat, destination), `can_reach_ship` per ship: today's code path.
//! - `adapter`: the same rules built by the string engine, converted (`rules_from_legacy`) and
//!   searched by the typed core: the first strangler stage (core below, string hooks above).
//! - `core`: the galaxy and board converted once per position, per seat the board facts and flags
//!   once, per (seat, destination) typed rules (`CoreRules::with_board`) and the typed search. A seat
//!   whose rules carry faction-hook state is answered by the string engine (counted).
//! - `legacy-search` / `core-search`: the rules built untimed, only the searches timed.
//!
//! Prints the CPU time and a digest of every answer; all modes must print the same digest.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use ti4_content::ContentStore;
use ti4_engine::movement::{Board, MovementRules};
use ti4_engine_core::movement::CoreRules;
use ti4_engine_core::{SystemIx, UnitIx};
use ti4_engine_core_bridge::adapter::{
    self, MoverFlags, Seats, board_from_state, galaxy_from_legacy, is_plain, rules_from_legacy,
    system_ix, unit_ix, uses_galaxy,
};
use ti4_engine_core_bridge::workload::{Position, ShipQuery, golden_positions, ships};
use ti4_model::id::PlayerId;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn thread_cpu_ns() -> u64 {
    std::fs::read_to_string("/proc/thread-self/schedstat")
        .ok()
        .and_then(|s| s.split_whitespace().next()?.parse().ok())
        .unwrap_or(0)
}

/// What one position asks: per seat, its ships; and the destinations.
struct Prepared {
    position: Position,
    seats: Vec<(PlayerId, Vec<ShipQuery>, bool)>,
    destinations: Vec<String>,
}

fn legacy_rules<'a>(p: &'a Position, player: &PlayerId, dest: &str) -> MovementRules<'a> {
    let content = ContentStore::embedded();
    let board = Board::for_player(&p.state, content, p.sources, player);
    let mut rules =
        MovementRules::with_laws(&p.galaxy, content, p.sources, dest, board, Some(&p.state));
    ti4_engine::action_cards::apply_movement_effects(&mut rules, &p.state, player);
    rules
}

fn answer_legacy(rules: &MovementRules<'_>, ship: &ShipQuery) -> u8 {
    let o = ship.origin.as_str();
    if rules.can_reach_ship(o, ship.move_value, Some(&ship.type_id)) {
        1
    } else if rules.can_reach_ship(o, ship.move_value + 1, Some(&ship.type_id)) {
        2
    } else {
        0
    }
}

fn answer_raw(rules: &CoreRules, origin: SystemIx, kind: Option<UnitIx>, mv: i32) -> u8 {
    if rules.can_reach_ship(origin, mv, kind) {
        1
    } else if rules.can_reach_ship(origin, mv + 1, kind) {
        2
    } else {
        0
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mode = args[0].as_str();
    let config = &args[1];
    let games: usize = args[2].parse().expect("GAMES");
    let stride: usize = args[3].parse().expect("STRIDE");
    let repeat: usize = args.get(4).map_or(1, |r| r.parse().expect("REPEAT"));
    let content = ContentStore::embedded();

    let (positions, _) = golden_positions(config, games, stride).expect("games");
    // Untimed: the questions, and whether each seat's rules are plain at every destination.
    let prepared: Vec<Prepared> = positions
        .into_iter()
        .map(|position| {
            let destinations: Vec<String> = position
                .galaxy
                .system_ids_holding_things()
                .into_iter()
                .map(str::to_owned)
                .collect();
            let seats = position
                .state
                .players
                .iter()
                .map(|player| {
                    let fleet = ships(&position.state, content, position.sources, &player.id);
                    // Plain: no hook state and the board facts the typed board gives (Aetherpassage
                    // edits them), at every destination.
                    let seats = Seats::of(&position.state).expect("seats");
                    let board =
                        board_from_state(&position.state, &seats, position.sources).expect("board");
                    let seat = seats.seat(player.id.as_str()).expect("seat");
                    let (enemy, tokens) = (board.enemy_ships(seat), board.own_command_tokens(seat));
                    let plain = destinations.iter().all(|d| {
                        let rules = legacy_rules(&position, &player.id, d);
                        let parts = rules.parts();
                        is_plain(&rules, &position.galaxy)
                            && adapter::sys_set_lossy(parts.board.enemy_ships.iter().map(String::as_str))
                                == enemy
                            && adapter::sys_set_lossy(
                                parts.board.own_command_tokens.iter().map(String::as_str),
                            ) == tokens
                    });
                    (player.id.clone(), fleet, plain)
                })
                .collect();
            Prepared {
                position,
                seats,
                destinations,
            }
        })
        .collect();

    let mut hasher = DefaultHasher::new();
    let mut queries = 0usize;
    let mut fallbacks = 0usize;
    let mut cpu = 0u64;
    for _ in 0..repeat {
        for (pi, prep) in prepared.iter().enumerate() {
            let p = &prep.position;
            match mode {
                "legacy" => {
                    let t = thread_cpu_ns();
                    for (si, (player, fleet, _)) in prep.seats.iter().enumerate() {
                        for (di, dest) in prep.destinations.iter().enumerate() {
                            let rules = legacy_rules(p, player, dest);
                            for (k, ship) in fleet.iter().enumerate() {
                                if ship.origin.as_str() == dest {
                                    continue;
                                }
                                queries += 1;
                                (pi, si, di, k, answer_legacy(&rules, ship)).hash(&mut hasher);
                            }
                        }
                    }
                    cpu += thread_cpu_ns() - t;
                }
                "adapter" => {
                    let t = thread_cpu_ns();
                    let raw_galaxy = galaxy_from_legacy(&p.galaxy).expect("mapped");
                    let adjacency = std::sync::Arc::new(raw_galaxy.adjacency());
                    for (si, (player, fleet, _)) in prep.seats.iter().enumerate() {
                        let mapped: Vec<(SystemIx, Option<UnitIx>)> = fleet
                            .iter()
                            .map(|s| {
                                (system_ix(s.origin.as_str()).expect("mapped"), unit_ix(&s.type_id))
                            })
                            .collect();
                        for (di, dest) in prep.destinations.iter().enumerate() {
                            let legacy = legacy_rules(p, player, dest);
                            let own;
                            let adj = if uses_galaxy(&legacy, &p.galaxy) {
                                &adjacency
                            } else {
                                own = std::sync::Arc::new(galaxy_from_legacy(legacy.parts().galaxy).expect("mapped").adjacency());
                                &own
                            };
                            let rules = rules_from_legacy(&legacy, p.sources, adj).expect("mapped");
                            for (k, ship) in fleet.iter().enumerate() {
                                if ship.origin.as_str() == dest {
                                    continue;
                                }
                                queries += 1;
                                let (o, kind) = mapped[k];
                                (pi, si, di, k, answer_raw(&rules, o, kind, ship.move_value))
                                    .hash(&mut hasher);
                            }
                        }
                    }
                    cpu += thread_cpu_ns() - t;
                }
                "core" => {
                    let t = thread_cpu_ns();
                    let raw_galaxy = galaxy_from_legacy(&p.galaxy).expect("mapped");
                    let adjacency = std::sync::Arc::new(raw_galaxy.adjacency());
                    let seats = Seats::of(&p.state).expect("seats");
                    let board = board_from_state(&p.state, &seats, p.sources).expect("board");
                    let scope = adapter::scope(p.sources);
                    let dests: Vec<SystemIx> = prep
                        .destinations
                        .iter()
                        .map(|d| system_ix(d).expect("mapped"))
                        .collect();
                    for (si, (player, fleet, plain)) in prep.seats.iter().enumerate() {
                        if !plain {
                            // Hook state: the string engine answers this seat.
                            for (di, dest) in prep.destinations.iter().enumerate() {
                                let rules = legacy_rules(p, player, dest);
                                for (k, ship) in fleet.iter().enumerate() {
                                    if ship.origin.as_str() == dest {
                                        continue;
                                    }
                                    queries += 1;
                                    fallbacks += 1;
                                    (pi, si, di, k, answer_legacy(&rules, ship)).hash(&mut hasher);
                                }
                            }
                            continue;
                        }
                        let seat = seats.seat(player.as_str()).expect("seat");
                        let enemy = board.enemy_ships(seat);
                        let tokens = board.own_command_tokens(seat);
                        // The destination-independent flags (laws, technologies, cards in play):
                        // read once per seat from the string engine, as the adapter would until
                        // those rules move into the core too.
                        let flags = MoverFlags::of(&legacy_rules(p, player, &prep.destinations[0]));
                        let mapped: Vec<(SystemIx, Option<UnitIx>)> = fleet
                            .iter()
                            .map(|s| {
                                (system_ix(s.origin.as_str()).expect("mapped"), unit_ix(&s.type_id))
                            })
                            .collect();
                        for (di, &dest) in dests.iter().enumerate() {
                            let mut rules = CoreRules::with_board(adjacency.clone(), scope, dest, enemy, tokens);
                            flags.apply(&mut rules);
                            for (k, ship) in fleet.iter().enumerate() {
                                let (o, kind) = mapped[k];
                                if o == dest {
                                    continue;
                                }
                                queries += 1;
                                (pi, si, di, k, answer_raw(&rules, o, kind, ship.move_value))
                                    .hash(&mut hasher);
                            }
                        }
                    }
                    cpu += thread_cpu_ns() - t;
                }
                "legacy-search" | "core-search" => {
                    // Rules for the whole position built untimed; one timed block of searches.
                    let raw_galaxy = galaxy_from_legacy(&p.galaxy).expect("mapped");
                    let adjacency = std::sync::Arc::new(raw_galaxy.adjacency());
                    let mut legacy_all = Vec::new();
                    for (player, _, _) in &prep.seats {
                        for dest in &prep.destinations {
                            legacy_all.push(legacy_rules(p, player, dest));
                        }
                    }
                    let copies: Vec<Option<std::sync::Arc<ti4_engine_core::galaxy::Adjacency>>> = legacy_all
                        .iter()
                        .map(|l| {
                            (!uses_galaxy(l, &p.galaxy)).then(|| {
                                std::sync::Arc::new(galaxy_from_legacy(l.parts().galaxy).expect("mapped").adjacency())
                            })
                        })
                        .collect();
                    let raw_all: Vec<CoreRules> = legacy_all
                        .iter()
                        .zip(&copies)
                        .map(|(l, c)| {
                            rules_from_legacy(l, p.sources, c.as_ref().unwrap_or(&adjacency))
                                .expect("mapped")
                        })
                        .collect();
                    let mapped: Vec<Vec<(SystemIx, Option<UnitIx>)>> = prep
                        .seats
                        .iter()
                        .map(|(_, fleet, _)| {
                            fleet
                                .iter()
                                .map(|s| {
                                    (system_ix(s.origin.as_str()).expect("mapped"), unit_ix(&s.type_id))
                                })
                                .collect()
                        })
                        .collect();
                    let n = prep.destinations.len();
                    let t = thread_cpu_ns();
                    for (si, (_, fleet, _)) in prep.seats.iter().enumerate() {
                        for (di, dest) in prep.destinations.iter().enumerate() {
                            for (k, ship) in fleet.iter().enumerate() {
                                if ship.origin.as_str() == dest {
                                    continue;
                                }
                                queries += 1;
                                let a = if mode == "legacy-search" {
                                    answer_legacy(&legacy_all[si * n + di], ship)
                                } else {
                                    let (o, kind) = mapped[si][k];
                                    answer_raw(&raw_all[si * n + di], o, kind, ship.move_value)
                                };
                                (pi, si, di, k, a).hash(&mut hasher);
                            }
                        }
                    }
                    cpu += thread_cpu_ns() - t;
                }
                other => panic!("mode {other}"),
            }
        }
    }
    println!(
        "mode {mode} positions {} queries {queries} fallbacks {fallbacks} cpu_ms {:.1} ns_per_query {:.0} digest {:016x}",
        prepared.len(),
        cpu as f64 / 1e6,
        cpu as f64 / queries.max(1) as f64,
        hasher.finish()
    );
}
