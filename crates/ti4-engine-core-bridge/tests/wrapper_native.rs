//! D2-8 lane D: the `Game` wrapper of the thin crate plays the same game as the bridge's `NativeGame` (same bot, same seat order), and a
//! step that fails leaves the position and the journal as they were.

use sha2::{Digest, Sha256};
use ti4_content::ContentStore;
use ti4_engine::native::{Game, GameError};
use ti4_engine::wire::choice::{Choice, ChoiceOption, Decider, IllegalChoice, Table};
use ti4_engine_core_bridge::native_play::{lobby_spec, play_game, End, NativeGame};

/// The bot of the native playthrough as a table decider: the rule bot's random pick, seeded by game seed, seat place and decision index.
struct BotDecider {
    seed: u64,
    seats: Vec<String>,
    index: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

impl Decider for BotDecider {
    fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
        let place = self.seats.iter().position(|s| s == choice.player.as_str()).unwrap_or(0);
        let (picked, _, _) = ti4_rule_bot::choose_explained(choice, None, self.seed, place, self.index.fetch_add(1, std::sync::atomic::Ordering::SeqCst));
        Ok(choice.options[picked.min(choice.options.len() - 1)].clone())
    }
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// `NativeGame::fingerprint` of a wrapper game: the position (JSON, sorted keys) and the chain of the answers.
fn fingerprint(game: &Game<'_>) -> String {
    let mut chain = String::new();
    for r in &game.table.log.records {
        let mut h = Sha256::new();
        h.update(chain.as_bytes());
        h.update(format!("{}\u{1}{}\u{1}{}", r.player, r.prompt, r.chosen).as_bytes());
        chain = hex(&h.finalize());
    }
    let mut h = Sha256::new();
    h.update(serde_json::to_value(&game.state).unwrap().to_string().as_bytes());
    h.update(chain.as_bytes());
    hex(&h.finalize())[..16].to_owned()
}

fn wrapper(faction: &str, players: usize, seed: u64) -> Game<'static> {
    let spec = lobby_spec(faction, players, seed);
    let legacy = spec.build().expect("a lobby game builds");
    let seats: Vec<String> = legacy.state.players.iter().map(|p| p.id.as_str().to_owned()).collect();
    let galaxy = legacy.galaxy().cloned().expect("a lobby game has a map");
    let table = Table::with_default(Box::new(BotDecider { seed, seats, index: Default::default() }));
    Game::try_with_table(legacy.state.clone(), ContentStore::embedded(), table).expect("playable").with_galaxy(galaxy)
}

#[test]
fn the_wrapper_plays_the_quick_game_of_the_native_playthrough_to_the_same_fingerprint() {
    let mut game = wrapper("sol", 4, 1);
    let mut steps = 0;
    while !game.state.finished {
        let result = game.step();
        assert_eq!(result.error, None, "step {steps}");
        steps += 1;
        assert!(steps < 3_000, "the game ends");
    }
    assert_eq!(game.steps(), 1165, "the steps of the native playthrough");
    assert_eq!(game.step().error, None, "a finished game stops quietly");
    // The one way left to pay is told, not asked and not journaled (`production::pay_step` of the string).
    assert!(
        game.table.log.records.iter().all(|r| !(r.offered.len() == 1 && r.offered[0].starts_with("pay"))),
        "a payment with a single way was put to the table"
    );
    assert_eq!(fingerprint(&game), "74d5675ac754ac81");
    // And the NativeGame of the bridge gives the same (the pin of its own test).
    let mut native = NativeGame::from_spec(&lobby_spec("sol", 4, 1)).unwrap();
    let played = play_game(&mut native, 2_000, 50);
    assert_eq!(played.end, End::Finished);
    assert_eq!(played.fingerprint, fingerprint(&game));
    assert_eq!(ti4_engine::product::gap_count(), 0);
}

#[test]
fn a_game_below_the_latest_rules_version_is_refused_at_creation() {
    let spec = lobby_spec("sol", 4, 1);
    let legacy = spec.build().unwrap();
    let mut state = legacy.state.clone();
    state.rules_version = ti4_model::state::RULES_VERSION - 1;
    let refused = Game::try_new(state, ContentStore::embedded());
    assert!(matches!(refused, Err(GameError::Unmapped(_))), "{:?}", refused.err());
}

