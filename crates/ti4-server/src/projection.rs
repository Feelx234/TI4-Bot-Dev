//! Pure projections transforming engine state and choices into redacted client views.

use std::collections::{BTreeMap, BTreeSet};
use ti4_engine::choice::Choice;
use ti4_model::id::PlanetId;
use ti4_model::state::{GameState, Player};
use ti4_model::view::{HIDDEN, redact_player, view_for};

use crate::map::GalaxyLayout;
use crate::protocol::PROTOCOL_VERSION;
use crate::protocol::server::{
    GameEvent, InitialSnapshotMsg, PendingChoiceEnvelope, StateUpdateMsg,
};
use crate::protocol::status::{PublicTurnStatus, ViewerRole};
use crate::protocol::view::{
    BoardTileView, BoardView, GameView, PlacedUnitView, PlanetView, PlayerView, SystemView,
    TableView,
};

/// Projects one player record that has already passed through the model's redaction boundary.
#[must_use]
pub fn project_player_view(player: &Player) -> PlayerView {
    PlayerView {
        id: player.id.clone(),
        faction: player.faction.clone(),
        victory_points: player.victory_points,
        trade_goods: player.trade_goods,
        commodities: player.commodities,
        tactic_tokens: player.tactic_tokens,
        fleet_tokens: player.fleet_tokens,
        strategic_tokens: player.strategic_tokens,
        passed: player.passed,
        strategy_cards: player.strategy_cards.clone(),
        exhausted_strategy_cards: player.exhausted_strategy_cards.clone(),
        technologies: player.technologies.clone(),
        exhausted_technologies: player.exhausted_technologies.clone(),
        relics: player.relics.clone(),
        exhausted_relics: player.exhausted_relics.clone(),
        action_cards_count: player.action_cards.len(),
        secret_objectives_count: player.secret_objectives.len(),
        held_action_cards: player
            .action_cards
            .iter()
            .filter(|card| card.as_str() != HIDDEN)
            .cloned()
            .collect(),
        held_secret_objectives: player
            .secret_objectives
            .iter()
            .filter(|objective| objective.as_str() != HIDDEN)
            .cloned()
            .collect(),
        scored_secret_objectives: player.plot_objectives.iter().cloned().collect(),
        leaders: player.leaders.clone(),
    }
}

/// Projects the board systems, planets, and units.
#[must_use]
pub fn project_board_view_with_map(state: &GameState, map_tiles: &[BoardTileView]) -> BoardView {
    let mut systems = BTreeMap::new();

    for (sys_id, sys_state) in &state.board {
        let mut planet_ids: BTreeSet<PlanetId> = sys_state.planet_control.keys().cloned().collect();
        planet_ids.extend(sys_state.planet_units.keys().cloned());
        planet_ids.extend(sys_state.purged_planets.iter().cloned());

        let mut planets = BTreeMap::new();
        for planet_id in planet_ids {
            planets.insert(
                planet_id.clone(),
                PlanetView {
                    planet_id: planet_id.clone(),
                    controlled_by: sys_state.planet_control.get(&planet_id).cloned(),
                    exhausted: state.exhausted_planets.contains(&planet_id),
                    attachments: state
                        .planet_attachments
                        .get(&planet_id)
                        .cloned()
                        .unwrap_or_default(),
                },
            );
        }

        let mut units = Vec::new();
        // Space units
        for u in &sys_state.units {
            units.push(PlacedUnitView {
                unit_type: u.type_id.clone(),
                owner: u.owner.clone(),
                planet: None,
                damaged: u.sustained_damage,
            });
        }
        // Planet units
        for (p_id, p_units) in &sys_state.planet_units {
            for u in p_units {
                units.push(PlacedUnitView {
                    unit_type: u.type_id.clone(),
                    owner: u.owner.clone(),
                    planet: Some(p_id.clone()),
                    damaged: u.sustained_damage,
                });
            }
        }

        systems.insert(
            sys_id.clone(),
            SystemView {
                system_id: sys_id.clone(),
                command_tokens: sys_state.command_tokens.clone(),
                planets,
                units,
            },
        );
    }

    BoardView {
        systems,
        active_system: state.active_system.clone(),
        map_tiles: map_tiles.to_vec(),
    }
}

/// Projects the board systems, planets, and units without map tiles.
#[must_use]
pub fn project_board_view(state: &GameState) -> BoardView {
    project_board_view_with_map(state, &[])
}

