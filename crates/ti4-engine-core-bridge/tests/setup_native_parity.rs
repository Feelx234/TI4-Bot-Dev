//! The native game setup (`ti4-engine-core-setup`: `start_game` builds the typed core position directly) against the string setup
//! plus `Codec::load` (the oracle): the same position, field by field (`==` on the whole `CoreState`, the Debug text of every slice),
//! the same galaxy, the same decks and first decision. See docs/SETUP_NATIVE_DESIGN.md.
//!
//! Run: `cargo test -p ti4-engine-core-bridge --test setup_native_parity`.

use ti4_content::ContentStore;
use ti4_content::galaxy::Galaxy;
use ti4_engine::core_sync::{Codec, Ctx, Sources};
use ti4_engine::game::Game;
use ti4_engine_core::state::{CoreState, SliceSet};
use ti4_engine_core_setup::{SpeakerChoice, TableSpec, start_game};
use ti4_host::maps::catalog::MapChoice;
use ti4_host::setup::{SpeakerOrder, TableSetup, create, draw};
use ti4_model::content_types::POK;
use ti4_model::id::{FactionId, PlayerId};

fn ids(count: usize) -> Vec<PlayerId> {
    (1..=count).map(|i| PlayerId::new(format!("p{i}"))).collect()
}

/// The old way: `create`, the game the callers make of it, and the core `Native::load` loads from its string position.
fn old_core(content: &ContentStore, setup: &TableSetup, players: &[PlayerId], drawn: &ti4_host::setup::Drawn) -> Result<(CoreState, Galaxy, Galaxy), String> {
    let (state, galaxy) = create(content, setup, players, drawn)?;
    let game = Game::with_seeded_random(state, content, setup.game_seed()).with_galaxy(galaxy.clone());
    let applied = game.galaxy().expect("the game has a galaxy").clone();
    let mut cx = Ctx::of(&game.state).map_err(|e| e.to_string())?;
    let core = Codec::load(&mut cx, &game.state, Sources { galaxy: Some(&applied), rng: Some(&game.rng_positions()) }, SliceSet::ALL).map_err(|e| e.to_string())?;
    Ok((core, applied, galaxy))
}

fn new_core(content: &ContentStore, setup: &TableSetup, players: &[PlayerId], drawn: &ti4_host::setup::Drawn, raw: Galaxy) -> Result<ti4_engine_core_setup::CoreStart, String> {
    let spec = TableSpec {
        content,
        players,
        factions: &drawn.factions,
        seed: setup.game_seed(),
        card_set: Some(&setup.strategy_cards),
        deck_sources: POK,
        speaker: match setup.speaker_order {
            SpeakerOrder::Random => SpeakerChoice::Seeded,
            SpeakerOrder::Seat => SpeakerChoice::Seat(0),
        },
        starting_tech: setup.starting_tech,
        rules_version: None,
    };
    start_game(&spec, raw).map_err(|e| e.to_string())
}

/// The first slice (or table) in which two cores differ, with the position of the first differing character.
fn first_difference(a: &CoreState, b: &CoreState) -> Option<String> {
    macro_rules! slices {
        ($($f:ident),*) => {{ vec![$((stringify!($f), format!("{:?}", a.$f), format!("{:?}", b.$f))),*] }};
    }
    for (name, x, y) in slices!(meta, seats, players, board, map, decks, cards, objectives, agenda, strategy, explore, trade, marks, transient, pending, rng, ledger, texts, owners, loaded) {
        if x != y {
            let at = x.chars().zip(y.chars()).position(|(p, q)| p != q).unwrap_or(x.len().min(y.len()));
            let lo = at.saturating_sub(120);
            let cut = |s: &str| s.chars().skip(lo).take(300).collect::<String>();
            return Some(format!("slice `{name}` differs at char {at}:\n new: ...{}\n old: ...{}", cut(&x), cut(&y)));
        }
    }
    None
}

