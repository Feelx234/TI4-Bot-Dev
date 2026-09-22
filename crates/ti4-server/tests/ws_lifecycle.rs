//! Integration tests for HTTP endpoints and WebSocket lifecycle in `ti4-server`.

#![allow(
    clippy::too_many_lines,
    clippy::collapsible_if,
    clippy::needless_borrow
)]

use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

use ti4_server::create_app;
use ti4_server::protocol::PROTOCOL_VERSION;
use ti4_server::protocol::client::ClientMessage;
use ti4_server::protocol::server::ServerMessage;
use ti4_server::protocol::status::{RejectionReason, ViewerRole};
use ti4_server::session::GameRegistry;
use ti4_server::ws::MAX_CLIENT_MESSAGE_BYTES;

/// Spawns the test server on an ephemeral port and returns its base URL and registry.
async fn spawn_test_server() -> (String, Arc<GameRegistry>) {
    let registry = Arc::new(GameRegistry::new());
    let app = create_app(registry.clone());

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral port");
    let addr = listener.local_addr().expect("local addr");

    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve axum app");
    });

    (format!("127.0.0.1:{}", addr.port()), registry)
}

async fn wait_for_rejection<S>(stream: &mut S) -> RejectionReason
where
    S: StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    while let Some(Ok(msg)) = stream.next().await {
        if let Ok(text) = msg.to_text() {
            if let Ok(ServerMessage::ActionRejected(r)) =
                serde_json::from_str::<ServerMessage>(text)
            {
                return r.reason;
            }
        }
    }
    panic!("Stream ended without ActionRejected");
}

async fn wait_for_accepted<S>(stream: &mut S) -> ti4_server::protocol::server::ActionAcceptedMsg
where
    S: StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    while let Some(Ok(msg)) = stream.next().await {
        if let Ok(text) = msg.to_text() {
            if let Ok(ServerMessage::ActionAccepted(a)) =
                serde_json::from_str::<ServerMessage>(text)
            {
                return a;
            }
        }
    }
    panic!("Stream ended without ActionAccepted");
}

async fn wait_for_protocol_error<S>(stream: &mut S) -> ti4_server::protocol::error::ErrorKind
where
    S: StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    while let Some(Ok(msg)) = stream.next().await {
        if let Ok(text) = msg.to_text()
            && let Ok(ServerMessage::Error(error)) = serde_json::from_str::<ServerMessage>(text)
        {
            return error.kind;
        }
    }
    panic!("Stream ended without ProtocolError");
}

async fn wait_for_state_update<S>(stream: &mut S) -> ti4_server::protocol::server::StateUpdateMsg
where
    S: StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    while let Some(Ok(msg)) = stream.next().await {
        if let Ok(text) = msg.to_text() {
            if let Ok(ServerMessage::StateUpdate(u)) = serde_json::from_str::<ServerMessage>(text) {
                return u;
            }
        }
    }
    panic!("Stream ended without StateUpdate");
}

async fn ready_and_start(client: &reqwest::Client, addr: &str, game_id: &str, tokens: &[&str]) {
    for token in tokens {
        let response = client
            .post(format!("http://{addr}/api/games/{game_id}/lobby/ready"))
            .header("x-ti4-seat-token", *token)
            .json(&serde_json::json!({ "ready": true }))
            .send()
            .await
            .expect("ready seat");
        assert_eq!(response.status(), reqwest::StatusCode::OK);
    }
    let response = client
        .post(format!("http://{addr}/api/games/{game_id}/lobby/start"))
        .header("x-ti4-seat-token", tokens[0])
        .send()
        .await
        .expect("start lobby");
    assert_eq!(response.status(), reqwest::StatusCode::OK);
}

