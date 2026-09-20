//! In-memory registry for active game sessions.

use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};
use ti4_model::id::PlayerId;

use crate::session::{GameSession, SessionConfig};

/// Summary of an active or completed game session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameSummary {
    pub game_id: String,
    pub is_finished: bool,
    pub pending_seat: Option<PlayerId>,
}

/// Thread-safe in-memory registry of active game sessions.
#[derive(Default)]
pub struct GameRegistry {
    sessions: RwLock<BTreeMap<String, Arc<GameSession>>>,
}

impl GameRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates and starts a new game session.
    ///
    /// # Errors
    ///
    /// Returns error if a game with the same ID already exists.
    pub fn create_game(&self, config: SessionConfig) -> Result<Arc<GameSession>, String> {
        let mut lock = self.sessions.write().expect("registry write lock");
        if lock.contains_key(&config.game_id) {
            return Err(format!("Game session '{}' already exists", config.game_id));
        }

        let session = Arc::new(GameSession::start(config));
        lock.insert(session.id().to_owned(), session.clone());
        Ok(session)
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
