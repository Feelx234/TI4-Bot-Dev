//! D2-8c parity: the native game setup and the position builders of the thin `ti4-engine`
//! (`crates/ti4-engine-next`: `setup`, `deck`, `seating`, `opening`, `starting_tech`,
//! `map_feasibility`, `fixtures`, `builders::*`) against the legacy engine's (`ti4-engine-legacy`),
//! on the same inputs: the same game, field by field (the serialised position and its `Debug` text,
//! which also holds the fields serde skips), the same deck orders and random draws, the same errors.
//!
//! The legacy side is the frozen reference. A test that fails here means the native copy drifted
//! (or a legacy fix landed that the copy lacks): never loosen an assertion, fix the copy.
//!
//! Run: `cargo test -p ti4-engine-core-bridge --test setup_parity`.

use std::collections::BTreeMap;

use serde::Serialize;
use ti4_content::ContentStore;
use ti4_model::content_types::{BASE, FULL, POK, SourceSet};
use ti4_model::id::{BreakthroughId, FactionId, ObjectiveId, PlayerId, StrategyCardId, SystemId, TechnologyId};
use ti4_model::state::GameState;

use ti4_engine as native;
use ti4_engine_legacy as legacy;

fn content() -> &'static ContentStore {
    ContentStore::embedded()
}

fn players(n: usize) -> Vec<PlayerId> {
    (0..n).map(|i| PlayerId::new(format!("p{i}"))).collect()
}

const SOURCES: [(&str, SourceSet); 3] = [("base", BASE), ("pok", POK), ("full", FULL)];
const SEEDS: [u64; 12] = [0, 1, 2, 3, 5, 7, 11, 42, 255, 0xDEAD_BEEF, 1 << 40, u64::MAX];
const CARD_SETS: [Option<&str>; 6] = [
    None,
    Some("te"),
    Some("pok"),
    Some("base_game_codex1"),
    Some("base_game"),
    Some("nope"),
];

fn json<T: Serialize>(value: &T) -> serde_json::Value {
    serde_json::to_value(value).expect("serialises")
}

/// The first path at which two JSON values differ.
fn first_difference(a: &serde_json::Value, b: &serde_json::Value, path: &str) -> Option<String> {
    use serde_json::Value::{Array, Object};
    match (a, b) {
        (Object(x), Object(y)) => {
            for key in x.keys().chain(y.keys()) {
                let (l, r) = (x.get(key), y.get(key));
                match (l, r) {
                    (Some(l), Some(r)) => {
                        if let Some(found) = first_difference(l, r, &format!("{path}.{key}")) {
                            return Some(found);
                        }
                    }
                    _ => return Some(format!("{path}.{key}: native {l:?} / legacy {r:?}")),
                }
            }
            None
        }
        (Array(x), Array(y)) => {
            if x.len() != y.len() {
                return Some(format!("{path}: native length {} / legacy length {}", x.len(), y.len()));
            }
            x.iter()
                .zip(y)
                .enumerate()
                .find_map(|(i, (l, r))| first_difference(l, r, &format!("{path}[{i}]")))
        }
        _ if a == b => None,
        _ => Some(format!("{path}: native {a} / legacy {b}")),
    }
}

/// The native position and the legacy one are the same game, field by field.
#[track_caller]
fn same_state(native: &GameState, legacy: &GameState, what: &str) {
    let (n, l) = (json(native), json(legacy));
    if let Some(at) = first_difference(&n, &l, "") {
        panic!("{what}: positions differ at {at}");
    }
    assert_eq!(
        format!("{native:?}"),
        format!("{legacy:?}"),
        "{what}: the Debug text (every field, serialised or not) differs"
    );
}

#[track_caller]
fn same_debug<T: std::fmt::Debug, U: std::fmt::Debug>(native: &T, legacy: &U, what: &str) {
    assert_eq!(format!("{native:?}"), format!("{legacy:?}"), "{what}");
}

// ---------------------------------------------------------------------------------------------
// setup

#[test]
fn start_game_is_the_same_game_on_every_seed_seat_count_scope_and_card_set() {
    for (name, sources) in SOURCES {
        for n in 1..=8 {
            let table = players(n);
            for seed in SEEDS {
                let speaker = (seed % 2 == 1).then(|| table[n - 1].clone());
                for card_set in CARD_SETS {
                    let what = format!("{name} {n}p seed {seed} set {card_set:?}");
                    let a = native::setup::start_game_seeded_with_card_set(
                        content(), &table, sources, speaker.clone(), seed, card_set,
                    );
                    let b = legacy::setup::start_game_seeded_with_card_set(
                        content(), &table, sources, speaker.clone(), seed, card_set,
                    );
                    match (a, b) {
                        (Ok(a), Ok(b)) => same_state(&a, &b, &what),
                        (Err(a), Err(b)) => same_debug(&a, &b, &what),
                        (a, b) => panic!("{what}: native {a:?} / legacy {b:?}"),
                    }
                }
            }
        }
    }
}

#[test]
fn the_plain_start_functions_and_their_errors_are_the_legacy_ones() {
    for (name, sources) in SOURCES {
        for n in 0..=8 {
            let table = players(n);
            same_debug(
                &native::setup::start_game(content(), &table, sources, None),
                &legacy::setup::start_game(content(), &table, sources, None),
                &format!("start_game {name} {n}p"),
            );
            same_debug(
                &native::setup::start_game_seeded(content(), &table, sources, None, 9),
                &legacy::setup::start_game_seeded(content(), &table, sources, None, 9),
                &format!("start_game_seeded {name} {n}p"),
            );
            same_debug(
                &native::setup::strategy_card_setup(content(), sources),
                &legacy::setup::strategy_card_setup(content(), sources),
                &format!("strategy_card_setup {name}"),
            );
        }
    }
    for alias in ["te", "pok", "base_game", "base_game_codex1", "", "x"] {
        same_debug(
            &native::setup::strategy_card_setup_for_set(content(), alias),
            &legacy::setup::strategy_card_setup_for_set(content(), alias),
            alias,
        );
    }
    for n in 0..=10 {
        assert_eq!(native::setup::cards_per_player(n), legacy::setup::cards_per_player(n));
    }
}

#[test]
fn the_seeded_speaker_is_the_legacy_draw_on_every_table_and_seed() {
    for n in 0..=8 {
        let table = players(n);
        for seed in (0..300_u64).chain([u64::MAX, 1 << 63, 0xFFFF_0000_FFFF]) {
            assert_eq!(
                native::seeded_speaker(&table, seed),
                legacy::seeded_speaker(&table, seed),
                "{n}p seed {seed}"
            );
        }
    }
}

