//! The replay export of a game (`GET /api/games/{id}/replay`, and `server feedback-save` for a
//! report of an online game): the history, and the table the game was set up from, as
//! [`ti4_host::export::ReplayExport`] (`docs/REPLAY_FORMAT.md`).
//!
//! The setup is not secret: it is what the lobby showed. The journal and the seed are exactly what
//! the export held before the setup was added (the decisions reveal the hands through the seed;
//! that is not widened here).

use std::collections::BTreeMap;

use ti4_host::export::{
    ExportBuild, ExportCheck, ExportHistory, ExportSeat, ExportSetup, FORMAT, FORMAT_VERSION,
    ReplayExport, decisions_hash, state_hash,
};
use ti4_host::map::GalaxyLayout;
use ti4_model::id::PlayerId;
use ti4_model::state::GameState;

use crate::session::SeatController;
use crate::storage::{GameHistory, PlayerLobbyRecord};

/// Everything the export is made of, read from a live session or from the data directory.
pub struct ExportInputs<'a> {
    pub game_id: &'a str,
    pub history: GameHistory,
    pub seed: Option<u64>,
    pub player_ids: Vec<PlayerId>,
    /// The opening state of the game.
    pub initial_state: &'a GameState,
    pub galaxy_layout: Option<GalaxyLayout>,
    pub seats: &'a BTreeMap<PlayerId, SeatController>,
    pub nicknames: &'a BTreeMap<PlayerId, String>,
    pub map_template: Option<String>,
    /// The table's record; without it (a test or development game) the export has no setup.
    pub lobby: Option<&'a PlayerLobbyRecord>,
    pub build_commit: String,
}

fn kind_of(controller: Option<&SeatController>) -> String {
    match controller {
        None | Some(SeatController::Human) => "human",
        Some(SeatController::BotSmart) => "smart",
        Some(SeatController::BotRandom) => "random",
        Some(SeatController::BotFirstOption) => "first_option",
        Some(SeatController::BotAlwaysDecline) => "always_decline",
        Some(SeatController::BotScripted(_)) => "scripted",
    }
    .to_owned()
}

/// The setup that rebuilds `initial` for `player_ids`, if one of the lobby's does.
fn setup_of(
    lobby: &PlayerLobbyRecord,
    initial: &GameState,
    player_ids: &[PlayerId],
) -> ExportSetup {
    let mut table = crate::session::table::setup_of(lobby);
    // The state is what was played; the lobby's switches are what it was asked for.
    table.thunders_edge = initial.thunders_edge;
    let factions = if table.scenario().ok().flatten().is_some() {
        Vec::new()
    } else {
        player_ids
            .iter()
            .filter_map(|id| initial.player(id).map(|p| p.faction.clone()))
            .collect()
    };
    // A table made before the starting technology was asked for is told by its opening state.
    let mut first = None;
    for starting_tech in [true, false] {
        table.starting_tech = starting_tech;
        let setup = ExportSetup::new(table.clone(), factions.clone(), initial);
        let rebuilt = setup.build(player_ids).map(|(state, _)| state_hash(&state));
        if rebuilt.as_deref() == Ok(setup.initial_state_hash.as_str()) {
            return setup;
        }
        first.get_or_insert(setup);
    }
    first.expect("two tries were made")
}

fn as_values<T: serde::Serialize>(items: &[T]) -> Vec<serde_json::Value> {
    items
        .iter()
        .map(|item| serde_json::to_value(item).expect("a history record is JSON"))
        .collect()
}

