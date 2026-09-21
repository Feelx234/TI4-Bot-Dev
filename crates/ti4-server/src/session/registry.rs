//! In-memory registry for active game sessions.

use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};
use ti4_model::id::PlayerId;

use crate::session::{GameSession, SessionConfig};

/// Summary of an active or completed game session.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct GameSummary {
    pub game_id: String,
    pub is_finished: bool,
    pub pending_seat: Option<PlayerId>,
}

/// Thread-safe in-memory registry of active game sessions.
#[derive(Default)]
pub struct GameRegistry {
    sessions: RwLock<BTreeMap<String, Arc<GameSession>>>,
    store: Option<Arc<crate::storage::FileGameStore>>,
}

impl GameRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self {
            sessions: RwLock::new(BTreeMap::new()),
            store: None,
        }
    }

    #[must_use]
    pub fn with_store(mut self, store: Arc<crate::storage::FileGameStore>) -> Self {
        self.store = Some(store);
        self
    }

    /// Returns the optional underlying game store.
    #[must_use]
    pub fn store(&self) -> Option<Arc<crate::storage::FileGameStore>> {
        self.store.clone()
    }

    /// Recovers all saved games found in the storage directory into the registry.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] on filesystem or recovery failure.
    pub fn recover_all_games(&self) -> Result<Vec<String>, crate::storage::StorageError> {
        let Some(store) = &self.store else {
            return Ok(Vec::new());
        };

        let saved = store.list_saved_games()?;
        let mut recovered = Vec::new();

        for game_id in saved {
            {
                let read_lock = self.sessions.read().expect("registry read lock");
                if read_lock.contains_key(&game_id) {
                    continue;
                }
            }

            let session = Arc::new(store.recover_session(&game_id)?);
            let mut write_lock = self.sessions.write().expect("registry write lock");
            write_lock.insert(game_id.clone(), session);
            recovered.push(game_id);
        }

        Ok(recovered)
    }

    /// Creates and starts a new game session.
    ///
    /// If storage is configured, atomically writes `init.json` before starting.
    ///
    /// # Errors
    ///
    /// Returns error if a game with the same ID already exists or storage write fails.
    pub fn create_game(&self, mut config: SessionConfig) -> Result<Arc<GameSession>, String> {
        crate::storage::validate_game_id(&config.game_id).map_err(|error| error.to_string())?;
        let mut lock = self.sessions.write().expect("registry write lock");
        if lock.contains_key(&config.game_id) {
            return Err(format!("Game session '{}' already exists", config.game_id));
        }

        if config.store.is_none() {
            config.store.clone_from(&self.store);
        }

        if let Some(store) = &config.store {
            if config.player_ids.is_empty() {
                return Err("durable sessions require an explicit player order".to_owned());
            }
            let init_record = crate::storage::GameInitRecord {
                game_id: config.game_id.clone(),
                seed: config.seed,
                player_ids: config.player_ids.clone(),
                initial_state: config.state.clone(),
                seats: config.seats.clone(),
                seat_tokens: config.seat_tokens.clone(),
                map_tiles: config.map_tiles.clone(),
            };
            store
                .save_init(&init_record)
                .map_err(|e| format!("Failed to save initial game configuration: {e}"))?;
        }

        let session = Arc::new(GameSession::start(config));
        lock.insert(session.id().to_owned(), session.clone());
        Ok(session)
    }

    /// Registers a recovered game session into the registry.
    ///
    /// # Errors
    ///
    /// Returns error if a game with the same ID already exists.
    pub fn register_recovered(&self, session: Arc<GameSession>) -> Result<(), String> {
        let mut lock = self.sessions.write().expect("registry write lock");
        if lock.contains_key(session.id()) {
            return Err(format!("Game session '{}' already exists", session.id()));
        }
        lock.insert(session.id().to_owned(), session);
        Ok(())
    }

    /// Looks up an active game session by ID.
    #[must_use]
    pub fn get_game(&self, game_id: &str) -> Option<Arc<GameSession>> {
        let lock = self.sessions.read().expect("registry read lock");
        lock.get(game_id).cloned()
    }

    /// Returns summaries for all registered game sessions.
    #[must_use]
    pub fn list_games(&self) -> Vec<GameSummary> {
        let lock = self.sessions.read().expect("registry read lock");
        lock.values()
            .map(|s| {
                let pending_seat = s.current_pending_decision().map(|(seat, _, _)| seat);
                GameSummary {
                    game_id: s.id().to_owned(),
                    is_finished: s.is_finished(),
                    pending_seat,
                }
            })
            .collect()
    }

    /// Removes and stops a game session from the registry.
    pub fn remove_game(&self, game_id: &str) -> Option<Arc<GameSession>> {
        let mut lock = self.sessions.write().expect("registry write lock");
        let session = lock.remove(game_id);
        if let Some(ref s) = session {
            s.stop();
        }
        session
    }
}
