//! WebSocket connection lifecycle and routing for live multiplayer sessions.

use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tracing::{debug, warn};

use ti4_model::id::PlayerId;

use crate::protocol::PROTOCOL_VERSION;
use crate::protocol::client::ClientMessage;
use crate::protocol::error::ErrorKind;
use crate::protocol::server::{ActionRejectedMsg, PongMsg, ProtocolErrorMsg, ServerMessage};
use crate::protocol::status::{RejectionReason, ViewerRole};
use crate::session::{GameRegistry, GameSession};

/// WebSocket upgrade handler for `GET /ws/games/{game_id}`.
pub async fn ws_handler(
    Path(game_id): Path<String>,
    ws: WebSocketUpgrade,
    State(registry): State<Arc<GameRegistry>>,
) -> Response {
    if let Some(session) = registry.get_game(&game_id) {
        ws.on_upgrade(move |socket| handle_socket(socket, game_id, session))
            .into_response()
    } else {
        (StatusCode::NOT_FOUND, format!("Game '{game_id}' not found")).into_response()
    }
}

/// Outbound queue bounded capacity to prevent slow consumers from buffering indefinitely.
const OUTBOUND_QUEUE_CAPACITY: usize = 128;

#[allow(clippy::too_many_lines)]
async fn handle_socket(socket: WebSocket, game_id: String, session: Arc<GameSession>) {
    let (mut ws_sender, mut ws_receiver) = socket.split();

    // Bounded outbound channel for messages destined for this client
    let (outbound_tx, mut outbound_rx) = mpsc::channel::<ServerMessage>(OUTBOUND_QUEUE_CAPACITY);

    // Outbound pump task: forwards ServerMessage as JSON text to WebSocket sink
    let outbound_task = tokio::spawn(async move {
        while let Some(msg) = outbound_rx.recv().await {
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

    // Inbound processing loop
    while let Some(msg_result) = ws_receiver.next().await {
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
            ClientMessage::Ping { sequence, .. } => {
                let _ = outbound_tx
                    .send(ServerMessage::Pong(PongMsg {
                        protocol_version: PROTOCOL_VERSION,
                        sequence,
                    }))
                    .await;
            }
            ClientMessage::Subscribe { seat_token, .. } => {
                let role = match seat_token {
                    Some(s) if s != "spectator" => ViewerRole::Player(PlayerId::new(s)),
                    _ => ViewerRole::Spectator,
                };
                current_role = Some(role.clone());

                // Subscribe to session updates
                let mpsc_rx = session.subscribe(role.clone());

                // Send immediate snapshot upon subscription
                let snapshot = session.get_snapshot(&role);
                let _ = outbound_tx
                    .send(ServerMessage::InitialSnapshot(snapshot))
                    .await;

                // Bridge session broadcast updates to tokio outbound queue
                let tx_clone = outbound_tx.clone();
                tokio::task::spawn_blocking(move || {
                    while let Ok(broadcast_msg) = mpsc_rx.recv() {
                        if tx_clone.blocking_send(broadcast_msg).is_err() {
                            break;
                        }
                    }
                });
            }
            ClientMessage::SubmitChoice {
                expected_version,
                nonce,
                option_id,
                ..
            } => match &current_role {
                Some(ViewerRole::Player(acting_seat)) => {
                    let res =
                        session.submit_choice(acting_seat, &nonce, expected_version, &option_id);
                    match res {
                        Ok(accepted) => {
                            let _ = outbound_tx
                                .send(ServerMessage::ActionAccepted(accepted))
                                .await;
                        }
                        Err(reason) => {
                            let _ = outbound_tx
                                .send(ServerMessage::ActionRejected(ActionRejectedMsg {
                                    protocol_version: PROTOCOL_VERSION,
                                    game_id: game_id.clone(),
                                    game_version: expected_version,
                                    reason,
                                }))
                                .await;
                        }
                    }
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
            },
        }
    }

    outbound_task.abort();
}
