use std::sync::Arc;
use std::time::{Duration, Instant};

use ti4_content::ContentStore;
use ti4_model::id::PlayerId;
use ti4_server::session::batch::{BatchRequest, MovementPlan, MovementStep};
use ti4_server::session::registry::{HistoryAction, HistoryError};
use ti4_server::session::{GameRegistry, GameSession, SeatController, SessionConfig};
use ti4_server::storage::FileGameStore;

#[test]
fn movement_batch_is_atomic_idempotent_and_undoable() {
    let path = std::env::temp_dir().join(format!("ti4_batch_{:032x}", rand::random::<u128>()));
    let store = Arc::new(FileGameStore::new(&path).unwrap());
    let registry = GameRegistry::new().with_store(store.clone());
    let host = PlayerId::new("p1");
    let guest = PlayerId::new("p2");
    let players = vec![host.clone(), guest.clone()];
    let (state, galaxy) =
        ti4_server::map::create_game_with_map(ContentStore::embedded(), &players, 42).unwrap();
    let tiles = ti4_server::map::build_board_tiles(ContentStore::embedded(), &galaxy);
    let config = SessionConfig::new("batch_probe", state)
        .with_seed(42)
        .with_player_ids(players)
        .with_galaxy(galaxy, tiles)
        .with_seat(host.clone(), SeatController::Human)
        .with_seat(guest, SeatController::Human);
    let session = registry.create_game(config).unwrap();
    let token = session.seat_tokens()[&host].clone();
    for _ in 0..4 {
        let (seat, nonce, version, _) = pending(&session);
        let choice = session
            .get_snapshot(&ti4_server::protocol::status::ViewerRole::Player(
                seat.clone(),
            ))
            .pending_choice
            .unwrap()
            .choice;
        let option = choice
            .options
            .iter()
            .find(|o| o.id == "tactical" || o.id == "22")
            .unwrap_or(&choice.options[0]);
        session
            .submit_choice(&seat, &nonce, version, &option.id)
            .unwrap();
    }
    let (seat, nonce, version, _) = pending(&session);
    assert_eq!(seat, host);
    let choice = session
        .get_snapshot(&ti4_server::protocol::status::ViewerRole::Player(
            host.clone(),
        ))
        .pending_choice
        .unwrap()
        .choice;
    assert_eq!(choice.context.as_ref().unwrap().subtype, "movement_step");
    let destination = session
        .current_state()
        .active_system
        .unwrap()
        .as_str()
        .to_owned();
    let mut request = BatchRequest {
        request_id: "confirm_1".into(),
        expected_version: version,
        nonce,
        plan: MovementPlan {
            destination,
            steps: vec![MovementStep::DoneMoving],
        },
    };
    let original = session.decision_log();
    if let Some(option) = choice.options.iter().find(|option| option.kind == "move") {
        let origin = option.payload["origin"].as_str().unwrap().to_owned();
        let unit = option.payload["unit"].as_str().unwrap().to_owned();
        let damaged = option.payload["damaged"].as_bool().unwrap();
        let mut steps = vec![MovementStep::Move {
            origin,
            unit,
            damaged,
        }];
        if option.payload["capacity"].as_i64().unwrap_or(0) > 0 {
            steps.push(MovementStep::DoneLoading);
        }
        steps.push(MovementStep::DoneMoving);
        let mut probe = request.clone();
        probe.request_id = "move_probe".into();
        probe.plan.steps = steps;
        let result = registry.submit_batch("batch_probe", &token, probe);
        assert!(result.is_ok(), "ship movement batch: {result:?}");
        let moved = registry.get_game("batch_probe").unwrap();
        assert_eq!(
            moved.decision_log().len(),
            original.len()
                + if option.payload["capacity"].as_i64().unwrap_or(0) > 0 {
                    3
                } else {
                    2
                }
        );
        registry
            .change_history(
                "batch_probe",
                &token,
                moved.game_version(),
                HistoryAction::UndoBatch,
            )
            .unwrap();
        let restored = registry.get_game("batch_probe").unwrap();
        let (_, fresh_nonce, fresh_version, _) = pending(&restored);
        request.nonce = fresh_nonce;
        request.expected_version = fresh_version;
    }
    let mut invalid = request.clone();
    invalid.plan.steps.insert(
        0,
        MovementStep::Move {
            origin: "missing".into(),
            unit: "carrier".into(),
            damaged: false,
        },
    );
    assert_eq!(
        registry
            .submit_batch("batch_probe", &token, invalid)
            .unwrap_err()
            .failed_step,
        0
    );
    assert_eq!(session.decision_log(), original);
    let committed = registry
        .submit_batch("batch_probe", &token, request.clone())
        .unwrap();
    assert_eq!(committed.start_cursor, original.len());
    assert_eq!(committed.end_cursor, original.len() + 1);
    let duplicate = registry
        .submit_batch("batch_probe", &token, request)
        .unwrap();
    assert_eq!(committed.batch_id, duplicate.batch_id);
    let active = registry.get_game("batch_probe").unwrap();
    assert_eq!(active.decision_log().len(), original.len() + 1);
    let recovered = store.recover_session("batch_probe").unwrap();
    recovered.wait_replayed().unwrap();
    assert_eq!(recovered.decision_log(), active.decision_log());
    assert_eq!(recovered.batches().len(), 1);
    recovered.stop();
    registry
        .change_history(
            "batch_probe",
            &token,
            active.game_version(),
            HistoryAction::UndoBatch,
        )
        .unwrap();
    let undone = registry.get_game("batch_probe").unwrap();
    assert_eq!(undone.decision_log(), original);
    registry
        .change_history(
            "batch_probe",
            &token,
            undone.game_version(),
            HistoryAction::RedoBatch,
        )
        .unwrap();
    assert_eq!(
        registry
            .get_game("batch_probe")
            .unwrap()
            .decision_log()
            .len(),
        original.len() + 1
    );
    drop(registry);
    std::fs::remove_dir_all(path).unwrap();
}

