//! What does not change in a game ([`Table`]) and how a game starts.

use std::rc::Rc;

use ti4_content::ContentStore;
use ti4_content::galaxy::Galaxy;
use ti4_engine::core_sync::Ctx;
use ti4_engine::live;
use ti4_engine::render::Render;
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::generated::PublicObjective;
use ti4_model::content_types::{ContentType, SourceSet};
use ti4_model::id::{PlayerId, SystemId};
use ti4_model::state::GameState;
use ti4_rule_bot_typed::seen_view::Owners;
use ti4_selfplay::spec::GameSpec;

use crate::position::Position;

/// The tables of one game, shared by every position of it (`Rc`).
pub struct Table {
    pub kernel: &'static Kernel,
    pub sources: SourceSet,
    /// The seat texts in seat order (`Seat(i)` is `seats[i]`).
    pub seats: Vec<String>,
    /// The faction text of every seat.
    pub factions: Vec<String>,
    /// The game seed (the rule bot's default stream seed).
    pub seed: u64,
    /// The seats as the typed rule bot's view names them.
    pub owners: Owners,
    /// The board as set up (the rule bot's view reads it; laws that change lanes later are not re-applied, as in the MCTS lab).
    pub galaxy: Galaxy,
    pub content: &'static ContentStore,
    /// The owner and text tables of the core (the materialised position of [`Position::materialized_json`]).
    pub cx: Ctx,
    /// The opening position as the string `GameState` (the fields no slice of the core carries).
    pub base: GameState,
    /// Lane C8: the public objectives this game's decks could hold, stage I then stage II (the setup's catalogue of the scope minus the
    /// objectives the map rules out, `map_feasibility`): which five of each stage were dealt is hidden, so a determinisation draws the
    /// unrevealed ones from these pools.
    pub objective_pools: [Vec<PublicObjective>; 2],
}

/// The public objectives of each stage the setup deals from (`deck::build_starting_decks_excluding`): the scope's catalogue by points,
/// minus what `map_feasibility` rules out on this board (a public rule: every seat can work it out).
fn objective_pools(content: &ContentStore, sources: SourceSet, galaxy: &Galaxy, base: &GameState) -> [Vec<PublicObjective>; 2] {
    use ti4_engine::generated::IdStr;
    use ti4_engine::map_feasibility::{census, unsatisfiable_public};
    let homes: Vec<SystemId> = base.players.iter().filter_map(|p| p.home_system.clone()).collect();
    let excluded = unsatisfiable_public(&census(content, sources, galaxy, &homes));
    let mut pools = [Vec::new(), Vec::new()];
    for record in content.from_sources(ContentType::PublicObjectives, sources) {
        let (Some(id), Some(points)) = (record.id(), record.int("points")) else { continue };
        let Some(objective) = PublicObjective::from_id(id) else { continue };
        if excluded.contains(&id) {
            continue;
        }
        match points {
            1 => pools[0].push(objective),
            2 => pools[1].push(objective),
            _ => {}
        }
    }
    for pool in &mut pools {
        pool.sort_unstable();
        pool.dedup();
    }
    pools
}

/// Set up the game of `spec` on the native setup (the product's `GameSpec::start`) and stop at its first question.
///
/// # Errors
/// A spec the native setup does not build.
pub fn start(spec: &GameSpec) -> Result<Position, String> {
    let Some(start) = spec.start()? else { return Err("not a native table (Thunder's Edge or an old rules version)".to_owned()) };
    let parts = start.into_parts().map_err(|e| e.to_string())?;
    let ti4_engine::core_setup::StartParts { core, cx, state, galaxy, seed } = parts;
    let factions: Vec<String> = cx.owners.ids().iter().map(|id| state.players.iter().find(|p| p.id.as_str() == id).map_or_else(String::new, |p| p.faction.as_str().to_owned())).collect();
    let render = Render::new(cx.owners.ids().to_vec()).with_factions(factions.clone()).with_codes(core.texts.codes.clone());
    let sources = ti4_model::content_types::POK;
    let seats = cx.owners.ids().to_vec();
    let owners = Owners::new(seats.iter().map(|s| PlayerId::new(s.as_str())).collect());
    let content = ContentStore::embedded();
    let objective_pools = objective_pools(content, sources, &galaxy, &state);
    let table = Table { kernel: live::kernel::kernel_for(sources), sources, seats, factions, seed, owners, galaxy, content, cx, base: state, objective_pools };
    Ok(Position::open(Rc::new(table), core, render))
}

/// The lobby game the server creates (the bridge's `native_play::lobby_spec`): `faction` at seat a, the other seats the next lobby factions
/// in a rotation fixed by the seed, `card_set te`, starting technology, the speaker drawn from the seed.
#[must_use]
pub fn lobby_spec(faction: &str, players: usize, seed: u64) -> GameSpec {
    let maps: &[&str] = match players {
        4 => &["4pOuterRim", "4pHyperlanes"],
        6 => &["6pStandard", "6pOuterRim"],
        _ => &["random"],
    };
    let lobby = ti4_engine::seating::LOBBY_FACTIONS;
    let mut factions = vec![faction.to_owned()];
    let mut at = usize::try_from(seed).unwrap_or(0) % lobby.len();
    while factions.len() < players {
        at = (at + 1) % lobby.len();
        let candidate = lobby[at];
        if !factions.iter().any(|f| f == candidate) {
            factions.push(candidate.to_owned());
        }
    }
    GameSpec {
        setup: format!("native-{faction}"),
        seed,
        players,
        map: maps[usize::try_from(seed).unwrap_or(0) % maps.len()].to_owned(),
        factions,
        speaker: "random".to_owned(),
        card_set: Some("te".to_owned()),
        rotation: None,
        starting_tech: true,
        thunders_edge: false,
        rules_version: None,
    }
}

/// The quick game of `native_play --quick` (4 players, seed 1, Sol at seat a): fingerprint `fd54dc6c956758bc`, 1,110 steps (int-29).
#[must_use]
pub fn quick_spec() -> GameSpec {
    lobby_spec("sol", 4, 1)
}

/// The four factions of the first MCTS setup (memory `project_mcts_bot_decisions`).
pub const MCTS_FACTIONS: [&str; 4] = ["sol", "letnev", "xxcha", "jolnar"];

/// The game of the first MCTS setup with `seed` (the MCTS lab's `spec`): the four factions in the seating `seed % 24`, the Learn-to-play
/// map, the speaker drawn from the seed, starting technology, the default card set.
#[must_use]
pub fn mcts_spec(seed: u64) -> GameSpec {
    let mut pool: Vec<&str> = MCTS_FACTIONS.to_vec();
    let mut k = usize::try_from(seed % 24).unwrap_or(0);
    let mut factions = Vec::with_capacity(4);
    for f in [6, 2, 1, 1] {
        factions.push(pool.remove(k / f).to_owned());
        k %= f;
    }
    GameSpec {
        setup: "mcts-4p-ltp".to_owned(),
        seed,
        players: 4,
        map: "4pLearnToPlayPoK".to_owned(),
        factions,
        speaker: "random".to_owned(),
        card_set: None,
        rotation: None,
        starting_tech: true,
        thunders_edge: false,
        rules_version: None,
    }
}
