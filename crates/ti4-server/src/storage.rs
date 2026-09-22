//! Local filesystem storage for durable game persistence, append-only decision logging,
//! and crash recovery.

use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use ti4_content::ContentStore;
use ti4_engine::choice::DecisionRecord;
use ti4_model::id::PlayerId;
use ti4_model::state::GameState;

use crate::protocol::server::GameEvent;
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
    #[error("Invalid game ID '{0}'")]
    InvalidGameId(String),
    #[error("Corrupt JSONL record in {path} at line {line}: {message}")]
    CorruptLog {
        path: PathBuf,
        line: usize,
        message: String,
    },
    #[error("Recovery replay canonical hashes differ for game '{0}'")]
    HashMismatch(String),
    #[error("Persistence record in {path} exceeds the {limit}-byte limit")]
    Oversized { path: PathBuf, limit: usize },
    #[error("Unsupported persistence format version {0}")]
    UnsupportedFormat(u16),
    #[error("Persistence identity mismatch for {field}")]
    IdentityMismatch { field: &'static str },
    #[error("Persistence checksum mismatch")]
    ChecksumMismatch,
    #[error("Snapshot state does not match replay for game '{0}'")]
    SnapshotMismatch(String),
}

const PERSISTENCE_FORMAT_VERSION: u16 = 1;
const MAX_INIT_BYTES: usize = 4 * 1024 * 1024;
const MAX_LOG_BYTES: usize = 64 * 1024 * 1024;
const MAX_LOG_RECORD_BYTES: usize = 64 * 1024;
const MAX_SNAPSHOT_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedEnvelope<T> {
    format_version: u16,
    content_identity: String,
    rules_identity: String,
    payload: T,
    checksum: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SessionSnapshot {
    decision_count: usize,
    state: GameState,
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
    pub seat_tokens: BTreeMap<PlayerId, String>,
    #[serde(default)]
    pub map_tiles: Vec<BoardTileView>,
}

/// Persisted lifecycle state for a game that has not yet entered the engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PersistedLobbyPhase {
    Lobby,
    Running,
}

/// Persisted configuration for one lobby seat.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedLobbySeat {
    pub controller: SeatController,
    pub ready: bool,
    pub seat_token: Option<String>,
    #[serde(default)]
    pub lease_expires_at_ms: Option<u64>,
}