fn play_on(game: &mut Game<'_>) {
    let mut steps = 0;
    while !game.state.finished {
        assert_eq!(game.step().error, None);
        steps += 1;
        assert!(steps < 3_000);
    }
}

/// A decider that fails (as the wasm host's does when the seats change) while its switch is on, and otherwise plays the bot.
struct Switchable {
    inner: BotDecider,
    /// Asks to answer before failing; `usize::MAX` = never fail.
    fail: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

impl Decider for Switchable {
    fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
        let left = self.fail.load(std::sync::atomic::Ordering::SeqCst);
        if left != usize::MAX && left > 0 {
            self.fail.store(left - 1, std::sync::atomic::Ordering::SeqCst);
        } else if left == 0 {
            return Err(IllegalChoice::DeciderFailed { player: choice.player.clone(), prompt: choice.prompt.clone(), reason: "the host changed its seats".to_owned() });
        }
        self.inner.choose(choice)
    }
}

/// Regression (plan 2.3): a decider that fails inside a step must come back as `step().error` with the position AND the journal as before the
/// step, and the step must be repeatable. Without the rollback of the core and of the decision log the retry diverges from the uninterrupted game.
#[test]
fn a_decider_that_fails_mid_step_leaves_the_position_and_the_journal_and_the_game_goes_on_identically() {
    let reference = {
        let mut g = wrapper("sol", 4, 1);
        play_on(&mut g);
        fingerprint(&g)
    };
    let spec = lobby_spec("sol", 4, 1);
    let legacy = spec.build().unwrap();
    let seats: Vec<String> = legacy.state.players.iter().map(|p| p.id.as_str().to_owned()).collect();
    let fail = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(usize::MAX));
    let index = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let table = Table::with_default(Box::new(Switchable { inner: BotDecider { seed: 1, seats, index: index.clone() }, fail: fail.clone() }));
    let mut game = Game::try_with_table(legacy.state.clone(), ContentStore::embedded(), table).unwrap().with_galaxy(legacy.galaxy().cloned().unwrap());
    let mut failures = 0;
    let mut steps = 0;
    while !game.state.finished {
        // Every 7th step the decider answers once and fails at its SECOND question: a step with a single question just succeeds; one with more
        // reports the failure after the first answer was applied (and journaled), which the rollback must take back.
        if steps % 7 == 5 {
            let (state, log, events, decided) = (serde_json::to_value(&game.state).unwrap(), game.table.log.records.len(), game.events.len(), index.load(std::sync::atomic::Ordering::SeqCst));
            fail.store(1, std::sync::atomic::Ordering::SeqCst);
            let result = game.step();
            fail.store(usize::MAX, std::sync::atomic::Ordering::SeqCst);
            if let Some(error) = result.error {
                assert!(matches!(error, GameError::IllegalChoice(IllegalChoice::DeciderFailed { .. })), "{error:?}");
                assert_eq!(serde_json::to_value(&game.state).unwrap(), state, "step {steps}: the position is as before");
                assert_eq!((game.table.log.records.len(), game.events.len()), (log, events), "step {steps}: the journal and the events are as before");
                index.store(decided, std::sync::atomic::Ordering::SeqCst);
                failures += 1;
            } else {
                steps += 1;
                continue;
            }
        }
        assert_eq!(game.step().error, None);
        steps += 1;
    }
    assert!(failures >= 10, "the decider failed mid-step {failures} times");
    // (The bot's decision index advanced on the refused asks of a failed step only if the decider is called: Switchable fails before the bot.)
    assert_eq!(fingerprint(&game), reference);
}