#[track_caller]
fn same_core(new: &CoreState, old: &CoreState, what: &str) {
    if let Some(diff) = first_difference(new, old) {
        panic!("{what}: {diff}");
    }
    assert!(new == old, "{what}: the cores differ in a field the slice texts do not show");
}

fn table(count: usize, seed: u64, set: &str, tech: bool, speaker: SpeakerOrder, map: MapChoice) -> TableSetup {
    let mut setup = TableSetup::new(count, seed);
    setup.strategy_cards = set.to_owned();
    setup.starting_tech = tech;
    setup.speaker_order = speaker;
    setup.map = map;
    setup
}

/// `false`: the string setup cannot make this table (a template with a tile the corpus lacks), so there is nothing to compare.
fn check(setup: &TableSetup, what: &str) -> bool {
    let content = ContentStore::embedded();
    let players = ids(setup.player_count);
    let drawn = draw(setup, &vec![None; setup.player_count]).expect("draws");
    let Ok((old, old_galaxy, raw)) = old_core(content, setup, &players, &drawn) else { return false };
    let new = new_core(content, setup, &players, &drawn, raw).unwrap_or_else(|e| panic!("{what}: the native setup fails: {e}"));
    same_core(&new.core, &old, what);
    assert_eq!(format!("{:?}", new.galaxy), format!("{old_galaxy:?}"), "{what}: the galaxy differs");
    true
}

#[test]
fn the_plain_tables_of_the_host_golden_are_the_same_position() {
    // The 252 plain rows of host_create_golden that the product creates (Thunder's Edge off): 2-8 seats x 3 card sets x starting
    // technology x speaker x 3 seeds, on the seeded random board.
    for count in 2..=8 {
        for set in ["te", "pok", "base_game_codex1"] {
            for tech in [false, true] {
                for speaker in [SpeakerOrder::Random, SpeakerOrder::Seat] {
                    for seed in [1_u64, 2, 17] {
                        let setup = table(count, seed, set, tech, speaker, MapChoice::Random);
                        assert!(check(&setup, &format!("plain {count}p {set} tech={tech} {speaker:?} seed={seed}")));
                    }
                }
            }
        }
    }
}

#[test]
fn every_map_template_is_the_same_position() {
    let loader = ti4_host::maps::TemplateLoader::load().expect("templates load");
    let mut checked = 0;
    for count in 2..=8 {
        for template in loader.templates_for(count) {
            for (set, tech, seed) in [("pok", true, 3_u64), ("te", false, 4)] {
                let setup = table(count, seed, set, tech, SpeakerOrder::Random, MapChoice::from_stored(Some(&template.alias)));
                if check(&setup, &format!("template {} {count}p {set} tech={tech} seed={seed}", template.alias)) {
                    checked += 1;
                }
            }
        }
    }
    assert!(checked >= 20, "only {checked} template tables");
}

