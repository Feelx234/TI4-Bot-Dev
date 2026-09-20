//! Authoritative game session vertical slice.

pub mod decider;
pub mod registry;
pub mod transport;
pub mod worker;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, mpsc};
use std::thread::JoinHandle;

use ti4_engine::choice::DecisionRecord;
use ti4_engine::fingerprint::CanonicalHash;
use ti4_model::id::PlayerId;
use ti4_model::state::GameState;

use crate::protocol::server::{ActionAcceptedMsg, InitialSnapshotMsg, ServerMessage};
use crate::protocol::status::{RejectionReason, ViewerRole};
use crate::session::decider::ChoiceSubmission;
use crate::session::worker::{SessionShared, Subscriber, spawn_session_worker};

pub use decider::RemoteHumanDecider;
pub use registry::GameRegistry;
pub use transport::MockClient;

/// Configuration for the controller occupying a table seat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SeatController {
    /// Remote human seat routed through channels to network/mock clients.
    Human,
    /// Automated bot always taking the first offered option.
    BotFirstOption,
    /// Automated bot declining when possible, otherwise taking first option.
    BotAlwaysDecline,
    /// Automated bot following a scripted sequence of option IDs.
    BotScripted(Vec<String>),
}

/// Configuration used to spawn an authoritative game session.
#[derive(Debug, Clone)]
pub struct SessionConfig {
    pub game_id: String,
    pub state: GameState,
    pub seats: BTreeMap<PlayerId, SeatController>,
    pub galaxy: Option<ti4_content::galaxy::Galaxy>,
    pub map_tiles: Vec<crate::protocol::view::BoardTileView>,
}

