//! Package D2-1: the alpha-log round trip on a tiny synthetic data directory (one stored game with three recorded
//! decisions), and the refusal of the live directory.

use std::collections::BTreeMap;
use std::path::PathBuf;

use ti4_content::ContentStore;
use ti4_engine_legacy::Game;
use ti4_engine::choice::{SeededRandom, Table};
use ti4_engine_core_bridge::alpha::{check_game, list_games, refuse_live_dir};
use ti4_engine_core_bridge::roundtrip::Checker;
use ti4_host::setup::{self, TableSetup};
use ti4_model::id::PlayerId;
use ti4_server::storage::{FileGameStore, GameInitRecord};

const GAME: &str = "game_00000000000000000000000000000001";

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ti4-alpha-roundtrip-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A started 3-player game, `n` of its decisions recorded by a random player, stored as the server stores it.
fn store_game(dir: &std::path::Path, n: usize) -> FileGameStore {
    let content = ContentStore::embedded();
    let setup = TableSetup::new(3, 5);
    let players: Vec<PlayerId> = (1..=3).map(|i| PlayerId::new(format!("p{i}"))).collect();
    let drawn = setup::draw(&setup, &[None, None, None]).expect("drawn");
    let (state, galaxy) = setup::create(content, &setup, &players, &drawn).expect("created");
    let table = Table::with_default(Box::new(SeededRandom::new(7)));
    let mut game = Game::with_table(state.clone(), content, table).with_galaxy(galaxy);
    for _ in 0..200 {
        if game.table.log.records.len() >= n {
            break;
        }
        let r = game.step();
        assert!(r.error.is_none(), "{:?}", r.error);
    }
    assert!(game.table.log.records.len() >= n, "the game asked for {n} decisions");
    let records = &game.table.log.records[..n];
    let storage = FileGameStore::new(dir).unwrap();
    storage
        .save_init(&GameInitRecord {
            game_id: GAME.to_owned(),
            seed: Some(setup.game_seed()),
            player_ids: players,
            initial_state: state,
            seats: BTreeMap::new(),
            seat_tokens: BTreeMap::new(),
            map_tiles: Vec::new(),
            map_template: setup.map.stored(),
        })
        .unwrap();
    for r in records {
        storage.append_decision(GAME, r).unwrap();
    }
    storage
}

#[test]
fn a_stored_game_round_trips_at_every_one_of_its_positions() {
    let dir = scratch("ok");
    let storage = store_game(&dir, 3);
    assert_eq!(list_games(&storage).unwrap(), vec![GAME.to_owned()]);
    let mut checker = Checker::new();
    let out = check_game(&storage, GAME, 1, &mut checker).expect("the game replays");
    assert_eq!(out.skipped, None);
    assert_eq!(out.decisions, 3);
    assert_eq!(out.positions, 4, "the initial state and the state after each of the three decisions");
    let s = &checker.stats;
    assert!(s.mismatches.is_empty(), "{:?}", s.mismatches);
    assert!(s.core_diffs.is_empty(), "{:?}", s.core_diffs);
    assert_eq!(s.unmapped_positions(), 0, "{:?}", s.unmapped);
    assert!(s.max_owners >= 3);
    // every other position: the first and the last are always checked
    let mut sparse = Checker::new();
    let out = check_game(&storage, GAME, 2, &mut sparse).unwrap();
    assert_eq!(out.positions, 3, "0, 2, 3");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_game_with_a_switched_off_feature_is_skipped_not_checked() {
    for (tag, tweak) in [("diplomacy", 0), ("bastion", 1)] {
        let dir = scratch(tag);
        let storage = store_game(&dir, 1);
        // rewrite the stored initial state with the feature on
        let mut init = storage.load_init(GAME).unwrap();
        if tweak == 0 {
            init.initial_state.diplomacy.enabled = true;
        } else {
            init.initial_state.players[0].faction = ti4_model::id::FactionId::new("bastion");
        }
        storage.save_init(&init).unwrap();
        let mut checker = Checker::new();
        let out = check_game(&storage, GAME, 1, &mut checker).expect("a skip is not a failure");
        assert!(out.skipped.is_some_and(|s| s.contains("switched off")), "{tag}");
        assert_eq!(checker.stats.positions, 0, "{tag}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn the_live_directory_is_refused_and_a_copy_is_not() {
    let err = refuse_live_dir(std::path::Path::new("/var/lib/ti4-public"));
    // on a machine without the directory there is nothing to refuse
    if std::path::Path::new("/var/lib/ti4-public").exists() {
        assert!(err.is_err());
    }
    let dir = scratch("copy");
    assert!(refuse_live_dir(&dir).is_ok());
    let _ = std::fs::remove_dir_all(&dir);
}