async fn create_game(
    client: &reqwest::Client,
    addr: &str,
    players: &[&str],
    bot_seats: &[&str],
    seed: u64,
) -> (String, String) {
    let response = client
        .post(format!("http://{addr}/api/games"))
        .json(&serde_json::json!({ "players": players, "bot_seats": bot_seats, "seed": seed }))
        .send()
        .await
        .expect("create game");
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let created: serde_json::Value = response.json().await.expect("created game body");
    (
        created["game_id"]
            .as_str()
            .expect("generated game ID")
            .to_owned(),
        created["creator_token"]
            .as_str()
            .expect("creator credential")
            .to_owned(),
    )
}

async fn claim_seat(client: &reqwest::Client, addr: &str, game_id: &str, seat: &str) -> String {
    let response = client
        .post(format!("http://{addr}/api/games/{game_id}/lobby/claim"))
        .json(&serde_json::json!({ "seat": seat }))
        .send()
        .await
        .expect("claim seat");
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    response
        .json::<serde_json::Value>()
        .await
        .expect("claim response")["credential"]
        .as_str()
        .expect("claimed credential")
        .to_owned()
}

#[tokio::test]
async fn http_health_and_games_crud() {
    let (addr, _) = spawn_test_server().await;
    let client = reqwest::Client::new();

    // 1. GET /health
    let res = client
        .get(format!("http://{addr}/health"))
        .send()
        .await
        .expect("get health");
    assert_eq!(res.status(), reqwest::StatusCode::OK);
    let body: serde_json::Value = res.json().await.expect("parse json");
    assert_eq!(body["status"], "ok");
    assert_eq!(body["protocol_version"], PROTOCOL_VERSION);

    // 2. POST /api/games -> create a game
    let (game_id, p1_token) = create_game(&client, &addr, &["p1", "p2", "p3"], &["p3"], 42).await;
    let p2_token = claim_seat(&client, &addr, &game_id, "p2").await;

    let res = client
        .post(format!("http://{addr}/api/games"))
        .json(&serde_json::json!({ "players": ["p2", "p1", "p3"] }))
        .send()
        .await
        .expect("post invalid game id");
    assert_eq!(res.status(), reqwest::StatusCode::BAD_REQUEST);

    let res = client
        .post(format!("http://{addr}/api/games"))
        .header("content-type", "application/json")
        .body(format!(
            "{{\"players\":[\"p1\",\"p2\"],\"padding\":\"{}\"}}",
            "x".repeat(9 * 1024)
        ))
        .send()
        .await
        .expect("post oversized request");
    assert_eq!(res.status(), reqwest::StatusCode::PAYLOAD_TOO_LARGE);

    // 3. GET /api/games -> list games
    let res = client
        .get(format!("http://{addr}/api/games"))
        .send()
        .await
        .expect("get games");
    assert_eq!(res.status(), reqwest::StatusCode::OK);
    let games: Vec<serde_json::Value> = res.json().await.expect("parse games list");
    assert!(games.iter().any(|g| g["game_id"] == game_id));
    ready_and_start(&client, &addr, &game_id, &[&p1_token, &p2_token]).await;

    // 4. GET /api/games/game_http_crud/snapshot with an unguessable seat capability.
    let res = client
        .get(format!("http://{addr}/api/games/{game_id}/snapshot"))
        .header("x-ti4-seat-token", &p1_token)
        .send()
        .await
        .expect("get snapshot p1");
    assert_eq!(res.status(), reqwest::StatusCode::OK);
    let snapshot: serde_json::Value = res.json().await.expect("parse snapshot");
    assert_eq!(snapshot["type"], "initial_snapshot");
    assert_eq!(snapshot["game_id"], game_id);
    assert_eq!(snapshot["viewer"]["role"], "player");

    let res = client
        .get(format!("http://{addr}/api/games/{game_id}/snapshot"))
        .header("x-ti4-seat-token", "p1")
        .send()
        .await
        .expect("get snapshot with forged capability");
    assert_eq!(res.status(), reqwest::StatusCode::FORBIDDEN);

    // 5. GET /api/games/game_http_crud/snapshot (spectator)
    let res = client
        .get(format!("http://{addr}/api/games/{game_id}/snapshot"))
        .send()
        .await
        .expect("get snapshot spectator");
    assert_eq!(res.status(), reqwest::StatusCode::OK);
    let snapshot: serde_json::Value = res.json().await.expect("parse spectator snapshot");
    assert_eq!(snapshot["type"], "initial_snapshot");
    assert_eq!(snapshot["viewer"]["role"], "spectator");
}

