//! Session worker thread managing the single authoritative `ti4_engine::Game`.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, mpsc};
use std::thread::{self, JoinHandle};

use ti4_content::ContentStore;
use ti4_engine::choice::{AlwaysDecline, Choice, DecisionRecord, FirstOption, Scripted, Table};
use ti4_engine::fingerprint::{CanonicalHash, CanonicalHashVersion, decision_hash};
use ti4_engine::game::Game;
use ti4_model::id::PlayerId;
use ti4_model::state::GameState;

use crate::projection::{project_state_update, project_turn_status};
use crate::protocol::PROTOCOL_VERSION;
use crate::protocol::choice::PendingChoiceDto;
use crate::protocol::server::{GameOverMsg, PendingChoiceMsg, ServerMessage, TurnStatusMsg};
use crate::protocol::status::ViewerRole;
use crate::session::decider::{ChoiceSubmission, RemoteHumanDecider};
use crate::session::{SeatController, SessionConfig};

/// State of a currently pending decision awaiting a human answer.
#[derive(Debug, Clone)]
pub struct PendingDecision {
    pub seat: PlayerId,
    pub nonce: String,
    pub game_version: u64,
    pub choice: Choice,
}

/// Active subscriber receiving real-time server messages.
pub struct Subscriber {
    pub viewer: ViewerRole,
    pub tx: mpsc::Sender<ServerMessage>,
}

/// Shared session state accessible across threads.
pub struct SessionShared {
    pub game_id: String,
    pub game_version: u64,
    pub latest_state: GameState,
    pub pending_decision: Option<PendingDecision>,
    pub seat_inboxes: BTreeMap<PlayerId, mpsc::Sender<ChoiceSubmission>>,
    pub subscribers: Vec<Subscriber>,
    pub decision_log: Vec<DecisionRecord>,
    pub events: Vec<String>,
    pub finished: bool,
    pub stopped: bool,
    pub error: Option<String>,
}

impl SessionShared {
    #[must_use]
    pub fn new(game_id: String, initial_state: GameState) -> Self {
        Self {
            game_id,
            game_version: 1,
            latest_state: initial_state,
            pending_decision: None,
            seat_inboxes: BTreeMap::new(),
            subscribers: Vec::new(),
            decision_log: Vec::new(),
            events: Vec::new(),
            finished: false,
            stopped: false,
            error: None,
        }
    }

    /// Broadcasts a newly raised decision to all subscribers.
    ///
    /// The actor receives the full `PendingChoiceMsg` with legal options.
    /// Opponents and spectators receive a redacted `TurnStatusMsg::WaitingForDecision`.
    pub fn broadcast_pending_decision(&mut self, choice: &Choice, nonce: &str) {
        let status = project_turn_status(&self.latest_state, Some(choice));

        self.subscribers.retain(|sub| {
            if sub.viewer.is_actor(&choice.player) {
                let choice_dto = PendingChoiceDto::from_choice(choice, nonce.to_owned(), true);
                let msg = ServerMessage::PendingChoice(PendingChoiceMsg {
                    protocol_version: PROTOCOL_VERSION,
                    game_id: self.game_id.clone(),
                    game_version: self.game_version,
                    choice: choice_dto,
                });
                sub.tx.send(msg).is_ok()
            } else {
                let msg = ServerMessage::TurnStatus(TurnStatusMsg {
                    protocol_version: PROTOCOL_VERSION,
                    game_id: self.game_id.clone(),
                    game_version: self.game_version,
                    status: status.clone(),
                });
                sub.tx.send(msg).is_ok()
            }
        });
    }

    /// Broadcasts a state update to all subscribers.
    pub fn broadcast_state_update(&mut self) {
        let pending = self
            .pending_decision
            .as_ref()
            .map(|p| (&p.choice, p.nonce.as_str()));

        let game_id = self.game_id.clone();
        let version = self.game_version;
        let state = &self.latest_state;

        self.subscribers.retain(|sub| {
            let update = project_state_update(&game_id, version, state, &sub.viewer, pending);
            sub.tx.send(ServerMessage::StateUpdate(update)).is_ok()
        });
    }

