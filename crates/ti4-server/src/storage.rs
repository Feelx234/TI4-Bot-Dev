//! Local filesystem storage for durable game persistence, append-only decision logging,
//! and crash recovery.

use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use ti4_content::ContentStore;
use ti4_engine::choice::DecisionRecord;
use ti4_model::id::PlayerId;
use ti4_model::state::GameState;

use crate::protocol::server::GameEventDto;
use crate::protocol::view::BoardTileView;
use crate::session::replay::replay_session;
use crate::session::{GameSession, SeatController, SessionConfig};

/// Errors encountered during game persistence or recovery.
#[derive(Debug, Error)]
pub enum StorageError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Replay error during recovery of game '{game_id}': {source}")]
    Replay {
        game_id: String,
        source: crate::session::replay::ReplayError,
    },
    #[error("Game '{0}' not found in storage")]
    NotFound(String),
    #[error("Seating/map error: {0}")]
    Map(String),
}

/// Initial configuration record saved atomically to `init.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameInitRecord {
    pub game_id: String,
    pub seed: Option<u64>,
    pub player_ids: Vec<PlayerId>,
    pub initial_state: GameState,
    pub seats: BTreeMap<PlayerId, SeatController>,
    #[serde(default)]
    pub map_tiles: Vec<BoardTileView>,
}

/// Durable file-based game storage manager.
#[derive(Debug, Clone)]
pub struct FileGameStore {
    base_dir: PathBuf,
}

impl FileGameStore {
    /// Creates a new store rooted at `base_dir`, ensuring the directory exists.
    ///
    /// # Errors
    ///
    /// Returns [`std::io::Error`] if directory creation fails.
    pub fn new(base_dir: impl Into<PathBuf>) -> std::io::Result<Self> {
        let base_dir = base_dir.into();
        fs::create_dir_all(&base_dir)?;
        Ok(Self { base_dir })
    }

    /// Returns the filesystem path to a game's directory.
    #[must_use]
    pub fn game_dir(&self, game_id: &str) -> PathBuf {
        self.base_dir.join(game_id)
    }

    /// Saves the initial game configuration atomically to `init.json`.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] if serialization or filesystem write fails.
    pub fn save_init(&self, record: &GameInitRecord) -> Result<(), StorageError> {
        let dir = self.game_dir(&record.game_id);
        fs::create_dir_all(&dir)?;
        let path = dir.join("init.json");
        atomic_write_json(&path, record)?;
        Ok(())
    }

    /// Appends an accepted decision record to `decisions.jsonl` with fsync.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] if append or sync fails.
    pub fn append_decision(
        &self,
        game_id: &str,
        record: &DecisionRecord,
    ) -> Result<(), StorageError> {
        let dir = self.game_dir(game_id);
        fs::create_dir_all(&dir)?;
        let path = dir.join("decisions.jsonl");
        append_json_line(&path, record)?;
        Ok(())
    }

    /// Appends an authoritative game event to `events.jsonl` with fsync.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] if append or sync fails.
    pub fn append_event(&self, game_id: &str, event: &GameEventDto) -> Result<(), StorageError> {
        let dir = self.game_dir(game_id);
        fs::create_dir_all(&dir)?;
        let path = dir.join("events.jsonl");
        append_json_line(&path, event)?;
        Ok(())
    }

    /// Loads the initial configuration for a game from `init.json`.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] if reading or parsing fails.
    pub fn load_init(&self, game_id: &str) -> Result<GameInitRecord, StorageError> {
        let path = self.game_dir(game_id).join("init.json");
        if !path.exists() {
            return Err(StorageError::NotFound(game_id.to_owned()));
        }
        let file = File::open(&path)?;
        let reader = BufReader::new(file);
        let record = serde_json::from_reader(reader)?;
        Ok(record)
    }

