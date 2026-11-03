//! The alpha-log round trip (package D2-1; docs/todo/D2-prereqs.md item 6): every position of every stored game of a
//! server data directory goes through `store(load(g)) == g`.
//!
//! The data directory is **always a copy** (`rsync -a --exclude feedback /var/lib/ti4-public/ $D/`, as
//! `recovery-check.sh` does): the tool opens the directory through `FileGameStore::new`, which creates directories, and
//! refuses the live one. Per game it takes the stored initial state, rebuilds the galaxy as server recovery does
//! (`storage.rs::recover_from_init`), then replays the recorded decisions with `ti4_host::replay::replay_session`
//! and checks the state after every `every`-th decision, the initial state and the state of `snapshot.json`.
//! Games of a turn-redo timeline (non-empty `rng_marks`) and the games the product refuses (a rules version below the newest,
//! Thunder's Edge content, a switched-off feature) are listed as skipped: the stored alpha games are all version 0, so the check of
//! stored games needs a data directory of the newest version (docs/D28E_RESUME.md).

use std::path::Path;

use ti4_content::ContentStore;
use ti4_content::galaxy::Galaxy;
use ti4_engine::choice::DecisionRecord;
use ti4_model::id::PlayerId;
use ti4_model::state::GameState;
use ti4_server::storage::FileGameStore;

use crate::roundtrip::Checker;

/// What was found for one game.
#[derive(Debug, Clone, Default)]
pub struct GameOutcome {
    pub id: String,
    /// Recorded decisions of the game.
    pub decisions: usize,
    /// Positions checked (initial state, every K-th decision, the snapshot).
    pub positions: usize,
    /// Why the game was not checked.
    pub skipped: Option<String>,
    pub rules_version: u32,
}

/// The initial record of a stored game, from the envelope the server writes or the plain record of the tests.
struct Init {
    seed: Option<u64>,
    player_ids: Vec<PlayerId>,
    initial_state: GameState,
    map_template: Option<String>,
    start_from: Option<String>,
}

fn load_init(storage: &FileGameStore, id: &str) -> Result<Init, String> {
    match storage.load_player_init(id) {
        Ok(rec) => Ok(Init {
            seed: Some(rec.seed),
            player_ids: rec.player_ids,
            initial_state: rec.initial_state,
            map_template: rec.map_template,
            start_from: rec.start_from,
        }),
        Err(first) => match storage.load_init(id) {
            Ok(rec) => Ok(Init {
                seed: rec.seed,
                player_ids: rec.player_ids,
                initial_state: rec.initial_state,
                map_template: rec.map_template,
                start_from: None,
            }),
            Err(second) => Err(format!("init.json: {first}; {second}")),
        },
    }
}

/// The galaxy as `recover_from_init` builds it.
fn galaxy_of(init: &Init) -> Result<Galaxy, String> {
    let content = ContentStore::embedded();
    if let Some(scenario) = &init.start_from {
        return ti4_host::view::scenario::start(content, scenario, &init.player_ids, init.seed.unwrap_or_default())
            .map(|(_, g)| g);
    }
    let seed = init.seed.ok_or("no seed: the map cannot be rebuilt")?;
    ti4_host::map::rebuild_galaxy(content, &init.player_ids, seed, init.map_template.as_deref(), &init.initial_state)
}

/// Why a stored game is outside the checked set, if it is: the product refuses it
/// (`ti4_host::disabled::check_playable`: a rules version below the newest, which is every alpha game of
/// 2026-10-12, Thunder's Edge content, a dropped faction or feature). Reported as refused, not as a difference.
fn skip_reason(state: &GameState) -> Option<String> {
    ti4_host::disabled::check_playable(state).err().map(|why| format!("refused: {why}"))
}

/// Refuse the live data directory: the tool reads a copy.
///
/// # Errors
/// `dir` is `/var/lib/ti4-public` itself (or resolves to it).
pub fn refuse_live_dir(dir: &Path) -> Result<(), String> {
    let live = Path::new("/var/lib/ti4-public");
    let (Ok(a), Ok(b)) = (dir.canonicalize(), live.canonicalize()) else { return Ok(()) };
    if a == b {
        return Err("refusing the live alpha directory: copy it first (rsync -a --exclude feedback /var/lib/ti4-public/ $D/)".to_owned());
    }
    Ok(())
}

/// Check one stored game; positions go into `checker` (use a fresh one per game for per-game numbers).
///
/// # Errors
/// A game that cannot be read or replayed (that is a failure of the tool run, not a skip).
pub fn check_game(storage: &FileGameStore, id: &str, every: usize, checker: &mut Checker) -> Result<GameOutcome, String> {
    let mut out = GameOutcome { id: id.to_owned(), ..GameOutcome::default() };
    let init = load_init(storage, id)?;
    out.rules_version = init.initial_state.rules_version;
    let history = storage.load_history(id).map_err(|e| format!("history.json: {e}"))?;
    if history.as_ref().is_some_and(|h| !h.rng_marks.is_empty()) {
        out.skipped = Some("turn-redo timeline (rng_marks; switched off)".to_owned());
        return Ok(out);
    }
    if let Some(why) = skip_reason(&init.initial_state) {
        out.skipped = Some(why);
        return Ok(out);
    }
    let decisions: Vec<DecisionRecord> = match &history {
        Some(h) => h.decisions.clone(),
        None => storage.load_decisions(id).map_err(|e| format!("decisions.jsonl: {e}"))?,
    };
    out.decisions = decisions.len();
    let galaxy = galaxy_of(&init)?;
    checker.new_game();
    let every = every.max(1);
    let mut k = 0;
    loop {
        let report = ti4_host::replay::replay_session(&init.initial_state, Some(&galaxy), &decisions[..k])
            .map_err(|e| format!("replay of {k} decisions: {e:?}"))?;
        checker.check_position(&format!("{id} after {k} decisions"), &report.final_state, &galaxy);
        out.positions += 1;
        if k == decisions.len() {
            break;
        }
        k = (k + every).min(decisions.len());
    }
    // The snapshot the server keeps beside the log: also a position of the game.
    let snapshot = storage.game_dir(id).map_err(|e| e.to_string())?.join("snapshot.json");
    if let Ok(bytes) = std::fs::read(&snapshot)
        && let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes)
        && let Some(state) = value.get("payload").unwrap_or(&value).get("state")
        && let Ok(state) = serde_json::from_value::<GameState>(state.clone())
    {
        checker.check_position(&format!("{id} snapshot"), &state, &galaxy);
        out.positions += 1;
    }
    Ok(out)
}

/// The stored games of a data directory, in id order.
///
/// # Errors
/// The directory cannot be listed.
pub fn list_games(storage: &FileGameStore) -> Result<Vec<String>, String> {
    let mut ids = storage.list_saved_games().map_err(|e| e.to_string())?;
    ids.sort();
    Ok(ids)
}