/// The game of `GameSpec::build` (the selfplay corpora, the golden games, `perf_ab`): any faction per seat, any pinned rules version.
#[allow(clippy::too_many_arguments)]
fn check_direct(count: usize, seed: u64, template: Option<&str>, factions: &[&str], card_set: Option<&str>, version: Option<u32>, speaker: Option<usize>, tech: bool, what: &str) -> bool {
    let content = ContentStore::embedded();
    let players: Vec<PlayerId> = ["a", "b", "c", "d", "e", "f", "g", "h"][..count].iter().map(|s| PlayerId::new(*s)).collect();
    let factions: Vec<FactionId> = factions.iter().map(|f| FactionId::new(*f)).collect();
    let assignments: std::collections::BTreeMap<_, _> = players.iter().cloned().zip(factions.iter().cloned()).collect();
    let old = ti4_host::map::create_game_with_template_seated_in(content, &players, seed, template, &assignments, card_set, POK).and_then(|(mut state, galaxy)| {
        if let Some(v) = version {
            state.rules_version = v;
        }
        if let Some(seat) = speaker {
            state.speaker = players[seat].clone();
        }
        if tech {
            ti4_engine::starting_tech::open(&mut state, content, POK);
        }
        let game = Game::with_seeded_random(state, content, seed).with_galaxy(galaxy.clone());
        let applied = game.galaxy().expect("galaxy").clone();
        let mut cx = Ctx::of(&game.state).map_err(|e| e.to_string())?;
        let core = Codec::load(&mut cx, &game.state, Sources { galaxy: Some(&applied), rng: Some(&game.rng_positions()) }, SliceSet::ALL).map_err(|e| e.to_string())?;
        Ok((core, applied, galaxy))
    });
    let Ok((old_core, applied, raw)) = old else { return false };
    let spec = TableSpec {
        content,
        players: &players,
        factions: &factions,
        seed,
        card_set,
        deck_sources: POK,
        speaker: SpeakerChoice::Seat(speaker.unwrap_or(0)),
        starting_tech: tech,
        rules_version: version,
    };
    // The core plays the newest rules version only (`Game` refuses the others at the load): so does the native setup.
    if version.is_some_and(|v| v != ti4_model::state::RULES_VERSION) {
        assert!(matches!(start_game(&spec, raw), Err(ti4_engine_core_setup::SetupError::RulesVersion(..))), "{what}: an older rules version is refused");
        return true;
    }
    let new = start_game(&spec, raw).unwrap_or_else(|e| panic!("{what}: the native setup fails where the string one works: {e}"));
    same_core(&new.core, &old_core, what);
    assert_eq!(format!("{:?}", new.galaxy), format!("{applied:?}"), "{what}: the galaxy differs");
    true
}

#[test]
fn every_faction_deployed_at_a_seat_is_the_same_position() {
    let all = ti4_engine_core_setup::factions::MODULE_ALIASES;
    let rest = ["sol", "hacan", "letnev", "xxcha", "jolnar", "l1z1x", "arborec", "mentak"];
    let mut made = 0;
    for faction in all {
        for count in [3_usize, 4, 6] {
            for seat in [0, count - 1] {
                let mut factions: Vec<&str> = rest.iter().copied().filter(|f| *f != faction).take(count).collect();
                factions.insert(seat, faction);
                factions.truncate(count);
                for tech in [false, true] {
                    // A Keleres variant needs its base faction unplayed: the table that seats it is built from the others.
                    if check_direct(count, 7, None, &factions, Some("pok"), None, Some(seat), tech, &format!("{faction} at seat {seat} of {count} tech={tech}")) {
                        made += 1;
                    }
                }
            }
        }
    }
    assert!(made >= 100, "only {made} tables the string setup could make");
}

#[test]
fn a_pinned_rules_version_a_named_speaker_and_the_card_sets_are_the_same_position() {
    let tables: [(usize, &[&str]); 3] = [
        (3, &["sol", "hacan", "letnev"]),
        (6, &["winnu", "argent", "keleresm", "ghost", "cabal", "empyrean"]),
        (8, &["sol", "hacan", "letnev", "xxcha", "jolnar", "l1z1x", "sardakk", "yin"]),
    ];
    let mut made = 0;
    for (count, factions) in tables {
        for version in [None, Some(ti4_model::state::MIN_SUPPORTED_RULES_VERSION), Some(ti4_model::state::RULES_VERSION - 1)] {
            for card_set in [None, Some("pok"), Some("te"), Some("base_game"), Some("base_game_codex1")] {
                for seat in 0..count {
                    for template in [None, Some("6pStandard")] {
                        if template.is_some() && count != 6 {
                            continue;
                        }
                        if check_direct(count, 11 + seat as u64, template, factions, card_set, version, Some(seat), seat % 2 == 0, &format!("{count}p v{version:?} {card_set:?} speaker {seat} {template:?}")) {
                            made += 1;
                        }
                    }
                }
            }
        }
    }
    assert!(made >= 100, "only {made} tables");
}

