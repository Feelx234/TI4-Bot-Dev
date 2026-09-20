//! HTTP endpoints for listing, creating, and inspecting game sessions.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};

use ti4_content::ContentStore;
use ti4_model::id::PlayerId;

use crate::protocol::server::InitialSnapshotMsg;
use crate::protocol::status::ViewerRole;
use crate::session::registry::GameSummary;
use crate::session::{GameRegistry, SeatController, SessionConfig};

/// Request to create a new game session.
#[derive(Debug, Deserialize)]
pub struct CreateGameRequest {
    pub game_id: Option<String>,
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
    pub players: Vec<String>,
}

/// Query parameters for snapshot requests.
#[derive(Debug, Deserialize)]
pub struct SnapshotQuery {
    pub seat: Option<String>,
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
    let game_id = payload
        .game_id
        .unwrap_or_else(|| format!("game_{:08x}", rand::random::<u32>()));

    if registry.get_game(&game_id).is_some() {
        return Err((
            StatusCode::CONFLICT,
            format!("Game '{game_id}' already exists"),
        ));
    }

    let player_names = if payload.players.is_empty() {
        vec!["p1".to_owned(), "p2".to_owned(), "p3".to_owned()]
    } else {
        payload.players
    };

    let player_ids: Vec<PlayerId> = player_names.iter().map(PlayerId::new).collect();
    let seed = payload.seed.unwrap_or_else(rand::random::<u64>);

    let content = ContentStore::embedded();
    let (state, galaxy) =
        crate::map::create_game_with_map(content, &player_ids, seed).map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to start game with map: {e}"),
            )
        })?;
    let map_tiles = crate::map::build_board_tiles(content, &galaxy);

    let mut config = SessionConfig::new(game_id.clone(), state).with_galaxy(galaxy, map_tiles);
    for p in player_ids {
        if payload.bot_seats.iter().any(|b| b == p.as_str()) {
            config = config.with_seat(p, SeatController::BotFirstOption);
        } else {
            config = config.with_seat(p, SeatController::Human);
        }
    }

    registry
        .create_game(config)
        .map_err(|e| (StatusCode::CONFLICT, e))?;

    Ok(Json(CreateGameResponse {
        game_id,
        players: player_names,
    }))
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
    Query(query): Query<SnapshotQuery>,
    State(registry): State<Arc<GameRegistry>>,
) -> Result<Json<InitialSnapshotMsg>, (StatusCode, String)> {
    let session = registry
        .get_game(&game_id)
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("Game '{game_id}' not found")))?;

    let viewer = match query.seat {
        Some(seat) if seat != "spectator" => ViewerRole::Player(PlayerId::new(seat)),
        _ => ViewerRole::Spectator,
    };

    let snapshot = session.get_snapshot(&viewer);
    Ok(Json(snapshot))
}
