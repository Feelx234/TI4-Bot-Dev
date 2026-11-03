//! What the product refuses to play, at the server's edges (docs/D28E_RESUME.md, owner decisions
//! of 2026-11-03): a game of a rules version below the newest, Thunder's Edge content (the table
//! switch, the `expedition` preset) and the dropped factions. Creation answers 400 with the
//! reason, recovery reports a refusal (`StorageError::Disabled`, `RecoveryFailure::is_refusal`)
//! and leaves every file alone, the replay endpoint will not export such a game. Each test fails
//! without the change that added it.

use std::sync::Arc;

use reqwest::StatusCode;
use serde_json::{Value, json};
use ti4_content::ContentStore;
use ti4_host::disabled::{DISABLED_FACTIONS, Disabled};
use ti4_model::id::{FactionId, PlayerId};
use ti4_model::state::{MIN_SUPPORTED_RULES_VERSION, RULES_VERSION};
use ti4_server::create_app;
use ti4_server::session::{GameRegistry, SeatController, SessionConfig};
use ti4_server::storage::{FileGameStore, StorageError};

async fn spawn(registry: Arc<GameRegistry>) -> String {
    let app = create_app(registry);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let address = listener.local_addr().expect("addr");
    tokio::spawn(async move { axum::serve(listener, app).await.expect("serve") });
    format!("http://{address}")
}

async fn create(base: &str, body: Value) -> (StatusCode, Value) {
    let response = reqwest::Client::new()
        .post(format!("{base}/api/games"))
        .json(&body)
        .send()
        .await
        .expect("create request");
    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    (status, serde_json::from_str(&text).unwrap_or(Value::String(text)))
}

#[test]
fn the_newest_rules_version_is_the_only_one_played() {
    // The decision of 2026-11-03: a new game is the newest, and nothing older is supported.
    assert_eq!(MIN_SUPPORTED_RULES_VERSION, RULES_VERSION);
}

#[tokio::test(flavor = "multi_thread")]
async fn creating_a_table_with_thunders_edge_content_is_refused_with_the_reason() {
    let base = spawn(Arc::new(GameRegistry::new())).await;
    for body in [
        json!({"player_count": 3, "nickname": "Ada", "thunders_edge": true}),
        json!({"player_count": 3, "nickname": "Ada", "thunders_edge": true, "fill_seats": "random"}),
        json!({"player_count": 3, "nickname": "Ada", "start_preset": "expedition"}),
        json!({"player_count": 3, "nickname": "Ada", "start_preset": "expedition+rot"}),
    ] {
        let (status, text) = create(&base, body.clone()).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}: {text}");
        let text = text.as_str().unwrap_or_default();
        assert!(
            text.contains("Thunder's Edge content") && text.contains("switched off"),
            "{body}: {text}"
        );
    }
    // The strategy card set `te` is not content; neither is `false` or leaving the field out.
    for body in [
        json!({"player_count": 3, "nickname": "Ada", "strategy_card_set": "te"}),
        json!({"player_count": 3, "nickname": "Ada", "thunders_edge": false}),
        json!({"player_count": 4, "nickname": "Ada", "strategy_card_set": "te", "fill_seats": "random"}),
    ] {
        let (status, text) = create(&base, body.clone()).await;
        assert_eq!(status, StatusCode::OK, "{body}: {text}");
        assert!(text["lobby"].get("thunders_edge").is_none_or(|on| on == false));
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn the_lobby_settings_cannot_switch_thunders_edge_content_on() {
    let registry = Arc::new(GameRegistry::new());
    let base = spawn(registry.clone()).await;
    let (status, created) =
        create(&base, json!({"player_count": 3, "nickname": "Ada"})).await;
    assert_eq!(status, StatusCode::OK, "{created}");
    let game = created["game_id"].as_str().unwrap();
    let token = created["player_session"].as_str().unwrap();
    for (on, ok) in [(true, false), (false, true)] {
        let response = reqwest::Client::new()
            .post(format!("{base}/api/games/{game}/lobby/settings"))
            .header("x-ti4-player-session", token)
            .json(&json!({"thunders_edge": on}))
            .send()
            .await
            .unwrap();
        let status = response.status();
        let text = response.text().await.unwrap();
        assert_eq!(status.is_success(), ok, "{on}: {status} {text}");
        if !ok {
            assert!(text.contains("Thunder's Edge content"), "{text}");
        }
    }
}

/// A started 3-seat game of the host and two smart bots on a store, stopped at its first question.
async fn saved_game(store: &Arc<FileGameStore>) -> String {
    ti4_server::session::random_bot::set_random_bot_delay_override(Some(0));
    let registry = Arc::new(GameRegistry::new().with_store(store.clone()));
    let base = spawn(registry.clone()).await;
    let (status, created) = create(&base, json!({"player_count": 3, "nickname": "Ada"})).await;
    assert_eq!(status, StatusCode::OK, "{created}");
    let game = created["game_id"].as_str().unwrap().to_owned();
    let token = created["player_session"].as_str().unwrap();
    let client = reqwest::Client::new();
    for _ in 0..2 {
        let response = client
            .post(format!("{base}/api/games/{game}/lobby/bots"))
            .header("x-ti4-player-session", token)
            .json(&json!({"kind": "smart"}))
            .send()
            .await
            .unwrap();
        assert!(response.status().is_success());
    }
    let response = client
        .post(format!("{base}/api/games/{game}/lobby/start"))
        .header("x-ti4-player-session", token)
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert!(response.status().is_success(), "{}", response.text().await.unwrap());
    let session = registry.get_game(&game).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while session.current_pending_decision().is_none() {
        assert!(std::time::Instant::now() < deadline, "the game never asked");
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    session.stop();
    game
}

fn directory_listing(store: &Arc<FileGameStore>, game: &str) -> Vec<(String, Vec<u8>)> {
    let mut files: Vec<(String, Vec<u8>)> = std::fs::read_dir(store.game_dir(game).unwrap())
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            (
                entry.file_name().to_string_lossy().into_owned(),
                std::fs::read(entry.path()).unwrap_or_default(),
            )
        })
        .collect();
    files.sort();
    files
}

