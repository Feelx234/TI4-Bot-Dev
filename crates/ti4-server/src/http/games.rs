//! HTTP endpoints for listing, creating, and inspecting game sessions.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use serde::{Deserialize, Serialize};

use ti4_model::id::PlayerId;

use crate::protocol::server::ServerMessage;
use crate::protocol::status::ViewerRole;
use crate::session::registry::{GameSummary, LobbyConfig, LobbyError, LobbyPhase, LobbyStatus};
use crate::session::{GameRegistry, SeatController};

const MAX_PLAYERS: usize = 8;
const MAX_PLAYER_ID_BYTES: usize = 64;

/// Request to create a new game session.
#[derive(Debug, Deserialize)]
pub struct CreateGameRequest {
    #[serde(default)]
    pub players: Vec<String>,
    pub seed: Option<u64>,
    #[serde(default)]
    pub bot_seats: Vec<String>,
}

/// Response after creating a game.
#[derive(Debug, Serialize)]
pub struct CreateGameResponse {
    pub game_id: String,
    pub creator_token: String,
    pub lobby: LobbyResponse,
}

/// Public state of the pre-game lobby or the resulting running session.
#[derive(Debug, Serialize)]
pub struct LobbyResponse {
    pub game_id: String,
    pub phase: LobbyPhase,
    pub lobby_version: u64,
    pub host_seat: String,
    pub roster: Vec<LobbySeatResponse>,
    pub viewer: Option<crate::protocol::status::ViewerRole>,
    pub can_start: bool,
}

/// Public roster entry. Capabilities are deliberately not represented here.
#[derive(Debug, Serialize)]
pub struct LobbySeatResponse {
    pub seat: String,
    pub controller: &'static str,
    pub ready: bool,
    pub available: bool,
}

#[derive(Debug, Deserialize)]
pub struct ReadyRequest {
    pub ready: bool,
}

#[derive(Debug, Deserialize)]
pub struct ClaimRequest {
    pub seat: String,
}

#[derive(Debug, Serialize)]
pub struct ClaimResponse {
    pub credential: String,
    pub lobby: LobbyResponse,
}

/// Handler for `GET /api/games`.
pub async fn list_games(State(registry): State<Arc<GameRegistry>>) -> Json<Vec<GameSummary>> {
    Json(registry.list_games())
}

/// Handler for `POST /api/games`.
pub async fn create_game(
    State(registry): State<Arc<GameRegistry>>,
    Json(payload): Json<CreateGameRequest>,
) -> Result<Json<CreateGameResponse>, (StatusCode, String)> {
    let player_names = if payload.players.is_empty() {
        vec!["p1".to_owned(), "p2".to_owned(), "p3".to_owned()]
    } else {
        payload.players
    };

    if !(2..=MAX_PLAYERS).contains(&player_names.len())
        || player_names
            .iter()
            .any(|player| player.is_empty() || player.len() > MAX_PLAYER_ID_BYTES)
        || player_names
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != player_names.len()
    {
        return Err((
            StatusCode::BAD_REQUEST,
            "players must contain 2-8 unique, non-empty 64-byte IDs".to_owned(),
        ));
    }
    if payload.bot_seats.iter().any(|seat| {
        seat.is_empty()
            || seat.len() > MAX_PLAYER_ID_BYTES
            || !player_names.iter().any(|player| player == seat)
    }) {
        return Err((
            StatusCode::BAD_REQUEST,
            "bot_seats must name configured players using bounded IDs".to_owned(),
        ));
    }
    if payload
        .bot_seats
        .iter()
        .collect::<std::collections::BTreeSet<_>>()
        .len()
        != payload.bot_seats.len()
    {
        return Err((
            StatusCode::BAD_REQUEST,
            "bot_seats must not contain duplicates".to_owned(),
        ));
    }
    if player_names.first().is_none_or(|seat| seat != "p1")
        || payload.bot_seats.iter().any(|seat| seat == "p1")
    {
        return Err((
            StatusCode::BAD_REQUEST,
            "p1 must be the first configured human creator seat".to_owned(),
        ));
    }

    let game_id = loop {
        let candidate = format!("game_{:032x}", rand::random::<u128>());
        if !registry.contains_game(&candidate) {
            break candidate;
        }
    };

    let player_ids: Vec<PlayerId> = player_names.iter().map(PlayerId::new).collect();
    let seed = payload.seed.unwrap_or_else(rand::random::<u64>);
    let seats = player_ids
        .iter()
        .map(|player_id| {
            let controller = if payload
                .bot_seats
                .iter()
                .any(|seat| seat == player_id.as_str())
            {
                SeatController::BotFirstOption
            } else {
                SeatController::Human
            };
            (player_id.clone(), controller)
        })
        .collect();
    let created = registry
        .create_lobby(LobbyConfig {
            game_id: game_id.clone(),
            host_seat: PlayerId::new("p1"),
            player_ids,
            seats,
            seed,
        })
        .map_err(|e| (StatusCode::CONFLICT, e))?;
    let creator_token = created.creator_token;
    let lobby = lobby_response(&created.lobby, Some(PlayerId::new("p1")));

    Ok(Json(CreateGameResponse {
        game_id,
        creator_token,
        lobby,
    }))
}

