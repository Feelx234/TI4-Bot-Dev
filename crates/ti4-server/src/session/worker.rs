//! Session worker thread managing the single authoritative `ti4_engine::Game`.

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex, mpsc};
use std::thread::{self, JoinHandle};

use ti4_content::ContentStore;
use ti4_engine::choice::{
    AlwaysDecline, Choice, ChoiceOption, Decider, DecisionRecord, FirstOption, IllegalChoice,
    Scripted, SeatObservation, Table,
};
use ti4_engine::fingerprint::{CanonicalHash, CanonicalHashVersion, decision_hash};
use ti4_engine::game::Game;
use ti4_model::id::PlayerId;
use ti4_model::state::GameState;

use crate::projection::project_turn_status;
use crate::protocol::PROTOCOL_VERSION;
use crate::protocol::choice::PendingChoiceDto;
use crate::protocol::server::{
    EventVisibility, GameEvent, GameEventKind, GameOverMsg, PendingChoiceMsg, ServerMessage,
    TurnStatusMsg,
};
use crate::protocol::status::ViewerRole;
use crate::session::decider::{ChoiceSubmission, RemoteHumanDecider};
use crate::session::{SeatController, SessionConfig};
use crate::storage::FileGameStore;

/// State of a currently pending decision awaiting a human answer.
#[derive(Debug, Clone)]
pub struct PendingDecision {
    pub seat: PlayerId,
    pub nonce: String,
    pub game_version: u64,
    pub choice: Choice,
    pub submission_state: PendingSubmissionState,
    pub reply_tx: Option<
        mpsc::Sender<
            Result<
                crate::protocol::server::ActionAcceptedMsg,
                crate::protocol::status::RejectionReason,
            >,
        >,
    >,
}

/// Lifecycle of an engine-generated choice while the worker waits for a client submission.
///
/// A submission is reserved before it enters the worker inbox so concurrent requests cannot
/// advance the same engine choice twice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingSubmissionState {
    AwaitingSubmission,
    Reserved,
}

/// Active subscriber receiving real-time server messages.
pub struct Subscriber {
    pub viewer: ViewerRole,
    pub tx: mpsc::SyncSender<ServerMessage>,
}

