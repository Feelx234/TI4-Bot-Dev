//! Pure projections transforming engine state and choices into redacted client views.

use std::collections::{BTreeMap, BTreeSet};
use ti4_engine::choice::Choice;
use ti4_model::id::PlanetId;
use ti4_model::state::{GameState, Phase, Player};

use crate::protocol::PROTOCOL_VERSION;
use crate::protocol::choice::PendingChoiceDto;
use crate::protocol::server::{InitialSnapshotMsg, StateUpdateMsg};
use crate::protocol::status::{PublicTurnStatus, ViewerRole};
use crate::protocol::view::{
    BoardView, GameView, PlacedUnitView, PlanetView, PlayerView, SystemView, TableView,
};

/// Projects one player's state for the given viewer role.
///
/// If `viewer` does not match `player.id`, all action cards and unrevealed secret objectives
/// are strictly redacted (empty lists). Public hand counts remain accurate.
#[must_use]
pub fn project_player_view(
    player: &Player,
    viewer: &ViewerRole,
    laws: &BTreeMap<String, String>,
) -> PlayerView {
    let is_own_seat = viewer.is_actor(&player.id);
    let secrets_revealed_by_law = laws
        .get("warrant")
        .is_some_and(|owner| owner == &player.id.to_string());

    let held_action_cards = if is_own_seat {
        player.action_cards.clone()
    } else {
        Vec::new()
    };

    let held_secret_objectives = if is_own_seat || secrets_revealed_by_law {
        player.secret_objectives.clone()
    } else {
        Vec::new()
    };

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
        held_action_cards,
        held_secret_objectives,
        scored_secret_objectives: player.plot_objectives.iter().cloned().collect(),
        leaders: player.leaders.clone(),
    }
}

/// Projects the board systems, planets, and units.
#[must_use]
pub fn project_board_view(state: &GameState) -> BoardView {
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
    }
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

/// Projects the entire game state for a specific viewer role.
#[must_use]
pub fn project_game_view(state: &GameState, viewer: &ViewerRole) -> GameView {
    let players = state
        .players
        .iter()
        .map(|p| project_player_view(p, viewer, &state.laws))
        .collect();

    GameView {
        round: state.round,
        phase: state.phase,
        speaker: state.speaker.clone(),
        seating_order: state.seating_order.clone(),
        active_player: state.active.clone(),
        finished: state.finished,
        players,
        board: project_board_view(state),
        table: project_table_view(state),
    }
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
        let stage = match choice.context.as_ref() {
            Some(ctx) => {
                if ctx.subtype.starts_with("reaction")
                    || ctx.subtype.contains("action_card")
                    || ctx.subtype.starts_with("timing")
                {
                    "Reaction Window".to_owned()
                } else if ctx.phase == Phase::Agenda {
                    "Agenda Voting".to_owned()
                } else if ctx.phase == Phase::Strategy {
                    "Strategy Phase Selection".to_owned()
                } else if ctx.phase == Phase::Status {
                    "Status Phase".to_owned()
                } else {
                    "Action Phase Decision".to_owned()
                }
            }
            None => match state.phase {
                Phase::Strategy => "Strategy Phase Selection".to_owned(),
                Phase::Action => "Action Phase Decision".to_owned(),
                Phase::Status => "Status Phase".to_owned(),
                Phase::Agenda => "Agenda Voting".to_owned(),
            },
        };

        return PublicTurnStatus::WaitingForDecision {
            seat: choice.player.clone(),
            phase: state.phase,
            round: state.round,
            stage,
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

/// Projects an initial snapshot for a connecting viewer.
#[must_use]
pub fn project_initial_snapshot(
    game_id: &str,
    game_version: u64,
    state: &GameState,
    viewer: &ViewerRole,
    pending_choice: Option<(&Choice, &str)>,
) -> InitialSnapshotMsg {
    let pending_choice_dto = pending_choice.and_then(|(choice, nonce)| {
        if viewer.is_actor(&choice.player) {
            Some(PendingChoiceDto::from_choice(
                choice,
                nonce.to_owned(),
                true,
            ))
        } else {
            None
        }
    });

    InitialSnapshotMsg {
        protocol_version: PROTOCOL_VERSION,
        game_id: game_id.to_owned(),
        game_version,
        viewer: viewer.clone(),
        view: project_game_view(state, viewer),
        pending_choice: pending_choice_dto,
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
    let pending_choice_dto = pending_choice.and_then(|(choice, nonce)| {
        if viewer.is_actor(&choice.player) {
            Some(PendingChoiceDto::from_choice(
                choice,
                nonce.to_owned(),
                true,
            ))
        } else {
            None
        }
    });

    StateUpdateMsg {
        protocol_version: PROTOCOL_VERSION,
        game_id: game_id.to_owned(),
        game_version,
        viewer: viewer.clone(),
        view: project_game_view(state, viewer),
        pending_choice: pending_choice_dto,
        turn_status: project_turn_status(state, pending_choice.map(|(c, _)| c)),
    }
}