/// The comparison above is not vacuous: the tables that ask a starting-technology pick really hold one, the picks that need no answer
/// are settled (technologies and upgraded units differ with and without the flag), and the objective filter changes some decks.
#[test]
fn the_starting_technology_the_feasibility_pass_and_the_marks_are_exercised() {
    let content = ContentStore::embedded();
    let players: Vec<PlayerId> = ["a", "b", "c", "d"].iter().map(|s| PlayerId::new(*s)).collect();
    let build = |factions: &[&str], tech: bool| {
        let fs: Vec<FactionId> = factions.iter().map(|f| FactionId::new(*f)).collect();
        let assignments: std::collections::BTreeMap<_, _> = players.iter().cloned().zip(fs.iter().cloned()).collect();
        let (_, raw) = ti4_host::map::create_game_with_template_seated_in(content, &players, 5, None, &assignments, Some("pok"), POK).expect("string table");
        let spec = TableSpec { content, players: &players, factions: &fs, seed: 5, card_set: Some("pok"), deck_sources: POK, speaker: SpeakerChoice::Seat(0), starting_tech: tech, rules_version: None };
        start_game(&spec, raw).expect("native table")
    };
    let with = build(&["winnu", "argent", "keleresx", "sol"], true);
    let without = build(&["winnu", "argent", "keleresx", "sol"], false);
    let picks = &with.core.transient.misc.starting_tech_picks;
    assert_eq!(picks.len(), 3, "Winnu, Argent and the Keleres ask: {picks:?}");
    assert!(without.core.transient.misc.starting_tech_picks.is_empty());
    assert_ne!(with.core.transient, without.core.transient);
    // The Keleres ask last.
    assert!(matches!(picks.as_slice().last().map(|p| p.source), Some(ti4_engine_core::state::StartingTechSource::OwnedByOthers)));
    // A table with a faction module baselines what its seats hold; one with none does not.
    assert!(!with.core.marks.tail.is_empty());
    let plain = build(&["sol", "hacan", "letnev", "xxcha"], false);
    assert!(plain.core.marks.tail.is_empty());
    // The pass removes objectives from the decks of some boards: a map with no empty system, wormhole or anomaly rules several out.
    let full = ti4_engine_core_setup::deck::build_starting_decks(content, POK, 5);
    let mut changed = 0;
    for seed in 0..40_u64 {
        let spec = TableSpec { content, players: &players, factions: &[FactionId::new("sol"), FactionId::new("hacan"), FactionId::new("letnev"), FactionId::new("xxcha")], seed, card_set: Some("pok"), deck_sources: POK, speaker: SpeakerChoice::Seat(0), starting_tech: false, rules_version: None };
        let assignments: std::collections::BTreeMap<_, _> = players.iter().cloned().zip(spec.factions.iter().cloned()).collect();
        let (_, raw) = ti4_host::map::create_game_with_template_seated_in(content, &players, seed, None, &assignments, Some("pok"), POK).expect("string table");
        let start = start_game(&spec, raw).expect("native table");
        let unfiltered = ti4_engine_core_setup::deck::build_starting_decks(content, POK, seed);
        if start.core.decks.secret.len() != unfiltered.secrets.len() - 4 {
            changed += 1;
        }
    }
    assert!(changed > 0, "the feasibility pass changed no secret deck in 40 boards (decks of seed 5: {} cards)", full.secrets.len());
}

