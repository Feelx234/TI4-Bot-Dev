//! Pure projections transforming engine state and choices into redacted client views.

use std::collections::{BTreeMap, BTreeSet};
use ti4_content::ContentStore;
use ti4_content::galaxy::Galaxy;
use ti4_engine::choice::Choice;
use ti4_model::content_types::{ContentType, POK, SourceSet};
use ti4_model::id::{ObjectiveId, PlanetId, PlayerId};
use ti4_model::state::{GameState, Player};
use ti4_model::view::{HIDDEN, redact_player, view_for};

use serde::{Deserialize, Serialize};

use crate::status::{PublicTurnStatus, ViewerRole};
use crate::view::{
    BoardTileView, BoardView, GameView, ObjectiveProgressView, PlacedUnitView, PlanetView,
    PlayerView, SystemView, TableView,
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
        exhausted_legendary: player.exhausted_legendary.clone(),
        relic_fragments: player
            .relic_fragments
            .iter()
            .filter(|(_, count)| **count > 0)
            .map(|(deck, count)| (deck.clone(), *count))
            .collect(),
        exploration_cards: player.exploration_cards.clone(),
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

/// Helper to construct public combat view if a space combat is detected
#[must_use]
pub fn project_combat_view(
    state: &GameState,
    pending_choice: Option<&Choice>,
    dice_rolls: &[crate::view::CombatDieRoll],
) -> Option<crate::view::CombatView> {
    let choice_ctx = pending_choice.and_then(|c| c.context.as_ref());
    let (sys_id, attacker, defender) = state.active_space_combat.clone()?;
    state.board.get(&sys_id)?;

    let active_player = pending_choice.map(|c| c.player.clone());
    let stage = choice_ctx.map(|ctx| ctx.subtype.clone());
    let attacker_hits = state.combat_round_hits.get(&attacker).copied();
    let defender_hits = state.combat_round_hits.get(&defender).copied();

    let dice_rolls: Vec<crate::view::CombatDieRoll> = if !dice_rolls.is_empty() {
        dice_rolls.to_vec()
    } else {
        state
            .combat_round_dice
            .iter()
            .map(|r| crate::view::CombatDieRoll {
                player: Some(r.player.clone()),
                unit: r.unit.clone(),
                roll: r.roll,
                target: r.target,
                hit: r.hit,
            })
            .collect()
    };

    let hits_to_assign = choice_ctx
        .and_then(|ctx| ctx.outstanding.first())
        .map(|o| usize::try_from(o.amount).unwrap_or(0))
        .or_else(|| {
            active_player.as_ref().and_then(|p| {
                let produced = if *p == attacker {
                    defender_hits
                } else if *p == defender {
                    attacker_hits
                } else {
                    None
                }?;
                // The HITS_TO_ASSIGN window is still open after Shields Holding resolves.
                // The round's produced-hit total stays unchanged, but the player's pending
                // cancellation grant must already be reflected in the live hit counter.
                let cancelled = choice_ctx
                    .filter(|ctx| {
                        ctx.subtype.starts_with("reaction_")
                            && ctx.subtype.ends_with("HITS_TO_ASSIGN")
                    })
                    .map_or(0, |_| ti4_engine::combat::cancellable_hits(state, p));
                Some((produced as usize).saturating_sub(cancelled))
            })
        });

    Some(crate::view::CombatView {
        system_id: sys_id,
        round: state.combat_presentation.round.max(1),
        battle_seq: state.combat_presentation.battle_seq,
        phase: state.combat_presentation.phase.clone(),
        round_start: state
            .combat_presentation
            .round_start
            .iter()
            .map(|u| PlacedUnitView {
                unit_type: u.type_id.clone(),
                owner: u.owner.clone(),
                planet: None,
                damaged: u.sustained_damage,
                galvanized: u.galvanized,
            })
            .collect(),
        barrage_start: state
            .combat_presentation
            .barrage_start
            .iter()
            .map(|u| PlacedUnitView {
                unit_type: u.type_id.clone(),
                owner: u.owner.clone(),
                planet: None,
                damaged: u.sustained_damage,
                galvanized: u.galvanized,
            })
            .collect(),
        barrage_hits: state.combat_presentation.barrage_hits.clone(),
        barrage_dice: state
            .combat_presentation
            .barrage_dice
            .iter()
            .map(|r| crate::view::CombatDieRoll {
                player: Some(r.player.clone()),
                unit: r.unit.clone(),
                roll: r.roll,
                target: r.target,
                hit: r.hit,
            })
            .collect(),
        remaining_hits: {
            let mut remaining = state.combat_presentation.remaining_hits.clone();
            if choice_ctx.is_some_and(|ctx| {
                ctx.subtype.starts_with("reaction_") && ctx.subtype.ends_with("HITS_TO_ASSIGN")
            }) {
                if let Some(player) = active_player.as_ref() {
                    if let Some(hits) = remaining.get_mut(player) {
                        *hits = hits
                            .saturating_sub(ti4_engine::combat::cancellable_hits(state, player));
                    }
                }
            }
            remaining
        },
        attacker,
        defender,
        active_player,
        stage,
        hits_to_assign,
        attacker_hits,
        defender_hits,
        dice_rolls,
    })
}

/// Projects the board systems, planets, and units without map tiles.
#[must_use]
pub fn project_board_view(state: &GameState) -> BoardView {
    project_board_view_with_map(state, &[])
}

/// Projects the board systems, planets, and units.
#[must_use]
pub fn project_board_view_with_map(state: &GameState, map_tiles: &[BoardTileView]) -> BoardView {
    project_board_view_full(state, map_tiles, None, &[], None)
}

/// Projects the full board view with combat and dice information.
#[must_use]
pub fn project_board_view_full(
    state: &GameState,
    map_tiles: &[BoardTileView],
    pending_choice: Option<&Choice>,
    dice_rolls: &[crate::view::CombatDieRoll],
    viewer: Option<&ViewerRole>,
) -> BoardView {
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
                galvanized: u.galvanized,
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
                    galvanized: u.galvanized,
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

    let invasion = state.active_invasion.as_ref().and_then(|active| {
        let content = ti4_content::ContentStore::embedded();
        let types = ti4_content::units::catalogue(content, ti4_model::content_types::POK);
        let space = state.system_state(&active.system);
        let mut odds_context = BTreeMap::new();
        if let Some(system) = systems.get(&active.system) {
            for planet in system.planets.keys() {
                let ground_force_types: Vec<String> = space
                    .on_planet(planet)
                    .iter()
                    .filter(|unit| {
                        types
                            .get(unit.type_id.as_str())
                            .is_some_and(ti4_content::units::UnitType::is_ground_force)
                    })
                    .map(|unit| unit.type_id.to_string())
                    .collect();
                let owners: BTreeSet<PlayerId> = space
                    .on_planet(planet)
                    .iter()
                    .filter(|unit| {
                        unit.owner != active.invader
                            && types
                                .get(unit.type_id.as_str())
                                .is_some_and(ti4_content::units::UnitType::is_ground_force)
                    })
                    .map(|unit| unit.owner.clone())
                    .collect();
                let opponent = space
                    .planet_control
                    .get(planet)
                    .filter(|owner| owners.contains(*owner))
                    .cloned()
                    .or_else(|| owners.iter().next().cloned());
                let mut additional_guns = BTreeMap::new();
                let mut available = true;
                if ti4_engine::entropic_scars::abilities_usable(
                    content,
                    ti4_model::content_types::POK,
                    &active.system,
                    Some(&active.system),
                ) {
                    for unit in space
                        .on_planet(planet)
                        .iter()
                        .filter(|unit| unit.owner != active.invader)
                    {
                        let Some(kind) = types.get(unit.type_id.as_str()) else {
                            continue;
                        };
                        if kind.space_cannon_hits_on().is_none() || kind.space_cannon_dice() == 0 {
                            continue;
                        }
                        if kind.base_type() == "pds"
                            && state.players.iter().any(|seat| {
                                seat.id != unit.owner
                                    && seat.disable_invasion.contains(&state.activation_seq)
                            })
                        {
                            if opponent.as_ref() == Some(&unit.owner) && kind.is_ground_force() {
                                available = false;
                            }
                            continue;
                        }
                        if opponent.as_ref() == Some(&unit.owner) && kind.is_ground_force() {
                            continue;
                        }
                        *additional_guns.entry(unit.type_id.to_string()).or_insert(0) += 1;
                    }
                } else if space.on_planet(planet).iter().any(|unit| {
                    opponent.as_ref() == Some(&unit.owner)
                        && types.get(unit.type_id.as_str()).is_some_and(|kind| {
                            kind.is_ground_force() && kind.space_cannon_hits_on().is_some()
                        })
                }) {
                    available = false;
                }
                let mut harrow_units = BTreeMap::new();
                if ti4_engine::faction_abilities::has(state, content, &active.invader, "harrow")
                    && ti4_engine::invasion::bombardable(
                        state,
                        content,
                        ti4_model::content_types::POK,
                        &active.system,
                        planet,
                        &active.invader,
                    )
                {
                    for unit in space.units_of(&active.invader) {
                        if types
                            .get(unit.type_id.as_str())
                            .is_some_and(|kind| kind.has_bombardment() && kind.bombard_dice() > 0)
                        {
                            *harrow_units.entry(unit.type_id.to_string()).or_insert(0) += 1;
                        }
                    }
                }
                odds_context.insert(
                    planet.clone(),
                    crate::view::InvasionOddsContext {
                        opponent,
                        available,
                        ground_force_types,
                        additional_guns,
                        harrow_units,
                    },
                );
            }
        }

        // H2: Filter out boring invasions (no defenders) from spectator views
        let has_defenders = odds_context.values().any(|ctx| ctx.opponent.is_some());
        if !has_defenders && viewer.is_some() && !viewer.as_ref().unwrap().is_actor(&active.invader)
        {
            // Hide this invasion for non-invading players when no defenders are present
            return None;
        }

        Some(crate::view::InvasionView {
            system_id: active.system.clone(),
            invasion_seq: active.seq,
            invader: active.invader.clone(),
            phase: active.phase.clone(),
            planets: systems
                .get(&active.system)
                .map_or_else(Vec::new, |system: &crate::view::SystemView| {
                    system.planets.keys().cloned().collect()
                }),
            current_planet: active.planet.clone(),
            defender: active.defender.clone(),
            ground_round: active.ground_round,
            odds_context,
            last_step: active.last_step.as_ref().map(|step| {
                let placed = |unit: &ti4_model::units::Unit| PlacedUnitView {
                    unit_type: unit.type_id.clone(),
                    owner: unit.owner.clone(),
                    planet: Some(step.planet.clone()),
                    damaged: unit.sustained_damage,
                    galvanized: unit.galvanized,
                };
                crate::view::InvasionStepView {
                    planet: step.planet.clone(),
                    kind: step.kind.clone(),
                    round: step.round,
                    before: step.before.iter().map(placed).collect(),
                    after: step.after.iter().map(placed).collect(),
                    dice: step.dice.clone(),
                    hits: step.hits.clone(),
                    harrow_hits: step.harrow_hits,
                }
            }),
        })
    });
    BoardView {
        systems,
        active_system: state.active_system.clone(),
        map_tiles: map_tiles.to_vec(),
        combat: project_combat_view(state, pending_choice, dice_rolls),
        invasion,
    }
}

fn canonical_objective_alias<'a>(content: &'a ContentStore, raw: &'a str) -> Option<&'a str> {
    if content.get(ContentType::PublicObjectives, raw).is_some() {
        return Some(raw);
    }
    for record in content.records(ContentType::PublicObjectives) {
        if let Some(name) = record.text("name") {
            let slug = name.to_ascii_lowercase().replace(' ', "_");
            if slug == raw {
                return record.id();
            }
        }
    }
    None
}

/// Projects table-level public objectives, laws, strategy cards, and objective progress.
#[must_use]
pub fn project_table_view_with_map(state: &GameState, map_tiles: &[BoardTileView]) -> TableView {
    let content = ContentStore::embedded();
    let sources = POK;
    let galaxy_opt = if !map_tiles.is_empty() {
        let tiles: Vec<_> = map_tiles
            .iter()
            .map(|tile| {
                (
                    tile.system_id.as_str(),
                    ti4_model::hex::Hex::new(tile.q, tile.r),
                )
            })
            .collect();
        Galaxy::placed(content, &tiles, sources).ok()
    } else {
        None
    };

    let mut objective_progress = BTreeMap::new();
    for player in &state.players {
        let mut position =
            ti4_engine::objectives::Position::new(state, content, sources, &player.id);
        if let Some(ref galaxy) = galaxy_opt {
            position = position.with_galaxy(galaxy);
        }
        let mut player_progress = BTreeMap::new();
        for raw_alias in &state.revealed_objectives {
            let canonical = canonical_objective_alias(content, raw_alias.as_str())
                .map(ObjectiveId::new)
                .unwrap_or_else(|| raw_alias.clone());

            if let Some(prog) = ti4_engine::objectives::counting_progress(&canonical, &position)
                .or_else(|| {
                    ti4_engine::objectives::remaining_position_progress(&canonical, &position)
                })
            {
                player_progress.insert(
                    raw_alias.clone(),
                    ObjectiveProgressView {
                        have: u32::try_from(prog.have).unwrap_or(0),
                        threshold: u32::try_from(prog.threshold).unwrap_or(0),
                        satisfied: prog.satisfied(),
                    },
                );
            } else if let Some(cost) =
                ti4_engine::objectives::bought_progress_at(&position, &canonical)
            {
                player_progress.insert(
                    raw_alias.clone(),
                    ObjectiveProgressView {
                        have: u32::try_from(cost.have.max(0)).unwrap_or(0),
                        threshold: u32::try_from(cost.target.max(0)).unwrap_or(0),
                        satisfied: cost.satisfied(),
                    },
                );
            }
        }
        if !player_progress.is_empty() {
            objective_progress.insert(player.id.clone(), player_progress);
        }
    }

    TableView {
        revealed_objectives: state.revealed_objectives.clone(),
        scored_objectives: state.scored_objectives.clone(),
        objective_progress,
        unclaimed_strategy_cards: state.unclaimed_strategy_cards.clone(),
        strategy_card_goods: state.strategy_card_goods.clone(),
        laws: state.laws.clone(),
    }
}

/// Projects table-level public objectives, laws, and strategy cards.
#[must_use]
pub fn project_table_view(state: &GameState) -> TableView {
    project_table_view_with_map(state, &[])
}

/// Projects the entire game state for a specific viewer role with static map tiles.
#[must_use]
pub fn project_game_view_with_map(
    state: &GameState,
    viewer: &ViewerRole,
    map_tiles: &[BoardTileView],
) -> GameView {
    project_game_view_full(state, viewer, map_tiles, None, &[])
}

/// Projects the entire game state for a specific viewer role with pending choice and combat dice.
#[must_use]
pub fn project_game_view_full(
    state: &GameState,
    viewer: &ViewerRole,
    map_tiles: &[BoardTileView],
    pending_choice: Option<&Choice>,
    dice_rolls: &[crate::view::CombatDieRoll],
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
        board: project_board_view_full(
            &redacted,
            map_tiles,
            pending_choice,
            dice_rolls,
            Some(viewer),
        ),
        table: project_table_view_with_map(&redacted, map_tiles),
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
        let winner = ti4_engine::objectives::leader(state);
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

/// Submission metadata around the engine's wire-serialized choice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PendingChoiceEnvelope {
    pub nonce: String,
    pub choice: Choice,
}

/// What a client is shown of a game: the part of a state update that depends on the game alone.
///
/// The server's `StateUpdateMsg` has these four fields under the same names, and the wasm build
/// hands them to its host as they are, so one client reads both.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionUpdate {
    pub viewer: ViewerRole,
    pub view: GameView,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending_choice: Option<PendingChoiceEnvelope>,
    pub turn_status: PublicTurnStatus,
    /// What the viewer needs to stage the pending choice of a tactical action. It is there
    /// for the seat that is asked, on a game that has its map ([`crate::tactical`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tactical: Option<crate::tactical::TacticalFacts>,
    /// The exploration decks and the planets that are still explored, on a game that has its
    /// map ([`crate::exploration`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exploration: Option<crate::exploration::ExplorationFacts>,
    /// The action card deck, its discard pile and the hand limit of the viewer
    /// ([`crate::action_cards`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action_cards: Option<crate::action_cards::ActionCardFacts>,
    /// The secret objective deck and what each seat has scored of it
    /// ([`crate::secret_objectives`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret_objectives: Option<crate::secret_objectives::SecretObjectiveFacts>,
    /// Where the victory points of every seat come from ([`crate::score`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score: Option<crate::score::ScoreFacts>,
    /// The agenda deck ([`crate::agendas`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agendas: Option<crate::agendas::AgendaFacts>,
}

