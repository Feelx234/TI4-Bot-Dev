//! Random bot seats: whole games played by `SeatController::BotRandom`, determinism, recovery,
//! undo/redo and the soak. Delay 0 everywhere (`with_random_bot_delay_ms`).

use std::sync::Arc;
use std::time::{Duration, Instant};

use ti4_content::ContentStore;
use ti4_engine::choice::DecisionRecord;
use ti4_model::id::PlayerId;
use ti4_server::session::{GameSession, SeatController, SessionConfig};

pub fn players(n: usize) -> Vec<PlayerId> {
    (1..=n).map(|i| PlayerId::new(format!("p{i}"))).collect()
}

/// An all-bot config (every seat `BotRandom`), delay 0.
pub fn bot_config(id: &str, n: usize, seed: u64, card_set: &str) -> SessionConfig {
    let ids = players(n);
    let (state, galaxy) = ti4_server::map::create_game_with_options(
        ContentStore::embedded(),
        &ids,
        seed,
        None,
        None,
        Some(card_set),
    )
    .expect("create game");
    let tiles = ti4_server::map::build_board_tiles(ContentStore::embedded(), &galaxy);
    let mut config = SessionConfig::new(id, state)
        .with_seed(seed)
        .with_player_ids(ids.clone())
        .with_galaxy(galaxy, tiles)
        .with_random_bot_delay_ms(0);
    for id in ids {
        config = config.with_seat(id, SeatController::BotRandom);
    }
    config
}

#[derive(Debug)]
pub struct Outcome {
    pub decisions: Vec<DecisionRecord>,
    pub finished: bool,
    pub error: Option<String>,
    pub stalled: bool,
    pub elapsed: Duration,
    pub round: u32,
}

/// Run a session until it finishes, errors, reaches `cap` decisions, or makes no progress for
/// `stall` seconds.
pub fn drive(session: &GameSession, cap: usize, stall: Duration, wall: Duration) -> Outcome {
    let start = Instant::now();
    let mut last_len = 0;
    let mut last_progress = Instant::now();
    let mut stalled = false;
    loop {
        let len = session.decision_log().len();
        if len != last_len {
            last_len = len;
            last_progress = Instant::now();
        }
        if session.is_finished() || session.error().is_some() || len >= cap {
            break;
        }
        if last_progress.elapsed() > stall || start.elapsed() > wall {
            stalled = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let state = session.current_state();
    Outcome {
        decisions: session.decision_log(),
        finished: session.is_finished(),
        error: session.error(),
        stalled,
        elapsed: start.elapsed(),
        round: state.round,
    }
}

#[test]
fn one_bot_game_plays_and_reports_speed() {
    let session = GameSession::start(bot_config("rb_one", 4, 7, "te"));
    let out = drive(&session, 4000, Duration::from_secs(20), Duration::from_secs(300));
    session.stop();
    eprintln!(
        "decisions={} finished={} round={} err={:?} stalled={} elapsed={:?} ({:.0}/s)",
        out.decisions.len(),
        out.finished,
        out.round,
        out.error,
        out.stalled,
        out.elapsed,
        out.decisions.len() as f64 / out.elapsed.as_secs_f64()
    );
    assert!(out.error.is_none(), "{:?}", out.error);
    let _ = Arc::new(());
}