impl SessionConfig {
    #[must_use]
    pub fn new(game_id: impl Into<String>, state: GameState) -> Self {
        Self {
            game_id: game_id.into(),
            state,
            seats: BTreeMap::new(),
            galaxy: None,
            map_tiles: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_seat(mut self, seat: PlayerId, controller: SeatController) -> Self {
        self.seats.insert(seat, controller);
        self
    }

    #[must_use]
    pub fn with_galaxy(
        mut self,
        galaxy: ti4_content::galaxy::Galaxy,
        map_tiles: Vec<crate::protocol::view::BoardTileView>,
    ) -> Self {
        self.galaxy = Some(galaxy);
        self.map_tiles = map_tiles;
        self
    }
}

/// Handle to an active authoritative game session.
pub struct GameSession {
    game_id: String,
    shared: Arc<Mutex<SessionShared>>,
    worker_handle: Mutex<Option<JoinHandle<()>>>,
}

impl GameSession {
    /// Starts a new session worker thread with the given configuration.
    #[must_use]
    pub fn start(config: SessionConfig) -> Self {
        let game_id = config.game_id.clone();
        let (shared, handle) = spawn_session_worker(config);

        Self {
            game_id,
            shared,
            worker_handle: Mutex::new(Some(handle)),
        }
    }

    /// Returns the session's unique game ID.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.game_id
    }

    /// Submits a choice for an active pending decision.
    ///
    /// Validates seat authentication, routes to that seat's inbox, and awaits validation.
    ///
    /// # Errors
    ///
    /// Returns [`RejectionReason`] if:
    /// - No choice is pending (`NoPendingChoice`).
    /// - The submitting seat does not match the active actor (`UnauthorizedSeat`).
    /// - The expected game version does not match (`StaleVersion`).
    /// - The nonce does not match (`StaleNonce`).
    /// - The option ID is not in the offered set (`UnknownOption`).
    pub fn submit_choice(
        &self,
        seat: &PlayerId,
        nonce: &str,
        expected_version: u64,
        option_id: &str,
    ) -> Result<ActionAcceptedMsg, RejectionReason> {
        let (tx, rx) = mpsc::channel();
        let submission = ChoiceSubmission {
            seat: seat.clone(),
            nonce: nonce.to_owned(),
            expected_version,
            option_id: option_id.to_owned(),
            reply_tx: tx,
        };

        let target_inbox = {
            let lock = self.shared.lock().expect("shared lock");
            let Some(pending) = &lock.pending_decision else {
                return Err(RejectionReason::NoPendingChoice);
            };

            if &pending.seat != seat {
                return Err(RejectionReason::UnauthorizedSeat {
                    seat: Some(seat.clone()),
                });
            }

            lock.seat_inboxes.get(seat).cloned()
        };

        let Some(inbox) = target_inbox else {
            return Err(RejectionReason::UnauthorizedSeat {
                seat: Some(seat.clone()),
            });
        };

        inbox
            .send(submission)
            .map_err(|_| RejectionReason::NoPendingChoice)?;

        rx.recv().unwrap_or(Err(RejectionReason::NoPendingChoice))
    }

    /// Subscribes a viewer role to receive live server messages.
    #[must_use]
    pub fn subscribe(&self, viewer: ViewerRole) -> mpsc::Receiver<ServerMessage> {
        let (tx, rx) = mpsc::channel();
        let mut lock = self.shared.lock().expect("shared lock");
        lock.subscribers.push(Subscriber { viewer, tx });
        rx
    }

    /// Fetches an initial snapshot for the given viewer role.
    #[must_use]
    pub fn get_snapshot(&self, viewer: &ViewerRole) -> InitialSnapshotMsg {
        let lock = self.shared.lock().expect("shared lock");
        let pending = lock
            .pending_decision
            .as_ref()
            .map(|p| (&p.choice, p.nonce.as_str()));

        crate::projection::project_initial_snapshot_with_map(
            &self.game_id,
            lock.game_version,
            &lock.latest_state,
            viewer,
            pending,
            &lock.map_tiles,
        )
    }

    /// Returns static board tiles for the game session.
    #[must_use]
    pub fn map_tiles(&self) -> Vec<crate::protocol::view::BoardTileView> {
        let lock = self.shared.lock().expect("shared lock");
        lock.map_tiles.clone()
    }

    /// Returns the currently pending decision details: `(seat, nonce, version)`.
    #[must_use]
    pub fn current_pending_decision(&self) -> Option<(PlayerId, String, u64)> {
        let lock = self.shared.lock().expect("shared lock");
        lock.pending_decision
            .as_ref()
            .map(|p| (p.seat.clone(), p.nonce.clone(), p.game_version))
    }

    /// Returns a clone of the authoritative latest `GameState`.
    #[must_use]
    pub fn current_state(&self) -> GameState {
        self.shared
            .lock()
            .expect("shared lock")
            .latest_state
            .clone()
    }

    /// Returns whether the game has finished.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.shared.lock().expect("shared lock").finished
    }

    /// Returns a copy of the accumulated decision log records.
    #[must_use]
    pub fn decision_log(&self) -> Vec<DecisionRecord> {
        self.shared
            .lock()
            .expect("shared lock")
            .decision_log
            .clone()
    }

    /// Returns the canonical hashes of all recorded decisions.
    #[must_use]
    pub fn decision_hashes(&self) -> Vec<CanonicalHash> {
        self.shared.lock().expect("shared lock").decision_hashes()
    }

    /// Returns the accumulated game event logs.
    #[must_use]
    pub fn events(&self) -> Vec<String> {
        self.shared.lock().expect("shared lock").events.clone()
    }

    /// Stops the worker thread cleanly.
    pub fn stop(&self) {
        {
            let mut lock = self.shared.lock().expect("shared lock");
            lock.stopped = true;
            // Dropping inboxes unblocks any waiting RemoteHumanDecider
            lock.seat_inboxes.clear();
        }

        let mut handle_lock = self.worker_handle.lock().expect("worker handle lock");
        if let Some(handle) = handle_lock.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for GameSession {
    fn drop(&mut self) {
        self.stop();
    }
}