fn pending(session: &GameSession) -> (PlayerId, String, u64, String) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some((seat, nonce, version)) = session.current_pending_decision() {
            let snapshot = session.get_snapshot(&ti4_server::protocol::status::ViewerRole::Player(
                seat.clone(),
            ));
            let option = snapshot.pending_choice.unwrap().choice.options[0]
                .id
                .clone();
            return (seat, nonce, version, option);
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for engine choice: {:?}",
            session.error()
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn undo_action_rewinds_the_whole_movement_pipeline_and_preserves_redo() {
    let registry = GameRegistry::new();
    let host = PlayerId::new("p1");
    let guest = PlayerId::new("p2");
    let players = vec![host.clone(), guest.clone()];
    let (state, galaxy) =
        ti4_server::map::create_game_with_map(ContentStore::embedded(), &players, 42).unwrap();
    let tiles = ti4_server::map::build_board_tiles(ContentStore::embedded(), &galaxy);
    let config = SessionConfig::new("probe", state)
        .with_player_ids(players)
        .with_galaxy(galaxy, tiles)
        .with_seat(host.clone(), SeatController::Human)
        .with_seat(guest, SeatController::Human);
    let session = registry.create_game(config).unwrap();
    let host_token = session.seat_tokens()[&host].clone();
    for i in 0..6 {
        let (seat, nonce, version, _) = pending(&session);
        let snapshot = session.get_snapshot(&ti4_server::protocol::status::ViewerRole::Player(
            seat.clone(),
        ));
        let choice = snapshot.pending_choice.unwrap().choice;
        if i == 4 || i == 5 {
            assert_eq!(choice.prompt, "movement");
        }
        let option = choice
            .options
            .iter()
            .find(|o| o.id == "tactical" || o.id == "22")
            .unwrap_or(&choice.options[0]);
        session
            .submit_choice(&seat, &nonce, version, &option.id)
            .unwrap();
    }
    let _ = pending(&session);
    let decisions = session.decision_log();
    assert_eq!(decisions[2].chosen, "tactical");
    assert_eq!(decisions[4].prompt, "movement");
    let restored = registry
        .change_history(
            "probe",
            &host_token,
            session.game_version(),
            HistoryAction::UndoPipeline,
        )
        .unwrap();
    assert_eq!(restored.history.cursor, 2);
    assert_eq!(restored.history.redo_count, 4);
    let replayed = registry.get_game("probe").unwrap();
    let _ = pending(&replayed);
    assert_eq!(replayed.decision_log(), decisions[..2]);
    registry
        .change_history(
            "probe",
            &host_token,
            replayed.game_version(),
            HistoryAction::Redo,
        )
        .unwrap();
    let redone = registry.get_game("probe").unwrap();
    let _ = pending(&redone);
    assert_eq!(redone.decision_log(), decisions[..3]);
    assert_eq!(redone.history_status().redo_count, 3);
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "exercises a complete durable branch lifecycle"
)]
fn host_rewinds_replays_and_branches_durably() {
    let path = std::env::temp_dir().join(format!("ti4_history_{:032x}", rand::random::<u128>()));
    let store = Arc::new(FileGameStore::new(&path).unwrap());
    let registry = GameRegistry::new().with_store(store.clone());
    let host = PlayerId::new("p1");
    let guest = PlayerId::new("p2");
    let players = vec![host.clone(), guest.clone()];
    let (state, galaxy) =
        ti4_server::map::create_game_with_map(ContentStore::embedded(), &players, 42).unwrap();
    let tiles = ti4_server::map::build_board_tiles(ContentStore::embedded(), &galaxy);
    let config = SessionConfig::new("history_game", state.clone())
        .with_seed(42)
        .with_player_ids(players)
        .with_galaxy(galaxy, tiles)
        .with_seat(host.clone(), SeatController::Human)
        .with_seat(guest.clone(), SeatController::Human);
    let session = registry.create_game(config).unwrap();
    let host_token = session.seat_tokens()[&host].clone();
    let guest_token = session.seat_tokens()[&guest].clone();
    let (seat, nonce, version, option) = pending(&session);
    session
        .submit_choice(&seat, &nonce, version, &option)
        .unwrap();
    let first = pending(&session);
    let after_first = session.current_state();
    session
        .submit_choice(&first.0, &first.1, first.2, &first.3)
        .unwrap();
    let _ = pending(&session);
    let second_log = session.decision_log();
    let second_hashes = session.decision_hashes();
    let snapshot = session.get_snapshot(&ti4_server::protocol::status::ViewerRole::Spectator);
    let first_event = snapshot
        .events
        .iter()
        .find(|event| event.decision_count == Some(1))
        .unwrap()
        .id
        .clone();
    assert!(matches!(
        registry.change_history(
            "history_game",
            &guest_token,
            session.game_version(),
            HistoryAction::Undo
        ),
        Err(HistoryError::Forbidden)
    ));
    assert!(matches!(
        registry.change_history("history_game", &host_token, version, HistoryAction::Undo),
        Err(HistoryError::Conflict(_))
    ));
    let restored = registry
        .change_history(
            "history_game",
            &host_token,
            session.game_version(),
            HistoryAction::Restore {
                event_id: first_event,
            },
        )
        .unwrap();
    let replayed = registry.get_game("history_game").unwrap();
    let _ = pending(&replayed);
    assert_eq!(restored.history.cursor, 1);
    assert_eq!(replayed.current_state(), after_first);
    assert_eq!(replayed.decision_log(), second_log[..1]);
    assert_eq!(replayed.history_status().redo_count, 1);
    let restarted_at_cursor = store.recover_session("history_game").unwrap();
    let _ = pending(&restarted_at_cursor);
    assert_eq!(restarted_at_cursor.history_status().cursor, 1);
    assert_eq!(restarted_at_cursor.history_status().redo_count, 1);
    assert_eq!(restarted_at_cursor.current_state(), after_first);
    restarted_at_cursor.stop();
    let history = store.load_history("history_game").unwrap().unwrap();
    let mut invalid_future = history.clone();
    invalid_future.redo[0].chosen = "not_an_offered_option".to_owned();
    store.save_history("history_game", &invalid_future).unwrap();
    assert!(
        store.recover_session("history_game").is_err(),
        "recovery must validate redo, not just the active prefix"
    );
    store.save_history("history_game", &history).unwrap();
    assert!(
        session
            .submit_choice(&first.0, &first.1, first.2, &first.3)
            .is_err()
    );
    let replayed_version = replayed.game_version();
    registry
        .change_history(
            "history_game",
            &host_token,
            replayed_version,
            HistoryAction::Redo,
        )
        .unwrap();
    let redone = registry.get_game("history_game").unwrap();
    let _ = pending(&redone);
    assert_eq!(redone.decision_hashes(), second_hashes);
    assert_eq!(redone.history_status().redo_count, 0);
    registry
        .change_history(
            "history_game",
            &host_token,
            redone.game_version(),
            HistoryAction::Undo,
        )
        .unwrap();
    let forked = registry.get_game("history_game").unwrap();
    let (seat, nonce, version, option) = pending(&forked);
    forked
        .submit_choice(&seat, &nonce, version, &option)
        .unwrap();
    let _ = pending(&forked);
    assert_eq!(forked.history_status().redo_count, 0);
    assert!(matches!(
        registry.change_history(
            "history_game",
            &host_token,
            forked.game_version(),
            HistoryAction::Redo
        ),
        Err(HistoryError::InvalidTarget)
    ));
    forked.stop();
    let recovered = store.recover_session("history_game").unwrap();
    let _ = pending(&recovered);
    assert_eq!(recovered.decision_log(), forked.decision_log());
    assert_eq!(recovered.history_status().redo_count, 0);
    recovered.stop();
    drop(registry);
    std::fs::remove_dir_all(path).unwrap();
}