#[tokio::test]
async fn websocket_full_lifecycle_and_rejections() {
    let (addr, _) = spawn_test_server().await;
    let client = reqwest::Client::new();

    // Create a game first
    let (game_id, p1_token) = create_game(&client, &addr, &["p1", "p2", "p3"], &["p3"], 100).await;
    let p2_token = claim_seat(&client, &addr, &game_id, "p2").await;
    ready_and_start(&client, &addr, &game_id, &[&p1_token, &p2_token]).await;

    // Connect WebSocket
    let ws_url = format!("ws://{addr}/ws/games/{game_id}");
    let (mut ws_stream, _) = tokio_tungstenite::connect_async(&ws_url)
        .await
        .expect("connect ws");

    // 1. Test Ping / Pong
    let ping_msg = ClientMessage::Ping {
        protocol_version: PROTOCOL_VERSION,
        sequence: 777,
    };
    ws_stream
        .send(Message::Text(
            serde_json::to_string(&ping_msg).unwrap().into(),
        ))
        .await
        .expect("send ping");

    let reply = ws_stream
        .next()
        .await
        .expect("receive pong")
        .expect("ws ok");
    let pong: ServerMessage = serde_json::from_str(&reply.to_text().unwrap()).unwrap();
    assert_eq!(
        pong,
        ServerMessage::Pong(ti4_server::protocol::server::PongMsg {
            protocol_version: PROTOCOL_VERSION,
            sequence: 777,
        })
    );

    // 2. Subscribe as Player p1
    let sub_msg = ClientMessage::Subscribe {
        protocol_version: PROTOCOL_VERSION,
        game_id: game_id.clone(),
        seat_token: Some(p1_token),
    };
    ws_stream
        .send(Message::Text(
            serde_json::to_string(&sub_msg).unwrap().into(),
        ))
        .await
        .expect("send subscribe");

    // Receive InitialSnapshot
    let reply = ws_stream
        .next()
        .await
        .expect("receive snapshot")
        .expect("ws ok");
    let snapshot_msg: ServerMessage = serde_json::from_str(&reply.to_text().unwrap()).unwrap();
    let initial_snapshot = match snapshot_msg {
        ServerMessage::InitialSnapshot(s) => s,
        other => panic!("Expected InitialSnapshot, got {other:?}"),
    };
    assert_eq!(
        initial_snapshot.viewer,
        ViewerRole::Player(ti4_model::id::PlayerId::new("p1"))
    );

    // A connection is bound to its first authorized viewer and cannot collect another seat feed.
    let second_subscribe = ClientMessage::Subscribe {
        protocol_version: PROTOCOL_VERSION,
        game_id: game_id.clone(),
        seat_token: Some(p2_token),
    };
    ws_stream
        .send(Message::Text(
            serde_json::to_string(&second_subscribe).unwrap().into(),
        ))
        .await
        .expect("send second subscribe");
    assert_eq!(
        wait_for_protocol_error(&mut ws_stream).await,
        ti4_server::protocol::error::ErrorKind::MalformedMessage
    );

    // Receive pending choice if one is pending, or read it from initial snapshot
    let (nonce, expected_version, option_id) = if let Some(choice) = initial_snapshot.pending_choice
    {
        (
            choice.nonce,
            initial_snapshot.game_version,
            choice.options[0].id.clone(),
        )
    } else {
        // Wait for choice broadcast
        let reply = ws_stream
            .next()
            .await
            .expect("receive choice")
            .expect("ws ok");
        let msg: ServerMessage = serde_json::from_str(&reply.to_text().unwrap()).unwrap();
        match msg {
            ServerMessage::PendingChoice(p) => (
                p.choice.nonce,
                p.game_version,
                p.choice.options[0].id.clone(),
            ),
            other => panic!("Expected PendingChoice, got {other:?}"),
        }
    };

    // 3. The protocol game ID is bound to the WebSocket path.
    let wrong_game_msg = ClientMessage::SubmitChoice {
        protocol_version: PROTOCOL_VERSION,
        game_id: "another_game".to_owned(),
        nonce: nonce.clone(),
        expected_version,
        option_id: option_id.clone(),
    };
    ws_stream
        .send(Message::Text(
            serde_json::to_string(&wrong_game_msg).unwrap().into(),
        ))
        .await
        .expect("send mismatched game id");
    assert_eq!(
        wait_for_rejection(&mut ws_stream).await,
        RejectionReason::NoPendingChoice
    );

    // 4. Stale nonce rejection over WS
    let bad_nonce_msg = ClientMessage::SubmitChoice {
        protocol_version: PROTOCOL_VERSION,
        game_id: game_id.clone(),
        nonce: "bad_nonce_123".to_owned(),
        expected_version,
        option_id: option_id.clone(),
    };
    ws_stream
        .send(Message::Text(
            serde_json::to_string(&bad_nonce_msg).unwrap().into(),
        ))
        .await
        .expect("send bad nonce");

    let reason = wait_for_rejection(&mut ws_stream).await;
    assert_eq!(reason, RejectionReason::StaleNonce);

    // 5. Stale version rejection over WS
    let bad_ver_msg = ClientMessage::SubmitChoice {
        protocol_version: PROTOCOL_VERSION,
        game_id: game_id.clone(),
        nonce: nonce.clone(),
        expected_version: expected_version + 999,
        option_id: option_id.clone(),
    };
    ws_stream
        .send(Message::Text(
            serde_json::to_string(&bad_ver_msg).unwrap().into(),
        ))
        .await
        .expect("send bad version");

    let reason = wait_for_rejection(&mut ws_stream).await;
    assert_eq!(
        reason,
        RejectionReason::StaleVersion {
            expected: expected_version + 999,
            current: expected_version,
        }
    );

    // 6. Valid submission over WS
    let valid_msg = ClientMessage::SubmitChoice {
        protocol_version: PROTOCOL_VERSION,
        game_id: game_id.clone(),
        nonce,
        expected_version,
        option_id,
    };
    ws_stream
        .send(Message::Text(
            serde_json::to_string(&valid_msg).unwrap().into(),
        ))
        .await
        .expect("send valid choice");

    let accepted = wait_for_accepted(&mut ws_stream).await;
    assert_eq!(accepted.game_id, game_id);

    // Following action acceptance, server emits state update
    let update = wait_for_state_update(&mut ws_stream).await;
    assert_eq!(update.game_id, game_id);
    assert!(update.game_version > expected_version);
}

