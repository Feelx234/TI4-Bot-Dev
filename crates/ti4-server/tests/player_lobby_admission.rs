use std::sync::{Arc, Barrier};

use ti4_server::session::GameRegistry;
use ti4_server::session::registry::LobbyError;
use ti4_server::storage::FileGameStore;

#[test]
fn concurrent_joins_resume_and_leave_preserve_stable_identities() {
    let registry = Arc::new(GameRegistry::new());
    let (initial, host, credential) = registry
        .create_player_lobby("players".into(), 3, 42)
        .unwrap();
    assert_eq!(initial.slots[0].occupant.as_ref(), Some(&host));
    assert!(initial.slots[1].occupant.is_none());
    let barrier = Arc::new(Barrier::new(3));
    let joins: Vec<_> = (0..2)
        .map(|_| {
            let registry = registry.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                registry.join_player_lobby("players", None).unwrap()
            })
        })
        .collect();
    barrier.wait();
    let joined: Vec<_> = joins
        .into_iter()
        .map(|thread| thread.join().unwrap())
        .collect();
    assert_ne!(joined[0].1, joined[1].1);
    let (view, _) = registry.player_lobby_status("players", None).unwrap();
    assert_eq!(
        view.slots[1].occupant.as_ref().unwrap(),
        &joined
            .iter()
            .find(|entry| entry.0.lobby_version == 2)
            .unwrap()
            .1
    );
    assert_eq!(
        view.slots[2].occupant.as_ref().unwrap(),
        &joined
            .iter()
            .find(|entry| entry.0.lobby_version == 3)
            .unwrap()
            .1
    );
    assert!(matches!(
        registry.join_player_lobby("players", None),
        Err(LobbyError::SeatUnavailable)
    ));
    let token = joined[0].2.as_ref().unwrap().as_str();
    let (same, player, new_token) = registry.join_player_lobby("players", Some(token)).unwrap();
    assert_eq!(player, joined[0].1);
    assert!(new_token.is_none());
    assert_eq!(same.lobby_version, 3);
    assert!(matches!(
        registry.join_player_lobby("players", Some("invalid")),
        Err(LobbyError::InvalidCapability)
    ));
    assert!(matches!(
        registry.leave_player_lobby("players", credential.as_str()),
        Err(LobbyError::HostRequired)
    ));
    let left = registry.leave_player_lobby("players", token).unwrap();
    assert_eq!(left.lobby_version, 4);
    assert!(matches!(
        registry.join_player_lobby("players", Some(token)),
        Err(LobbyError::InvalidCapability)
    ));
    let (_, replacement, _) = registry.join_player_lobby("players", None).unwrap();
    assert_ne!(replacement, joined[0].1);
}

