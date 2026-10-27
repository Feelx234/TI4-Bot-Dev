//! Sizes and clone costs of the core state against the targets of docs/ENGINE_CORE_STATE.md 1.6:
//! `size_of::<CoreState>() <= 4 KB`, `size_of::<Player>() <= 600 B`, `CoreState::clone` allocates
//! nothing (apart from the log tail on the first append) and takes <= 300 ns.

use ti4_engine_core::state::{Board, CoreState, Player, Players, Rng, Meta, Piece, PlanetCell, SysCell};
use ti4_engine_core::Seat;

#[test]
fn print_sizes_and_check_targets() {
    let sizes = [
        ("CoreState", std::mem::size_of::<CoreState>()),
        ("Player", std::mem::size_of::<Player>()),
        ("Players", std::mem::size_of::<Players>()),
        ("Board", std::mem::size_of::<Board>()),
        ("SysCell", std::mem::size_of::<SysCell>()),
        ("PlanetCell", std::mem::size_of::<PlanetCell>()),
        ("Piece", std::mem::size_of::<Piece>()),
        ("Meta", std::mem::size_of::<Meta>()),
        ("Rng", std::mem::size_of::<Rng>()),
    ];
    for (name, size) in sizes {
        eprintln!("size_of::<{name}>() = {size}");
    }
    assert!(std::mem::size_of::<CoreState>() <= 4096);
    assert!(std::mem::size_of::<Player>() <= 600);
}

#[test]
fn clone_time() {
    let mut s = CoreState::all_loaded();
    let mut p = Vec::new();
    for i in 0..6u8 {
        p.push(Player::new(Seat(i), ti4_engine_core::generated::Faction::ALL[i as usize]));
    }
    s.players = Players::from_vec(p);
    let n = 200_000;
    let start = std::time::Instant::now();
    let mut keep = 0usize;
    for _ in 0..n {
        let c = std::hint::black_box(&s).clone();
        keep += c.meta.round as usize;
        std::hint::black_box(&c);
    }
    let per = start.elapsed().as_nanos() as f64 / f64::from(n);
    eprintln!("CoreState::clone = {per:.0} ns (debug or release as built), keep={keep}");
}

#[test]
fn player_parts() {
    use ti4_engine_core::state::*;
    use std::mem::size_of;
    eprintln!("econ {} tech {} cards {} leaders {} timers {} combat {} prod {} cardt {} misc {}",
        size_of::<PlayerEcon>(), size_of::<PlayerTech>(), size_of::<PlayerCards>(), size_of::<PlayerLeaders>(),
        size_of::<Timers>(), size_of::<CombatTimers>(), size_of::<ProductionTimers>(), size_of::<CardTimers>(), size_of::<PlayerMisc>());
}
