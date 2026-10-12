//! WebSocket connection lifecycle and routing for live multiplayer sessions.

use std::sync::Arc;
use std::time::Duration;

use axum::extract::ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tracing::{debug, warn};

use crate::protocol::PROTOCOL_VERSION;
use crate::protocol::client::ClientMessage;
use crate::protocol::error::ErrorKind;
use crate::protocol::server::{
    ActionRejectedMsg, DraftAppliedMsg, DraftKind, DraftOutcomeMsg, PlanningRejection,
    PlanningResultMsg, PlanningUpdateMsg, PongMsg, ProtocolErrorMsg, ServerMessage,
};
use crate::protocol::status::{RejectionReason, ViewerRole};
use crate::session::{GameRegistry, GameSession};

/// WebSocket upgrade handler for `GET /ws/games/{game_id}`.
pub async fn ws_handler(
    Path(game_id): Path<String>,
    ws: WebSocketUpgrade,
    State(registry): State<Arc<GameRegistry>>,
) -> Response {
    if let Some(session) = registry.get_game(&game_id) {
        ws.max_frame_size(MAX_CLIENT_MESSAGE_BYTES)
            .max_message_size(MAX_CLIENT_MESSAGE_BYTES)
            .on_upgrade(move |socket| handle_socket(socket, game_id, session, registry))
            .into_response()
    } else {
        (StatusCode::NOT_FOUND, format!("Game '{game_id}' not found")).into_response()
    }
}

/// Outbound queue bounded capacity to prevent slow consumers from buffering indefinitely.
const OUTBOUND_QUEUE_CAPACITY: usize = 128;
/// Maximum complete client WebSocket message, including JSON framing. A script of a draft is
/// the largest (`RunDraft`, `ApplyDraft`); the local game gives its requests the same room.
pub const MAX_CLIENT_MESSAGE_BYTES: usize = 64 * 1024;

/// The newest draft request of a connection: its `request_id` and its script.
type DraftRequest = Option<(String, Vec<ti4_view::planning::draft::DraftStep>)>;