/// The export of a game.
#[must_use]
pub fn build(inputs: ExportInputs<'_>) -> ReplayExport {
    let ExportInputs {
        game_id,
        history,
        seed,
        player_ids,
        initial_state,
        galaxy_layout,
        seats,
        nicknames,
        map_template,
        lobby,
        build_commit,
    } = inputs;
    let setup = lobby.map(|lobby| setup_of(lobby, initial_state, &player_ids));
    // The alias of the lobby when the game's init record (a table without a store) has none.
    let map_template =
        map_template.or_else(|| setup.as_ref().and_then(|setup| setup.table.map.stored()));
    let seats_out = player_ids
        .iter()
        .enumerate()
        .filter_map(|(seat, id)| {
            let player = initial_state.player(id)?;
            let kind = kind_of(seats.get(id));
            Some(ExportSeat {
                seat,
                player_id: id.clone(),
                nickname: nicknames.get(id).cloned(),
                rule_bot_version: (kind == "smart").then_some(ti4_rule_bot::tuning::BOT_VERSION),
                kind,
                faction: player.faction.clone(),
            })
        })
        .collect();
    let check = ExportCheck {
        decisions: history.decisions.len(),
        decisions_hash: decisions_hash(&history.decisions),
    };
    ReplayExport {
        format: FORMAT.to_owned(),
        version: FORMAT_VERSION,
        format_version: FORMAT_VERSION,
        game_id: game_id.to_owned(),
        seed,
        player_ids,
        map_template,
        setup,
        seats: seats_out,
        map_layout: galaxy_layout,
        build: Some(ExportBuild {
            build_commit: Some(build_commit),
            engine_version: Some(env!("CARGO_PKG_VERSION").to_owned()),
            content_digest: Some(ti4_content::provenance::embedded_digest().corpus),
            wasm_sha256: None,
            rules_version: initial_state.rules_version,
            rule_bot_version: Some(ti4_rule_bot::tuning::BOT_VERSION),
        }),
        check: Some(check),
        history: ExportHistory {
            decisions: history.decisions,
            redo: history.redo,
            events: as_values(&history.events),
            redo_events: as_values(&history.redo_events),
            event_counter: history.event_counter,
            revision: history.revision,
            generation: history.generation,
            batches: as_values(&history.batches),
            rng_marks: history.rng_marks,
        },
    }
}

/// The export of a game from its files in the data directory (what a restart would recover), for
/// `server feedback-save` and the like.
///
/// # Errors
/// A message when the game's records are missing or unreadable.
pub fn from_store(
    store: &crate::storage::FileGameStore,
    game_id: &str,
    build_commit: &str,
) -> Result<ReplayExport, String> {
    let init = store
        .load_player_init(game_id)
        .map_err(|e| format!("{game_id}: {e}"))?;
    let nicknames = store
        .load_player_sessions(game_id)
        .map(|sessions| sessions.nicknames)
        .unwrap_or_default();
    let lobby = store.load_player_lobby(game_id).ok().flatten();
    let history = match store.load_history(game_id).map_err(|e| e.to_string())? {
        Some(history) => history,
        None => {
            let decisions = store.load_decisions(game_id).map_err(|e| e.to_string())?;
            let events = store.load_events(game_id).map_err(|e| e.to_string())?;
            GameHistory {
                decisions,
                redo: Vec::new(),
                events,
                redo_events: Vec::new(),
                event_counter: 0,
                revision: 0,
                generation: 0,
                batches: Vec::new(),
                rng_marks: ti4_host::rng_force::RngMarks::new(),
            }
        }
    };
    let galaxy_layout = ti4_host::map::rebuild_galaxy(
        ti4_content::ContentStore::embedded(),
        &init.player_ids,
        init.seed,
        init.map_template.as_deref(),
        &init.initial_state,
    )
    .ok()
    .map(|galaxy| GalaxyLayout::from_galaxy(&galaxy, ti4_model::content_types::POK));
    let seats = init.seats.clone().unwrap_or_default();
    Ok(build(ExportInputs {
        game_id,
        history,
        seed: Some(init.seed),
        player_ids: init.player_ids.clone(),
        initial_state: &init.initial_state,
        galaxy_layout,
        seats: &seats,
        nicknames: &nicknames,
        map_template: init.map_template.clone(),
        lobby: lobby.as_ref(),
        build_commit: build_commit.to_owned(),
    }))
}