#[test]
fn the_shuffled_decks_are_the_legacy_ones_in_the_same_order() {
    let exclusions = [
        (vec![], vec![]),
        (
            vec!["stake_your_claim".to_owned(), "centralize_trade".to_owned(), "x".to_owned()],
            vec!["x".to_owned()],
        ),
        (vec![], vec!["become_a_legend".to_owned(), "unveil_flagship".to_owned()]),
    ];
    for (name, sources) in SOURCES {
        for seed in SEEDS.into_iter().chain(0..30) {
            let a = native::build_starting_decks(content(), sources, seed);
            let b = legacy::build_starting_decks(content(), sources, seed);
            assert_eq!(json_decks(&a), json_decks_legacy(&b), "{name} seed {seed}");
            for (public, secret) in &exclusions {
                let a = native::deck::build_starting_decks_excluding(
                    content(), sources, seed,
                    &native::deck::Exclusions { public: public.clone(), secret: secret.clone() },
                );
                let b = legacy::deck::build_starting_decks_excluding(
                    content(), sources, seed,
                    &legacy::deck::Exclusions { public: public.clone(), secret: secret.clone() },
                );
                assert_eq!(json_decks(&a), json_decks_legacy(&b), "{name} seed {seed} excluding");
            }
        }
    }
    assert_eq!(native::deck::EXPLORATION_TRAITS, legacy::deck::EXPLORATION_TRAITS);
    assert_eq!(native::deck::UNOFFICIAL_ACTION_CARDS, legacy::deck::UNOFFICIAL_ACTION_CARDS);
}

fn json_decks(d: &native::StartingDecks) -> String {
    format!("{:?}", (&d.objectives, &d.exploration, &d.relics, &d.agendas, &d.action_cards, &d.secrets))
}

fn json_decks_legacy(d: &legacy::StartingDecks) -> String {
    format!("{:?}", (&d.objectives, &d.exploration, &d.relics, &d.agendas, &d.action_cards, &d.secrets))
}

#[test]
fn the_unofficial_action_cards_are_dropped_and_counted_like_the_legacy_deck() {
    for version in [0, 29, ti4_model::state::RULES_OFFICIAL_ACTION_DECK, ti4_model::state::RULES_VERSION] {
        for seed in [0_u64, 4, 9] {
            let mut a = native::start_game_seeded(content(), &players(4), POK, None, seed).unwrap();
            let mut b = legacy::start_game_seeded(content(), &players(4), POK, None, seed).unwrap();
            a.rules_version = version;
            b.rules_version = version;
            assert_eq!(native::deck::action_deck_len(&a), legacy::deck::action_deck_len(&b));
            native::deck::drop_unofficial_action_cards(&mut a);
            legacy::deck::drop_unofficial_action_cards(&mut b);
            same_state(&a, &b, &format!("drop at version {version}"));
            assert_eq!(native::deck::action_deck_len(&a), legacy::deck::action_deck_len(&b));
        }
    }
}

// ---------------------------------------------------------------------------------------------
// seating

#[test]
fn the_seating_tables_and_constants_are_the_legacy_ones() {
    assert_eq!(native::seating::LOBBY_FACTIONS, legacy::seating::LOBBY_FACTIONS);
    assert_eq!(native::seating::LOBBY_BASE_FACTIONS, legacy::seating::LOBBY_BASE_FACTIONS);
    assert_eq!(native::seating::IN_SCOPE_FACTIONS, legacy::seating::IN_SCOPE_FACTIONS);
    assert_eq!(native::seating::EXTRA_SEAT_FACTIONS, legacy::seating::EXTRA_SEAT_FACTIONS);
    for (a, b) in [
        (native::seating::MECATOL, legacy::seating::MECATOL),
        (native::seating::MECATOL_TE, legacy::seating::MECATOL_TE),
        (native::seating::CREUSS_GATE, legacy::seating::CREUSS_GATE),
        (native::seating::CREUSS_HOME, legacy::seating::CREUSS_HOME),
        (native::seating::SORROW, legacy::seating::SORROW),
        (native::seating::CRIMSON_HOME, legacy::seating::CRIMSON_HOME),
        (native::seating::LOCKED_NEXUS, legacy::seating::LOCKED_NEXUS),
        (native::seating::OPEN_NEXUS, legacy::seating::OPEN_NEXUS),
        (native::seating::KELERES_FAMILY, legacy::seating::KELERES_FAMILY),
    ] {
        assert_eq!(a, b);
    }
    for i in 0..40 {
        assert_eq!(native::seating::seat_faction(i), legacy::seating::seat_faction(i));
    }
    for id in ["18", "112", "17", "x", "51"] {
        assert_eq!(native::seating::is_mecatol(id), legacy::seating::is_mecatol(id));
    }
    for id in ["mr", "mrte", "x"] {
        assert_eq!(native::seating::is_mecatol_planet(id), legacy::seating::is_mecatol_planet(id));
    }
    for (_, sources) in SOURCES {
        assert_eq!(
            native::seating::mecatol_for(content(), sources),
            legacy::seating::mecatol_for(content(), sources)
        );
    }
}

/// The faction aliases the faction modules of the legacy engine list, in dispatch order: the native
/// alias list must be exactly this.
#[test]
fn the_wide_roster_aliases_are_the_legacy_faction_modules_in_order() {
    let legacy_aliases: Vec<&str> = legacy::factions::MODULES.iter().map(|m| m.alias).collect();
    assert_eq!(native::builders::factions::MODULE_ALIASES.to_vec(), legacy_aliases);
    assert_eq!(native::builders::factions::KELERES_VARIANTS, legacy::factions::keleres::VARIANTS);
    assert_eq!(
        native::builders::factions::NOT_CHOOSABLE_DURING_SETUP,
        legacy::factions::obsidian::NOT_CHOOSABLE_DURING_SETUP
    );
    assert_eq!(native::builders::factions::NEKRO, legacy::factions::nekro::FACTION);
    for alias in legacy_aliases.iter().copied().chain(["generic", "keleres", ""]) {
        assert_eq!(
            native::builders::factions::is_keleres_faction(alias),
            legacy::factions::keleres::is_keleres_faction(alias)
        );
        assert_eq!(
            native::builders::factions::purges_alliance(alias),
            alias == "mahact",
        );
        for tag in [alias, "keleres", "keleresm", "KELERES"] {
            for unit in [false, true] {
                assert_eq!(
                    native::builders::factions::tag_belongs_to(tag, alias, unit),
                    legacy::factions::keleres::tag_belongs_to(tag, alias, unit),
                    "{tag} {alias} {unit}"
                );
            }
        }
    }
}

#[test]
fn faction_assignments_and_the_lobby_draw_are_the_legacy_ones() {
    for n in 0..=10 {
        let table = players(n);
        assert_eq!(native::seating::seat_in_scope(&table), legacy::seating::seat_in_scope(&table));
        for rotation in 0..8 {
            assert_eq!(
                native::seating::seat_in_scope_rotated(&table, rotation),
                legacy::seating::seat_in_scope_rotated(&table, rotation),
                "{n} {rotation}"
            );
        }
        for seed in SEEDS {
            let all = native::seating::LOBBY_FACTIONS;
            for candidates in [&all[..], &all[..3], &["sol", "sol"][..], &["obsidian", "sol"][..], &[][..]] {
                same_debug(
                    &native::seating::seeded_faction_assignments(candidates, &table, seed),
                    &legacy::seating::seeded_faction_assignments(candidates, &table, seed),
                    &format!("{n} {seed} {candidates:?}"),
                );
            }
        }
    }
    let mut dup = players(2);
    dup.push(PlayerId::new("p0"));
    same_debug(
        &native::seating::seeded_faction_assignments(&["sol", "hacan", "xxcha"], &dup, 1),
        &legacy::seating::seeded_faction_assignments(&["sol", "hacan", "xxcha"], &dup, 1),
        "duplicate player",
    );
    let pools: [Vec<Option<&str>>; 7] = [
        vec![],
        vec![None; 4],
        vec![Some("sol"), None, Some("yin"), None],
        vec![Some("sol"), Some("sol")],
        vec![Some("keleresm")],
        vec![None; 9],
        native::seating::LOBBY_FACTIONS.iter().map(|f| Some(*f)).chain([None, None]).collect(),
    ];
    for pool in &pools {
        for seed in SEEDS {
            same_debug(
                &native::seating::lobby_faction_draw(pool, seed),
                &legacy::seating::lobby_faction_draw(pool, seed),
                &format!("{pool:?} {seed}"),
            );
        }
    }
}

