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
    index: usize,
}

impl Decider for BotDecider {
    fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
        let place = self.seats.iter().position(|s| s == choice.player.as_str()).unwrap_or(0);
        let legacy: ti4_engine_legacy::choice::Choice = serde_json::from_value(serde_json::to_value(choice).unwrap()).unwrap();
        let (picked, _, _) = ti4_rule_bot::choose_explained(&legacy, None, self.seed, place, self.index);
        self.index += 1;
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
    let table = Table::with_default(Box::new(BotDecider { seed, seats, index: 0 }));
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
