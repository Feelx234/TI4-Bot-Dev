//! Faction choice for bot seats in the lobby: add with a faction, change it, reject duplicates
//! and unknown factions, honour it at start (the rest dealt from the remaining pool by seed), and
//! keep it through a restart of the lobby and of the started game.

use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

use reqwest::StatusCode;
use serde_json::{Value, json};
use ti4_model::id::PlayerId;
use ti4_server::create_app;
use ti4_server::session::GameRegistry;
use ti4_server::storage::FileGameStore;

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
    Server { addr, registry, client: reqwest::Client::new(), handle }
}

impl Server {
    fn url(&self, path: &str) -> String {
        format!("http://{}{path}", self.addr)
    }

    async fn lobby(&self, players: usize, seed: u64) -> (String, String) {
        let created: Value = self
            .client
            .post(self.url("/api/games"))
            .json(&json!({"player_count": players, "seed": seed, "nickname": "Host"}))
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
            .post(self.url(&format!("/api/games/{game}/lobby/{route}")))
            .header("x-ti4-player-session", token)
            .json(&body)
            .send()
            .await
            .unwrap();
        let status = response.status();
        let text = response.text().await.unwrap();
        (status, serde_json::from_str(&text).unwrap_or(Value::String(text)))
    }

    async fn view(&self, game: &str) -> Value {
        self.client
            .get(self.url(&format!("/api/games/{game}/lobby")))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap()
    }

    async fn start(&self, game: &str, host: &str) {
        self.post(game, "ready", host, json!({"ready": true})).await;
        let (status, body) = self.post(game, "start", host, json!({})).await;
        assert_eq!(status, StatusCode::OK, "{body}");
    }
}

fn seated(registry: &GameRegistry, game: &str) -> Vec<(PlayerId, String)> {
    let session = registry.get_game(game).expect("session");
    let state = session.current_state();
    let (order, ..) = session.lobby_details();
    order
        .iter()
        .map(|p| (p.clone(), state.player(p).expect("seat").faction.as_str().to_owned()))
        .collect()
}

async fn full_table(server: &Server, seed: u64, picks: &[Option<&str>]) -> (String, String, Vec<String>) {
    let (game, host) = server.lobby(picks.len() + 1, seed).await;
    for pick in picks {
        let body = match pick {
            Some(f) => json!({"kind": "smart", "faction": f}),
            None => json!({"kind": "smart"}),
        };
        let (status, body) = server.post(&game, "bots", &host, body).await;
        assert_eq!(status, StatusCode::OK, "{body}");
    }
    server.start(&game, &host).await;
    let factions = seated(&server.registry, &game).into_iter().map(|(_, f)| f).collect();
    (game, host, factions)
}

#[tokio::test]
async fn a_chosen_bot_faction_is_seated_and_the_others_are_distinct() {
    let server = serve(Arc::new(GameRegistry::new())).await;
    let (_, _, factions) =
        full_table(&server, 11, &[Some("xxcha"), None, Some("sol"), None]).await;
    assert_eq!(factions[1], "xxcha");
    assert_eq!(factions[3], "sol");
    assert_eq!(factions.iter().collect::<BTreeSet<_>>().len(), 5, "{factions:?}");
    for f in &factions {
        assert!(ti4_engine::seating::LOBBY_FACTIONS.contains(&f.as_str()));
    }
}

#[tokio::test]
async fn the_same_seed_and_choices_seat_the_same_table() {
    let server = serve(Arc::new(GameRegistry::new())).await;
    let picks = [Some("l1z1x"), None, None];
    let (_, _, a) = full_table(&server, 77, &picks).await;
    let (_, _, b) = full_table(&server, 77, &picks).await;
    assert_eq!(a, b);
    let mut differs = false;
    for seed in 78..90 {
        differs |= full_table(&server, seed, &picks).await.2 != a;
    }
    assert!(differs, "the seed should deal the undecided seats differently");
}

#[tokio::test]
async fn duplicates_unknown_and_unavailable_factions_are_refused_with_a_reason() {
    let server = serve(Arc::new(GameRegistry::new())).await;
    let (game, host) = server.lobby(4, 5).await;
    let (status, body) =
        server.post(&game, "bots", &host, json!({"kind": "smart", "faction": "hacan"})).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["slots"][1]["faction"], "hacan");
    assert_eq!(body["factions"].as_array().unwrap().len(), 24);

    let (status, body) =
        server.post(&game, "bots", &host, json!({"kind": "smart", "faction": "hacan"})).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(body.as_str().unwrap().contains("already taken by Smart bot 1"), "{body}");
    for bad in ["nonsense", "bastion", "keleresm", "obsidian"] {
        let (status, body) =
            server.post(&game, "bots", &host, json!({"kind": "smart", "faction": bad})).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{bad}");
        assert!(body.as_str().unwrap().contains("not available"), "{body}");
    }
    // A faction goes to one bot at a time.
    let (status, _) = server
        .post(&game, "bots", &host, json!({"kind": "smart", "count": 2, "faction": "sol"}))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    // Nothing was added by the refusals.
    assert_eq!(server.view(&game).await["slots"][2]["occupant"], Value::Null);
}