/// The fork rule: a fork continues exactly like the original (here with its journal), and a worker's fork is `Send`.
#[test]
fn a_fork_keeping_the_log_continues_like_the_original_and_the_worker_fork_is_send() {
    fn is_send<T: Send>(_: &T) {}
    let mut original = wrapper("sol", 4, 1);
    for _ in 0..400 {
        assert_eq!(original.step().error, None);
    }
    let mut fork = original.fork_keeping_log();
    // The fork's table is a fresh first-option table with the journal: the original's bot decider is not forked (as in the string engine).
    let worker = original.fork_for_worker();
    is_send(&worker);
    assert_eq!(serde_json::to_value(&fork.state).unwrap(), serde_json::to_value(&original.state).unwrap());
    assert_eq!(fork.rolls(), original.rolls());
    assert_eq!(fork.events, original.events);
    assert_eq!(fork.rng_positions(), original.rng_positions());
    let legal = original.legal_options();
    assert_eq!(fork.legal_options(), legal);
    // The same answers on both: each step of the fork gives the original's position.
    fork.table = Table::with_default(Box::new(ti4_engine::wire::choice::FirstOption));
    original.table = Table::with_default(Box::new(ti4_engine::wire::choice::FirstOption));
    for _ in 0..200 {
        let (a, b) = (original.step(), fork.step());
        assert_eq!(a, b);
        assert_eq!(serde_json::to_value(&fork.state).unwrap(), serde_json::to_value(&original.state).unwrap());
        if original.state.finished {
            break;
        }
    }
}

/// Side channels (plan 2.4): the dice feed, the undo latches, the strategic watch, the aftermath handoff and the draft's lone card.
#[test]
fn the_side_channels_of_the_string_are_fed_by_the_native_loop() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    let mut game = wrapper("sol", 4, 1);
    let observation = ti4_engine::wire::observation::ExecutionObservation::default();
    game.bind_observation(observation.clone());
    let strategic = Arc::new(AtomicUsize::new(0));
    let aftermath = Arc::new(AtomicUsize::new(0));
    let (s2, a2) = (strategic.clone(), aftermath.clone());
    game.on_strategic_action_chosen(move |_, _| {
        s2.fetch_add(1, Ordering::SeqCst);
    });
    game.on_aftermath(move |_, _, _| {
        a2.fetch_add(1, Ordering::SeqCst);
    });
    let mut seen_randomness = false;
    let mut auto = 0;
    while !game.state.finished {
        assert_eq!(game.step().error, None);
        let activity = observation.take();
        seen_randomness |= activity.randomness;
        auto += game.table.take_auto_resolved().len();
    }
    assert!(seen_randomness, "a game with combat moves the dice stream, which the undo latch reports");
    assert!(auto >= 1, "the last card of the draft is taken with a note, not asked");
    assert!(strategic.load(Ordering::SeqCst) >= 4, "strategic actions were told to the watch");
    assert!(aftermath.load(Ordering::SeqCst) >= 1, "the aftermath handoff was told");
    assert!(!game.rolls().is_empty(), "the dice of the game are in the roll feed");
    assert!(game.rolls().iter().all(|r| !r.faces.is_empty() && r.reason != ""), "every roll has dice and a reason");
}

/// Moved from `ti4-engine-next/src/deck.rs` (the native modules do not name the legacy crate): the string draw skips the extra vetos only from
/// the official-deck rules version.
#[test]
fn a_draw_skips_the_extra_vetos_only_from_the_new_version() {
    use ti4_model::id::{ActionCardId, PlayerId};
    let a = PlayerId::new("a");
    for (version, expected) in [
        (ti4_model::state::RULES_OFFICIAL_ACTION_DECK, ["sabotage", "veto"]),
        (ti4_model::state::RULES_OFFICIAL_ACTION_DECK - 1, ["veto3", "sabotage"]),
    ] {
        let mut state = ti4_engine::fixtures::game(&["a", "b"]);
        state.rules_version = version;
        state.action_card_deck = ["veto3", "sabotage", "veto4", "veto", "bribery"].into_iter().map(ActionCardId::new).collect();
        let mut table = ti4_engine_legacy::choice::Table::with_default(Box::new(ti4_engine_legacy::choice::Scripted::new(Vec::<String>::new())));
        let drawn = ti4_engine::action_cards::draw(&mut state, ContentStore::embedded(), &mut table, &a, 2).expect("drawn");
        let drawn: Vec<&str> = drawn.iter().map(ActionCardId::as_str).collect();
        assert_eq!(drawn, expected, "version {version}");
    }
}