/// Handler for `POST /api/games/{game_id}/lobby/claim`.
pub async fn claim_seat(
    Path(game_id): Path<String>,
    State(registry): State<Arc<GameRegistry>>,
    Json(payload): Json<ClaimRequest>,
) -> Result<Json<ClaimResponse>, (StatusCode, String)> {
    let (status, credential) = registry
        .claim_seat(&game_id, &payload.seat)
        .map_err(lobby_error)?;
    Ok(Json(ClaimResponse {
        credential,
        lobby: lobby_status_response(status),
    }))
}

/// Handler for authenticated lease renewal.
pub async fn heartbeat(
    Path(game_id): Path<String>,
    headers: HeaderMap,
    State(registry): State<Arc<GameRegistry>>,
) -> Result<Json<LobbyResponse>, (StatusCode, String)> {
    let token = seat_token(&headers).ok_or_else(|| lobby_error(LobbyError::InvalidCapability))?;
    registry
        .authenticate_and_renew(&game_id, token)
        .map_err(lobby_error)?;
    let status = registry
        .lobby_status(&game_id, Some(token))
        .map_err(lobby_error)?;
    Ok(Json(lobby_status_response(status)))
}

/// Handler for `GET /api/games/{game_id}/lobby`.
pub async fn get_lobby(
    Path(game_id): Path<String>,
    headers: HeaderMap,
    State(registry): State<Arc<GameRegistry>>,
) -> Result<Json<LobbyResponse>, (StatusCode, String)> {
    let status = registry
        .lobby_status(&game_id, seat_token(&headers))
        .map_err(lobby_error)?;
    Ok(Json(lobby_status_response(status)))
}

/// Handler for `POST /api/games/{game_id}/lobby/ready`.
pub async fn set_ready(
    Path(game_id): Path<String>,
    headers: HeaderMap,
    State(registry): State<Arc<GameRegistry>>,
    Json(payload): Json<ReadyRequest>,
) -> Result<Json<LobbyResponse>, (StatusCode, String)> {
    let token = seat_token(&headers).ok_or_else(|| lobby_error(LobbyError::InvalidCapability))?;
    let status = registry
        .set_ready(&game_id, token, payload.ready)
        .map_err(lobby_error)?;
    Ok(Json(lobby_status_response(status)))
}