/// A pick with nothing to choose is settled while the table is made (the Keleres next to one seat that owns exactly their two techs).
#[test]
fn a_starting_pick_that_needs_no_answer_is_settled_in_the_same_way() {
    let mut forced = 0;
    for (a, b) in [("keleresx", "sol"), ("keleresa", "hacan"), ("keleresm", "letnev"), ("keleresx", "jolnar"), ("keleresa", "l1z1x"), ("winnu", "sol"), ("argent", "hacan"), ("sol", "keleresx"), ("ghost", "keleresm"), ("nekro", "keleresx"), ("winnu", "keleresa")] {
        assert!(check_direct(2, 9, None, &[a, b], Some("pok"), None, Some(0), true, &format!("{a} + {b} with the picks")), "{a} + {b}");
        let content = ContentStore::embedded();
        let players: Vec<PlayerId> = ["a", "b"].iter().map(|s| PlayerId::new(*s)).collect();
        let fs = [FactionId::new(a), FactionId::new(b)];
        let assignments: std::collections::BTreeMap<_, _> = players.iter().cloned().zip(fs.iter().cloned()).collect();
        let (_, raw) = ti4_host::map::create_game_with_template_seated_in(content, &players, 9, None, &assignments, Some("pok"), POK).expect("table");
        let mk = |tech| start_game(&TableSpec { content, players: &players, factions: &fs, seed: 9, card_set: Some("pok"), deck_sources: POK, speaker: SpeakerChoice::Seat(0), starting_tech: tech, rules_version: None }, raw.clone()).expect("native");
        let (with, without) = (mk(true), mk(false));
        if with.core.players != without.core.players {
            forced += 1;
        }
    }
    assert!(forced >= 2, "{forced} tables had a pick settled at creation");
}

// ---------------------------------------------------------------------------------------------
// the game of the native start (`Game::from_start`) against the game of the string state

use ti4_engine::core_setup::GameStart;

fn spec_of<'a>(setup: &TableSetup, players: &'a [PlayerId], drawn: &'a ti4_host::setup::Drawn, content: &'a ContentStore) -> TableSpec<'a> {
    TableSpec {
        content,
        players,
        factions: &drawn.factions,
        seed: setup.game_seed(),
        card_set: Some(Box::leak(setup.strategy_cards.clone().into_boxed_str())),
        deck_sources: POK,
        speaker: match setup.speaker_order {
            SpeakerOrder::Random => SpeakerChoice::Seeded,
            SpeakerOrder::Seat => SpeakerChoice::Seat(0),
        },
        starting_tech: setup.starting_tech,
        rules_version: None,
    }
}

#[test]
fn a_native_start_is_the_game_of_the_string_state_to_the_first_decisions_and_beyond() {
    let content = ContentStore::embedded();
    let mut played = 0;
    for (count, seed, tech, template) in [(3, 1_u64, false, None), (4, 2, true, None), (6, 3, true, Some("6pStandard")), (2, 17, true, None), (5, 4, false, None), (8, 5, true, None)] {
        let mut setup = table(count, seed, "pok", tech, SpeakerOrder::Random, MapChoice::from_stored(template));
        setup.starting_tech = tech;
        let players = ids(count);
        let drawn = draw(&setup, &vec![None; count]).expect("draws");
        let (state, galaxy) = create(content, &setup, &players, &drawn).expect("string table");
        let mut old = Game::with_seeded_random(state.clone(), content, setup.game_seed()).with_galaxy(galaxy.clone());
        let start = GameStart::new(&spec_of(&setup, &players, &drawn, content), galaxy).expect("native table");
        // The opening state a server stores is the one `create` returned, field by field.
        let opening = start.opening_state().expect("shown");
        assert_eq!(serde_json::to_string(&opening).unwrap(), serde_json::to_string(&state).unwrap(), "{count}p: the opening state's JSON");
        assert_eq!(format!("{opening:?}"), format!("{state:?}"), "{count}p: the opening state's Debug text");
        let mut new = Game::from_start(start, content).expect("game");
        assert_eq!(format!("{:?}", new.state), format!("{:?}", old.state), "{count}p: the position the game holds");
        assert_eq!(format!("{:?}", new.galaxy()), format!("{:?}", old.galaxy()), "{count}p: the board");
        assert_eq!(new.rng_positions(), old.rng_positions());
        assert_eq!(format!("{:?}", new.legal_options()), format!("{:?}", old.legal_options()), "{count}p: the first question");
        for step in 0..250 {
            let (a, b) = (new.step(), old.step());
            assert_eq!(format!("{a:?}"), format!("{b:?}"), "{count}p seed {seed}: step {step}");
            assert_eq!(new.table.log.records, old.table.log.records, "{count}p seed {seed}: the decisions at step {step}");
            if a.finished {
                break;
            }
            played += 1;
        }
        assert_eq!(format!("{:?}", new.state), format!("{:?}", old.state), "{count}p: the position after the steps");
        assert_eq!(new.rng_positions(), old.rng_positions());
        assert!(old.table.log.records.len() > 20, "{count}p: the game made decisions");
    }
    assert!(played > 1000);
}