#[allow(clippy::too_many_lines)]
async fn handle_socket(
    socket: WebSocket,
    game_id: String,
    session: Arc<GameSession>,
    registry: Arc<GameRegistry>,
) {
    let (mut ws_sender, mut ws_receiver) = socket.split();

    // Bounded outbound channel for messages destined for this client
    let (outbound_tx, mut outbound_rx) = mpsc::channel::<ServerMessage>(OUTBOUND_QUEUE_CAPACITY);
    let (replacement_tx, mut replacement_rx) = tokio::sync::oneshot::channel::<()>();
    let mut replacement_tx = Some(replacement_tx);
    let outbound_registry = registry.clone();
    let outbound_game = game_id.clone();
    let outbound_credential = Arc::new(std::sync::Mutex::new(None::<String>));
    let credential_for_pump = outbound_credential.clone();
    let session_for_pump = session.clone();

    // Outbound pump task: forwards ServerMessage as JSON text to WebSocket sink
    let outbound_task = tokio::spawn(async move {
        loop {
            let msg = tokio::select! {
                msg = outbound_rx.recv() => match msg { Some(msg) => msg, None => break },
                result = &mut replacement_rx => {
                    if result.is_ok() {
                        let _ = ws_sender.send(Message::Close(Some(CloseFrame {
                            code: 4001,
                            reason: "session replaced".into(),
                        }))).await;
                    }
                    break;
                }
            };
            let credential = credential_for_pump.lock().expect("credential lock").clone();
            if credential.as_deref().is_some_and(|token| {
                outbound_registry
                    .authenticate_player_session(&outbound_game, token)
                    .is_err()
            }) {
                break;
            }
            if let ServerMessage::PlanningUpdate(update) = &msg {
                let Some(player) = credential.as_deref().and_then(|token| {
                    outbound_registry
                        .authenticate_player_session(&outbound_game, token)
                        .ok()
                }) else {
                    continue;
                };
                let current = match update.draft {
                    DraftKind::Tactical => session_for_pump
                        .planning_attempt_is_current(&player, update.envelope.identity),
                    DraftKind::Secondary => session_for_pump
                        .secondary_attempt_is_current(&player, update.envelope.identity),
                };
                if !current {
                    continue;
                }
            }
            match serde_json::to_string(&msg) {
                Ok(text) => {
                    if ws_sender.send(Message::Text(text.into())).await.is_err() {
                        break;
                    }
                }
                Err(err) => {
                    warn!("Failed to serialize ServerMessage: {err}");
                }
            }
        }
    });

    let mut current_role: Option<ViewerRole> = None;
    let mut current_token: Option<String> = None;
    let mut connection_id: Option<u64> = None;
    let mut auth_check = tokio::time::interval(Duration::from_millis(250));
    // The draft requests of this connection. One is run at a time and only the newest waits,
    // so a page that sends a script for each change cannot queue work.
    let mut drafts: Option<tokio::sync::watch::Sender<DraftRequest>> = None;

    // Inbound processing loop
    loop {
        let msg_result = tokio::select! {
            msg = ws_receiver.next() => match msg { Some(msg) => msg, None => break },
            _ = auth_check.tick() => {
                if session.error().is_some() {
                    break;
                }
                if registry.get_game(&game_id).is_none_or(|current| !Arc::ptr_eq(&current, &session)) {
                    if let Some(tx) = replacement_tx.take() {
                        let _ = tx.send(());
                    }
                    break;
                }
                if current_token.as_deref().is_some_and(|token| registry.authenticate_player_session(&game_id, token).is_err()) {
                    break;
                }
                continue;
            }
        };
        let ws_msg = match msg_result {
            Ok(msg) => msg,
            Err(err) => {
                debug!("WebSocket read error: {err}");
                break;
            }
        };

        let text = match ws_msg {
            Message::Text(t) => t.to_string(),
            Message::Binary(b) => {
                let Ok(s) = String::from_utf8(b.to_vec()) else {
                    let _ = outbound_tx
                        .send(ServerMessage::Error(ProtocolErrorMsg {
                            protocol_version: PROTOCOL_VERSION,
                            kind: ErrorKind::MalformedMessage,
                            message: "Invalid UTF-8 in binary payload".to_owned(),
                        }))
                        .await;
                    continue;
                };
                s
            }
            Message::Ping(_) | Message::Pong(_) => continue,
            Message::Close(_) => break,
        };

        let client_msg: ClientMessage = match serde_json::from_str(&text) {
            Ok(m) => m,
            Err(err) => {
                let _ = outbound_tx
                    .send(ServerMessage::Error(ProtocolErrorMsg {
                        protocol_version: PROTOCOL_VERSION,
                        kind: ErrorKind::MalformedMessage,
                        message: err.to_string(),
                    }))
                    .await;
                continue;
            }
        };

        if let Err(field) = client_msg.validate_bounds() {
            let _ = outbound_tx
                .send(ServerMessage::Error(ProtocolErrorMsg {
                    protocol_version: PROTOCOL_VERSION,
                    kind: ErrorKind::MalformedMessage,
                    message: format!("Invalid or oversized {field}"),
                }))
                .await;
            continue;
        }

        // Validate protocol version
        if client_msg.protocol_version() != PROTOCOL_VERSION {
            let _ = outbound_tx
                .send(ServerMessage::Error(ProtocolErrorMsg {
                    protocol_version: PROTOCOL_VERSION,
                    kind: ErrorKind::UnsupportedVersion,
                    message: format!(
                        "Unsupported protocol version {}. Expected {}",
                        client_msg.protocol_version(),
                        PROTOCOL_VERSION
                    ),
                }))
                .await;
            continue;
        }

        match client_msg {
            message @ (ClientMessage::StartPlanning { .. }
            | ClientMessage::ResetPlanning { .. }
            | ClientMessage::EditPlanningMovement { .. }
            | ClientMessage::ApplyPlanning { .. }
            | ClientMessage::SubmitPlanningChoice { .. }) => {
                let request_id = match &message {
                    ClientMessage::SubmitPlanningChoice { request_id, .. } => request_id.as_deref(),
                    _ => None,
                };
                let (message_game_id, answer) = match &message {
                    ClientMessage::StartPlanning { game_id, .. } => (game_id, None),
                    ClientMessage::ResetPlanning {
                        game_id, identity, ..
                    }
                    | ClientMessage::EditPlanningMovement {
                        game_id, identity, ..
                    } => (game_id, Some((*identity, ""))),
                    ClientMessage::ApplyPlanning {
                        game_id, identity, ..
                    } => (game_id, Some((*identity, ""))),
                    ClientMessage::SubmitPlanningChoice {
                        game_id,
                        identity,
                        option_id,
                        ..
                    } => (game_id, Some((*identity, option_id.as_str()))),
                    _ => unreachable!(),
                };
                let rejection = if message_game_id != &game_id {
                    Some(PlanningRejection::WrongGame)
                } else if let (Some(ViewerRole::Player(player)), Some(token)) =
                    (&current_role, &current_token)
                {
                    if let ClientMessage::ApplyPlanning {
                        identity,
                        nonce,
                        expected_version,
                        ..
                    } = &message
                    {
                        registry
                            .apply_player_planning(
                                &game_id,
                                token,
                                player,
                                &session,
                                *identity,
                                nonce,
                                *expected_version,
                            )
                            .err()
                    } else if let ClientMessage::EditPlanningMovement { identity, .. } = &message {
                        registry
                            .edit_player_planning_movement(
                                &game_id, token, player, &session, *identity,
                            )
                            .err()
                    } else if matches!(message, ClientMessage::ResetPlanning { .. }) {
                        registry
                            .reset_player_planning(
                                &game_id,
                                token,
                                player,
                                &session,
                                answer.expect("reset identity").0,
                            )
                            .err()
                    } else {
                        registry
                            .submit_player_planning_with_request_id(
                                &game_id, token, player, &session, answer, request_id,
                            )
                            .err()
                    }
                } else {
                    Some(PlanningRejection::Unauthorized)
                };
                let _ = outbound_tx
                    .send(ServerMessage::PlanningResult(PlanningResultMsg {
                        protocol_version: PROTOCOL_VERSION,
                        game_id: game_id.clone(),
                        draft: DraftKind::Tactical,
                        identity: answer.map(|(identity, _)| identity),
                        rejection,
                    }))
                    .await;
            }
            ClientMessage::SecondaryPlanning {
                game_id: message_game_id,
                request,
                ..
            } => {
                let rejection = if message_game_id != game_id {
                    Some(PlanningRejection::WrongGame)
                } else if let (Some(ViewerRole::Player(player)), Some(token)) =
                    (&current_role, &current_token)
                {
                    registry
                        .player_secondary_planning(&game_id, token, player, &session, &request)
                        .err()
                } else {
                    Some(PlanningRejection::Unauthorized)
                };
                let _ = outbound_tx
                    .send(ServerMessage::PlanningResult(PlanningResultMsg {
                        protocol_version: PROTOCOL_VERSION,
                        game_id: game_id.clone(),
                        draft: DraftKind::Secondary,
                        identity: request.identity(),
                        rejection,
                    }))
                    .await;
            }
            ClientMessage::RunDraft {
                game_id: message_game_id,
                request_id,
                script,
                ..
            } => {
                let (Some(ViewerRole::Player(player)), Some(token), true) =
                    (&current_role, &current_token, message_game_id == game_id)
                else {
                    let _ = outbound_tx
                        .send(ServerMessage::Error(ProtocolErrorMsg {
                            protocol_version: PROTOCOL_VERSION,
                            kind: ErrorKind::Unauthorized,
                            message: "only a seated player of this game can run a draft".to_owned(),
                        }))
                        .await;
                    continue;
                };
                let requests = drafts.get_or_insert_with(|| {
                    let (requests, mut newest) = tokio::sync::watch::channel::<DraftRequest>(None);
                    let registry = registry.clone();
                    let session = session.clone();
                    let game_id = game_id.clone();
                    let token = token.clone();
                    let player = player.clone();
                    let outbound_tx = outbound_tx.clone();
                    tokio::spawn(async move {
                        while newest.changed().await.is_ok() {
                            let Some((request_id, script)) = newest.borrow_and_update().clone()
                            else {
                                continue;
                            };
                            let run = {
                                let registry = registry.clone();
                                let session = session.clone();
                                let game_id = game_id.clone();
                                let token = token.clone();
                                let player = player.clone();
                                tokio::task::spawn_blocking(move || {
                                    registry.run_player_draft(
                                        &game_id, &token, &player, &session, &script,
                                    )
                                })
                            };
                            let Ok(Some((base_version, outcome))) = run.await else {
                                return;
                            };
                            let message = ServerMessage::DraftOutcome(DraftOutcomeMsg {
                                protocol_version: PROTOCOL_VERSION,
                                game_id: game_id.clone(),
                                request_id,
                                base_version,
                                outcome: Box::new(outcome),
                            });
                            if outbound_tx.send(message).await.is_err() {
                                return;
                            }
                        }
                    });
                    requests
                });
                let _ = requests.send(Some((request_id, script)));
            }
            ClientMessage::ApplyDraft {
                game_id: message_game_id,
                nonce,
                expected_version,
                script,
                ..
            } => {
                let (Some(ViewerRole::Player(player)), Some(token), true) =
                    (&current_role, &current_token, message_game_id == game_id)
                else {
                    let _ = outbound_tx
                        .send(ServerMessage::ActionRejected(ActionRejectedMsg {
                            protocol_version: PROTOCOL_VERSION,
                            game_id: game_id.clone(),
                            game_version: expected_version,
                            reason: RejectionReason::UnauthorizedSeat { seat: None },
                        }))
                        .await;
                    continue;
                };
                let registry = registry.clone();
                let session = session.clone();
                let game_id = game_id.clone();
                let token = token.clone();
                let player = player.clone();
                let outbound_tx = outbound_tx.clone();
                tokio::spawn(async move {
                    let applied = {
                        let game_id = game_id.clone();
                        let nonce = nonce.clone();
                        tokio::task::spawn_blocking(move || {
                            registry.apply_player_draft(
                                &game_id,
                                &token,
                                &player,
                                &session,
                                &nonce,
                                expected_version,
                                &script,
                            )
                        })
                    };
                    let message = match applied.await {
                        // The script is in the game: the seat is told where it ends.
                        Ok(Ok(None)) => return,
                        Ok(Ok(Some(stopped))) => ServerMessage::DraftApplied(DraftAppliedMsg {
                            protocol_version: PROTOCOL_VERSION,
                            game_id,
                            nonce,
                            stopped: Some(stopped),
                        }),
                        Ok(Err(reason)) => ServerMessage::ActionRejected(ActionRejectedMsg {
                            protocol_version: PROTOCOL_VERSION,
                            game_id,
                            game_version: expected_version,
                            reason,
                        }),
                        Err(error) => {
                            warn!(%game_id, %nonce, %error, "apply_draft task failed");
                            return;
                        }
                    };
                    let _ = outbound_tx.send(message).await;
                });
            }
            ClientMessage::Ping { sequence, .. } => {
                if let (Some(token), Some(connection)) = (&current_token, connection_id) {
                    if registry.ping_player(&game_id, token, connection).is_err() {
                        break;
                    }
                }
                let _ = outbound_tx
                    .send(ServerMessage::Pong(PongMsg {
                        protocol_version: PROTOCOL_VERSION,
                        sequence,
                    }))
                    .await;
            }
            ClientMessage::Subscribe {
                game_id: message_game_id,
                player_session,
                ..
            } => {
                if message_game_id != game_id {
                    let _ = outbound_tx
                        .send(ServerMessage::Error(ProtocolErrorMsg {
                            protocol_version: PROTOCOL_VERSION,
                            kind: ErrorKind::MalformedMessage,
                            message: "Subscribe game_id does not match the WebSocket path"
                                .to_owned(),
                        }))
                        .await;
                    continue;
                }
                if current_role.is_some() {
                    let _ = outbound_tx
                        .send(ServerMessage::Error(ProtocolErrorMsg {
                            protocol_version: PROTOCOL_VERSION,
                            kind: ErrorKind::MalformedMessage,
                            message: "A connection may subscribe to only one viewer role"
                                .to_owned(),
                        }))
                        .await;
                    continue;
                }
                let role = match player_session {
                    Some(token) => {
                        if let Ok((seat, connection)) = registry.connect_player(&game_id, &token) {
                            connection_id = Some(connection);
                            *outbound_credential.lock().expect("credential lock") =
                                Some(token.clone());
                            current_token = Some(token);
                            ViewerRole::Player(seat)
                        } else {
                            let _ = outbound_tx
                                .send(ServerMessage::Error(ProtocolErrorMsg {
                                    protocol_version: PROTOCOL_VERSION,
                                    kind: ErrorKind::Unauthorized,
                                    message: "Invalid player session".to_owned(),
                                }))
                                .await;
                            continue;
                        }
                    }
                    _ => ViewerRole::Spectator,
                };
                current_role = Some(role.clone());

                // Subscribe to session updates
                let subscription = session.subscribe(role.clone());

                // Send immediate snapshot upon subscription
                let snapshot = session.get_snapshot(&role);
                let _ = outbound_tx
                    .send(ServerMessage::InitialSnapshot(snapshot))
                    .await;

                // Bridge session broadcast updates to tokio outbound queue
                let tx_clone = outbound_tx.clone();
                let registry_for_updates = registry.clone();
                let game_for_updates = game_id.clone();
                let token_for_updates = current_token.clone();
                let session_for_updates = session.clone();
                tokio::spawn(async move {
                    let mut check = tokio::time::interval(Duration::from_millis(50));
                    let mut planning = None;
                    let mut secondary_planning = None;
                    let mut last_planning_status = None;
                    loop {
                        tokio::select! {
                            _ = check.tick() => {},
                            _ = tx_clone.closed() => return,
                        }
                        if let ViewerRole::Player(player) = &role {
                            // Publish retirement/availability before draining replacement offers.
                            // This also covers phases where no replacement worker exists.
                            let status = session_for_updates.planning_status(player);
                            if last_planning_status.as_ref() != Some(&status) {
                                if tx_clone
                                    .send(ServerMessage::PlanningStatus(status.clone()))
                                    .await
                                    .is_err()
                                {
                                    return;
                                }
                                last_planning_status = Some(status);
                            }
                            if planning.is_none() {
                                planning = session_for_updates.subscribe_planning(player);
                            }
                            if secondary_planning.is_none() {
                                secondary_planning =
                                    session_for_updates.subscribe_secondary_planning(player);
                            }
                            for (draft, subscription) in [
                                (DraftKind::Tactical, &mut planning),
                                (DraftKind::Secondary, &mut secondary_planning),
                            ] {
                                let Some(receiver) = subscription else {
                                    continue;
                                };
                                loop {
                                    match receiver.try_recv() {
                                        Ok(envelope) => {
                                            let current = match draft {
                                                DraftKind::Tactical => session_for_updates
                                                    .planning_attempt_is_current(
                                                        player,
                                                        envelope.identity,
                                                    ),
                                                DraftKind::Secondary => session_for_updates
                                                    .secondary_attempt_is_current(
                                                        player,
                                                        envelope.identity,
                                                    ),
                                            };
                                            if !current {
                                                continue;
                                            }
                                            if tx_clone
                                                .send(ServerMessage::PlanningUpdate(
                                                    PlanningUpdateMsg {
                                                        protocol_version: PROTOCOL_VERSION,
                                                        game_id: game_for_updates.clone(),
                                                        draft,
                                                        envelope,
                                                    },
                                                ))
                                                .await
                                                .is_err()
                                            {
                                                return;
                                            }
                                        }
                                        Err(std::sync::mpsc::TryRecvError::Empty) => break,
                                        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                                            *subscription = None;
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                        loop {
                            match subscription.try_recv() {
                                Ok(broadcast_msg) => {
                                    if token_for_updates.as_deref().is_some_and(|token| {
                                        registry_for_updates
                                            .authenticate_player_session(&game_for_updates, token)
                                            .is_err()
                                    }) || tx_clone.send(broadcast_msg).await.is_err()
                                    {
                                        return;
                                    }
                                }
                                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                                Err(std::sync::mpsc::TryRecvError::Disconnected) => return,
                            }
                        }
                    }
                });
            }
            ClientMessage::SetReactionMode {
                game_id: message_game_id,
                card,
                mode,
                ..
            } => {
                let refusal = if message_game_id != game_id {
                    Some((
                        ErrorKind::MalformedMessage,
                        "set_reaction_mode game_id does not match the WebSocket path".to_owned(),
                    ))
                } else {
                    match &current_role {
                        // The seat is the connection's own, never taken from the message.
                        Some(ViewerRole::Player(seat)) => session
                            .set_reaction_mode(seat, &card, mode)
                            .err()
                            .map(|message| (ErrorKind::MalformedMessage, message)),
                        Some(ViewerRole::Spectator) | None => Some((
                            ErrorKind::Unauthorized,
                            "only a seated player can change reaction modes".to_owned(),
                        )),
                    }
                };
                if let Some((kind, message)) = refusal {
                    let _ = outbound_tx
                        .send(ServerMessage::Error(ProtocolErrorMsg {
                            protocol_version: PROTOCOL_VERSION,
                            kind,
                            message,
                        }))
                        .await;
                }
            }
            ClientMessage::SubmitChoice {
                game_id: message_game_id,
                expected_version,
                nonce,
                option_id,
                ..
            } => {
                if message_game_id != game_id {
                    let _ = outbound_tx
                        .send(ServerMessage::ActionRejected(ActionRejectedMsg {
                            protocol_version: PROTOCOL_VERSION,
                            game_id: game_id.clone(),
                            game_version: expected_version,
                            reason: RejectionReason::NoPendingChoice,
                        }))
                        .await;
                    continue;
                }
                match &current_role {
                    Some(ViewerRole::Player(acting_seat)) => {
                        let Some(token) = current_token.as_deref() else {
                            let _ = outbound_tx
                                .send(ServerMessage::ActionRejected(ActionRejectedMsg {
                                    protocol_version: PROTOCOL_VERSION,
                                    game_id: game_id.clone(),
                                    game_version: expected_version,
                                    reason: RejectionReason::UnauthorizedSeat {
                                        seat: Some(acting_seat.clone()),
                                    },
                                }))
                                .await;
                            continue;
                        };
                        let registry = registry.clone();
                        let session = session.clone();
                        let game_id = game_id.clone();
                        let token = token.to_owned();
                        let acting_seat = acting_seat.clone();
                        let outbound_tx = outbound_tx.clone();
                        tokio::spawn(async move {
                            debug!(%game_id, %nonce, expected_version, %option_id, "submit_choice received");
                            let log_game_id = game_id.clone();
                            let log_nonce = nonce.clone();
                            let result = tokio::task::spawn_blocking(move || {
                                registry.submit_player_choice(
                                    &game_id,
                                    &token,
                                    &acting_seat,
                                    &session,
                                    &nonce,
                                    expected_version,
                                    &option_id,
                                )
                            })
                            .await;
                            let message = match result {
                                Ok(Ok(accepted)) => {
                                    debug!(game_id = %log_game_id, nonce = %log_nonce, "submit_choice accepted");
                                    ServerMessage::ActionAccepted(accepted)
                                }
                                Ok(Err(reason)) => {
                                    debug!(game_id = %log_game_id, nonce = %log_nonce, ?reason, "submit_choice rejected");
                                    ServerMessage::ActionRejected(ActionRejectedMsg {
                                        protocol_version: PROTOCOL_VERSION,
                                        game_id: message_game_id.clone(),
                                        game_version: expected_version,
                                        reason,
                                    })
                                }
                                Err(error) => {
                                    // The client gets no reply at all in this case.
                                    warn!(game_id = %log_game_id, nonce = %log_nonce, %error, "submit_choice task failed");
                                    return;
                                }
                            };
                            let _ = outbound_tx.send(message).await;
                        });
                    }
                    Some(ViewerRole::Spectator) | None => {
                        let _ = outbound_tx
                            .send(ServerMessage::ActionRejected(ActionRejectedMsg {
                                protocol_version: PROTOCOL_VERSION,
                                game_id: game_id.clone(),
                                game_version: expected_version,
                                reason: RejectionReason::UnauthorizedSeat { seat: None },
                            }))
                            .await;
                    }
                }
            }
        }
    }

    if let (Some(ViewerRole::Player(player)), Some(connection)) = (&current_role, connection_id) {
        registry.disconnect_player(&game_id, player, connection);
    }
    if replacement_tx.is_none() {
        let _ = outbound_task.await;
    } else {
        outbound_task.abort();
    }
}