#[test]
fn readiness_start_and_recovery_preserve_current_credentials() {
    let dir = std::env::temp_dir().join(format!("ti4_pil02_{:032x}", rand::random::<u128>()));
    let store = Arc::new(FileGameStore::new(&dir).unwrap());
    let registry = GameRegistry::new().with_store(store.clone());
    let (_, host, token) = registry
        .create_player_lobby("durable".into(), 2, 19)
        .unwrap();
    let (_, other, other_token) = registry.join_player_lobby("durable", None).unwrap();
    let other_token = other_token.unwrap();
    assert!(matches!(
        registry.start_player_lobby("durable", token.as_str()),
        Err(LobbyError::HumansNotReady)
    ));
    assert!(matches!(
        registry.set_player_ready("durable", "invalid", true),
        Err(LobbyError::InvalidCapability)
    ));
    assert!(matches!(
        registry.start_player_lobby("durable", other_token.as_str()),
        Err(LobbyError::HostRequired)
    ));
    let restarted = GameRegistry::new().with_store(store.clone());
    assert_eq!(restarted.recover_all_games().unwrap(), vec!["durable"]);
    assert_eq!(
        restarted
            .join_player_lobby("durable", Some(token.as_str()))
            .unwrap()
            .1,
        host
    );
    restarted
        .set_player_ready("durable", token.as_str(), true)
        .unwrap();
    restarted
        .set_player_ready("durable", other_token.as_str(), true)
        .unwrap();
    assert_eq!(
        restarted
            .start_player_lobby("durable", token.as_str())
            .unwrap()
            .phase,
        ti4_server::session::registry::LobbyPhase::Running
    );
    assert!(matches!(
        restarted.join_player_lobby("durable", None),
        Err(LobbyError::AlreadyRunning)
    ));
    assert_eq!(
        restarted
            .join_player_lobby("durable", Some(other_token.as_str()))
            .unwrap()
            .1,
        other
    );
    assert_eq!(
        store.load_player_init("durable").unwrap().player_ids,
        vec![host, other]
    );
    let post_start = GameRegistry::new().with_store(store.clone());
    post_start.recover_all_games().unwrap();
    assert_eq!(
        post_start
            .join_player_lobby("durable", Some(token.as_str()))
            .unwrap()
            .2,
        None
    );
    post_start.remove_game("durable");
    restarted.remove_game("durable");
    drop(post_start);
    drop(restarted);
    drop(registry);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn failed_persistence_rolls_back_admission_readiness_and_leave() {
    let dir =
        std::env::temp_dir().join(format!("ti4_pil02_failure_{:032x}", rand::random::<u128>()));
    let store = Arc::new(FileGameStore::new(&dir).unwrap());
    let registry = GameRegistry::new().with_store(store.clone());
    let (_, host, host_token) = registry
        .create_player_lobby("failure".into(), 3, 9)
        .unwrap();
    let (_, other, other_token) = registry.join_player_lobby("failure", None).unwrap();
    let other_token = other_token.unwrap();
    let before = registry.player_lobby_status("failure", None).unwrap().0;
    let temporary = store.game_dir("failure").unwrap().join("lobby.tmp");
    std::fs::create_dir(&temporary).unwrap();
    assert!(matches!(
        registry.join_player_lobby("failure", None),
        Err(LobbyError::Storage(_))
    ));
    assert!(matches!(
        registry.set_player_ready("failure", host_token.as_str(), true),
        Err(LobbyError::Storage(_))
    ));
    assert!(matches!(
        registry.leave_player_lobby("failure", other_token.as_str()),
        Err(LobbyError::Storage(_))
    ));
    let after = registry.player_lobby_status("failure", None).unwrap().0;
    assert_eq!(after.lobby_version, before.lobby_version);
    assert_eq!(after.slots[0].occupant.as_ref(), Some(&host));
    assert_eq!(after.slots[1].occupant.as_ref(), Some(&other));
    assert!(after.slots[2].occupant.is_none());
    assert!(!after.slots[0].ready);
    let recovered = GameRegistry::new().with_store(store.clone());
    recovered.recover_all_games().unwrap();
    assert_eq!(
        recovered
            .player_lobby_status("failure", None)
            .unwrap()
            .0
            .lobby_version,
        before.lobby_version
    );
    std::fs::remove_dir(&temporary).unwrap();
    registry.join_player_lobby("failure", None).unwrap();
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn http_create_join_spectate_and_takeover_placeholder() {
    let registry = Arc::new(GameRegistry::new());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server =
        tokio::spawn(axum::serve(listener, ti4_server::create_app(registry)).into_future());
    let client = reqwest::Client::new();
    let base = format!("http://{addr}/api/games");
    for (body, status) in [
        (
            serde_json::json!({"player_count": 0}),
            reqwest::StatusCode::BAD_REQUEST,
        ),
        (
            serde_json::json!({"player_count": 2, "bot_seats": ["p2"]}),
            reqwest::StatusCode::UNPROCESSABLE_ENTITY,
        ),
    ] {
        assert_eq!(
            client
                .post(&base)
                .json(&body)
                .send()
                .await
                .unwrap()
                .status(),
            status
        );
    }
    let created: serde_json::Value = client
        .post(&base)
        .json(&serde_json::json!({"player_count": 2, "seed": 42}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let game = created["game_id"].as_str().unwrap();
    let credential = created["player_session"].as_str().unwrap();
    assert!(!created["lobby"].to_string().contains(credential));
    let url = format!("{base}/{game}/lobby");
    let before: serde_json::Value = client.get(&url).send().await.unwrap().json().await.unwrap();
    assert_eq!(before["lobby_version"], 1);
    let joined: serde_json::Value = client
        .post(format!("{url}/join"))
        .json(&serde_json::json!({"kind": "new"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        joined["lobby"]["slots"][1]["occupant"],
        joined["player"]["id"]
    );
    let resumed: serde_json::Value = client
        .post(format!("{url}/join"))
        .header("x-ti4-player-session", credential)
        .json(&serde_json::json!({"kind": "new"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(resumed["player"]["id"], created["player"]["id"]);
    assert!(resumed.get("player_session").is_none());
    assert_eq!(
        client
            .post(format!("{url}/join"))
            .json(&serde_json::json!({"kind": "takeover", "player_id": joined["player"]["id"]}))
            .send()
            .await
            .unwrap()
            .status(),
        reqwest::StatusCode::CONFLICT
    );
    assert_eq!(
        client
            .post(format!("{url}/join"))
            .json(&serde_json::json!({"kind": "new"}))
            .send()
            .await
            .unwrap()
            .status(),
        reqwest::StatusCode::CONFLICT
    );
    let after: serde_json::Value = client.get(&url).send().await.unwrap().json().await.unwrap();
    assert_eq!(after["lobby_version"], 2);
    assert!(!after.to_string().contains(credential));
    let other_token = joined["player_session"].as_str().unwrap();
    assert_eq!(
        client
            .get(&url)
            .header("x-ti4-player-session", "invalid")
            .send()
            .await
            .unwrap()
            .status(),
        reqwest::StatusCode::FORBIDDEN
    );
    assert_eq!(
        client
            .post(format!("{url}/leave"))
            .header("x-ti4-player-session", credential)
            .send()
            .await
            .unwrap()
            .status(),
        reqwest::StatusCode::FORBIDDEN
    );
    assert_eq!(
        client
            .post(format!("{url}/leave"))
            .header("x-ti4-player-session", other_token)
            .send()
            .await
            .unwrap()
            .status(),
        reqwest::StatusCode::OK
    );
    assert_eq!(
        client
            .post(format!("{url}/join"))
            .header("x-ti4-player-session", other_token)
            .json(&serde_json::json!({"kind":"new"}))
            .send()
            .await
            .unwrap()
            .status(),
        reqwest::StatusCode::FORBIDDEN
    );
    let replacement: serde_json::Value = client
        .post(format!("{url}/join"))
        .json(&serde_json::json!({"kind":"new"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_ne!(replacement["player"]["id"], joined["player"]["id"]);
    server.abort();
}