/// What the facts of an update are made from, beside the state of the game.
#[derive(Clone, Copy)]
pub struct FactsContext<'a> {
    pub content: &'a ContentStore,
    pub sources: SourceSet,
    /// The map of the game. Without it an update has no `tactical` and no `exploration`.
    pub galaxy: Option<&'a Galaxy>,
}

/// Projects what `viewer` is shown of `state` while the game waits in `pending_choice`.
///
/// With `facts` the update has its six facts ([`fill_facts`]); every host that shows them
/// gives the context. Without, they are all absent.
#[must_use]
pub fn project_session_update(
    state: &GameState,
    viewer: &ViewerRole,
    pending_choice: Option<(&Choice, &str)>,
    map_tiles: &[BoardTileView],
    facts: Option<&FactsContext<'_>>,
) -> SessionUpdate {
    let choice = pending_choice.map(|(choice, _)| choice);
    let mut update = SessionUpdate {
        viewer: viewer.clone(),
        view: project_game_view_full(state, viewer, map_tiles, choice, &[]),
        pending_choice: project_pending_choice(viewer, pending_choice),
        turn_status: project_turn_status(state, choice),
        tactical: None,
        exploration: None,
        action_cards: None,
        secret_objectives: None,
        score: None,
        agendas: None,
    };
    if let Some(context) = facts {
        fill_facts(&mut update, state, context);
    }
    update
}

