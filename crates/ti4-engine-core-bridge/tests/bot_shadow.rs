//! Tests of the shadow harness (docs/todo/D3-bots.md D3-0 item 3): the self-check compares every call with a
//! position and finds nothing; each mutant of the typed half is caught; the games are played exactly as
//! `workload::golden_games` plays them (same decision hashes); `--spec` replays one game of a config.

use std::cell::RefCell;
use std::rc::Rc;

use ti4_engine::choice::{Choice, ChoiceOption, Decider, IllegalChoice, SeatObservation, SeededRandom};
use ti4_engine_core_bridge::shadow::{self, Config, Mode, Mutant, NoTyped};
use ti4_engine_core_bridge::workload;
use ti4_selfplay::spec::SEATS;

/// `tools/engine_profile/profile-20.json` cut after `max_steps` engine steps per game (a test must not play 2400 decisions in debug).
fn short_config(max_steps: usize) -> String {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tools/engine_profile/profile-20.json");
    let mut value: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(root).unwrap()).unwrap();
    value["limits"]["max_steps"] = serde_json::json!(max_steps);
    // Tests run in parallel threads: every call writes its own file (a shared one is read while another truncates it).
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("bot_shadow_test_{}_{max_steps}_{n}.json", std::process::id()));
    std::fs::write(&path, value.to_string()).unwrap();
    path.to_string_lossy().into_owned()
}

fn run(config: &str, games: &[usize], cfg: &Config) -> shadow::Report {
    shadow::run(config, cfg, &|i| games.contains(&i), || Box::new(NoTyped)).unwrap()
}

/// The decider calls with a position and a non-empty choice, counted by a decider that knows nothing of the shadow.
struct Counter {
    seed: u64,
    decided: usize,
    random: SeededRandom,
    seeing: Rc<RefCell<usize>>,
}

impl Decider for Counter {
    fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
        self.decided += 1;
        self.random.choose(choice)
    }
    fn choose_seeing(&mut self, choice: &Choice, seen: &SeatObservation<'_>) -> Result<ChoiceOption, IllegalChoice> {
        let index = self.decided;
        self.decided += 1;
        if choice.options.is_empty() {
            return self.random.choose(choice);
        }
        *self.seeing.borrow_mut() += 1;
        let seat = SEATS.iter().position(|s| *s == choice.player.as_str()).unwrap_or(0);
        let (picked, _, _) = ti4_rule_bot::choose_explained(choice, Some(seen), self.seed, seat, index);
        Ok(choice.options[picked].clone())
    }
}

fn counted_calls(config: &str, game: usize, max_steps: usize) -> usize {
    let (_, specs) = workload::specs(config).unwrap();
    let spec = &specs[game];
    let mut g = spec.build().unwrap();
    let seeing = Rc::new(RefCell::new(0));
    g.table.set_default(Box::new(Counter {
        seed: spec.seed,
        decided: 0,
        random: SeededRandom::new(spec.seed),
        seeing: Rc::clone(&seeing),
    }));
    for _ in 0..max_steps {
        if g.state.finished {
            break;
        }
        assert!(g.step().error.is_none());
    }
    drop(g);
    let n = *seeing.borrow();
    n
}

#[test]
fn self_check_compares_every_call_and_finds_nothing() {
    let config = short_config(900);
    let report = run(&config, &[0], &Config::default());
    let expected = counted_calls(&config, 0, 900);
    assert!(expected > 100, "the game must reach real decisions, got {expected}");
    assert_eq!(report.calls(), expected as u64, "calls with a position");
    assert_eq!(report.compared(), expected as u64, "every call is compared in self-check mode");
    assert!(report.mismatches.is_empty(), "{}", shadow::render(&report, Mode::SelfCheck));
    assert!((report.coverage("*") - 1.0).abs() < f64::EPSILON);
    assert!(report.by_subtype.contains_key("draft_strategy_card") || report.by_subtype.len() > 3);
}

#[test]
fn the_shadow_plays_the_game_the_workload_plays() {
    let config = short_config(900);
    let report = run(&config, &[0], &Config::default());
    let (_, hashes) = workload::golden_games(&config, 1, 1_000_000, &|_| true, &mut |_, _| {}).unwrap();
    assert_eq!(report.games.len(), 1);
    assert_eq!(report.games[0].decisions_hash, hashes[0]);
}

#[test]
fn every_mutant_of_the_typed_half_is_caught() {
    let config = short_config(600);
    for mutant in [Mutant::Pick, Mutant::Smart, Mutant::Rng] {
        let cfg = Config { mutant, ..Config::default() };
        let report = run(&config, &[0], &cfg);
        assert!(!report.mismatches.is_empty(), "mutant {mutant:?} was not caught");
        let m = &report.mismatches[0];
        assert!(m.spec.contains("\"seed\""), "a mismatch carries the spec of its game: {}", m.spec);
        assert!(!m.options.is_empty());
        match mutant {
            Mutant::Pick => assert_ne!(m.string.picked, m.typed.picked),
            Mutant::Smart => assert_ne!(m.string.smart, m.typed.smart),
            Mutant::Rng => assert_ne!(m.string.word_pos, m.typed.word_pos),
            Mutant::None => unreachable!(),
        }
        // the game under test is never changed by the typed half
        let clean = run(&config, &[0], &Config::default());
        assert_eq!(report.games[0].decisions_hash, clean.games[0].decisions_hash);
        assert_eq!(report.games[0].choices_hash, clean.games[0].choices_hash);
    }
}