#[tokio::test]
async fn spectator_role_isolation_and_reconnection() {
    let (addr, _) = spawn_test_server().await;
    let client = reqwest::Client::new();

    let (game_id, p1_token) =
        create_game(&client, &addr, &["p1", "p2", "p3"], &["p2", "p3"], 200).await;
    ready_and_start(&client, &addr, &game_id, &[&p1_token]).await;

    // 1. Connect Spectator
    let ws_url = format!("ws://{addr}/ws/games/{game_id}");
    let (mut spec_stream, _) = tokio_tungstenite::connect_async(&ws_url)
        .await
        .expect("connect spectator");

    let sub_spec = ClientMessage::Subscribe {
        protocol_version: PROTOCOL_VERSION,
        game_id: game_id.clone(),
        seat_token: None, // Spectator
    };
    spec_stream
        .send(Message::Text(
            serde_json::to_string(&sub_spec).unwrap().into(),
        ))
        .await
        .expect("send spectator subscribe");

    let reply = spec_stream
        .next()
        .await
        .expect("receive snapshot")
        .expect("ws ok");
    let msg: ServerMessage = serde_json::from_str(&reply.to_text().unwrap()).unwrap();
    let spec_snapshot = match msg {
        ServerMessage::InitialSnapshot(s) => s,
        other => panic!("Expected InitialSnapshot, got {other:?}"),
    };

    assert_eq!(spec_snapshot.viewer, ViewerRole::Spectator);
    assert_eq!(
        spec_snapshot.pending_choice, None,
        "Spectator must never see pending choice details"
    );
    for player in &spec_snapshot.view.players {
        assert!(
            player.held_action_cards.is_empty(),
            "Spectator must never see private cards"
        );
        assert!(
            player.held_secret_objectives.is_empty(),
            "Spectator must never see secret objectives"
        );
    }

    // 2. Spectator submitting a choice is rejected as UnauthorizedSeat
    let illegal_choice = ClientMessage::SubmitChoice {
        protocol_version: PROTOCOL_VERSION,
        game_id: game_id.clone(),
        nonce: "some_nonce".to_owned(),
        expected_version: spec_snapshot.game_version,
        option_id: "any_opt".to_owned(),
    };
    spec_stream
        .send(Message::Text(
            serde_json::to_string(&illegal_choice).unwrap().into(),
        ))
        .await
        .expect("send illegal choice");

    let reason = wait_for_rejection(&mut spec_stream).await;
    assert_eq!(reason, RejectionReason::UnauthorizedSeat { seat: None });

    // 3. Close spectator connection and reconnect
    drop(spec_stream);
    tokio::time::sleep(Duration::from_millis(50)).await;

    let (mut reconnected_stream, _) = tokio_tungstenite::connect_async(&ws_url)
        .await
        .expect("reconnect ws");

    let sub_reconnect = ClientMessage::Subscribe {
        protocol_version: PROTOCOL_VERSION,
        game_id: game_id.clone(),
        seat_token: Some(p1_token),
    };
    reconnected_stream
        .send(Message::Text(
            serde_json::to_string(&sub_reconnect).unwrap().into(),
        ))
        .await
        .expect("subscribe as p1 on reconnect");

    let reply = reconnected_stream
        .next()
        .await
        .expect("receive snapshot")
        .expect("ws ok");
    let recon_msg: ServerMessage = serde_json::from_str(&reply.to_text().unwrap()).unwrap();
    match recon_msg {
        ServerMessage::InitialSnapshot(s) => {
            assert_eq!(
                s.viewer,
                ViewerRole::Player(ti4_model::id::PlayerId::new("p1"))
            );
        }
        other => panic!("Expected InitialSnapshot, got {other:?}"),
    }
}

#[tokio::test]
async fn websocket_rejects_oversized_messages_before_deserialization() {
    let (addr, _) = spawn_test_server().await;
    let client = reqwest::Client::new();
    let (game_id, p1_token) =
        create_game(&client, &addr, &["p1", "p2", "p3"], &["p2", "p3"], 1).await;
    ready_and_start(&client, &addr, &game_id, &[&p1_token]).await;

    let (mut stream, _) =
        tokio_tungstenite::connect_async(format!("ws://{addr}/ws/games/{game_id}"))
            .await
            .expect("connect ws");
    stream
        .send(Message::Text(
            "x".repeat(MAX_CLIENT_MESSAGE_BYTES + 1).into(),
        ))
        .await
        .expect("send oversized frame");

    let result = tokio::time::timeout(Duration::from_secs(1), stream.next())
        .await
        .expect("server must close an oversized frame");
    assert!(
        !matches!(result, Some(Ok(Message::Text(_)))),
        "oversized payload must not reach JSON deserialization"
    );
}