/// The failure the registry reports for the only saved game, as a refusal.
fn refusal_of(store: &Arc<FileGameStore>, game: &str) -> Disabled {
    let registry = Arc::new(GameRegistry::new().with_store(store.clone()));
    let mut report = registry.recover_all_games_report().expect("a report");
    assert!(report.recovered.is_empty(), "{:?}", report.recovered);
    assert_eq!(report.failed.len(), 1, "{:?}", report.failed);
    let failure = report.failed.remove(0);
    assert_eq!(failure.game_id, game);
    assert!(failure.is_refusal(), "a refusal, not damage: {failure:?}");
    let StorageError::Disabled(reason) = failure.error else {
        panic!("a refusal: {failure:?}");
    };
    reason
}

#[tokio::test(flavor = "multi_thread")]
async fn a_save_of_an_older_rules_version_or_with_dropped_content_is_refused_and_every_file_kept() {
    let dir = std::env::temp_dir().join(format!("ti4_refusals_{:016x}", rand::random::<u64>()));
    let store = Arc::new(FileGameStore::new(&dir).unwrap());
    let game = saved_game(&store).await;

    // Untouched, the save is the newest version and loads.
    {
        let registry = Arc::new(GameRegistry::new().with_store(store.clone()));
        let report = registry.recover_all_games_report().expect("a report");
        assert!(report.failed.is_empty(), "{:?}", report.failed);
        assert_eq!(report.recovered, vec![game.clone()]);
        registry.get_game(&game).unwrap().stop();
    }
    let original = store.load_player_init(&game).unwrap();
    assert_eq!(original.initial_state.rules_version, RULES_VERSION);

    // Every older version, the first game of the alpha (no field, version 0) included.
    for older in [0, 1, 27, MIN_SUPPORTED_RULES_VERSION - 1] {
        let mut tampered = original.clone();
        tampered.initial_state.rules_version = older;
        store.save_player_init(&tampered).unwrap();
        let before = directory_listing(&store, &game);
        let reason = refusal_of(&store, &game);
        assert_eq!(
            reason,
            Disabled::OlderVersion {
                found: older,
                min: MIN_SUPPORTED_RULES_VERSION
            }
        );
        assert!(reason.to_string().contains("older version"), "{reason}");
        // Nothing was touched, written or deleted by the refusal.
        assert_eq!(directory_listing(&store, &game), before, "version {older}");
    }

    // Thunder's Edge content in the initial state.
    store.save_player_init(&original).unwrap();
    let mut tampered = original.clone();
    tampered.initial_state.thunders_edge = true;
    store.save_player_init(&tampered).unwrap();
    assert_eq!(refusal_of(&store, &game), Disabled::ThundersEdge);

    // Each dropped faction seated: Last Bastion, the Crimson Rebellion, the Keleres (three
    // forms), the Deepwrought Scholarate, the Firmament, the Obsidian, the Ral Nel Consortium.
    for faction in DISABLED_FACTIONS {
        let mut tampered = original.clone();
        tampered.initial_state.players[1].faction = FactionId::new(faction);
        store.save_player_init(&tampered).unwrap();
        assert_eq!(
            refusal_of(&store, &game),
            Disabled::Faction(faction.to_owned()),
            "{faction}"
        );
    }

    // The version wins when a save is old and uses dropped content as well.
    let mut tampered = original.clone();
    tampered.initial_state.rules_version = 0;
    tampered.initial_state.thunders_edge = true;
    store.save_player_init(&tampered).unwrap();
    assert!(refusal_of(&store, &game).is_older_version());

    // Put back, the save loads again: the refusals changed nothing.
    store.save_player_init(&original).unwrap();
    let registry = Arc::new(GameRegistry::new().with_store(store.clone()));
    let report = registry.recover_all_games_report().expect("a report");
    assert_eq!(report.recovered, vec![game.clone()], "{:?}", report.failed);
    registry.get_game(&game).unwrap().stop();
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_replay_endpoint_does_not_export_a_game_of_an_older_version() {
    let registry = Arc::new(GameRegistry::new());
    let host = PlayerId::new("p1");
    let guest = PlayerId::new("p2");
    let players = vec![host.clone(), guest.clone()];
    let (mut state, galaxy) =
        ti4_host::map::create_game_with_template(ContentStore::embedded(), &players, 42, None)
            .unwrap();
    let tiles = ti4_host::map::build_board_tiles(ContentStore::embedded(), &galaxy);
    state.rules_version = MIN_SUPPORTED_RULES_VERSION - 1;
    let config = SessionConfig::new("replay_old", state)
        .with_seed(42)
        .with_player_ids(players)
        .with_galaxy(galaxy, tiles)
        .with_seat(host.clone(), SeatController::Human)
        .with_seat(guest, SeatController::Human);
    let session = registry.create_game(config).unwrap();
    let token = session.seat_tokens()[&host].clone();
    let base = spawn(registry).await;
    let response = reqwest::Client::new()
        .get(format!("{base}/api/games/replay_old/replay"))
        .header("x-ti4-player-session", token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let text = response.text().await.unwrap();
    assert!(text.contains("older version"), "{text}");
}

/// A lobby saved by a build that still had the switch (`thunders_edge` true in its record) recovers
/// as a lobby, but starting it is refused with the reason: no game of Thunder's Edge content exists.
#[tokio::test(flavor = "multi_thread")]
async fn a_saved_lobby_with_the_old_switch_cannot_be_started() {
    let dir = std::env::temp_dir().join(format!("ti4_refusals_lobby_{:016x}", rand::random::<u64>()));
    let store = Arc::new(FileGameStore::new(&dir).unwrap());
    let registry = Arc::new(GameRegistry::new().with_store(store.clone()));
    let base = spawn(registry).await;
    let (status, created) = create(&base, json!({"player_count": 3, "nickname": "Ada"})).await;
    assert_eq!(status, StatusCode::OK, "{created}");
    let game = created["game_id"].as_str().unwrap().to_owned();
    let token = created["player_session"].as_str().unwrap().to_owned();
    // The record as an older build wrote it with the switch on.
    let mut record = store.load_player_lobby(&game).unwrap().expect("a lobby record");
    assert!(!record.thunders_edge);
    record.thunders_edge = true;
    store.save_player_lobby(&record).unwrap();
    let recovered = Arc::new(GameRegistry::new().with_store(store.clone()));
    let report = recovered.recover_all_games_report().expect("a report");
    assert!(report.failed.is_empty(), "{:?}", report.failed);
    let base = spawn(recovered).await;
    let client = reqwest::Client::new();
    for _ in 0..2 {
        let response = client
            .post(format!("{base}/api/games/{game}/lobby/bots"))
            .header("x-ti4-player-session", &token)
            .json(&json!({"kind": "smart"}))
            .send()
            .await
            .unwrap();
        assert!(response.status().is_success());
    }
    let response = client
        .post(format!("{base}/api/games/{game}/lobby/start"))
        .header("x-ti4-player-session", &token)
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert!(response.status().is_client_error(), "{}", response.status());
    let text = response.text().await.unwrap();
    assert!(text.contains("Thunder's Edge content") && text.contains("switched off"), "{text}");
    let _ = std::fs::remove_dir_all(dir);
}
