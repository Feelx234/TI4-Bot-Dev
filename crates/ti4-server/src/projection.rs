//! Projections that wrap a redacted view in a protocol message. The views are made in `ti4-view`.

use std::collections::BTreeMap;
use ti4_engine::choice::Choice;
use ti4_model::state::GameState;

pub use ti4_view::projection::*;

use crate::map::GalaxyLayout;
use crate::protocol::PROTOCOL_VERSION;
use crate::protocol::server::{GameEvent, InitialSnapshotMsg, StateUpdateMsg};
use crate::protocol::status::ViewerRole;
use crate::protocol::view::BoardTileView;

/// Projects an initial snapshot for a connecting viewer with static map tiles.
#[must_use]
pub fn project_initial_snapshot_with_map(
    game_id: &str,
    game_version: u64,
    state: &GameState,
    viewer: &ViewerRole,
    pending_choice: Option<(&Choice, &str)>,
    map_tiles: &[BoardTileView],
    galaxy_layout: &GalaxyLayout,
    events: &[GameEvent],
) -> InitialSnapshotMsg {
    InitialSnapshotMsg {
        protocol_version: PROTOCOL_VERSION,
        game_id: game_id.to_owned(),
        game_version,
        viewer: viewer.clone(),
        view: project_game_view_full(
            state,
            viewer,
            map_tiles,
            pending_choice.map(|(c, _)| c),
            &[],
        ),
        state: redacted_state(state, viewer),
        galaxy_layout: galaxy_layout.clone(),
        pending_choice: project_pending_choice(viewer, pending_choice),
        turn_status: project_turn_status(state, pending_choice.map(|(c, _)| c)),
        events: events
            .iter()
            .filter_map(|event| event.for_viewer(viewer))
            .collect(),
        history: crate::protocol::server::HistoryStatus::default(),
        current_path: None,
        reaction_modes: BTreeMap::new(),
    }
}

/// Projects an initial snapshot for a connecting viewer.
#[must_use]
pub fn project_initial_snapshot(
    game_id: &str,
    game_version: u64,
    state: &GameState,
    viewer: &ViewerRole,
    pending_choice: Option<(&Choice, &str)>,
) -> InitialSnapshotMsg {
    project_initial_snapshot_with_map(
        game_id,
        game_version,
        state,
        viewer,
        pending_choice,
        &[],
        &GalaxyLayout {
            version: 1,
            active_sources: Vec::new(),
            placements: Vec::new(),
            off_map_system_ids: Vec::new(),
        },
        &[],
    )
}

/// Projects a versioned state update message with static map tiles.
#[must_use]
pub fn project_state_update_with_map(
    game_id: &str,
    game_version: u64,
    state: &GameState,
    viewer: &ViewerRole,
    pending_choice: Option<(&Choice, &str)>,
    map_tiles: &[BoardTileView],
    galaxy_layout: &GalaxyLayout,
) -> StateUpdateMsg {
    let SessionUpdate {
        viewer: _,
        view,
        pending_choice: pending,
        turn_status,
        // The server does not send these yet.
        tactical: _,
        exploration: _,
        action_cards: _,
        secret_objectives: _,
        score: _,
        agendas: _,
    } = project_session_update(state, viewer, pending_choice, map_tiles, None);
    StateUpdateMsg {
        history: crate::protocol::server::HistoryStatus::default(),
        current_path: None,
        protocol_version: PROTOCOL_VERSION,
        game_id: game_id.to_owned(),
        game_version,
        viewer: viewer.clone(),
        view,
        state: redacted_state(state, viewer),
        galaxy_layout: galaxy_layout.clone(),
        pending_choice: pending,
        turn_status,
        auto_resolved: Vec::new(),
        reaction_modes: BTreeMap::new(),
    }
}

/// Projects a versioned state update message.
#[must_use]
pub fn project_state_update(
    game_id: &str,
    game_version: u64,
    state: &GameState,
    viewer: &ViewerRole,
    pending_choice: Option<(&Choice, &str)>,
) -> StateUpdateMsg {
    project_state_update_with_map(
        game_id,
        game_version,
        state,
        viewer,
        pending_choice,
        &[],
        &GalaxyLayout {
            version: 1,
            active_sources: Vec::new(),
            placements: Vec::new(),
            off_map_system_ids: Vec::new(),
        },
    )
}