// ---------------------------------------------------------------------------------------------
// the pieces the setup crate owns, against the thin crate's originals

#[test]
fn the_speaker_of_the_setup_crate_is_the_seeded_speaker_of_the_thin_crate() {
    for seats in 0..=8_usize {
        let table = ids(seats);
        for seed in (0..320_u64).chain([u64::MAX, 1 << 40, 0xDEAD_BEEF]) {
            let thin = ti4_engine::seeded_speaker(&table, seed).map(|p| p.to_string());
            let new = ti4_engine_core_setup::rng::seeded_speaker_index(seats, seed).map(|i| table[i].to_string());
            assert_eq!(new, thin, "{seats} seats seed {seed}");
        }
    }
}

#[test]
fn the_typed_galaxy_of_the_setup_crate_is_the_one_the_thin_crate_loads() {
    let content = ContentStore::embedded();
    let mut boards = 0;
    for count in 2..=8_usize {
        for seed in [1_u64, 2, 3] {
            let setup = table(count, seed, "pok", false, SpeakerOrder::Seat, MapChoice::Random);
            let players = ids(count);
            let drawn = draw(&setup, &vec![None; count]).expect("draws");
            let (_, mut galaxy) = create(content, &setup, &players, &drawn).expect("table");
            ti4_engine::laws::apply_to_galaxy(&ti4_model::state::GameState::new(&players, &[], Default::default(), None, 1), &mut galaxy);
            let thin = ti4_engine::ties::core_adapter::typed_map(&galaxy).expect("expressible");
            let (typed, adjacency) = ti4_engine_core_setup::galaxy::typed_map(&galaxy).expect("expressible");
            assert_eq!(*typed, *thin.galaxy, "{count}p seed {seed}");
            assert_eq!(*adjacency, *thin.adjacency, "{count}p seed {seed}");
            boards += 1;
        }
    }
    assert_eq!(boards, 21);
}

#[test]
fn the_tribuni_check_of_the_setup_crate_says_what_the_thin_crate_says() {
    let aliases = ["sol", "mentak", "xxcha", "argent", "keleresm", "keleresx", "keleresa", "hacan"];
    for a in aliases {
        for b in aliases {
            for c in aliases {
                let seats = [a, b, c];
                let thin = ti4_engine::seating::validate_tribuni(seats).map_err(|e| e.to_string());
                let new = ti4_engine_core_setup::factions::validate_tribuni(seats);
                assert_eq!(new, thin, "{seats:?}");
            }
        }
    }
}

#[test]
fn the_wide_roster_and_the_strategy_sets_are_shared_with_the_thin_crate() {
    let content = ContentStore::embedded();
    assert_eq!(ti4_engine_core_setup::factions::MODULE_ALIASES, ti4_engine::builders::factions::MODULE_ALIASES);
    for sources in [ti4_model::content_types::BASE, POK, ti4_model::content_types::FULL] {
        for set in [None, Some("te"), Some("pok"), Some("base_game"), Some("base_game_codex1"), Some("nope")] {
            let new = ti4_engine_core_setup::strategy::strategy_card_setup(content, sources, set);
            let thin = match set {
                Some(alias) => ti4_engine::setup::strategy_card_setup_for_set(content, alias),
                None => ti4_engine::setup::strategy_card_setup(content, sources),
            };
            assert_eq!(new, thin.ok(), "{sources:?} {set:?}");
        }
    }
}