#[test]
fn the_wide_rosters_and_their_draws_are_the_legacy_ones() {
    for (name, sources) in SOURCES {
        assert_eq!(
            native::seating::wide_roster(content(), sources),
            legacy::seating::wide_roster(content(), sources),
            "{name}"
        );
        for n in [1_usize, 2, 3, 5, 8, 12, 40] {
            let table = players(n);
            for seed in 0..40_u64 {
                same_debug(
                    &native::seating::seat_wide(content(), &table, sources, seed),
                    &legacy::seating::seat_wide(content(), &table, sources, seed),
                    &format!("wide {name} {n}p {seed}"),
                );
                same_debug(
                    &native::seating::seat_new(content(), &table, sources, seed),
                    &legacy::seating::seat_new(content(), &table, sources, seed),
                    &format!("new {name} {n}p {seed}"),
                );
            }
            for (a, b) in [
                (native::seating::FactionRoster::InScope, legacy::seating::FactionRoster::InScope),
                (native::seating::FactionRoster::Wide, legacy::seating::FactionRoster::Wide),
                (native::seating::FactionRoster::New, legacy::seating::FactionRoster::New),
            ] {
                same_debug(
                    &native::seating::seat_roster(a, content(), &table, sources, 3),
                    &legacy::seating::seat_roster(b, content(), &table, sources, 3),
                    &format!("roster {name} {n}p"),
                );
            }
        }
    }
    let mut dup = players(3);
    dup.push(PlayerId::new("p1"));
    same_debug(
        &native::seating::seat_wide(content(), &dup, POK, 1),
        &legacy::seating::seat_wide(content(), &dup, POK, 1),
        "duplicate player",
    );
}

#[test]
fn the_tribuni_rules_are_the_legacy_ones() {
    let all: Vec<&str> = legacy::factions::MODULES.iter().map(|m| m.alias).collect();
    for variant in all.iter().copied().chain(["x"]) {
        assert_eq!(native::seating::tribuni_base(variant), legacy::seating::tribuni_base(variant));
    }
    let tables: Vec<Vec<&str>> = vec![
        vec![],
        vec!["keleresm"],
        vec!["keleresm", "mentak"],
        vec!["keleresx", "mentak", "argent"],
        vec!["keleresx", "xxcha"],
        vec!["keleresa", "keleresm"],
        vec!["sol", "hacan", "argent", "keleresa"],
        vec!["mentak", "xxcha", "argent"],
    ];
    for table in &tables {
        assert_eq!(
            native::seating::tribuni_variants_available(table.iter().copied()),
            legacy::seating::tribuni_variants_available(table.iter().copied()),
            "{table:?}"
        );
        same_debug(
            &native::seating::validate_tribuni(table.iter().copied()),
            &legacy::seating::validate_tribuni(table.iter().copied()),
            &format!("{table:?}"),
        );
    }
}

/// Every faction of the corpus the engine knows, plus ids that are not factions.
fn every_faction() -> Vec<String> {
    let mut all: Vec<String> = legacy::factions::MODULES.iter().map(|m| m.alias.to_owned()).collect();
    all.extend(native::seating::LOBBY_FACTIONS.iter().map(|f| (*f).to_owned()));
    all.extend(["generic".to_owned(), "nonexistent".to_owned(), "ghost".to_owned()]);
    all.sort();
    all.dedup();
    all
}

#[test]
fn deploying_every_faction_leaves_the_same_position_as_the_legacy_deploy() {
    for (name, sources) in SOURCES {
        for faction in every_faction() {
            // Three seats; seats 1 and 2 hold Mentak and Xxcha so the Tribuni refuses the matching Keleres.
            let table = players(3);
            let mut a = native::start_game_seeded(content(), &table, sources, None, 5).unwrap();
            let mut b = legacy::start_game_seeded(content(), &table, sources, None, 5).unwrap();
            for (p, f) in [(&table[1], "mentak"), (&table[2], "xxcha")] {
                let ra = native::deploy(&mut a, content(), p, &FactionId::new(f), sources);
                let rb = legacy::deploy(&mut b, content(), p, &FactionId::new(f), sources);
                same_debug(&ra, &rb, &format!("{name} {f}"));
            }
            let id = FactionId::new(faction.as_str());
            let ra = native::deploy(&mut a, content(), &table[0], &id, sources);
            let rb = legacy::deploy(&mut b, content(), &table[0], &id, sources);
            same_debug(&ra, &rb, &format!("{name} deploy {faction}"));
            same_state(&a, &b, &format!("{name} deploy {faction}"));
            // An unknown seat.
            let ra = native::deploy(&mut a, content(), &PlayerId::new("nobody"), &id, sources);
            let rb = legacy::deploy(&mut b, content(), &PlayerId::new("nobody"), &id, sources);
            same_debug(&ra, &rb, "unknown seat");
            same_state(&a, &b, &format!("{name} after the unknown seat"));
        }
    }
}