#[test]
fn source_mode_without_a_source_skips_every_call() {
    let config = short_config(400);
    let cfg = Config { mode: Mode::Source, ..Config::default() };
    let report = run(&config, &[0], &cfg);
    assert!(report.calls() > 0);
    assert_eq!(report.compared(), 0);
    let skipped: u64 = report.by_subtype.values().flat_map(|s| s.skipped.values()).sum();
    assert_eq!(skipped, report.calls(), "Compared + Skipped = calls");
    assert!(report.coverage("*") < f64::EPSILON);
    assert!(report.mismatches.is_empty());
}

#[test]
fn a_spec_replays_its_game() {
    let config = short_config(500);
    let (_, specs) = workload::specs(&config).unwrap();
    let spec = serde_json::to_string(&specs[3]).unwrap();
    let alone = shadow::config_of_spec(&spec).unwrap();
    let path = std::env::temp_dir().join(format!("bot_shadow_spec_test_{}_{}.json", std::process::id(), line!()));
    std::fs::write(&path, alone).unwrap();
    // the one-game config carries the default limits; the cut is the flag `--max-steps`
    let from_spec = run(path.to_str().unwrap(), &[0], &Config { max_steps: Some(500), ..Config::default() });
    let from_config = run(&config, &[3], &Config::default());
    assert_eq!(from_spec.games[0].decisions_hash, from_config.games[0].decisions_hash);
    assert_eq!(from_spec.games[0].choices_hash, from_config.games[0].choices_hash);
}

#[test]
fn bot_version_flag_reaches_both_halves() {
    let config = short_config(500);
    let mut cfg = Config::default();
    cfg.bot.insert("bot_version".to_owned(), "1".to_owned());
    let v1 = run(&config, &[0], &cfg);
    assert!(v1.mismatches.is_empty());
    let v6 = run(&config, &[0], &Config::default());
    // version 1 plays another game than version 6 (its tuning differs), so the decision chain differs
    assert_ne!(v1.games[0].decisions_hash, v6.games[0].decisions_hash);
    let mut bad = Config::default();
    bad.bot.insert("bot_version".to_owned(), "not-a-number".to_owned());
    assert!(shadow::run(&config, &bad, &|_| true, || Box::new(NoTyped)).is_err());
}

#[test]
fn the_typed_view_agrees_with_the_string_view() {
    let config = short_config(1500);
    let cfg = Config { seen_stride: 7, ..Config::default() };
    let report = run(&config, &[0], &cfg);
    let p = &report.parity;
    assert!(p.positions > 50, "positions: {}", p.positions);
    assert!(p.unmapped.is_empty(), "positions the typed state could not hold: {:?}", p.unmapped);
    assert!(p.compared > 5000, "accessor answers compared: {}", p.compared);
    assert!(p.mismatches.is_empty(), "{:#?}", &p.mismatches[..p.mismatches.len().min(20)]);
}

/// `profile-20` with Thunder's Edge on (the game's source scope then differs from the full scope of the hook kernel,
/// and the Fracture's planets hold neutral units though they are not in that scope): the typed view must read the
/// game's scope and the scope-free planets of a system. Found by the first Thunder's Edge parity run.
fn thunders_edge_config(max_steps: usize) -> String {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tools/engine_profile/profile-20.json");
    let mut value: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(root).unwrap()).unwrap();
    value["limits"]["max_steps"] = serde_json::json!(max_steps);
    for setup in value["setups"].as_array_mut().unwrap() {
        setup["thunders_edge"] = serde_json::json!(true);
        setup["seeds"] = serde_json::json!("1..3");
    }
    let path = std::env::temp_dir().join(format!("bot_shadow_te_{}.json", std::process::id()));
    std::fs::write(&path, value.to_string()).unwrap();
    path.to_string_lossy().into_owned()
}

#[test]
fn the_typed_view_agrees_with_the_string_view_in_thunders_edge_games() {
    let config = thunders_edge_config(9000);
    let cfg = Config { seen_stride: 15, ..Config::default() };
    // games 2 and 3 of the config: the Fracture is in play and holds neutral units late in the game, and a seat controls
    // planets outside the game's scope
    let report = run(&config, &[2, 3], &cfg);
    let p = &report.parity;
    assert!(p.positions > 50, "positions: {}", p.positions);
    assert!(p.mismatches.is_empty(), "{:#?}", &p.mismatches[..p.mismatches.len().min(20)]);
    // positions whose Transient names an owner outside the table are the documented bail D3-bail-08
    assert!(p.unmapped.keys().all(|k| k.starts_with("Transient: owner not in the table")), "{:?}", p.unmapped);
}