/// Handler for `POST /api/games/{game_id}/lobby/start`.
pub async fn start_lobby(
    Path(game_id): Path<String>,
    headers: HeaderMap,
    State(registry): State<Arc<GameRegistry>>,
) -> Result<Json<LobbyResponse>, (StatusCode, String)> {
    let token = seat_token(&headers).ok_or_else(|| lobby_error(LobbyError::InvalidCapability))?;
    registry.start_lobby(&game_id, token).map_err(lobby_error)?;
    let status = registry
        .lobby_status(&game_id, Some(token))
        .map_err(lobby_error)?;
    Ok(Json(lobby_status_response(status)))
}

fn seat_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get("x-ti4-seat-token")
        .and_then(|token| token.to_str().ok())
}

fn lobby_error(error: LobbyError) -> (StatusCode, String) {
    let status = match error {
        LobbyError::NotFound => StatusCode::NOT_FOUND,
        LobbyError::InvalidCapability
        | LobbyError::HumanSeatRequired
        | LobbyError::HostRequired => StatusCode::FORBIDDEN,
        LobbyError::HumansNotReady
        | LobbyError::AlreadyRunning
        | LobbyError::NotInLobby
        | LobbyError::SeatUnavailable => StatusCode::CONFLICT,
        LobbyError::Map(_) | LobbyError::Storage(_) => StatusCode::INTERNAL_SERVER_ERROR,
    };
    (status, error.message())
}

fn lobby_status_response(status: LobbyStatus) -> LobbyResponse {
    lobby_response(&status.lobby, status.viewer)
}

fn lobby_response(
    lobby: &crate::session::registry::LobbyState,
    viewer: Option<PlayerId>,
) -> LobbyResponse {
    let can_start = viewer.as_ref() == Some(&lobby.host_seat)
        && lobby.phase == LobbyPhase::Lobby
        && lobby
            .seats
            .values()
            .all(|seat| seat.controller != SeatController::Human || seat.ready);
    LobbyResponse {
        game_id: lobby.game_id.clone(),
        phase: lobby.phase,
        lobby_version: lobby.lobby_version,
        host_seat: lobby.host_seat.to_string(),
        roster: lobby
            .seats
            .iter()
            .map(|(seat, lobby_seat)| LobbySeatResponse {
                seat: seat.to_string(),
                controller: if lobby_seat.controller == SeatController::Human {
                    "human"
                } else {
                    "bot"
                },
                ready: lobby_seat.controller != SeatController::Human || lobby_seat.ready,
                available: lobby_seat.controller == SeatController::Human
                    && lobby_seat.seat_token.is_none(),
            })
            .collect(),
        viewer: viewer.map(ViewerRole::Player),
        can_start,
    }
}

/// Handler for `GET /api/games/{game_id}/map`.
pub async fn get_map(
    Path(game_id): Path<String>,
    State(registry): State<Arc<GameRegistry>>,
) -> Result<Json<Vec<crate::protocol::view::BoardTileView>>, (StatusCode, String)> {
    let session = registry
        .get_game(&game_id)
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("Game '{game_id}' not found")))?;

    Ok(Json(session.map_tiles()))
}

/// Handler for `GET /api/games/{game_id}/snapshot`.
pub async fn get_snapshot(
    Path(game_id): Path<String>,
    headers: HeaderMap,
    State(registry): State<Arc<GameRegistry>>,
) -> Result<Json<ServerMessage>, (StatusCode, String)> {
    let session = registry
        .get_game(&game_id)
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("Game '{game_id}' not found")))?;

    let viewer = match headers.get("x-ti4-seat-token") {
        Some(token) => ViewerRole::Player(
            registry
                .authenticate_and_renew(
                    &game_id,
                    token.to_str().map_err(|_| {
                        (
                            StatusCode::BAD_REQUEST,
                            "Invalid seat capability".to_owned(),
                        )
                    })?,
                )
                .map_err(lobby_error)?,
        ),
        None => ViewerRole::Spectator,
    };

    Ok(Json(ServerMessage::InitialSnapshot(
        session.get_snapshot(&viewer),
    )))
}