#[test]
fn deployed_tables_reach_the_same_position_through_the_whole_setup() {
    // A complete table: 2..=8 lobby factions deployed in seat order, then the opening and the
    // feasibility pass, at every scope, on several seeds.
    for (name, sources) in SOURCES {
        for n in 2..=8 {
            for seed in [0_u64, 3, 17, 99] {
                let table = players(n);
                let drawn_a = native::seating::lobby_faction_draw(&vec![None; n], seed).unwrap();
                let drawn_b = legacy::seating::lobby_faction_draw(&vec![None; n], seed).unwrap();
                assert_eq!(drawn_a, drawn_b);
                let mut a = native::start_game_seeded(content(), &table, sources, None, seed).unwrap();
                let mut b = legacy::start_game_seeded(content(), &table, sources, None, seed).unwrap();
                for (player, faction) in table.iter().zip(&drawn_a) {
                    native::deploy(&mut a, content(), player, faction, sources).unwrap();
                    legacy::deploy(&mut b, content(), player, faction, sources).unwrap();
                }
                same_state(&a, &b, &format!("{name} {n}p seed {seed} deployed"));
                // The board, then the map-feasibility pass.
                let assignments: BTreeMap<PlayerId, FactionId> = table.iter().cloned().zip(drawn_a.iter().cloned()).collect();
                let filler_a = native::seating::map_filler(content(), 36, sources, seed);
                let filler_b = legacy::seating::map_filler(content(), 36, sources, seed);
                assert_eq!(filler_a, filler_b);
                let refs: Vec<&str> = filler_a.iter().map(SystemId::as_str).collect();
                let ga = native::build_board(content(), &assignments, &refs, sources);
                let gb = legacy::build_board(content(), &assignments, &refs, sources);
                same_debug(&ga, &gb, &format!("{name} {n}p {seed} board"));
                let (Ok(ga), Ok(gb)) = (ga, gb) else { continue };
                let ca = native::map_feasibility::apply(&mut a, content(), sources, &ga);
                let cb = legacy::map_feasibility::apply(&mut b, content(), sources, &gb);
                assert_eq!(ca, cb, "{name} {n}p {seed} feasibility");
                same_state(&a, &b, &format!("{name} {n}p seed {seed} after the feasibility pass"));
                let homes: Vec<SystemId> = a.players.iter().filter_map(|s| s.home_system.clone()).collect();
                same_debug(
                    &native::map_feasibility::census(content(), sources, &ga, &homes),
                    &legacy::map_feasibility::census(content(), sources, &gb, &homes),
                    "census",
                );
                // Opening measures.
                let start_a = native::opening::snapshot(&a, content(), sources);
                let start_b = legacy::opening::snapshot(&b, content(), sources);
                assert_eq!(start_a, start_b);
                same_debug(
                    &native::opening::measure(&a, &start_a, &BTreeMap::new(), content(), sources),
                    &legacy::opening::measure(&b, &start_b, &BTreeMap::new(), content(), sources),
                    "measure",
                );
                assert_eq!(
                    native::seating::nexus_is_triggered(&a),
                    legacy::seating::nexus_is_triggered(&b)
                );
                assert_eq!(native::seating::mecatol_on(&a), legacy::seating::mecatol_on(&b));
                assert_eq!(native::seating::mecatol_in_galaxy(&ga), legacy::seating::mecatol_in_galaxy(&gb));
            }
        }
    }
}