/// Projects table-level public objectives, laws, and strategy cards.
#[must_use]
pub fn project_table_view(state: &GameState) -> TableView {
    TableView {
        revealed_objectives: state.revealed_objectives.clone(),
        scored_objectives: state.scored_objectives.clone(),
        unclaimed_strategy_cards: state.unclaimed_strategy_cards.clone(),
        strategy_card_goods: state.strategy_card_goods.clone(),
        laws: state.laws.clone(),
    }
}

/// Projects the entire game state for a specific viewer role with static map tiles.
#[must_use]
pub fn project_game_view_with_map(
    state: &GameState,
    viewer: &ViewerRole,
    map_tiles: &[BoardTileView],
) -> GameView {
    let redacted = redacted_state(state, viewer);
    let players = redacted.players.iter().map(project_player_view).collect();

    GameView {
        round: redacted.round,
        phase: redacted.phase,
        speaker: redacted.speaker.clone(),
        seating_order: redacted.seating_order.clone(),
        active_player: redacted.active.clone(),
        finished: redacted.finished,
        players,
        board: project_board_view_with_map(&redacted, map_tiles),
        table: project_table_view(&redacted),
    }
}

/// Applies the model's authoritative redaction for a protocol viewer.
#[must_use]
pub fn redacted_state(state: &GameState, viewer: &ViewerRole) -> GameState {
    match viewer {
        ViewerRole::Player(seat) => view_for(state, seat),
        ViewerRole::Spectator => {
            let mut spectator = state.clone();
            for player in &mut spectator.players {
                *player = redact_player(player);
            }
            spectator
        }
    }
}

/// Projects the entire game state for a specific viewer role.
#[must_use]
pub fn project_game_view(state: &GameState, viewer: &ViewerRole) -> GameView {
    project_game_view_with_map(state, viewer, &[])
}

/// Projects public turn status without disclosing another player's private reactions or legal choices.
#[must_use]
pub fn project_turn_status(state: &GameState, pending_choice: Option<&Choice>) -> PublicTurnStatus {
    if state.finished {
        let winner = state
            .players
            .iter()
            .max_by_key(|p| p.victory_points)
            .map(|p| p.id.clone());
        return PublicTurnStatus::GameOver { winner };
    }

    if let Some(choice) = pending_choice {
        return PublicTurnStatus::WaitingForDecision {
            seat: choice.player.clone(),
            phase: state.phase,
            round: state.round,
            // The existence of a choice is public; its context can reveal a private reaction.
            stage: "Waiting for player".to_owned(),
        };
    }

    if let Some(active) = &state.active {
        PublicTurnStatus::ActiveTurn {
            player: active.clone(),
            phase: state.phase,
            round: state.round,
        }
    } else {
        PublicTurnStatus::PhaseTransition {
            phase: state.phase,
            round: state.round,
        }
    }
}

/// Projects a pending choice only to its owning seat.
#[must_use]
pub fn project_pending_choice(
    viewer: &ViewerRole,
    pending_choice: Option<(&Choice, &str)>,
) -> Option<PendingChoiceEnvelope> {
    pending_choice.and_then(|(choice, nonce)| {
        viewer
            .is_actor(&choice.player)
            .then(|| PendingChoiceEnvelope {
                nonce: nonce.to_owned(),
                choice: choice.clone(),
            })
    })
}

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
        view: project_game_view_with_map(state, viewer, map_tiles),
        state: redacted_state(state, viewer),
        galaxy_layout: galaxy_layout.clone(),
        pending_choice: project_pending_choice(viewer, pending_choice),
        turn_status: project_turn_status(state, pending_choice.map(|(c, _)| c)),
        events: events
            .iter()
            .filter(|event| event.visibility.permits(viewer))
            .cloned()
            .collect(),
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
    StateUpdateMsg {
        protocol_version: PROTOCOL_VERSION,
        game_id: game_id.to_owned(),
        game_version,
        viewer: viewer.clone(),
        view: project_game_view_with_map(state, viewer, map_tiles),
        state: redacted_state(state, viewer),
        galaxy_layout: galaxy_layout.clone(),
        pending_choice: project_pending_choice(viewer, pending_choice),
        turn_status: project_turn_status(state, pending_choice.map(|(c, _)| c)),
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