/// Shared session state accessible across threads.
pub struct SessionShared {
    pub game_id: String,
    pub game_version: u64,
    pub latest_state: GameState,
    pub pending_decision: Option<PendingDecision>,
    pub seat_inboxes: BTreeMap<PlayerId, mpsc::Sender<ChoiceSubmission>>,
    pub seat_tokens: BTreeMap<PlayerId, String>,
    pub subscribers: BTreeMap<u64, Subscriber>,
    pub next_subscriber_id: u64,
    pub decision_log: Vec<DecisionRecord>,
    pub finished: bool,
    pub stopped: bool,
    pub error: Option<String>,
    pub map_tiles: Vec<crate::protocol::view::BoardTileView>,
    pub event_log: Vec<GameEvent>,
    pub event_counter: u64,
    pub store: Option<Arc<FileGameStore>>,
    pub snapshot_decision_count: usize,
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
            seat_tokens: BTreeMap::new(),
            subscribers: BTreeMap::new(),
            next_subscriber_id: 0,
            decision_log: Vec::new(),
            event_log: Vec::new(),
            event_counter: 0,
            finished: false,
            stopped: false,
            error: None,
            map_tiles: Vec::new(),
            store: None,
            snapshot_decision_count: 0,
        }
    }

    fn publish(&mut self, message: impl Fn(&ViewerRole) -> ServerMessage) {
        self.subscribers
            .retain(|_, subscriber| subscriber.tx.try_send(message(&subscriber.viewer)).is_ok());
    }

    /// Records an authoritative event and broadcasts it to all subscribers.
    pub fn record_and_broadcast_event(
        &mut self,
        visibility: EventVisibility,
        event: GameEventKind,
        version: Option<u64>,
    ) -> Result<(), String> {
        self.event_counter += 1;
        let id = format!("{}-{}", self.game_id, self.event_counter);
        let timestamp = current_utc_time_string();
        let entry = GameEvent {
            id,
            timestamp,
            version,
            visibility,
            event,
        };
        if let Some(store) = &self.store {
            store
                .append_event(&self.game_id, &entry)
                .map_err(|error| format!("failed to persist event: {error}"))?;
        }

        self.event_log.push(entry.clone());

        let visible = entry.visibility.clone();
        let msg = ServerMessage::Event(crate::protocol::server::GameEventMsg {
            protocol_version: PROTOCOL_VERSION,
            game_id: self.game_id.clone(),
            entry,
        });

        self.subscribers.retain(|_, subscriber| {
            !visible.permits(&subscriber.viewer) || subscriber.tx.try_send(msg.clone()).is_ok()
        });
        Ok(())
    }

    /// Broadcasts a newly raised decision to all subscribers.
    ///
    /// The actor receives the full `PendingChoiceMsg` with legal options.
    /// Opponents and spectators receive a redacted `TurnStatusMsg::WaitingForDecision`.
    pub fn broadcast_pending_decision(&mut self, choice: &Choice, nonce: &str) {
        let status = project_turn_status(&self.latest_state, Some(choice));
        let game_id = self.game_id.clone();
        let game_version = self.game_version;

        self.publish(|viewer| {
            if viewer.is_actor(&choice.player) {
                let choice_dto = PendingChoiceDto::from_choice(choice, nonce.to_owned(), true);
                ServerMessage::PendingChoice(PendingChoiceMsg {
                    protocol_version: PROTOCOL_VERSION,
                    game_id: game_id.clone(),
                    game_version,
                    choice: choice_dto,
                })
            } else {
                ServerMessage::TurnStatus(TurnStatusMsg {
                    protocol_version: PROTOCOL_VERSION,
                    game_id: game_id.clone(),
                    game_version,
                    status: status.clone(),
                })
            }
        });
    }

    /// Broadcasts a state update to all subscribers.
    pub fn broadcast_state_update(&mut self) {
        let pending = self
            .pending_decision
            .as_ref()
            .map(|p| (p.choice.clone(), p.nonce.clone()));

        let game_id = self.game_id.clone();
        let version = self.game_version;
        let state = self.latest_state.clone();
        let map_tiles = self.map_tiles.clone();
        self.publish(|viewer| {
            let update = crate::projection::project_state_update_with_map(
                &game_id,
                version,
                &state,
                viewer,
                pending
                    .as_ref()
                    .map(|(choice, nonce)| (choice, nonce.as_str())),
                &map_tiles,
            );
            ServerMessage::StateUpdate(update)
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

        self.publish(|_| msg.clone());
    }

    /// Records and broadcasts every terminal-game consequence through one path.
    fn finish_session(&mut self, game: &Game) -> Result<(), String> {
        self.finished = true;
        let winner = game
            .state
            .players
            .iter()
            .max_by_key(|player| player.victory_points)
            .map(|player| player.id.clone());
        let final_scores = game
            .state
            .players
            .iter()
            .map(|player| {
                (
                    player.id.clone(),
                    player.victory_points.max(0).cast_unsigned(),
                )
            })
            .collect();
        self.record_and_broadcast_event(
            EventVisibility::Public,
            GameEventKind::GameFinished {
                winner: winner.clone(),
            },
            Some(self.game_version),
        )?;
        self.broadcast_game_over(winner, final_scores);
        Ok(())
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

struct ReplayingDecider {
    prior_queue: Arc<Mutex<VecDeque<DecisionRecord>>>,
    inner: Box<dyn Decider>,
}

impl ReplayingDecider {
    fn try_replay(&self, choice: &Choice) -> Option<Result<ChoiceOption, IllegalChoice>> {
        let next_prior = {
            let mut lock = self.prior_queue.lock().expect("prior queue lock");
            lock.pop_front()
        };

        next_prior.map(|record| {
            if let Some(opt) = choice.options.iter().find(|o| o.id == record.chosen) {
                Ok(opt.clone())
            } else {
                Err(IllegalChoice::ScriptDiverged {
                    player: choice.player.clone(),
                    wanted: record.chosen,
                    offered: choice.options.iter().map(|o| o.id.clone()).collect(),
                })
            }
        })
    }
}

impl Decider for ReplayingDecider {
    fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
        if let Some(res) = self.try_replay(choice) {
            return res;
        }
        self.inner.choose(choice)
    }

    fn choose_seeing(
        &mut self,
        choice: &Choice,
        seen: &SeatObservation<'_>,
    ) -> Result<ChoiceOption, IllegalChoice> {
        if let Some(res) = self.try_replay(choice) {
            return res;
        }
        self.inner.choose_seeing(choice, seen)
    }
}

/// Spawns the dedicated session worker thread for an active game session.
#[allow(clippy::too_many_lines)]
#[must_use]
pub fn spawn_session_worker(config: SessionConfig) -> (Arc<Mutex<SessionShared>>, JoinHandle<()>) {
    let prior_count = config.prior_decisions.len();
    let prior_queue = Arc::new(Mutex::new(VecDeque::from(config.prior_decisions.clone())));

    let mut initial_shared = SessionShared::new(config.game_id.clone(), config.state.clone());
    initial_shared.seat_tokens.clone_from(&config.seat_tokens);
    initial_shared.map_tiles.clone_from(&config.map_tiles);
    initial_shared.store.clone_from(&config.store);

    if !config.prior_events.is_empty() {
        initial_shared.event_counter = config.prior_events.len() as u64;
        let last_version = config
            .prior_events
            .iter()
            .filter_map(|e| e.version)
            .max()
            .unwrap_or(1);
        initial_shared.game_version = last_version;
        initial_shared.event_log.clone_from(&config.prior_events);
    }
    if !config.prior_decisions.is_empty() {
        initial_shared
            .decision_log
            .clone_from(&config.prior_decisions);
    }

    let shared = Arc::new(Mutex::new(initial_shared));

    let worker_shared = shared.clone();
    let handle = thread::spawn(move || {
        let mut table = Table::new();

        // Configure table deciders
        for (seat, controller) in config.seats {
            let inner: Box<dyn Decider> = match controller {
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
                    Box::new(decider)
                }
                SeatController::BotFirstOption => Box::new(FirstOption),
                SeatController::BotAlwaysDecline => Box::new(AlwaysDecline),
                SeatController::BotScripted(script) => Box::new(Scripted::new(script)),
            };

            if prior_count > 0 {
                table.seat(
                    seat,
                    Box::new(ReplayingDecider {
                        prior_queue: prior_queue.clone(),
                        inner,
                    }),
                );
            } else {
                table.seat(seat, inner);
            }
        }

        let mut game = Game::with_table(config.state, ContentStore::embedded(), table);
        if let Some(galaxy) = config.galaxy {
            game = game.with_galaxy(galaxy);
        }

        if prior_count == 0 {
            // Emit initial game initialization event
            let mut lock = worker_shared.lock().expect("shared lock");
            let round = game.state.round;
            let speaker = &game.state.speaker;
            let version = lock.game_version;
            if let Err(error) = lock.record_and_broadcast_event(
                EventVisibility::Public,
                GameEventKind::GameInitialized {
                    round,
                    phase: game.state.phase,
                    speaker: speaker.clone(),
                },
                Some(version),
            ) {
                lock.error = Some(error);
                return;
            }
        } else {
            // Replay prior decisions to reach current state
            while game.table.log.records.len() < prior_count {
                let result = game.step();
                if let Some(err) = result.error {
                    let mut lock = worker_shared.lock().expect("shared lock");
                    lock.error = Some(format!("recovery replay failed: {err}"));
                    return;
                }
                if result.finished && game.table.log.records.len() < prior_count {
                    let mut lock = worker_shared.lock().expect("shared lock");
                    lock.error = Some("recovery replay ended before all decisions".to_owned());
                    return;
                }
            }
        }

        let mut prev_round = game.state.round;
        let mut prev_phase = game.state.phase;
        let mut prev_decision_count = game.table.log.records.len();

        'worker: loop {
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
            }

            // Check if finished
            if game.state.finished {
                let mut lock = worker_shared.lock().expect("shared lock");
                if let Err(error) = lock.finish_session(&game) {
                    lock.error = Some(error);
                    break;
                }
                break;
            }

            // Step the engine (will block if human decision is required)
            let result = game.step();

            // Record outcome
            {
                let mut lock = worker_shared.lock().expect("shared lock");
                let mut accepted_reply = None;
                lock.latest_state = game.state.clone();
                lock.decision_log.clone_from(&game.table.log.records);

                if let Some(err) = result.error {
                    lock.error = Some(err.to_string());
                    if let Some(reply_tx) = lock
                        .pending_decision
                        .as_mut()
                        .and_then(|pending| pending.reply_tx.take())
                    {
                        let _ = reply_tx.send(Err(
                            crate::protocol::status::RejectionReason::ValidationFailed {
                                message: "engine transition failed".to_owned(),
                            },
                        ));
                    }
                    break 'worker;
                }

                // Any newly resolved decisions:
                if game.table.log.records.len() > prev_decision_count {
                    for record in &game.table.log.records[prev_decision_count..] {
                        if let Some(store) = &lock.store
                            && let Err(err) = store.append_decision(&lock.game_id, record)
                        {
                            lock.error =
                                Some(format!("failed to persist accepted decision: {err}"));
                            if let Some(reply_tx) = lock
                                .pending_decision
                                .as_mut()
                                .and_then(|pending| pending.reply_tx.take())
                            {
                                let _ = reply_tx.send(Err(
                                    crate::protocol::status::RejectionReason::ValidationFailed {
                                        message: "decision could not be persisted".to_owned(),
                                    },
                                ));
                            }
                            break 'worker;
                        }
                        let version = lock.game_version;
                        let event_error = lock
                            .record_and_broadcast_event(
                                EventVisibility::Public,
                                GameEventKind::DecisionResolved,
                                Some(version),
                            )
                            .err();
                        if let Some(error) = event_error {
                            lock.error = Some(error);
                            if let Some(reply_tx) = lock
                                .pending_decision
                                .as_mut()
                                .and_then(|pending| pending.reply_tx.take())
                            {
                                let _ = reply_tx.send(Err(
                                    crate::protocol::status::RejectionReason::ValidationFailed {
                                        message: "event could not be persisted".to_owned(),
                                    },
                                ));
                            }
                            break 'worker;
                        }
                        if accepted_reply.is_none()
                            && let Some(reply_tx) = lock
                                .pending_decision
                                .take()
                                .and_then(|pending| pending.reply_tx)
                        {
                            accepted_reply = Some((
                                reply_tx,
                                crate::protocol::server::ActionAcceptedMsg {
                                    protocol_version: PROTOCOL_VERSION,
                                    game_id: lock.game_id.clone(),
                                    game_version: version,
                                    option_id: record.chosen.clone(),
                                },
                            ));
                        }
                    }
                    prev_decision_count = game.table.log.records.len();
                }

                if game.table.log.records.len() - lock.snapshot_decision_count >= 32 {
                    if let Some(store) = &lock.store
                        && let Err(error) = store.save_snapshot(
                            &lock.game_id,
                            game.table.log.records.len(),
                            &game.state,
                        )
                    {
                        lock.error = Some(format!("failed to persist snapshot: {error}"));
                        if let Some((reply_tx, _)) = accepted_reply {
                            let _ = reply_tx.send(Err(
                                crate::protocol::status::RejectionReason::ValidationFailed {
                                    message: "snapshot could not be persisted".to_owned(),
                                },
                            ));
                        }
                        break 'worker;
                    }
                    lock.snapshot_decision_count = game.table.log.records.len();
                }

                // Phase transition:
                if game.state.phase != prev_phase || game.state.round != prev_round {
                    let round = game.state.round;
                    let version = lock.game_version;
                    if let Err(error) = lock.record_and_broadcast_event(
                        EventVisibility::Public,
                        GameEventKind::PhaseTransition {
                            phase: game.state.phase,
                            round,
                        },
                        Some(version),
                    ) {
                        lock.error = Some(error);
                        break 'worker;
                    }
                    prev_phase = game.state.phase;
                    prev_round = game.state.round;
                }

                if result.finished {
                    if let Err(error) = lock.finish_session(&game) {
                        lock.error = Some(error);
                        break;
                    }
                    if let Some((reply_tx, accepted)) = accepted_reply {
                        let _ = reply_tx.send(Ok(accepted));
                    }
                    break;
                }

                if let Some((reply_tx, accepted)) = accepted_reply {
                    let _ = reply_tx.send(Ok(accepted));
                }

                lock.game_version += 1;
                lock.broadcast_state_update();
            }
        }
    });

    (shared, handle)
}

fn current_utc_time_string() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let total_secs = now.as_secs();
    let hours = (total_secs / 3600) % 24;
    let mins = (total_secs / 60) % 60;
    let secs = total_secs % 60;
    format!("{hours:02}:{mins:02}:{secs:02}")
}
