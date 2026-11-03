//! Positions for the surface parity tests (lane B): games on a real map (the random-board setup of the host), played forward by seeded
//! random deciders on the LEGACY engine, so the board, hands, laws and objectives have content. Each position carries its galaxy.
#![allow(dead_code)]

use ti4_content::ContentStore;
use ti4_content::galaxy::Galaxy;
use ti4_engine_legacy as old;
use ti4_model::content_types::{POK, SourceSet};
use ti4_model::id::PlayerId;
use ti4_model::state::GameState;

pub struct Pos {
    pub label: String,
    pub state: GameState,
    pub galaxy: Galaxy,
    pub sources: SourceSet,
}

pub fn content() -> &'static ContentStore {
    ContentStore::embedded()
}

/// `(players, seed, steps)` of the corpus: openings and mid-game positions of 3, 4 and 6 seats.
pub const SPECS: [(usize, u64, usize); 15] = [
    (3, 3, 0),
    (3, 3, 120),
    (4, 9, 40),
    (4, 9, 200),
    (4, 9, 500),
    (4, 5, 900),
    (6, 2, 60),
    (6, 2, 400),
    (6, 7, 1200),
    (3, 11, 1500),
    // (lane B2) two seats, five seats and more late games: more factions, more objectives revealed, more cards in play.
    (2, 21, 300),
    (5, 13, 700),
    (5, 8, 1600),
    (6, 17, 1000),
    (4, 23, 1300),
];

pub fn positions() -> Vec<Pos> {
    SPECS.iter().map(|&(players, seed, steps)| position(players, seed, steps)).collect()
}

pub fn position(players: usize, seed: u64, steps: usize) -> Pos {
    let content = content();
    let ids: Vec<PlayerId> = ["a", "b", "c", "d", "e", "f"][..players].iter().map(|n| PlayerId::new(*n)).collect();
    let (state, galaxy) = ti4_host::map::create_game_with_template(content, &ids, seed, None).expect("a map");
    let mut table = old::choice::Table::new();
    for (i, id) in ids.iter().enumerate() {
        table.seat(id.clone(), Box::new(old::choice::SeededRandom::new(seed * 10 + i as u64)));
    }
    let mut game = old::game::Game::with_table(state, content, table).with_galaxy(galaxy.clone());
    for _ in 0..steps {
        if game.step().finished {
            break;
        }
    }
    Pos { label: format!("{players}p seed {seed} step {steps} round {}", game.state.round), state: game.state.clone(), galaxy, sources: POK }
}