    /// Broadcasts terminal game over to all subscribers.
    pub fn broadcast_game_over(
        &mut self,
        winner: Option<PlayerId>,
        final_scores: BTreeMap<PlayerId, u32>,
    ) {
        let msg = ServerMessage::GameOver(GameOverMsg {
            protocol_version: PROTOCOL_VERSION,
            game_id: self.game_id.clone(),
            game_version: self.game_version,
            winner,
            final_scores,
        });

        self.subscribers
            .retain(|sub| sub.tx.send(msg.clone()).is_ok());
    }

    /// Returns canonical hashes of all recorded decisions.
    #[must_use]
    pub fn decision_hashes(&self) -> Vec<CanonicalHash> {
        self.decision_log
            .iter()
            .map(|record| decision_hash(CanonicalHashVersion::V1, record))
            .collect()
    }
}

/// Spawns the dedicated session worker thread for an active game session.
#[must_use]
pub fn spawn_session_worker(config: SessionConfig) -> (Arc<Mutex<SessionShared>>, JoinHandle<()>) {
    let shared = Arc::new(Mutex::new(SessionShared::new(
        config.game_id.clone(),
        config.state.clone(),
    )));

    let worker_shared = shared.clone();
    let handle = thread::spawn(move || {
        let mut table = Table::new();

        // Configure table deciders
        for (seat, controller) in config.seats {
            match controller {
                SeatController::Human => {
                    let (inbox_tx, inbox_rx) = mpsc::channel();
                    {
                        let mut lock = worker_shared.lock().expect("shared lock");
                        lock.seat_inboxes.insert(seat.clone(), inbox_tx);
                    }
                    let decider = RemoteHumanDecider::new(
                        seat.clone(),
                        config.game_id.clone(),
                        worker_shared.clone(),
                        inbox_rx,
                    );
                    table.seat(seat, Box::new(decider));
                }
                SeatController::BotFirstOption => {
                    table.seat(seat, Box::new(FirstOption));
                }
                SeatController::BotAlwaysDecline => {
                    table.seat(seat, Box::new(AlwaysDecline));
                }
                SeatController::BotScripted(script) => {
                    table.seat(seat, Box::new(Scripted::new(script)));
                }
            }
        }

        let mut game = Game::with_table(config.state, ContentStore::embedded(), table);

        loop {
            // Check stop signal
            {
                let lock = worker_shared.lock().expect("shared lock");
                if lock.stopped {
                    break;
                }
            }

            // Sync state to shared cache
            {
                let mut lock = worker_shared.lock().expect("shared lock");
                lock.latest_state = game.state.clone();
                lock.decision_log.clone_from(&game.table.log.records);
                lock.events.clone_from(&game.events);
            }

            // Check if finished
            if game.state.finished {
                let mut lock = worker_shared.lock().expect("shared lock");
                lock.finished = true;
                let winner = game
                    .state
                    .players
                    .iter()
                    .max_by_key(|p| p.victory_points)
                    .map(|p| p.id.clone());
                let mut final_scores = BTreeMap::new();
                for p in &game.state.players {
                    final_scores.insert(p.id.clone(), p.victory_points.max(0).cast_unsigned());
                }
                lock.broadcast_game_over(winner, final_scores);
                break;
            }

            // Step the engine (will block if human decision is required)
            let result = game.step();

            // Record outcome
            {
                let mut lock = worker_shared.lock().expect("shared lock");
                lock.latest_state = game.state.clone();
                lock.decision_log.clone_from(&game.table.log.records);
                lock.events.clone_from(&game.events);

                if let Some(err) = result.error {
                    lock.error = Some(err.to_string());
                    break;
                }

                lock.game_version += 1;
                lock.broadcast_state_update();

                if result.finished {
                    lock.finished = true;
                    let winner = game
                        .state
                        .players
                        .iter()
                        .max_by_key(|p| p.victory_points)
                        .map(|p| p.id.clone());
                    let mut final_scores = BTreeMap::new();
                    for p in &game.state.players {
                        final_scores.insert(p.id.clone(), p.victory_points.max(0).cast_unsigned());
                    }
                    lock.broadcast_game_over(winner, final_scores);
                    break;
                }
            }
        }
    });

    (shared, handle)
}
