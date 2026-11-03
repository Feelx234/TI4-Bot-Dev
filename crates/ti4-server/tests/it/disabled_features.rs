//! The switched-off features (`docs/DISABLE_XF123.md`) are refused at the server's edges:
//! Last Bastion and the Crimson Rebellion cannot be chosen (human seat, bot seat, seat-faction),
//! the turn-redo route answers 403, and a saved game that uses structured diplomacy, one of the
//! two factions or a turn redo is reported as not loadable, with its files kept.

use std::sync::Arc;
use std::time::{Duration, Instant};

use reqwest::StatusCode;
use serde_json::{Value, json};
use ti4_host::disabled::DISABLED_FACTIONS;
use ti4_model::id::FactionId;
use ti4_server::create_app;
use ti4_server::session::GameRegistry;
use ti4_server::storage::{FileGameStore, GameHistory, StorageError};

struct Server {
    addr: std::net::SocketAddr,
    registry: Arc<GameRegistry>,
    client: reqwest::Client,
    handle: tokio::task::JoinHandle<()>,
}

impl Drop for Server {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

async fn serve(registry: Arc<GameRegistry>) -> Server {
    ti4_server::session::random_bot::set_random_bot_delay_override(Some(0));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = create_app(registry.clone());
    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    Server {
        addr,
        registry,
        client: reqwest::Client::new(),
        handle,
    }
}

impl Server {
    fn url(&self, path: &str) -> String {
        format!("http://{}{path}", self.addr)
    }

    /// A lobby of `players` seats.
    async fn lobby(&self, players: usize) -> (String, String) {
        let created: Value = self
            .client
            .post(self.url("/api/games"))
            .json(&json!({
                "player_count": players, "seed": 5, "nickname": "Host",
            }))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        (
            created["game_id"].as_str().unwrap().to_owned(),
            created["player_session"].as_str().unwrap().to_owned(),
        )
    }