#[tokio::test]
#[expect(
    clippy::too_many_lines,
    reason = "exercises HTTP authorization and socket replacement together"
)]
async fn http_history_requires_the_current_host_and_replaces_connected_sessions() {
    use futures_util::{SinkExt, StreamExt};
    let registry = Arc::new(GameRegistry::new());
    let host = PlayerId::new("host");
    let guest = PlayerId::new("guest");
    let (state, galaxy) = ti4_server::map::create_game_with_map(
        ContentStore::embedded(),
        &[host.clone(), guest.clone()],
        84,
    )
    .unwrap();
    let config = SessionConfig::new("history_http", state)
        .with_galaxy(
            galaxy.clone(),
            ti4_server::map::build_board_tiles(ContentStore::embedded(), &galaxy),
        )
        .with_player_ids(vec![host.clone(), guest.clone()])
        .with_seat(host.clone(), SeatController::Human)
        .with_seat(guest.clone(), SeatController::Human);
    let session = registry.create_game(config).unwrap();
    let host_token = session.seat_tokens()[&host].clone();
    let guest_token = session.seat_tokens()[&guest].clone();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let router = ti4_server::create_app(registry.clone());
    let server = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let (mut socket, _) =
        tokio_tungstenite::connect_async(format!("ws://{addr}/ws/games/history_http"))
            .await
            .unwrap();
    socket
        .send(tokio_tungstenite::tungstenite::Message::Text(
            serde_json::json!({
                "type": "subscribe", "protocol_version": 3, "game_id": "history_http",
            })
            .to_string()
            .into(),
        ))
        .await
        .unwrap();
    let first_message = tokio::time::timeout(Duration::from_secs(5), socket.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(
        first_message
            .to_text()
            .unwrap()
            .contains("initial_snapshot")
    );
    let (seat, nonce, version, option) = pending(&session);
    session
        .submit_choice(&seat, &nonce, version, &option)
        .unwrap();
    let _ = pending(&session);
    let client = reqwest::Client::new();
    let url = format!("http://{addr}/api/games/history_http/history");
    let payload = serde_json::json!({
        "expected_version": session.game_version(),
        "action": "undo_pipeline"
    });
    let guest_result = client
        .post(&url)
        .header("x-ti4-player-session", guest_token)
        .json(&payload)
        .send()
        .await
        .unwrap();
    assert_eq!(
        guest_result.status(),
        reqwest::StatusCode::FORBIDDEN,
        "{}",
        guest_result.text().await.unwrap_or_default()
    );
    let result = client
        .post(&url)
        .header("x-ti4-player-session", host_token)
        .json(&payload)
        .send()
        .await
        .unwrap();
    assert_eq!(
        result.status(),
        reqwest::StatusCode::OK,
        "{}",
        result.text().await.unwrap_or_default()
    );
    let replacement = registry.get_game("history_http").unwrap();
    let _ = pending(&replacement);
    assert_eq!(replacement.history_status().cursor, 0);
    let mut closed = false;
    for _ in 0..8 {
        match tokio::time::timeout(Duration::from_secs(1), socket.next()).await {
            Ok(None | Some(Ok(tokio_tungstenite::tungstenite::Message::Close(_))))
            | Ok(Some(Err(_))) => {
                closed = true;
                break;
            }
            _ => {}
        }
    }
    assert!(closed, "old connection must close on session replacement");
    replacement.stop();
    server.abort();
}