/// What `ti4_host::disabled::check_playable` refuses, the wrapper refuses with `GameError::Disabled` at creation and at the load (Thunder's Edge
/// content, structured diplomacy, a deck reserve, a dropped faction); the strategy card set `te` is not content.
#[test]
fn the_wrapper_refuses_what_check_playable_refuses_with_disabled() {
    let legacy = lobby_spec("sol", 4, 1).build().unwrap();
    let refused = |edit: &dyn Fn(&mut ti4_model::state::GameState)| {
        let mut state = legacy.state.clone();
        edit(&mut state);
        Game::try_new(state, ContentStore::embedded()).err()
    };
    assert!(matches!(refused(&|s| s.thunders_edge = true), Some(GameError::Disabled(_))));
    assert!(matches!(refused(&|s| s.diplomacy.enabled = true), Some(GameError::Disabled(_))));
    assert!(matches!(refused(&|s| s.thunders_edge_system = Some(ti4_model::id::SystemId::new("1"))), Some(GameError::Disabled(_))), "the flipped Thunder's Edge system is content too");
    assert!(matches!(refused(&|s| s.players[0].faction = ti4_model::id::FactionId::new("ralnel")), Some(GameError::Disabled(_))));
    assert_eq!(refused(&|_| {}), None, "the lobby game itself is playable");
    // A game that was refused never steps: its first step reports the same refusal.
    let mut state = legacy.state.clone();
    state.thunders_edge = true;
    let mut game = Game::new(state, ContentStore::embedded());
    assert!(matches!(game.step().error, Some(GameError::Disabled(_))));
}

/// Regression (server crash recovery hung): `install_table` carries the decision log of the table it replaces over to the new one, as the string
/// engine's did; a resumed session wires its deciders into a forked game and must keep the log the fork had.
#[test]
fn installing_a_table_keeps_the_decision_log_of_the_game() {
    let mut game = wrapper("sol", 4, 1);
    for _ in 0..40 {
        assert_eq!(game.step().error, None);
    }
    let logged = game.table.log.records.len();
    assert!(logged > 0, "the first steps asked questions");
    game.install_table(Table::new());
    assert_eq!(game.table.log.records.len(), logged);
}

/// `laws::apply_to_galaxy` is native now: it leaves the map exactly as the string's does, on every position of a played game and on positions with
/// each law, token and piece it reads.
#[test]
fn the_native_apply_to_galaxy_equals_the_legacy_one() {
    let spec = lobby_spec("sol", 4, 1);
    let legacy = spec.build().unwrap();
    let galaxy = legacy.galaxy().cloned().unwrap();
    let same = |state: &ti4_model::state::GameState, what: &str| {
        let (mut a, mut b) = (galaxy.clone(), galaxy.clone());
        ti4_engine::laws::apply_to_galaxy(state, &mut a);
        ti4_engine_legacy::laws::apply_to_galaxy(state, &mut b);
        assert_eq!(a, b, "{what}");
        a
    };
    let mut game = wrapper("sol", 4, 1);
    for i in 0..600 {
        if i % 20 == 0 {
            same(&game.state, &format!("step {i}"));
        }
        assert_eq!(game.step().error, None);
    }
    let mut state = game.state.clone();
    let plain = same(&state, "as played");
    for law in ["travel_ban", "wormhole_recon", "nexus"] {
        state.laws.insert(law.to_owned(), "for".to_owned());
    }
    state.nexus_unlocked = true;
    state.wormhole_tokens.insert("GAMMA".to_owned(), ti4_model::id::SystemId::new("19"));
    state.ion_storm = Some((ti4_model::id::SystemId::new("20"), "ALPHA".to_owned()));
    let loaded = same(&state, "laws, nexus, tokens, ion storm");
    assert_ne!(loaded, plain, "the position changed the map");
    let player = state.players[0].id.clone();
    state.active = Some(player.clone());
    state.active_system = Some(ti4_model::id::SystemId::new("18"));
    state.player_mut(&player).unwrap().spatial_conduit = Some(state.activation_seq);
    let seq = state.activation_seq;
    state.player_mut(&player).unwrap().lost_star.push(seq);
    same(&state, "conduit and lost star");
    state.system_mut(&ti4_model::id::SystemId::new("21")).units.push(ti4_model::units::Unit::new(ti4_model::id::UnitTypeId::new("ghost_flagship"), player));
    same(&state, "a Ghost flagship");
    state.laws.clear();
    state.laws.insert("travel_ban".to_owned(), "for".to_owned());
    state.faction_marks.insert("keleres|laws_blank".to_owned(), state.turn_seq.to_string());
    state.phase = ti4_model::state::Phase::Action;
    same(&state, "laws blanked");
}
