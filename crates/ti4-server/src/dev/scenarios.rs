//! Authoritative dev scenario presets and builder logic.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use ti4_content::ContentStore;
use ti4_model::content_types::POK;
use ti4_model::id::{PlayerId, StrategyCardId, SystemId, UnitTypeId};
use ti4_model::units::Unit;

use crate::session::{GameRegistry, SeatController, SessionConfig};
use crate::storage::{
    LobbySlotId, PLAYER_RECORD_VERSION, PersistedLobbyPhase, PlayerLobbyMember, PlayerLobbyRecord,
    PlayerLobbySlot, PlayerSession, generate_player_id,
};

/// Summary metadata for a dev scenario available to launch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioSummary {
    pub id: String,
    pub title: String,
    pub category: String,
    pub description: String,
    pub player_count: usize,
    pub human_faction: String,
    pub opponent_factions: Vec<String>,
}

/// Request to launch a specific dev scenario.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LaunchScenarioRequest {
    pub scenario_id: String,
    pub seed: Option<u64>,
}

/// Response returned when a dev scenario is launched.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchScenarioResponse {
    pub game_id: String,
    pub player_session: String,
    pub player_id: String,
    pub scenario_id: String,
}

/// List of all registered dev scenarios.
#[must_use]
pub fn available_scenarios() -> Vec<ScenarioSummary> {
    vec![
        ScenarioSummary {
            id: "tactical_action".to_owned(),
            title: "Tactical Action & Movement".to_owned(),
            category: "Tactical".to_owned(),
            description: "Player 1 (Federation of Sol) is active in Round 1 Action Phase with full tactic pool and unexhausted fleet at Jord (Home #01). Activate an adjacent sector, move Carrier, Cruiser, Fighters, and Infantry, and resolve planetary exploration or landing.".to_owned(),
            player_count: 3,
            human_faction: "Federation of Sol".to_owned(),
            opponent_factions: vec!["Emirates of Hacan".to_owned(), "Barony of Letnev".to_owned()],
        },
        ScenarioSummary {
            id: "space_combat".to_owned(),
            title: "Space Combat Encounter".to_owned(),
            category: "Combat".to_owned(),
            description: "Hostile Barony of Letnev ships (Cruiser, Destroyer, 2 Fighters) are positioned in the adjacent border system. Activate the contested sector and move your fleet in to trigger the full space combat flow: anti-fighter barrage, combat rolls, sustain damage, hit assignment, and retreat choices against the bot.".to_owned(),
            player_count: 3,
            human_faction: "Federation of Sol".to_owned(),
            opponent_factions: vec!["Emirates of Hacan".to_owned(), "Barony of Letnev".to_owned()],
        },
    ]
}

/// Builds and registers a scenario session into the registry.
pub fn launch_scenario(
    registry: &Arc<GameRegistry>,
    scenario_id: &str,
    seed: Option<u64>,
) -> Result<LaunchScenarioResponse, String> {
    let seed = seed.unwrap_or_else(rand::random::<u64>);
    let (config, lobby_record, human_player, session_token) = match scenario_id {
        "tactical_action" => build_tactical_scenario(seed)?,
        "space_combat" => build_space_combat_scenario(seed)?,
        other => return Err(format!("Unknown scenario '{other}'")),
    };

    let game_id = config.game_id.clone();
    registry.launch_dev_scenario(config, lobby_record)?;

    Ok(LaunchScenarioResponse {
        game_id,
        player_session: session_token,
        player_id: human_player.to_string(),
        scenario_id: scenario_id.to_owned(),
    })
}

fn setup_base_3p_game(
    seed: u64,
    game_prefix: &str,
) -> Result<
    (
        SessionConfig,
        PlayerLobbyRecord,
        PlayerId,
        String,
        ti4_content::galaxy::Galaxy,
        PlayerId,
        PlayerId,
    ),
    String,