/// Fills the six facts of `update`, which was projected from `state`: the one place that does.
///
/// `tactical` is of the choice in the update, so only the seat that is asked has it.
pub fn fill_facts(update: &mut SessionUpdate, state: &GameState, context: &FactsContext<'_>) {
    let FactsContext {
        content,
        sources,
        galaxy,
    } = *context;
    update.tactical = update.pending_choice.as_ref().and_then(|pending| {
        crate::tactical::project_tactical_facts(state, content, sources, galaxy?, &pending.choice)
    });
    update.exploration = galaxy.map(|galaxy| {
        crate::exploration::project_exploration_facts(state, content, sources, galaxy)
    });
    update.action_cards = Some(crate::action_cards::project_action_card_facts(
        state,
        content,
        &update.viewer,
    ));
    update.secret_objectives = Some(crate::secret_objectives::project_secret_objective_facts(
        state, content,
    ));
    update.score = Some(crate::score::project_score_facts(state, content));
    update.agendas = Some(crate::agendas::project_agenda_facts(state));
}

#[cfg(test)]
mod tests {
    use super::*;
    use ti4_model::id::{FactionId, ObjectiveId, PlayerId};
    use ti4_model::state::GameState;

    fn reaction_offer() -> (Choice, String) {
        use ti4_engine::choice::ChoiceOption;
        use ti4_engine::decision_context::{DecisionContext, DecisionSource, DecisionTrigger};
        let mut payload = BTreeMap::new();
        payload.insert("player".to_owned(), "player_1".into());
        payload.insert("card".to_owned(), "fs1".into());
        let event = ti4_engine::event::Event::new(4, "ACTION_CARD_PLAYED", payload);
        let choice = Choice::new(
            PlayerId::new("player_2"),
            "when ACTION_CARD_PLAYED",
            vec![
                ChoiceOption::labelled(
                    "reaction:x:ACTION_CARD_PLAYED:when",
                    "ability",
                    "Play Sabotage",
                ),
                ChoiceOption::decline(),
            ],
        )
        .contextualized(
            DecisionContext::new(
                PlayerId::new("player_2"),
                DecisionSource::Reaction("ACTION_CARD_PLAYED".to_owned()),
                "reaction_when_ACTION_CARD_PLAYED",
                ti4_model::state::Phase::Action,
                1,
            )
            .optional(true)
            .with_trigger(DecisionTrigger::from_event(&event, "when", &[])),
        );
        (choice, "7".to_owned())
    }