    /// Loads all recorded decisions from `decisions.jsonl`.
    ///
    /// Tolerant to an incomplete trailing write (e.g. abrupt power cut).
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] on I/O error.
    pub fn load_decisions(&self, game_id: &str) -> Result<Vec<DecisionRecord>, StorageError> {
        let path = self.game_dir(game_id).join("decisions.jsonl");
        Ok(read_json_lines(&path)?)
    }

    /// Loads all recorded events from `events.jsonl`.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] on I/O error.
    pub fn load_events(&self, game_id: &str) -> Result<Vec<GameEventDto>, StorageError> {
        let path = self.game_dir(game_id).join("events.jsonl");
        Ok(read_json_lines(&path)?)
    }

    /// Lists all game IDs found in the storage directory with an `init.json`.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] on directory traversal error.
    pub fn list_saved_games(&self) -> Result<Vec<String>, StorageError> {
        let mut games = Vec::new();
        for entry in fs::read_dir(&self.base_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                let init_path = entry.path().join("init.json");
                if init_path.exists() {
                    let name = entry.file_name();
                    if let Some(s) = name.to_str() {
                        games.push(s.to_owned());
                    }
                }
            }
        }
        games.sort();
        Ok(games)
    }

    /// Recovers a game session by loading initial state and replaying recorded decisions.
    ///
    /// The recovered session is ready to accept choices and will append future decisions
    /// to the existing store.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] if recovery fails.
    pub fn recover_session(self: &Arc<Self>, game_id: &str) -> Result<GameSession, StorageError> {
        let init_record = self.load_init(game_id)?;
        let decisions = self.load_decisions(game_id)?;
        let events = self.load_events(game_id)?;

        let content = ContentStore::embedded();
        let galaxy = if let Some(seed) = init_record.seed {
            let (_, g) = crate::map::create_game_with_map(content, &init_record.player_ids, seed)
                .map_err(|e| StorageError::Map(e.to_string()))?;
            Some(g)
        } else {
            None
        };

        let map_tiles = if init_record.map_tiles.is_empty() {
            if let Some(g) = &galaxy {
                crate::map::build_board_tiles(content, g)
            } else {
                Vec::new()
            }
        } else {
            init_record.map_tiles.clone()
        };

        // Replay all accepted decisions to reach the current recovered state
        let replay_report = replay_session(&init_record.initial_state, galaxy.as_ref(), &decisions)
            .map_err(|source| StorageError::Replay {
                game_id: game_id.to_owned(),
                source,
            })?;

        if !replay_report.hashes_match {
            tracing::warn!(
                game_id = %game_id,
                "Recovery replay canonical hashes differed from original log"
            );
        }

        // Configure session starting at initial state with prior history for live continuation
        let mut config =
            SessionConfig::new(&init_record.game_id, init_record.initial_state.clone())
                .with_store(self.clone())
                .with_prior_history(decisions.clone(), events.clone());

        if let Some(seed) = init_record.seed {
            config = config.with_seed(seed);
        }
        config = config.with_player_ids(init_record.player_ids.clone());

        if let Some(g) = galaxy.clone() {
            config = config.with_galaxy(g, map_tiles);
        }

        for (seat, controller) in &init_record.seats {
            config = config.with_seat(seat.clone(), controller.clone());
        }

        let session = GameSession::start_recovered(
            config,
            init_record.initial_state,
            galaxy,
            decisions,
            events,
        );

        Ok(session)
    }
}

fn atomic_write_json<T: Serialize>(path: &Path, value: &T) -> std::io::Result<()> {
    let tmp_path = path.with_extension("tmp");
    let file = File::create(&tmp_path)?;
    let mut writer = std::io::BufWriter::new(file);
    serde_json::to_writer_pretty(&mut writer, value)?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    writer.get_ref().sync_all()?;
    drop(writer);
    fs::rename(&tmp_path, path)?;
    Ok(())
}

fn append_json_line<T: Serialize>(path: &Path, value: &T) -> std::io::Result<()> {
    let file = OpenOptions::new().create(true).append(true).open(path)?;
    let mut writer = std::io::BufWriter::new(file);
    serde_json::to_writer(&mut writer, value)?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    writer.get_ref().sync_data()?;
    Ok(())
}

fn read_json_lines<T: for<'de> Deserialize<'de>>(path: &Path) -> std::io::Result<Vec<T>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut items = Vec::new();
    for (idx, line_res) in reader.lines().enumerate() {
        let line = line_res?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        match serde_json::from_str::<T>(trimmed) {
            Ok(item) => items.push(item),
            Err(e) => {
                tracing::warn!(
                    line = idx + 1,
                    error = %e,
                    "Skipping unparseable line in JSONL file: {}",
                    path.display()
                );
            }
        }
    }
    Ok(items)
}