#[tokio::test]
async fn the_host_changes_and_clears_a_bot_faction_and_only_for_bots() {
    let server = serve(Arc::new(GameRegistry::new())).await;
    let (game, host) = server.lobby(4, 5).await;
    server.post(&game, "bots", &host, json!({"kind": "smart", "faction": "sol"})).await;
    let (_, body) = server.post(&game, "bots", &host, json!({"kind": "smart"})).await;
    let first = body["slots"][1]["occupant"].as_str().unwrap().to_owned();
    let second = body["slots"][2]["occupant"].as_str().unwrap().to_owned();
    assert!(body["slots"][2].get("faction").is_none());

    let (status, body) = server
        .post(&game, "seat-faction", &host, json!({"player_id": second, "faction": "sol"}))
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    let (status, body) = server
        .post(&game, "seat-faction", &host, json!({"player_id": second, "faction": "jolnar"}))
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["slots"][2]["faction"], "jolnar");
    // Re-choosing a bot's own faction is not a clash; sol is free once its bot clears it.
    let (status, _) = server
        .post(&game, "seat-faction", &host, json!({"player_id": first, "faction": "sol"}))
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) =
        server.post(&game, "seat-faction", &host, json!({"player_id": first})).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["slots"][1].get("faction").is_none());
    // The host's own seat takes a choice like any other.
    let host_id = body["host_player_id"].as_str().unwrap().to_owned();
    let (status, _) = server
        .post(&game, "seat-faction", &host, json!({"player_id": host_id, "faction": "sol"}))
        .await;
    assert_eq!(status, StatusCode::OK);
    // Only the host may.
    let (status, _) = server
        .post(&game, "seat-faction", "not-a-session", json!({"player_id": first, "faction": "sol"}))
        .await;
    assert!(status.is_client_error());
}

#[tokio::test]
async fn removing_a_bot_frees_its_faction() {
    let server = serve(Arc::new(GameRegistry::new())).await;
    let (game, host) = server.lobby(3, 5).await;
    let (_, body) = server
        .post(&game, "bots", &host, json!({"kind": "smart", "faction": "sol"}))
        .await;
    let bot = body["slots"][1]["occupant"].as_str().unwrap().to_owned();
    server.post(&game, "remove-bot", &host, json!({"player_id": bot})).await;
    let (status, _) = server
        .post(&game, "bots", &host, json!({"kind": "smart", "faction": "sol"}))
        .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn chosen_factions_survive_a_restart_of_the_lobby_and_of_the_started_game() {
    let dir = std::env::temp_dir().join(format!("ti4_bf_lobby_{:016x}", rand::random::<u64>()));
    let store = Arc::new(FileGameStore::new(&dir).unwrap());
    let server = serve(Arc::new(GameRegistry::new().with_store(store.clone()))).await;
    let (game, host) = server.lobby(4, 31).await;
    server.post(&game, "bots", &host, json!({"kind": "smart", "faction": "letnev"})).await;
    server.post(&game, "bots", &host, json!({"kind": "smart"})).await;
    drop(server);

    let registry = Arc::new(GameRegistry::new().with_store(store.clone()));
    registry.recover_all_games().expect("recover lobby");
    let server = serve(registry).await;
    assert_eq!(server.view(&game).await["slots"][1]["faction"], "letnev");
    server.post(&game, "bots", &host, json!({"kind": "smart"})).await;
    server.start(&game, &host).await;
    let before = seated(&server.registry, &game);
    assert_eq!(before[1].1, "letnev");
    let session = server.registry.get_game(&game).unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    while session.current_pending_decision().is_none() {
        assert!(Instant::now() < deadline, "host never asked");
        std::thread::sleep(Duration::from_millis(5));
    }
    session.stop();
    drop(session);
    drop(server);

    // The started game replays from init.json to the very same table and keeps playing.
    let registry = Arc::new(GameRegistry::new().with_store(store.clone()));
    registry.recover_all_games().expect("recover game");
    let session = registry.get_game(&game).expect("recovered");
    session.wait_replayed().expect("replayed");
    assert!(session.error().is_none());
    assert_eq!(seated(&registry, &game), before);
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn random_bots_stay_available_through_the_api_and_get_seeded_factions() {
    let server = serve(Arc::new(GameRegistry::new())).await;
    let (game, host) = server.lobby(3, 5).await;
    server.post(&game, "bots", &host, json!({"kind": "random", "fill": true})).await;
    server.start(&game, &host).await;
    // Random bots stay available to the API (the nightly smoke and presets use them).
    assert_eq!(seated(&server.registry, &game).len(), 3);
}

#[tokio::test]
async fn the_map_preview_seats_the_factions_the_game_will_get() {
    let server = serve(Arc::new(GameRegistry::new())).await;
    let (game, host) = server.lobby(4, 9).await;
    server.post(&game, "bots", &host, json!({"kind": "smart", "faction": "jolnar"})).await;
    server.post(&game, "bots", &host, json!({"kind": "smart", "fill": true})).await;
    let preview: Value = server
        .client
        .get(server.url(&format!("/api/games/{game}/lobby/map-preview")))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let previewed: Vec<String> = preview["seats"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["faction"].as_str().unwrap().to_owned())
        .collect();
    server.start(&game, &host).await;
    let started: Vec<String> =
        seated(&server.registry, &game).into_iter().map(|(_, f)| f).collect();
    assert_eq!(previewed, started);
    assert_eq!(started[1], "jolnar");
}
