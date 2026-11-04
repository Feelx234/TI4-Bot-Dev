//! Turn redo through the registry and HTTP. The feature is switched off by decision (`ti4_host::disabled::TURN_REDO_ENABLED`,
//! docs/DISABLE_XF123.md): a redone timeline needs a deck reserve, which the core does not carry. The tests that rewound a seat, played the new
//! turn, auto-played the others and restored the original (permissions, chains, restart, bot seats) described a feature nobody can reach; they are
//! replaced by the refusal they now stand for. The old scenarios are in the history of this file (before the integration-21 commit).

use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::json;

use ti4_content::ContentStore;
use ti4_host::protocol::status::ViewerRole;
use ti4_model::id::PlayerId;
use ti4_server::session::registry::HistoryError;
use ti4_server::session::{GameRegistry, GameSession, SeatController, SessionConfig};

const GAME: &str = "redo_refused";

struct Table {
    registry: Arc<GameRegistry>,
    host_token: String,
    guest_token: String,
}

fn table() -> Table {
    let registry = Arc::new(GameRegistry::new());
    let players = vec![PlayerId::new("p1"), PlayerId::new("p2"), PlayerId::new("p3")];
    let (state, galaxy) = ti4_host::map::create_game_with_template(ContentStore::embedded(), &players, 42, None).unwrap();
    let tiles = ti4_host::map::build_board_tiles(ContentStore::embedded(), &galaxy);
    let config = SessionConfig::new(GAME, state)
        .with_seed(42)
        .with_player_ids(players)
        .with_galaxy(galaxy, tiles)
        .with_seat(PlayerId::new("p1"), SeatController::Human)
        .with_seat(PlayerId::new("p2"), SeatController::Human)
        .with_seat(PlayerId::new("p3"), SeatController::BotFirstOption);
    let session = registry.create_game(config).unwrap();
    let tokens = session.seat_tokens();
    Table { registry, host_token: tokens[&PlayerId::new("p1")].clone(), guest_token: tokens[&PlayerId::new("p2")].clone() }
}

impl Table {
    fn session(&self) -> Arc<GameSession> {
        self.registry.get_game(GAME).expect("game")
    }

    /// Plays `n` decisions with the first option.
    fn play(&self, n: usize) {
        for _ in 0..n {
            let deadline = Instant::now() + Duration::from_secs(20);
            loop {
                let session = self.session();
                if let Some((seat, nonce, version)) = session.current_pending_decision() {
                    let option = session.get_snapshot(&ViewerRole::Player(seat.clone())).pending_choice.expect("pending").choice.options[0].id.clone();
                    if session.submit_choice(&seat, &nonce, version, &option).is_ok() {
                        break;
                    }
                }
                assert!(Instant::now() < deadline, "no decision became pending");
                std::thread::sleep(Duration::from_millis(2));
            }
        }
    }
}

fn is_off(result: &Result<impl std::fmt::Debug, HistoryError>) -> bool {
    matches!(result, Err(HistoryError::Forbidden(why)) if why == ti4_host::disabled::TURN_REDO_OFF)
}

#[test]
fn a_redo_request_is_refused_whoever_asks_and_the_game_is_untouched() {
    let t = table();
    t.play(30);
    let session = t.session();
    let (version, decisions) = (session.game_version(), session.decision_log().len());
    for (token, seat) in [(&t.host_token, Some("p1")), (&t.host_token, Some("p2")), (&t.guest_token, None), (&"not-a-token".to_owned(), Some("p1"))] {
        let result = t.registry.turn_redo_request(GAME, token, version, seat, Some(1));
        assert!(is_off(&result), "{seat:?}: {result:?}");
    }
    assert_eq!(t.registry.turn_redo_status(GAME, &t.host_token).unwrap(), None, "nothing is in flight");
    let session = t.session();
    assert_eq!((session.game_version(), session.decision_log().len()), (version, decisions));
    assert!(session.rng_marks().is_empty(), "no marks, no reservation");
    t.play(3);
    assert!(t.session().error().is_none(), "{:?}", t.session().error());
}

#[test]
fn the_other_commands_have_no_redo_to_act_on() {
    let t = table();
    t.play(10);
    let version = t.session().game_version();
    assert!(t.registry.turn_redo_autoplay(GAME, &t.host_token, version).is_err());
    assert!(t.registry.turn_redo_restore(GAME, &t.host_token, version).is_err());
    // (`keep` with nothing in flight changes nothing and the game stays as it was.)
    let _ = t.registry.turn_redo_keep(GAME, &t.host_token);
    assert_eq!(t.session().game_version(), version);
}

#[tokio::test(flavor = "multi_thread")]
async fn every_http_command_answers_403_with_the_switched_off_text() {
    let t = tokio::task::spawn_blocking(|| {
        let t = table();
        t.play(10);
        t
    })
    .await
    .unwrap();
    let app = ti4_server::create_app(t.registry.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let base = format!("http://{}", listener.local_addr().expect("addr"));
    tokio::spawn(async move { axum::serve(listener, app).await.expect("serve") });
    let version = t.session().game_version();
    for body in [
        json!({"expected_version": version, "action": "request", "seat": "p1"}),
        json!({"expected_version": version, "action": "autoplay"}),
        json!({"expected_version": version, "action": "restore"}),
        json!({"expected_version": version, "action": "keep"}),
    ] {
        let response = reqwest::Client::new()
            .post(format!("{base}/api/games/{GAME}/turn-redo"))
            .header("x-ti4-player-session", &t.host_token)
            .json(&body)
            .send()
            .await
            .expect("request");
        assert_eq!(response.status(), reqwest::StatusCode::FORBIDDEN, "{body}");
        assert_eq!(response.text().await.unwrap(), ti4_host::disabled::TURN_REDO_OFF);
    }
}