> {
    let content = ContentStore::embedded();

    let mut existing_players = BTreeMap::new();
    let p1 = generate_player_id(&existing_players);
    let session1 = PlayerSession::generate();
    existing_players.insert(
        p1.clone(),
        PlayerLobbyMember {
            ready: true,
            session: session1.clone(),
            nickname: "Player 1 (Sol)".to_owned(),
        },
    );

    let p2 = generate_player_id(&existing_players);
    let session2 = PlayerSession::generate();
    existing_players.insert(
        p2.clone(),
        PlayerLobbyMember {
            ready: true,
            session: session2.clone(),
            nickname: "Bot (Hacan)".to_owned(),
        },
    );

    let p3 = generate_player_id(&existing_players);
    let session3 = PlayerSession::generate();
    existing_players.insert(
        p3.clone(),
        PlayerLobbyMember {
            ready: true,
            session: session3.clone(),
            nickname: "Bot (Letnev)".to_owned(),
        },
    );

    let player_ids = vec![p1.clone(), p2.clone(), p3.clone()];
    let (mut state, galaxy) =
        crate::map::create_game_with_map(content, &player_ids, seed).map_err(|e| e.to_string())?;

    // Sol: Leadership (initiative 1), Hacan: Trade (5), Letnev: Warfare (6)
    let strat_sol = StrategyCardId::new("leadership");
    let strat_hacan = StrategyCardId::new("trade");
    let strat_letnev = StrategyCardId::new("warfare");

    state.player_mut(&p1).expect("p1").strategy_cards = vec![strat_sol.clone()];
    state.player_mut(&p2).expect("p2").strategy_cards = vec![strat_hacan.clone()];
    state.player_mut(&p3).expect("p3").strategy_cards = vec![strat_letnev.clone()];

    state
        .unclaimed_strategy_cards
        .retain(|c| c != &strat_sol && c != &strat_hacan && c != &strat_letnev);

    // Transition to Action Phase with p1 as active
    ti4_engine::phase::advance_phase(&mut state);
    ti4_engine::phase::begin_action_turn(&mut state, &p1);

    if let Some(sol) = state.player_mut(&p1) {
        sol.tactic_tokens = 3;
        sol.fleet_tokens = 3;
        sol.strategic_tokens = 2;
    }

    let map_tiles = crate::map::build_board_tiles(content, &galaxy);
    let game_id = format!("{game_prefix}_{:016x}", rand::random::<u64>());

    let mut config = SessionConfig::new(&game_id, state)
        .with_seed(seed)
        .with_player_ids(player_ids.clone())
        .with_galaxy(galaxy.clone(), map_tiles);
    config.seats.insert(p1.clone(), SeatController::Human);
    config
        .seats
        .insert(p2.clone(), SeatController::BotFirstOption);
    config
        .seats
        .insert(p3.clone(), SeatController::BotFirstOption);
    config
        .seat_tokens
        .insert(p1.clone(), session1.as_str().to_owned());
    config
        .seat_tokens
        .insert(p2.clone(), session2.as_str().to_owned());
    config
        .seat_tokens
        .insert(p3.clone(), session3.as_str().to_owned());

    let slots = player_ids
        .iter()
        .enumerate()
        .map(|(i, pid)| PlayerLobbySlot {
            slot_id: LobbySlotId(format!("slot_{}", i + 1)),
            occupant: Some(pid.clone()),
        })
        .collect();

    let lobby_record = PlayerLobbyRecord {
        schema_version: PLAYER_RECORD_VERSION,
        game_id: game_id.clone(),
        phase: PersistedLobbyPhase::Running,
        host_player_id: p1.clone(),
        slots,
        players: existing_players,
        seed,
        lobby_version: 1,
    };

    Ok((
        config,
        lobby_record,
        p1,
        session1.as_str().to_owned(),
        galaxy,
        p2,
        p3,
    ))
}

fn build_tactical_scenario(
    seed: u64,
) -> Result<(SessionConfig, PlayerLobbyRecord, PlayerId, String), String> {
    let (config, lobby_record, p1, token, _, _, _) =
        setup_base_3p_game(seed, "dev_tactical")?;
    Ok((config, lobby_record, p1, token))
}

fn build_space_combat_scenario(
    seed: u64,
) -> Result<(SessionConfig, PlayerLobbyRecord, PlayerId, String), String> {
    let (mut config, lobby_record, p1, token, galaxy, _, p3) =
        setup_base_3p_game(seed, "dev_combat")?;
    let content = ContentStore::embedded();

    // Find an adjacent system to Sol's home (01) that is not an impassable anomaly
    let adjacent_ids = galaxy.adjacent("01");
    let border_system = adjacent_ids
        .into_iter()
        .find(|sys_id| {
            if let Some(sys) = ti4_content::galaxy::system(content, sys_id, POK) {
                !sys.is_supernova() && !sys.is_asteroid_field()
            } else {
                false
            }
        })
        .unwrap_or("18");

    let border_sys_id = SystemId::new(border_system);

    // Place Letnev's fleet in the border system
    let sys_state = config.state.system_mut(&border_sys_id);
    sys_state.command_tokens.clear();
    sys_state.units.clear();
    sys_state
        .units
        .push(Unit::new(UnitTypeId::new("cruiser"), p3.clone()));
    sys_state
        .units
        .push(Unit::new(UnitTypeId::new("destroyer"), p3.clone()));
    sys_state
        .units
        .push(Unit::new(UnitTypeId::new("fighter"), p3.clone()));
    sys_state
        .units
        .push(Unit::new(UnitTypeId::new("fighter"), p3.clone()));

    Ok((config, lobby_record, p1, token))
}