#[test]
fn the_setup_crate_refuses_what_the_product_refuses() {
    use ti4_engine_core_setup::SetupError;
    let content = ContentStore::embedded();
    let players = ids(4);
    let factions: Vec<FactionId> = ["sol", "hacan", "letnev", "xxcha"].iter().map(|f| FactionId::new(*f)).collect();
    let setup = table(4, 1, "pok", false, SpeakerOrder::Seat, MapChoice::Random);
    let drawn = draw(&setup, &vec![None; 4]).expect("draws");
    let (_, galaxy) = create(content, &setup, &players, &drawn).expect("table");
    let ok = TableSpec { content, players: &players, factions: &factions, seed: 1, card_set: Some("pok"), deck_sources: POK, speaker: SpeakerChoice::Seat(0), starting_tech: false, rules_version: None };
    start_game(&ok, galaxy.clone()).expect("a plain table");
    let refused = |spec: &TableSpec<'_>| start_game(spec, galaxy.clone()).expect_err("refused");
    assert_eq!(refused(&TableSpec { deck_sources: ti4_model::content_types::FULL, ..ok }), SetupError::ThundersEdge);
    assert_eq!(refused(&TableSpec { players: &[], factions: &[], ..ok }), SetupError::NoPlayers);
    assert_eq!(refused(&TableSpec { factions: &factions[..3], ..ok }), SetupError::FactionCount { players: 4, factions: 3 });
    assert_eq!(refused(&TableSpec { card_set: Some("nope"), ..ok }), SetupError::NoStrategyCardSet);
    assert_eq!(refused(&TableSpec { speaker: SpeakerChoice::Seat(4), ..ok }), SetupError::SpeakerSeat(4));
    let nine = ids(9);
    let nine_factions: Vec<FactionId> = (0..9).map(|_| FactionId::new("sol")).collect();
    assert_eq!(refused(&TableSpec { players: &nine, factions: &nine_factions, ..ok }), SetupError::TooManyPlayers(9));
    let twins = [PlayerId::new("p1"), PlayerId::new("p1"), PlayerId::new("p2"), PlayerId::new("p3")];
    assert!(matches!(refused(&TableSpec { players: &twins, ..ok }), SetupError::Seating(_)));
    let unknown: Vec<FactionId> = ["sol", "hacan", "letnev", "nobody"].iter().map(|f| FactionId::new(*f)).collect();
    assert_eq!(refused(&TableSpec { factions: &unknown, ..ok }), SetupError::UnknownFaction("nobody".to_owned()));
    let mentak: Vec<FactionId> = ["sol", "hacan", "mentak", "keleresm"].iter().map(|f| FactionId::new(*f)).collect();
    assert!(matches!(refused(&TableSpec { factions: &mentak, ..ok }), SetupError::Seating(_)));
}

#[test]
fn the_stream_names_of_the_setup_crate_are_the_ones_the_codec_and_the_string_engine_use() {
    use ti4_engine_core::state::{RngDomain as D, Texts};
    let texts = Texts::default();
    let domains = [
        D::Dice, D::Objectives, D::Agendas, D::ActionCards, D::Secrets, D::Relics, D::Galaxy, D::Exploration, D::Map, D::Speaker, D::Fracture, D::Neuraloop,
        D::YssarilHero, D::Direct, D::DeckA, D::DeckB, D::RetryProbe, D::SeatingFactionAssignment, D::SeatingLobbyDraw, D::WideRoster, D::KeleresVariant, D::YinAscendant,
    ];
    for d in domains {
        let ours = ti4_engine_core_setup::rng::domain_text(d).expect("a literal domain");
        assert_eq!(ti4_engine::core_sync::codec::rng::domain_text_in(&texts, d).expect("the codec knows it"), ours, "{d:?}");
        // and the seed of the stream is the string engine's
        assert_eq!(ti4_engine_core_setup::rng::derive_seed(42, ours), ti4_engine::rng::GameRng::derive_seed(42, ours));
    }
    assert!(ti4_engine_core_setup::rng::domain_text(D::Dyn(3, 0)).is_none());
}