    async fn post(&self, game: &str, route: &str, token: &str, body: Value) -> (StatusCode, Value) {
        let response = self
            .client
            .post(self.url(&format!("/api/games/{game}/{route}")))
            .header("x-ti4-player-session", token)
            .json(&body)
            .send()
            .await
            .unwrap();
        let status = response.status();
        let text = response.text().await.unwrap();
        (
            status,
            serde_json::from_str(&text).unwrap_or(Value::String(text)),
        )
    }
}

#[tokio::test]
async fn every_dropped_faction_cannot_be_chosen_for_any_seat() {
    let server = serve(Arc::new(GameRegistry::new())).await;
    {
        let (game, host) = server.lobby(4).await;
        for faction in DISABLED_FACTIONS {
            // A bot seat added with the faction.
            let (status, body) = server
                .post(
                    &game,
                    "lobby/bots",
                    &host,
                    json!({"kind": "smart", "faction": faction}),
                )
                .await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{faction}: {body}");
            assert!(body.as_str().unwrap().contains("not available"), "{body}");
            // The host's own seat.
            let (status, body) = server
                .post(
                    &game,
                    "lobby/seat-faction",
                    &host,
                    json!({"player_id": null, "faction": faction}),
                )
                .await;
            assert!(status.is_client_error(), "{faction}: {status} {body}");
        }
        // Nothing was seated by the refusals.
        let view: Value = server
            .client
            .get(server.url(&format!("/api/games/{game}/lobby")))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(view["slots"][1]["occupant"], Value::Null);
    }
}

#[tokio::test]
async fn no_random_table_seats_them_either() {
    // The lobby's pool is what every random seat draws from.
    for faction in DISABLED_FACTIONS {
        assert!(!ti4_engine::seating::LOBBY_FACTIONS.contains(&faction));
    }
    for seed in 0..200 {
        let drawn = ti4_engine::seating::lobby_faction_draw(&[None; 8], seed).unwrap();
        assert!(drawn.iter().all(|f| !DISABLED_FACTIONS.contains(&f.as_str())));
    }
    assert!(ti4_engine::seating::lobby_faction_draw(&[Some("crimson")], 1).is_err());
    assert!(ti4_engine::seating::lobby_faction_draw(&[Some("bastion")], 1).is_err());
}

/// A started 3-seat game on a store: the host, two bots, and the game running.
async fn started_game(store: &Arc<FileGameStore>) -> (Server, String, String) {
    let server = serve(Arc::new(GameRegistry::new().with_store(store.clone()))).await;
    let (game, host) = server.lobby(3).await;
    for _ in 0..2 {
        let (status, body) = server
            .post(&game, "lobby/bots", &host, json!({"kind": "smart"}))
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
    }
    let (status, body) = server.post(&game, "lobby/start", &host, json!({})).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let session = server.registry.get_game(&game).unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    while session.current_pending_decision().is_none() {
        assert!(Instant::now() < deadline, "the game never asked");
        std::thread::sleep(Duration::from_millis(5));
    }
    session.stop();
    (server, game, host)
}

#[tokio::test]
async fn the_turn_redo_route_is_switched_off() {
    let dir = std::env::temp_dir().join(format!("ti4_off_redo_{:016x}", rand::random::<u64>()));
    let store = Arc::new(FileGameStore::new(&dir).unwrap());
    let (server, game, host) = started_game(&store).await;
    for command in [
        json!({"expected_version": 0, "action": "request", "turns": 1}),
        json!({"expected_version": 0, "action": "autoplay"}),
        json!({"expected_version": 0, "action": "restore"}),
        json!({"expected_version": 0, "action": "keep"}),
    ] {
        let (status, body) = server.post(&game, "turn-redo", &host, command.clone()).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{command}: {body}");
        assert_eq!(body.as_str().unwrap(), ti4_host::disabled::TURN_REDO_OFF);
    }
    // Asking where a redo stands still works (nothing is in flight).
    let status = server
        .client
        .get(server.url(&format!("/api/games/{game}/turn-redo")))
        .header("x-ti4-player-session", &host)
        .send()
        .await
        .unwrap()
        .status();
    assert_eq!(status, StatusCode::OK);
    // No history was written: a refused redo leaves the game as it was.
    assert!(store.load_history(&game).unwrap().is_none());
    drop(server);
    let _ = std::fs::remove_dir_all(dir);
}

/// What the registry says about the saved game after a restart.
fn recovery_error(store: &Arc<FileGameStore>, game: &str) -> StorageError {
    let registry = Arc::new(GameRegistry::new().with_store(store.clone()));
    let mut report = registry.recover_all_games_report().expect("a report");
    assert!(report.recovered.is_empty(), "{:?}", report.recovered);
    assert_eq!(report.failed.len(), 1, "{:?}", report.failed);
    let failure = report.failed.remove(0);
    assert_eq!(failure.game_id, game);
    failure.error
}

#[tokio::test]
async fn a_save_that_uses_a_switched_off_feature_is_refused_and_kept() {
    let dir = std::env::temp_dir().join(format!("ti4_off_save_{:016x}", rand::random::<u64>()));
    let store = Arc::new(FileGameStore::new(&dir).unwrap());
    let (server, game, _) = started_game(&store).await;
    drop(server);

    // Untouched, the save loads.
    {
        let registry = Arc::new(GameRegistry::new().with_store(store.clone()));
        let report = registry.recover_all_games_report().expect("a report");
        assert!(report.failed.is_empty(), "{:?}", report.failed);
        assert_eq!(report.recovered, vec![game.clone()]);
        registry.get_game(&game).unwrap().stop();
    }
    let original = store.load_player_init(&game).unwrap();
    let files_before = std::fs::read_dir(store.game_dir(&game).unwrap())
        .unwrap()
        .count();

    // Structured diplomacy on in the initial state.
    let mut tampered = original.clone();
    tampered.initial_state.diplomacy.enabled = true;
    store.save_player_init(&tampered).unwrap();
    let error = recovery_error(&store, &game);
    assert!(matches!(error, StorageError::Disabled(_)), "{error}");
    assert!(error.to_string().contains("diplomacy"), "{error}");

    // Each of the two factions seated.
    for faction in DISABLED_FACTIONS {
        let mut tampered = original.clone();
        tampered.initial_state.players[1].faction = FactionId::new(faction);
        store.save_player_init(&tampered).unwrap();
        let error = recovery_error(&store, &game);
        assert!(matches!(error, StorageError::Disabled(_)), "{error}");
        assert!(error.to_string().contains(faction), "{error}");
    }

    // A history of a turn redo (it carries `rng_marks` and with them the deck reserve).
    store.save_player_init(&original).unwrap();
    let mut history = GameHistory {
        decisions: store.load_decisions(&game).unwrap(),
        redo: Vec::new(),
        events: store.load_events(&game).unwrap(),
        redo_events: Vec::new(),
        event_counter: 0,
        revision: 1,
        generation: 0,
        batches: Vec::new(),
        rng_marks: ti4_host::rng_force::RngMarks::new(),
    };
    history
        .rng_marks
        .deck_plan
        .push(ti4_model::deck_reserve::DeckSegment {
            from: 0,
            until: 1,
            reserved: Vec::new(),
        });
    store.save_history(&game, &history).unwrap();
    let error = recovery_error(&store, &game);
    assert!(matches!(error, StorageError::Disabled(_)), "{error}");
    assert!(error.to_string().contains("turn redo"), "{error}");

    // The refusals deleted nothing (history.json is the one file that was added).
    let files_after = std::fs::read_dir(store.game_dir(&game).unwrap())
        .unwrap()
        .count();
    assert_eq!(files_after, files_before + 1);
    let _ = std::fs::remove_dir_all(dir);
}
