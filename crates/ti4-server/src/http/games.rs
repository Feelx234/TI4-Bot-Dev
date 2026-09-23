//! HTTP endpoints for listing, creating, and inspecting game sessions.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use serde::{Deserialize, Serialize};

use ti4_model::id::PlayerId;

use crate::protocol::server::ServerMessage;
use crate::protocol::status::ViewerRole;
use crate::session::GameRegistry;
use crate::session::registry::{GameSummary, LobbyError, PlayerLobbyView};

const MAX_PLAYERS: usize = 8;

/// Request to create a new game session.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateGameRequest {
    pub player_count: usize,
    pub seed: Option<u64>,
}

/// Response after creating a game.
#[derive(Debug, Serialize)]
pub struct CreateGameResponse {
    pub game_id: String,
    /// Private to the creating client; never included in a public lobby view.
    pub player_session: String,
    pub player: PlayerIdentity,
    pub lobby: PlayerLobbyView,
}

#[derive(Debug, Serialize)]
pub struct PlayerIdentity {
    pub id: PlayerId,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadyRequest {
    pub ready: bool,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum JoinRequest {
    New,
    Takeover { player_id: PlayerId },
}

#[derive(Debug, Serialize)]
pub struct JoinResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub player_session: Option<String>,
    pub player: PlayerIdentity,
    pub lobby: PlayerLobbyView,
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
    if !(2..=MAX_PLAYERS).contains(&payload.player_count) {
        return Err((
            StatusCode::BAD_REQUEST,
            "player_count must be 2-8".to_owned(),
        ));
    }

    let game_id = loop {
        let candidate = format!("game_{:032x}", rand::random::<u128>());
        if !registry.contains_game(&candidate) {
            break candidate;
        }
    };

    let seed = payload.seed.unwrap_or_else(rand::random::<u64>);
    let (lobby, player, session) = registry
        .create_player_lobby(game_id.clone(), payload.player_count, seed)
        .map_err(lobby_error)?;

    Ok(Json(CreateGameResponse {
        game_id,
        player_session: session.as_str().to_owned(),
        player: PlayerIdentity { id: player },
        lobby,
    }))
}

/// One entry point for new admissions, credential reconnects and (later) takeover.
pub async fn join_lobby(
    Path(game_id): Path<String>,
    headers: HeaderMap,
    State(registry): State<Arc<GameRegistry>>,
    Json(payload): Json<JoinRequest>,
) -> Result<Json<JoinResponse>, (StatusCode, String)> {
    if matches!(payload, JoinRequest::Takeover { .. }) {
        return Err(lobby_error(LobbyError::TakeoverUnavailable));
    }
    let (lobby, player, session) = registry
        .join_player_lobby(&game_id, player_session(&headers)?)
        .map_err(lobby_error)?;
    Ok(Json(JoinResponse {
        player_session: session.map(|value| value.as_str().to_owned()),
        player: PlayerIdentity { id: player },
        lobby,
    }))
}

/// Authenticated lobby heartbeat; does not renew or rotate the credential.
pub async fn heartbeat(
    Path(game_id): Path<String>,
    headers: HeaderMap,
    State(registry): State<Arc<GameRegistry>>,
) -> Result<Json<PlayerLobbyView>, (StatusCode, String)> {
    let token = require_player_session(&headers)?;
    Ok(Json(
        registry
            .player_heartbeat(&game_id, token)
            .map_err(lobby_error)?,
    ))
}

/// Handler for `GET /api/games/{game_id}/lobby`.
pub async fn get_lobby(
    Path(game_id): Path<String>,
    headers: HeaderMap,
    State(registry): State<Arc<GameRegistry>>,
) -> Result<Json<PlayerLobbyView>, (StatusCode, String)> {
    Ok(Json(
        registry
            .player_lobby_status(&game_id, player_session(&headers)?)
            .map_err(lobby_error)?
            .0,
    ))
}

/// Retire an authenticated, non-host lobby participant.
pub async fn leave_lobby(
    Path(game_id): Path<String>,
    headers: HeaderMap,
    State(registry): State<Arc<GameRegistry>>,
) -> Result<Json<PlayerLobbyView>, (StatusCode, String)> {
    Ok(Json(
        registry
            .leave_player_lobby(&game_id, require_player_session(&headers)?)
            .map_err(lobby_error)?,
    ))
}

/// Handler for `POST /api/games/{game_id}/lobby/ready`.
pub async fn set_ready(
    Path(game_id): Path<String>,
    headers: HeaderMap,
    State(registry): State<Arc<GameRegistry>>,
    Json(payload): Json<ReadyRequest>,
) -> Result<Json<PlayerLobbyView>, (StatusCode, String)> {
    Ok(Json(
        registry
            .set_player_ready(&game_id, require_player_session(&headers)?, payload.ready)
            .map_err(lobby_error)?
            .0,
    ))
}

/// Handler for `POST /api/games/{game_id}/lobby/start`.
pub async fn start_lobby(
    Path(game_id): Path<String>,
    headers: HeaderMap,
    State(registry): State<Arc<GameRegistry>>,
) -> Result<Json<PlayerLobbyView>, (StatusCode, String)> {
    Ok(Json(
        registry
            .start_player_lobby(&game_id, require_player_session(&headers)?)
            .map_err(lobby_error)?,
    ))
}

fn player_session(headers: &HeaderMap) -> Result<Option<&str>, (StatusCode, String)> {
    headers
        .get("x-ti4-player-session")
        .map(|header| {
            header
                .to_str()
                .map_err(|_| lobby_error(LobbyError::InvalidCapability))
        })
        .transpose()
}

fn require_player_session(headers: &HeaderMap) -> Result<&str, (StatusCode, String)> {
    player_session(headers)?.ok_or_else(|| lobby_error(LobbyError::InvalidCapability))
}

fn lobby_error(error: LobbyError) -> (StatusCode, String) {
    let message = error.message();
    let status = match error {
        LobbyError::NotFound => StatusCode::NOT_FOUND,
        LobbyError::InvalidCapability
        | LobbyError::HumanSeatRequired
        | LobbyError::HostRequired => StatusCode::FORBIDDEN,
        LobbyError::HumansNotReady
        | LobbyError::AlreadyRunning
        | LobbyError::NotInLobby
        | LobbyError::SeatUnavailable
        | LobbyError::TakeoverUnavailable => StatusCode::CONFLICT,
        LobbyError::Map(_) | LobbyError::Storage(_) => StatusCode::INTERNAL_SERVER_ERROR,
    };
    (status, message)
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

    let viewer = match headers.get("x-ti4-player-session") {
        Some(token) => ViewerRole::Player(
            registry
                .player_lobby_status(
                    &game_id,
                    Some(
                        token
                            .to_str()
                            .map_err(|_| lobby_error(LobbyError::InvalidCapability))?,
                    ),
                )
                .map_err(lobby_error)?
                .1
                .expect("authenticated player"),
        ),
        None => ViewerRole::Spectator,
    };

    Ok(Json(ServerMessage::InitialSnapshot(
        session.get_snapshot(&viewer),
    )))
}