/// Durable pre-game metadata. Engine state is intentionally absent until start succeeds.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LobbyRecord {
    pub game_id: String,
    pub phase: PersistedLobbyPhase,
    pub host_seat: PlayerId,
    pub player_ids: Vec<PlayerId>,
    pub seats: BTreeMap<PlayerId, PersistedLobbySeat>,
    pub seed: u64,
    pub lobby_version: u64,
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
    pub fn game_dir(&self, game_id: &str) -> Result<PathBuf, StorageError> {
        validate_game_id(game_id)?;
        Ok(self.base_dir.join(game_id))
    }

    /// Saves the initial game configuration atomically to `init.json`.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] if serialization or filesystem write fails.
    pub fn save_init(&self, record: &GameInitRecord) -> Result<(), StorageError> {
        let dir = self.game_dir(&record.game_id)?;
        fs::create_dir_all(&dir)?;
        let path = dir.join("init.json");
        atomic_write_json(&path, record)?;
        Ok(())
    }

    /// Saves lobby metadata atomically before a game session exists.
    pub fn save_lobby(&self, record: &LobbyRecord) -> Result<(), StorageError> {
        let dir = self.game_dir(&record.game_id)?;
        fs::create_dir_all(&dir)?;
        atomic_write_json(&dir.join("lobby.json"), record)?;
        Ok(())
    }

    /// Loads durable lobby metadata when present.
    pub fn load_lobby(&self, game_id: &str) -> Result<Option<LobbyRecord>, StorageError> {
        let path = self.game_dir(game_id)?.join("lobby.json");
        if !path.exists() {
            return Ok(None);
        }
        let record: LobbyRecord = read_json_file(&path, MAX_INIT_BYTES)?;
        if record.game_id != game_id {
            return Err(StorageError::IdentityMismatch { field: "game_id" });
        }
        Ok(Some(record))
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
        let dir = self.game_dir(game_id)?;
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
    pub fn append_event(&self, game_id: &str, event: &GameEvent) -> Result<(), StorageError> {
        let dir = self.game_dir(game_id)?;
        fs::create_dir_all(&dir)?;
        let path = dir.join("events.jsonl");
        append_json_line(&path, event)?;
        Ok(())
    }

    /// Atomically stores a bounded replay-validation snapshot.
    pub fn save_snapshot(
        &self,
        game_id: &str,
        decision_count: usize,
        state: &GameState,
    ) -> Result<(), StorageError> {
        let dir = self.game_dir(game_id)?;
        fs::create_dir_all(&dir)?;
        atomic_write_json(
            &dir.join("snapshot.json"),
            &SessionSnapshot {
                decision_count,
                state: state.clone(),
            },
        )?;
        Ok(())
    }

    /// Loads the initial configuration for a game from `init.json`.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] if reading or parsing fails.
    pub fn load_init(&self, game_id: &str) -> Result<GameInitRecord, StorageError> {
        let path = self.game_dir(game_id)?.join("init.json");
        if !path.exists() {
            return Err(StorageError::NotFound(game_id.to_owned()));
        }
        let record: GameInitRecord = read_json_file(&path, MAX_INIT_BYTES)?;
        if record.game_id != game_id {
            return Err(StorageError::IdentityMismatch { field: "game_id" });
        }
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
        let path = self.game_dir(game_id)?.join("decisions.jsonl");
        read_json_lines(&path)
    }

    /// Loads all recorded events from `events.jsonl`.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] on I/O error.
    pub fn load_events(&self, game_id: &str) -> Result<Vec<GameEvent>, StorageError> {
        let path = self.game_dir(game_id)?.join("events.jsonl");
        read_json_lines(&path)
    }

    fn load_snapshot(&self, game_id: &str) -> Result<Option<SessionSnapshot>, StorageError> {
        let path = self.game_dir(game_id)?.join("snapshot.json");
        if !path.exists() {
            return Ok(None);
        }
        Ok(Some(read_json_file(&path, MAX_SNAPSHOT_BYTES)?))
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

    /// Lists lobby IDs, including lobbies that have subsequently started.
    pub fn list_saved_lobbies(&self) -> Result<Vec<String>, StorageError> {
        let mut games = Vec::new();
        for entry in fs::read_dir(&self.base_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() && entry.path().join("lobby.json").exists() {
                let name = entry.file_name();
                if let Some(s) = name.to_str() {
                    games.push(s.to_owned());
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
            return Err(StorageError::HashMismatch(game_id.to_owned()));
        }

        if let Some(snapshot) = self.load_snapshot(game_id)? {
            if snapshot.decision_count > decisions.len() {
                return Err(StorageError::SnapshotMismatch(game_id.to_owned()));
            }
            let snapshot_replay = replay_session(
                &init_record.initial_state,
                galaxy.as_ref(),
                &decisions[..snapshot.decision_count],
            )
            .map_err(|source| StorageError::Replay {
                game_id: game_id.to_owned(),
                source,
            })?;
            if state_checksum(&snapshot_replay.final_state)? != state_checksum(&snapshot.state)? {
                return Err(StorageError::SnapshotMismatch(game_id.to_owned()));
            }
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
        if !init_record.seat_tokens.is_empty() {
            config.seat_tokens = init_record.seat_tokens;
        }

        let session = GameSession::start_recovered(config, decisions, events);

        Ok(session)
    }
}

fn atomic_write_json<T: Serialize + Clone>(path: &Path, value: &T) -> std::io::Result<()> {
    let tmp_path = path.with_extension("tmp");
    let file = File::create(&tmp_path)?;
    let mut writer = std::io::BufWriter::new(file);
    let envelope = persist(value)?;
    serde_json::to_writer_pretty(&mut writer, &envelope)?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    writer.get_ref().sync_all()?;
    drop(writer);
    fs::rename(&tmp_path, path)?;
    Ok(())
}

fn append_json_line<T: Serialize + Clone>(path: &Path, value: &T) -> std::io::Result<()> {
    let file = OpenOptions::new().create(true).append(true).open(path)?;
    let mut writer = std::io::BufWriter::new(file);
    let envelope = persist(value)?;
    serde_json::to_writer(&mut writer, &envelope)?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    writer.get_ref().sync_data()?;
    Ok(())
}

/// Validates that a game ID is one bounded, portable filesystem component.
pub fn validate_game_id(game_id: &str) -> Result<(), StorageError> {
    const MAX_GAME_ID_BYTES: usize = 64;
    let valid = !game_id.is_empty()
        && game_id.len() <= MAX_GAME_ID_BYTES
        && game_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'));
    if valid {
        Ok(())
    } else {
        Err(StorageError::InvalidGameId(game_id.to_owned()))
    }
}

fn read_json_file<T: Serialize + for<'de> Deserialize<'de>>(
    path: &Path,
    limit: usize,
) -> Result<T, StorageError> {
    let bytes = read_bounded(path, limit)?;
    let envelope = serde_json::from_slice(&bytes)?;
    validate_envelope(envelope)
}

fn read_json_lines<T: Serialize + for<'de> Deserialize<'de>>(
    path: &Path,
) -> Result<Vec<T>, StorageError> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let bytes = read_bounded(path, MAX_LOG_BYTES)?;
    let has_complete_final_line = bytes.last() == Some(&b'\n');
    let text = std::str::from_utf8(&bytes).map_err(|error| StorageError::CorruptLog {
        path: path.to_owned(),
        line: 1,
        message: error.to_string(),
    })?;
    let mut items = Vec::new();
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    for (idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.len() > MAX_LOG_RECORD_BYTES {
            return Err(StorageError::Oversized {
                path: path.to_owned(),
                limit: MAX_LOG_RECORD_BYTES,
            });
        }
        let parsed = serde_json::from_str::<PersistedEnvelope<T>>(trimmed)
            .map_err(StorageError::from)
            .and_then(validate_envelope);
        match parsed {
            Ok(item) => items.push(item),
            Err(_error) if idx + 1 == lines.len() && !has_complete_final_line => {
                // A power loss may leave exactly the final append torn; all earlier records are durable.
                break;
            }
            Err(error) => {
                return Err(StorageError::CorruptLog {
                    path: path.to_owned(),
                    line: idx + 1,
                    message: error.to_string(),
                });
            }
        }
    }
    Ok(items)
}

fn read_bounded(path: &Path, limit: usize) -> Result<Vec<u8>, StorageError> {
    if fs::metadata(path)?.len() > limit as u64 {
        return Err(StorageError::Oversized {
            path: path.to_owned(),
            limit,
        });
    }
    Ok(fs::read(path)?)
}

fn persist<T: Serialize + Clone>(payload: &T) -> Result<PersistedEnvelope<T>, serde_json::Error> {
    let mut envelope = PersistedEnvelope {
        format_version: PERSISTENCE_FORMAT_VERSION,
        content_identity: content_identity(),
        rules_identity: env!("CARGO_PKG_VERSION").to_owned(),
        payload: payload.clone(),
        checksum: String::new(),
    };
    envelope.checksum = envelope_checksum(&envelope)?;
    Ok(envelope)
}

fn validate_envelope<T: Serialize>(envelope: PersistedEnvelope<T>) -> Result<T, StorageError> {
    if envelope.format_version != PERSISTENCE_FORMAT_VERSION {
        return Err(StorageError::UnsupportedFormat(envelope.format_version));
    }
    if envelope.content_identity != content_identity() {
        return Err(StorageError::IdentityMismatch {
            field: "content_identity",
        });
    }
    if envelope.rules_identity != env!("CARGO_PKG_VERSION") {
        return Err(StorageError::IdentityMismatch {
            field: "rules_identity",
        });
    }
    if envelope_checksum(&envelope)? != envelope.checksum {
        return Err(StorageError::ChecksumMismatch);
    }
    Ok(envelope.payload)
}

fn envelope_checksum<T: Serialize>(
    envelope: &PersistedEnvelope<T>,
) -> Result<String, serde_json::Error> {
    #[derive(Serialize)]
    struct ChecksumInput<'a, T> {
        format_version: u16,
        content_identity: &'a str,
        rules_identity: &'a str,
        payload: &'a T,
    }
    let bytes = serde_json::to_vec(&ChecksumInput {
        format_version: envelope.format_version,
        content_identity: &envelope.content_identity,
        rules_identity: &envelope.rules_identity,
        payload: &envelope.payload,
    })?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn state_checksum(state: &GameState) -> Result<String, StorageError> {
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(state)?)))
}

fn content_identity() -> String {
    format!(
        "{:x}",
        Sha256::digest(include_bytes!("../../ti4-content/content/CHECKSUMS.sha256"))
    )
}