    #[test]
    fn the_reaction_trigger_reaches_the_asked_seat_and_only_that_seat() {
        let (choice, nonce) = reaction_offer();
        let owner = ViewerRole::Player(PlayerId::new("player_2"));
        let sent =
            project_pending_choice(&owner, Some((&choice, &nonce))).expect("the owner is asked");
        let json = serde_json::to_value(&sent).unwrap();
        let trigger = &json["choice"]["context"]["trigger"];
        assert_eq!(trigger["kind"], "action_card_played");
        assert_eq!(trigger["actor"], "player_1");
        assert_eq!(trigger["card"], "fs1");
        assert_eq!(trigger["relation"], "when");
        assert_eq!(trigger["event_id"], 4);
        assert!(
            trigger.get("chain").is_none(),
            "an empty chain stays off the wire"
        );
        let other = ViewerRole::Player(PlayerId::new("player_1"));
        assert!(project_pending_choice(&other, Some((&choice, &nonce))).is_none());
        assert!(project_pending_choice(&ViewerRole::Spectator, Some((&choice, &nonce))).is_none());
        // And a message written before triggers existed still decodes, with none.
        let mut old = json.clone();
        old["choice"]["context"]
            .as_object_mut()
            .unwrap()
            .remove("trigger");
        let back: PendingChoiceEnvelope = serde_json::from_value(old).unwrap();
        assert!(back.choice.context.unwrap().trigger.is_none());
    }