#[test]
fn boards_neutral_systems_and_the_nexus_are_the_legacy_ones() {
    for (name, sources) in SOURCES {
        for count in [0, 1, 6, 30, 36, usize::MAX] {
            assert_eq!(
                native::neutral_systems(content(), count, sources),
                legacy::neutral_systems(content(), count, sources),
                "{name} {count}"
            );
        }
        for seed in SEEDS {
            for count in [1, 12, 36] {
                assert_eq!(
                    native::seating::map_filler(content(), count, sources, seed),
                    legacy::seating::map_filler(content(), count, sources, seed),
                    "{name} {seed}"
                );
            }
        }
        // Every table size with the in-scope factions, with a full filler and with too little of it.
        for n in 1..=8 {
            let assign = native::seating::seat_in_scope(&players(n));
            for filler_count in [0, 5, 20, 36] {
                let filler: Vec<SystemId> = native::neutral_systems(content(), filler_count, sources);
                let refs: Vec<&str> = filler.iter().map(SystemId::as_str).collect();
                same_debug(
                    &native::build_board(content(), &assign, &refs, sources),
                    &legacy::build_board(content(), &assign, &refs, sources),
                    &format!("{name} {n}p filler {filler_count}"),
                );
            }
            same_debug(
                &native::home_systems(content(), &assign),
                &legacy::home_systems(content(), &assign),
                "home systems",
            );
        }
        // The nexus on a bare galaxy (no Creuss/Crimson gate), twice (idempotent).
        for ids in [vec!["18"], vec!["18", "17"], vec!["18", "94"], vec!["18", "17", "94"]] {
            let Ok(base) = ti4_content::galaxy::Galaxy::build(content(), &ids, sources, 1) else { continue };
            let (mut a, mut b) = (base.clone(), base);
            for _ in 0..2 {
                same_debug(
                    &native::seating::place_wormhole_nexus(&mut a, content(), sources),
                    &legacy::seating::place_wormhole_nexus(&mut b, content(), sources),
                    &format!("{name} {ids:?}"),
                );
                assert_eq!(a, b, "{name} {ids:?}");
            }
            same_debug(
                &native::seating::place_creuss_home(&mut a, content(), sources),
                &legacy::seating::place_creuss_home(&mut b, content(), sources),
                "creuss",
            );
            same_debug(
                &native::seating::place_crimson_home(&mut a, content(), sources),
                &legacy::seating::place_crimson_home(&mut b, content(), sources),
                "crimson",
            );
            assert_eq!(a, b);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// starting technology

/// A table of the named factions, set up and deployed on both sides, with the picks opened.
fn tech_tables(sources: SourceSet, factions: &[&str]) -> (GameState, GameState) {
    let table: Vec<PlayerId> = (0..factions.len()).map(|i| PlayerId::new(format!("s{i}"))).collect();
    let mut a = native::start_game_seeded(content(), &table, sources, None, 4).unwrap();
    let mut b = legacy::start_game_seeded(content(), &table, sources, None, 4).unwrap();
    for (p, f) in table.iter().zip(factions) {
        let id = FactionId::new(*f);
        let ra = native::deploy(&mut a, content(), p, &id, sources);
        let rb = legacy::deploy(&mut b, content(), p, &id, sources);
        same_debug(&ra, &rb, f);
    }
    (a, b)
}

#[test]
fn the_starting_technology_picks_open_ask_and_settle_like_the_legacy_ones() {
    let tables: [(&str, SourceSet, &[&str]); 7] = [
        ("fixed", POK, &["sol", "hacan", "sardakk"]),
        ("winnu", POK, &["winnu", "sol"]),
        ("winnu base", BASE, &["winnu", "sol"]),
        ("argent keleres", POK, &["keleresm", "argent", "hacan"]),
        ("keleres nekro", POK, &["keleresx", "nekro", "sardakk"]),
        ("keleres alone", POK, &["keleresa", "sol"]),
        ("deepwrought", FULL, &["deepwrought", "sol", "winnu", "argent"]),
    ];
    for (name, sources, factions) in tables {
        let (mut a, mut b) = tech_tables(sources, factions);
        native::starting_tech::open(&mut a, content(), sources);
        legacy::starting_tech::open(&mut b, content(), sources);
        same_state(&a, &b, &format!("{name} opened"));
        // Answer every pick, first option, then the last one, then the middle one.
        for pick_rule in 0..3 {
            let (mut a, mut b) = (a.clone(), b.clone());
            let mut steps = 0;
            loop {
                let ca = native::starting_tech::choice(&a, content(), sources);
                let cb = legacy::starting_tech::choice(&b, content(), sources);
                assert_eq!(
                    ca.as_ref().map(json),
                    cb.as_ref().map(json),
                    "{name} rule {pick_rule} step {steps}: the question"
                );
                let (Some(ca), Some(cb)) = (ca, cb) else { break };
                let at = match pick_rule {
                    0 => 0,
                    1 => ca.options.len() - 1,
                    _ => ca.options.len() / 2,
                };
                let pa = native::starting_tech::apply(&mut a, content(), sources, ca.options[at].clone());
                let pb = legacy::starting_tech::apply(&mut b, content(), sources, cb.options[at].clone());
                same_debug(&pa, &pb, &format!("{name} step {steps}"));
                same_state(&a, &b, &format!("{name} rule {pick_rule} step {steps}"));
                steps += 1;
                assert!(steps < 20);
            }
        }
        // A refused answer, and an answer with nothing asked.
        let bogus = native::starting_tech::choice(&a, content(), sources).map(|c| {
            let mut option = c.options[0].clone();
            option.id = "nope".to_owned();
            option
        });
        if let Some(option) = bogus {
            let mut a2 = a.clone();
            let mut b2 = b.clone();
            let mut lopt = legacy::starting_tech::choice(&b, content(), sources).unwrap().options[0].clone();
            lopt.id = "nope".to_owned();
            same_debug(
                &native::starting_tech::apply(&mut a2, content(), sources, option),
                &legacy::starting_tech::apply(&mut b2, content(), sources, lopt),
                "refused answer",
            );
        }
        assert_eq!(native::starting_tech::KIND, legacy::starting_tech::KIND);
        assert_eq!(native::starting_tech::SUBTYPE, legacy::starting_tech::SUBTYPE);
        assert_eq!(native::starting_tech::KELERES_PICKS, legacy::starting_tech::KELERES_PICKS);
    }
}

/// The legacy game accepts a position the native setup built and asks the starting-technology picks
/// before the first strategy card, as it does for its own (the test that stays out of the thin crate
/// because it needs the legacy `Game`).
#[test]
fn a_natively_built_position_plays_in_the_legacy_harness() {
    let (mut a, _) = tech_tables(POK, &["winnu", "argent"]);
    native::starting_tech::open(&mut a, content(), POK);
    a.speaker = PlayerId::new("s0");
    let mut game = legacy::Game::with_seeded_random(a, content(), 7).with_sources(POK);
    let first = game.legal_options().expect("a question");
    assert_eq!(first.context.as_ref().unwrap().subtype, native::starting_tech::SUBTYPE);
    let mut asked = 0;
    while game
        .legal_options()
        .is_some_and(|c| c.context.is_some_and(|x| x.subtype == native::starting_tech::SUBTYPE))
    {
        assert!(game.step().resolved_choice);
        asked += 1;
        assert!(asked < 10);
    }
    assert_eq!(asked, 3, "one Winnu pick, two Argent picks");
    assert!(game.state.starting_tech_picks.is_empty());
    let next = game.legal_options().expect("then the draft");
    assert_eq!(next.context.unwrap().subtype, "draft_strategy_card");
}

// ---------------------------------------------------------------------------------------------
// fixtures

#[test]
fn the_fixture_positions_are_the_legacy_ones() {
    for names in [vec!["a"], vec!["a", "b"], vec!["a", "b", "c", "d", "e", "f"]] {
        same_state(&native::fixtures::game(&names), &legacy::fixtures::game(&names), "game");
        same_state(&native::fixtures::real_game(&names), &legacy::fixtures::real_game(&names), "real_game");
    }
    for (_, sources) in SOURCES {
        let seats = [("a", "sol"), ("b", "hacan"), ("c", "nekro"), ("d", "mahact"), ("e", "empyrean")];
        same_state(
            &native::fixtures::seated_game(&seats, sources),
            &legacy::fixtures::seated_game(&seats, sources),
            "seated_game",
        );
        same_state(
            &native::fixtures::seated_dealt(&seats, sources),
            &legacy::fixtures::seated_dealt(&seats, sources),
            "seated_dealt",
        );
        let mut a = native::fixtures::seated_game(&seats, sources);
        let mut b = legacy::fixtures::seated_game(&seats, sources);
        native::fixtures::redeal_notes(&mut a, sources);
        legacy::fixtures::redeal_notes(&mut b, sources);
        same_state(&a, &b, "redeal_notes");
    }
    for count in [0, 1, 5, 12] {
        assert_eq!(native::fixtures::plain_systems(count), legacy::fixtures::plain_systems(count));
        assert_eq!(native::fixtures::non_home_planets(count), legacy::fixtures::non_home_planets(count));
    }
    for kind in ["nebula", "supernova", "asteroid field", "gravity rift"] {
        assert_eq!(native::fixtures::a_system_where(kind), legacy::fixtures::a_system_where(kind));
    }
    assert_eq!(native::fixtures::a_placed_planet(), legacy::fixtures::a_placed_planet());
    // put / put_on_planet, and the hubs.
    let (system, planet) = native::fixtures::a_placed_planet();
    let (mut a, mut b) = (native::fixtures::game(&["a", "b"]), legacy::fixtures::game(&["a", "b"]));
    let owner = PlayerId::new("a");
    native::fixtures::put(&mut a, &system, "cruiser", &owner, 3);
    legacy::fixtures::put(&mut b, &system, "cruiser", &owner, 3);
    native::fixtures::put_on_planet(&mut a, &system, &planet, "infantry", &owner, 2);
    legacy::fixtures::put_on_planet(&mut b, &system, &planet, "infantry", &owner, 2);
    same_state(&a, &b, "put");
    let (ha, hb) = (native::fixtures::plain_hub(), legacy::fixtures::plain_hub());
    assert_eq!((&ha.galaxy, &ha.centre, &ha.outer), (&hb.galaxy, &hb.centre, &hb.outer));
    assert_eq!(ha.across(&ha.outer[0]), hb.across(&hb.outer[0]));
    let id = native::fixtures::plain_systems(3)[1].clone();
    let (ha, hb) = (native::fixtures::hub_with_centre(&id), legacy::fixtures::hub_with_centre(&id));
    assert_eq!((&ha.galaxy, &ha.centre, &ha.outer), (&hb.galaxy, &hb.centre, &hb.outer));
    let (ha, hb) = (native::fixtures::hub_with_outer(&id), legacy::fixtures::hub_with_outer(&id));
    assert_eq!((&ha.galaxy, &ha.centre, &ha.outer), (&hb.galaxy, &hb.centre, &hb.outer));
}

// ---------------------------------------------------------------------------------------------
// the position builders

fn dealt_table(sources: SourceSet, n: usize, seed: u64) -> (GameState, GameState) {
    let table = players(n);
    let drawn = native::seating::lobby_faction_draw(&vec![None; n], seed).unwrap();
    let mut a = native::start_game_seeded(content(), &table, sources, None, seed).unwrap();
    let mut b = legacy::start_game_seeded(content(), &table, sources, None, seed).unwrap();
    for (player, faction) in table.iter().zip(&drawn) {
        native::deploy(&mut a, content(), player, faction, sources).unwrap();
        legacy::deploy(&mut b, content(), player, faction, sources).unwrap();
    }
    (a, b)
}

#[test]
fn leaders_are_dealt_like_the_legacy_ones() {
    for (name, sources) in SOURCES {
        for faction in every_faction() {
            assert_eq!(
                native::builders::leaders::for_faction(content(), sources, &faction),
                legacy::leaders::for_faction(content(), sources, &faction),
                "{name} {faction}"
            );
            assert_eq!(
                native::builders::leaders::starting_states(content(), sources, &faction),
                legacy::leaders::starting_states(content(), sources, &faction),
                "{name} {faction}"
            );
        }
    }
    use ti4_model::content_types::ContentType;
    for record in content().records(ContentType::Leaders) {
        let Some(id) = record.id().or_else(|| record.text("alias")) else { continue };
        let leader = ti4_model::id::LeaderId::new(id);
        assert_eq!(
            native::builders::leaders::kind_of(content(), &leader),
            legacy::leaders::kind_of(content(), &leader),
            "{id}"
        );
    }
    for (_, sources) in SOURCES {
        let (mut a, mut b) = dealt_table(sources, 4, 2);
        for p in players(4).iter().chain([&PlayerId::new("nobody")]) {
            native::builders::leaders::deploy(&mut a, content(), sources, p);
            legacy::leaders::deploy(&mut b, content(), sources, p);
        }
        same_state(&a, &b, "leaders::deploy");
    }
}

#[test]
fn promissory_notes_are_dealt_taken_and_received_like_the_legacy_ones() {
    use native::builders::promissory as np;
    for (name, sources) in SOURCES {
        let (mut a, mut b) = dealt_table(sources, 6, 8);
        // A table with Mahact (Alliance purged) and Empyrean (extra notes).
        let seats = [("a", "mahact"), ("b", "empyrean"), ("c", "nekro"), ("d", "sol")];
        let mut s = (native::fixtures::seated_game(&seats, sources), legacy::fixtures::seated_game(&seats, sources));
        native::builders::promissory::deal(&mut s.0, content(), sources);
        legacy::promissory::deal(&mut s.1, content(), sources);
        same_state(&s.0, &s.1, &format!("{name} dealt (mahact/empyrean)"));
        native::builders::promissory::deal(&mut a, content(), sources);
        legacy::promissory::deal(&mut b, content(), sources);
        same_state(&a, &b, &format!("{name} dealt"));

        let notes: Vec<String> = a.promissory_notes.keys().cloned().collect();
        assert_eq!(notes, b.promissory_notes.keys().cloned().collect::<Vec<_>>());
        for note in &notes {
            assert_eq!(np::alias_of(note), legacy::promissory::alias_of(note));
            assert_eq!(np::owner_of(note), legacy::promissory::owner_of(note));
            assert_eq!(np::is_play_area(content(), note), legacy::promissory::is_play_area(content(), note), "{note}");
        }
        for text in ["nonsense", "support:sol", "x:y:z", ""] {
            assert_eq!(np::alias_of(text), legacy::promissory::alias_of(text));
            assert_eq!(np::owner_of(text), legacy::promissory::owner_of(text));
        }
        assert_eq!(np::support("sol"), legacy::promissory::support("sol"));
        assert_eq!(np::note_id("cf", "sol"), legacy::promissory::note_id("cf", "sol"));
        assert_eq!(np::SUPPORT_PREFIX, legacy::promissory::SUPPORT_PREFIX);
        assert_eq!(np::GENERIC, legacy::promissory::GENERIC);
        for p in players(6).iter().chain([&PlayerId::new("nobody")]) {
            assert_eq!(np::faction_name(&a, p), legacy::promissory::faction_name(&b, p));
            assert_eq!(np::held_by(&a, p), legacy::promissory::held_by(&b, p));
            for note in notes.iter().take(40) {
                assert_eq!(np::may_receive(&a, p, note), legacy::promissory::may_receive(&b, p, note));
            }
        }
        for faction in ["sol", "mahact", "nobody"] {
            assert_eq!(np::seat_of(&a, faction), legacy::promissory::seat_of(&b, faction));
        }
        // Every note to every holder (the faceup rule runs inside `take`).
        for note in &notes {
            for holder in players(6) {
                let (mut a2, mut b2) = (a.clone(), b.clone());
                np::take(&mut a2, content(), &holder, note);
                legacy::promissory::take(&mut b2, content(), &holder, note);
                same_state(&a2, &b2, &format!("{name} take {note} -> {holder}"));
            }
        }
        // Support for the Throne, to everyone, twice, and for notes that are not Supports.
        for note in notes.iter().map(|n| np::support(&np::owner_of(n).unwrap())).chain(["support:ghost".to_owned(), "cf:sol".to_owned()]) {
            for holder in players(6) {
                let (mut a2, mut b2) = (a.clone(), b.clone());
                for _ in 0..2 {
                    let ra = np::receive(&mut a2, &holder, &note);
                    let rb = legacy::promissory::receive(&mut b2, &holder, &note);
                    assert_eq!(ra, rb, "{name} receive {note} -> {holder}");
                }
                same_state(&a2, &b2, &format!("{name} receive {note} -> {holder}"));
            }
        }
    }
    // The faceup rule at the rules version that changed the Promise of Protection.
    for version in [0, ti4_model::state::RULES_FACTION_EFFECTS - 1, ti4_model::state::RULES_FACTION_EFFECTS] {
        let (mut a, mut b) = dealt_table(POK, 6, 1);
        a.rules_version = version;
        b.rules_version = version;
        for note in a.promissory_notes.keys().cloned().collect::<Vec<_>>() {
            np::take(&mut a, content(), &PlayerId::new("p0"), &note);
            legacy::promissory::take(&mut b, content(), &PlayerId::new("p0"), &note);
        }
        same_state(&a, &b, &format!("take all at version {version}"));
    }
}

#[test]
fn technologies_are_gained_and_upgrades_applied_like_the_legacy_ones() {
    use native::builders::technology as nt;
    let active = legacy::technology::active_aliases(content(), FULL);
    for (name, sources) in SOURCES {
        for faction_set in [0_usize, 1, 2] {
            let (mut a, mut b) = dealt_table(sources, 6, faction_set as u64);
            // Units of every kind on the board of seat p0, so the upgrades have something to replace.
            let (system, planet) = native::fixtures::a_placed_planet();
            for kind in ["carrier", "cruiser", "destroyer", "dreadnought", "fighter", "infantry", "mech", "pds", "spacedock", "flagship", "warsun"] {
                native::fixtures::put(&mut a, &system, kind, &PlayerId::new("p0"), 1);
                legacy::fixtures::put(&mut b, &system, kind, &PlayerId::new("p0"), 1);
                native::fixtures::put_on_planet(&mut a, &system, &planet, kind, &PlayerId::new("p0"), 1);
                legacy::fixtures::put_on_planet(&mut b, &system, &planet, kind, &PlayerId::new("p0"), 1);
            }
            let me = PlayerId::new("p0");
            assert_eq!(
                nt::unit_swaps(&a, content(), sources, &me),
                legacy::technology::unit_swaps(&b, content(), sources, &me)
            );
            for tech in &active {
                let (mut a2, mut b2) = (a.clone(), b.clone());
                nt::grant(&mut a2, &me, tech);
                legacy::technology::grant(&mut b2, &me, tech);
                same_state(&a2, &b2, &format!("{name} grant {tech}"));
                nt::apply_unit_upgrades(&mut a2, content(), sources, &me);
                legacy::technology::apply_unit_upgrades(&mut b2, content(), sources, &me);
                same_state(&a2, &b2, &format!("{name} upgrades after {tech}"));
                nt::gain(&mut a, content(), sources, &me, tech);
                legacy::technology::gain(&mut b, content(), sources, &me, tech);
                same_state(&a, &b, &format!("{name} gain {tech} (cumulative)"));
                assert_eq!(
                    nt::unit_swaps(&a, content(), sources, &me),
                    legacy::technology::unit_swaps(&b, content(), sources, &me)
                );
            }
        }
    }
    // The Table's Grace: Mentak with Cruiser II.
    let seats = [("a", "mentak"), ("b", "sol")];
    for holds in [false, true] {
        let mut a = native::fixtures::seated_game(&seats, FULL);
        let mut b = legacy::fixtures::seated_game(&seats, FULL);
        for state in [&mut a, &mut b] {
            state.player_mut(&PlayerId::new("a")).unwrap().breakthrough =
                holds.then(|| BreakthroughId::new("mentakbt"));
        }
        let me = PlayerId::new("a");
        let (system, _) = native::fixtures::a_placed_planet();
        native::fixtures::put(&mut a, &system, "cruiser", &me, 2);
        legacy::fixtures::put(&mut b, &system, "cruiser", &me, 2);
        native::builders::technology::gain(&mut a, content(), FULL, &me, &TechnologyId::new("cr2"));
        legacy::technology::gain(&mut b, content(), FULL, &me, &TechnologyId::new("cr2"));
        same_state(&a, &b, &format!("mentak cr2 (grace {holds})"));
    }
    // An unseated player.
    let (mut a, mut b) = dealt_table(POK, 3, 1);
    let ghost = PlayerId::new("ghost");
    nt::gain(&mut a, content(), POK, &ghost, &TechnologyId::new("cr2"));
    legacy::technology::gain(&mut b, content(), POK, &ghost, &TechnologyId::new("cr2"));
    same_state(&a, &b, "unseated gain");
}

#[test]
fn objectives_are_revealed_and_points_ledgered_like_the_legacy_ones() {
    use native::builders::objectives as no;
    use ti4_model::content_types::ContentType;
    assert_eq!(no::VICTORY_TARGET, legacy::objectives::VICTORY_TARGET);
    for category in [ContentType::PublicObjectives, ContentType::SecretObjectives] {
        for record in content().records(category) {
            let Some(id) = record.id() else { continue };
            let alias = ObjectiveId::new(id);
            assert_eq!(no::points_for(content(), &alias), legacy::objectives::points_for(content(), &alias), "{id}");
            assert_eq!(no::stage_of(content(), &alias), legacy::objectives::stage_of(content(), &alias), "{id}");
        }
    }
    assert_eq!(
        no::points_for(content(), &ObjectiveId::new("nope")),
        legacy::objectives::points_for(content(), &ObjectiveId::new("nope"))
    );
    for (name, sources) in SOURCES {
        for seed in [0_u64, 6, 30] {
            let (mut a, mut b) = dealt_table(sources, 4, seed);
            for stage in [1_u8, 2, 3, 1, 2, 1, 1, 1, 1, 2] {
                assert_eq!(
                    no::reveal_stage(&mut a, content(), stage),
                    legacy::objectives::reveal_stage(&mut b, content(), stage),
                    "{name} {seed} stage {stage}"
                );
                same_state(&a, &b, &format!("{name} {seed} reveal_stage {stage}"));
            }
            for _ in 0..12 {
                assert_eq!(no::reveal_top(&mut a), legacy::objectives::reveal_top(&mut b));
            }
            same_state(&a, &b, &format!("{name} {seed} reveal_top"));
            for (who, delta, reason) in [
                ("p0", 1, "a"), ("p0", 5, "b"), ("p0", 9, "c"), ("p1", -3, "d"), ("p1", 2, "e"),
                ("p1", -1, "f"), ("p2", 11, "g"), ("p2", -20, "h"), ("nobody", 1, "i"), ("p3", 0, "j"),
            ] {
                let who = PlayerId::new(who);
                assert_eq!(
                    no::adjust_victory_points(&mut a, &who, delta, reason),
                    legacy::objectives::adjust_victory_points(&mut b, &who, delta, reason)
                );
            }
            same_state(&a, &b, &format!("{name} {seed} adjust_victory_points"));
        }
    }
}

#[test]
fn the_phase_is_advanced_and_the_action_turn_begun_like_the_legacy_ones() {
    for (name, sources) in SOURCES {
        for n in [3_usize, 6] {
            for naalu_gift_round in [None, Some("1"), Some("2")] {
                let (mut a, mut b) = dealt_table(sources, n, 2);
                // Make one seat a Naalu so the initiative override is exercised.
                for state in [&mut a, &mut b] {
                    state.player_mut(&PlayerId::new("p1")).unwrap().faction = FactionId::new("naalu");
                    state.round = 2;
                    if let Some(round) = naalu_gift_round {
                        state.faction_marks.insert("naalu:gift_round".to_owned(), round.to_owned());
                    }
                }
                let mut cards: Vec<StrategyCardId> = a.unclaimed_strategy_cards.clone();
                for p in players(n) {
                    let card = cards.remove(0);
                    assert!(a.deal_strategy_card(&p, card.clone()));
                    assert!(b.deal_strategy_card(&p, card));
                }
                a.players[0].stability = true;
                b.players[0].stability = true;
                for step in 0..6 {
                    let oa = native::builders::phase::advance_phase(&mut a);
                    let ob = legacy::phase::advance_phase(&mut b);
                    same_debug(&oa, &ob, &format!("{name} {n}p {naalu_gift_round:?} advance {step}"));
                    same_state(&a, &b, &format!("{name} {n}p {naalu_gift_round:?} advance {step}"));
                    for p in players(n) {
                        native::builders::phase::begin_action_turn(&mut a, &p);
                        legacy::phase::begin_action_turn(&mut b, &p);
                    }
                    same_state(&a, &b, &format!("{name} begin_action_turn {step}"));
                    // The custodians lifted: the status phase leads on to the agenda phase.
                    a.custodians_removed = step % 2 == 0;
                    b.custodians_removed = step % 2 == 0;
                }
            }
        }
    }
    let (mut a, mut b) = dealt_table(POK, 3, 2);
    native::builders::phase::stock_unclaimed_cards(&mut a);
    legacy::phase::stock_unclaimed_cards(&mut b);
    same_state(&a, &b, "stock_unclaimed_cards");
}

#[test]
fn gained_fragments_and_legacy_device_marks_are_written_like_the_legacy_ones() {
    let (mut a, mut b) = dealt_table(POK, 3, 2);
    for (who, kind) in [("p0", "cultural"), ("p0", "CULTURAL"), ("p1", "frontier"), ("p2", "hazardous"), ("nobody", "x")] {
        native::builders::exploration::gain_fragment(&mut a, &PlayerId::new(who), kind);
        legacy::exploration::gain_fragment(&mut b, &PlayerId::new(who), kind);
    }
    same_state(&a, &b, "gain_fragment");
    let record = |chosen: &str, offered: &[&str]| -> serde_json::Value {
        serde_json::json!({"player": "p0", "prompt": "p", "chosen": chosen, "offered": offered})
    };
    let cases = [
        vec![],
        vec![record("pass", &["pass", "play_area:ed2"])],
        vec![record("play_area:ed2:ws", &["play_area:ed2:ws"])],
        vec![record("pass", &["x", "play_area:ed2:ws"])],
        vec![record("play_area:ed2", &["play_area:ed2"]), record("a", &["b"])],
    ];
    for records in cases {
        let ra: Vec<native::choice::DecisionRecord> = serde_json::from_value(serde_json::Value::Array(records.clone())).unwrap();
        let rb: Vec<legacy::choice::DecisionRecord> = serde_json::from_value(serde_json::Value::Array(records)).unwrap();
        let (mut a2, mut b2) = (a.clone(), b.clone());
        native::builders::exploration::mark_legacy_device_ids(&mut a2, &ra);
        legacy::exploration::mark_legacy_device_ids(&mut b2, &rb);
        same_state(&a2, &b2, "mark_legacy_device_ids");
        assert_eq!(a2.legacy_device_ids, b2.legacy_device_ids);
    }
}

#[test]
fn map_edits_are_made_recorded_and_replayed_like_the_legacy_ones() {
    use native::builders::movement as nm;
    for (name, sources) in SOURCES {
        for n in [3_usize, 6] {
            let (a0, b0) = dealt_table(sources, n, 4);
            let table = players(n);
            let assignments: BTreeMap<PlayerId, FactionId> = a0.players.iter().map(|s| (s.id.clone(), s.faction.clone())).collect();
            let filler = native::neutral_systems(content(), 36, sources);
            let refs: Vec<&str> = filler.iter().map(SystemId::as_str).collect();
            let galaxy = native::build_board(content(), &assignments, &refs, sources).unwrap();
            let _ = (&table, &b0);
            let ids: Vec<String> = galaxy.system_ids().iter().map(|s| (*s).to_owned()).collect();
            let edits: Vec<(nm::MapEdit, legacy::movement::MapEdit)> = vec![
                (nm::MapEdit::Swap { a: ids[1].clone(), b: ids[4].clone() }, legacy::movement::MapEdit::Swap { a: ids[1].clone(), b: ids[4].clone() }),
                (nm::MapEdit::Swap { a: ids[2].clone(), b: ids[2].clone() }, legacy::movement::MapEdit::Swap { a: ids[2].clone(), b: ids[2].clone() }),
                (nm::MapEdit::Swap { a: "zz".into(), b: ids[2].clone() }, legacy::movement::MapEdit::Swap { a: "zz".into(), b: ids[2].clone() }),
                (nm::MapEdit::Replace { old: ids[5].clone(), new: nm::NOVA_SEED.into() }, legacy::movement::MapEdit::Replace { old: ids[5].clone(), new: legacy::movement::NOVA_SEED.into() }),
                (nm::MapEdit::Replace { old: ids[6].clone(), new: nm::NOVA_SEED.into() }, legacy::movement::MapEdit::Replace { old: ids[6].clone(), new: legacy::movement::NOVA_SEED.into() }),
                (nm::MapEdit::Replace { old: "q".into(), new: "81".into() }, legacy::movement::MapEdit::Replace { old: "q".into(), new: "81".into() }),
            ];
            assert_eq!(nm::NOVA_SEED, legacy::movement::NOVA_SEED);
            let (mut a, mut b) = (a0.clone(), b0.clone());
            // Contents to carry with a replaced tile.
            native::fixtures::put(&mut a, &SystemId::new(ids[5].as_str()), "cruiser", &PlayerId::new("p0"), 2);
            legacy::fixtures::put(&mut b, &SystemId::new(ids[5].as_str()), "cruiser", &PlayerId::new("p0"), 2);
            for state in [&mut a, &mut b] {
                state.frontier_tokens.insert(SystemId::new(ids[5].as_str()));
                state.ion_storm = Some((SystemId::new(ids[6].as_str()), "ALPHA".to_owned()));
            }
            let (mut ga, mut gb) = (galaxy.clone(), galaxy.clone());
            for (i, (ea, eb)) in edits.iter().enumerate() {
                let ra = nm::apply_map_edit(&mut a, &mut ga, content(), sources, ea);
                let rb = legacy::movement::apply_map_edit(&mut b, &mut gb, content(), sources, eb);
                same_debug(&ra, &rb, &format!("{name} {n}p edit {i}"));
                same_state(&a, &b, &format!("{name} {n}p edit {i}"));
                assert_eq!(ga, gb, "{name} {n}p galaxy after edit {i}");
                same_debug(&nm::recorded_map_edits(&a), &legacy::movement::recorded_map_edits(&b), "recorded");
            }
            // Replaying onto a fresh galaxy gets the same map, twice (idempotent).
            let (mut ra, mut rb) = (galaxy.clone(), galaxy.clone());
            for _ in 0..2 {
                same_debug(
                    &nm::replay_map_edits(&a, &mut ra, content(), sources),
                    &legacy::movement::replay_map_edits(&b, &mut rb, content(), sources),
                    "replay",
                );
                assert_eq!(ra, rb);
            }
            assert_eq!(ra, ga);
        }
    }
}

/// The native modules refer to no legacy path and to no module of another lane that is not on the
/// join list: a source scan, so a `crate::<module>` that falls through the glob to the legacy engine
/// is a failure of this test, not a surprise at the flip.
#[test]
fn the_native_modules_name_only_native_modules_and_the_listed_joins() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../ti4-engine-next/src");
    // Modules another lane owns (lane A wire types, lane B queries): the joins. Each is a path the
    // flip must make native; the list is what docs/D28C_RESUME.md names.
    let joins = ["rng", "choice", "decision_context", "technology"];
    let mut files = vec![];
    for name in ["setup", "deck", "seating", "opening", "starting_tech", "map_feasibility", "fixtures", "builders"] {
        files.push(root.join(format!("{name}.rs")));
    }
    for entry in std::fs::read_dir(root.join("builders")).unwrap() {
        files.push(entry.unwrap().path());
    }
    let own: Vec<String> = ["setup", "deck", "seating", "opening", "starting_tech", "map_feasibility", "fixtures", "builders"]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
    let mut bad = vec![];
    for file in files {
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(!text.contains("ti4_engine_legacy"), "{file:?} names the legacy crate");
        for (number, line) in text.lines().enumerate() {
            let code = line.split("//").next().unwrap_or("");
            for part in code.split("crate::").skip(1) {
                let module: String = part.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
                if module.is_empty() || own.contains(&module) || joins.contains(&module.as_str()) {
                    continue;
                }
                bad.push(format!("{}:{}: crate::{module}", file.display(), number + 1));
            }
        }
    }
    // Test modules are allowed to use more (they run the legacy harness only through the bridge).
    bad.retain(|line| {
        let (file, rest) = line.split_once(':').unwrap();
        let number: usize = rest.split(':').next().unwrap().parse().unwrap();
        let text = std::fs::read_to_string(file).unwrap();
        let tests_at = text.find("#[cfg(test)]").map_or(usize::MAX, |at| text[..at].lines().count() + 1);
        number < tests_at
    });
    assert!(bad.is_empty(), "native modules reach into the legacy glob:\n{}", bad.join("\n"));
}