    #[test]
    fn the_facts_are_filled_with_a_context_and_only_then() {
        let seat = PlayerId::new("player_1");
        let state = GameState::new(std::slice::from_ref(&seat), &[], BTreeMap::new(), None, 1);
        let viewer = ViewerRole::Player(seat);

        let bare = project_session_update(&state, &viewer, None, &[], None);
        assert!(bare.tactical.is_none() && bare.exploration.is_none());
        assert!(bare.action_cards.is_none() && bare.secret_objectives.is_none());
        assert!(bare.score.is_none() && bare.agendas.is_none());

        // Without the map there is nothing to say about the board.
        let context = FactsContext {
            content: ContentStore::embedded(),
            sources: POK,
            galaxy: None,
        };
        let filled = project_session_update(&state, &viewer, None, &[], Some(&context));
        assert!(filled.tactical.is_none() && filled.exploration.is_none());
        assert!(filled.action_cards.is_some() && filled.secret_objectives.is_some());
        assert!(filled.score.is_some() && filled.agendas.is_some());
        let mut again = bare;
        fill_facts(&mut again, &state, &context);
        assert_eq!(again, filled);
    }

    #[test]
    fn a_tied_game_goes_to_the_first_seat_in_initiative_order() {
        let ids: Vec<PlayerId> = ["player_1", "player_2", "player_3"]
            .iter()
            .map(|id| PlayerId::new(*id))
            .collect();
        let mut state = GameState::new(&ids, &[], BTreeMap::new(), None, 1);
        state.players = ids
            .iter()
            .map(|id| {
                let mut player = Player::new(id.clone());
                player.victory_points = 3;
                player
            })
            .collect();
        state.finished = true;
        // 98.8, 61.15a: a three-way tie is broken by initiative order. With nobody holding a
        // strategy card that is seating order, so the first seat wins, not the last.
        match project_turn_status(&state, None) {
            PublicTurnStatus::GameOver { winner } => assert_eq!(winner, Some(ids[0].clone())),
            other => panic!("expected game over, got {other:?}"),
        }
    }

    #[test]
    fn project_table_view_calculates_objective_progress() {
        let p1 = PlayerId::new("player_1");
        let mut state = GameState::new(std::slice::from_ref(&p1), &[], BTreeMap::new(), None, 1);
        let mut player = Player::new(p1.clone());
        player.faction = FactionId::new("hacan");
        player.trade_goods = 7;
        player.tactic_tokens = 2;
        player.strategic_tokens = 2;
        state.players = vec![player];

        state.revealed_objectives = vec![
            ObjectiveId::new("trade_routes"),
            ObjectiveId::new("centralize_trade"),
            ObjectiveId::new("lead"),
        ];

        let table = project_table_view(&state);
        let p1_progress = table.objective_progress.get(&p1).expect("p1 progress");

        let trade_routes = p1_progress
            .get(&ObjectiveId::new("trade_routes"))
            .expect("trade_routes");
        assert_eq!(trade_routes.have, 5);
        assert_eq!(trade_routes.threshold, 5);
        assert!(trade_routes.satisfied);

        let centralize = p1_progress
            .get(&ObjectiveId::new("centralize_trade"))
            .expect("centralize");
        assert_eq!(centralize.have, 7);
        assert_eq!(centralize.threshold, 10);
        assert!(!centralize.satisfied);

        let lead = p1_progress.get(&ObjectiveId::new("lead")).expect("lead");
        assert_eq!(lead.have, 3);
        assert_eq!(lead.threshold, 3);
        assert!(lead.satisfied);
    }
}
