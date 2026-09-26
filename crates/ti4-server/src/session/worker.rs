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
use crate::protocol::server::{
    EventVisibility, GameEvent, GameEventKind, GameOverMsg, MovementFact, PendingChoiceMsg,
    ServerMessage, TurnStatusMsg,
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
#[expect(
    clippy::struct_excessive_bools,
    reason = "worker lifecycle and durable history are independent states"
)]
pub struct SessionShared {
    pub game_id: String,
    pub game_version: u64,
    pub latest_state: GameState,
    pub pending_decision: Option<PendingDecision>,
    pub seat_inboxes: BTreeMap<PlayerId, mpsc::Sender<ChoiceSubmission>>,
    pub seat_tokens: BTreeMap<PlayerId, String>,
    pub player_ids: Vec<PlayerId>,
    pub seats: BTreeMap<PlayerId, SeatController>,
    pub seed: Option<u64>,
    pub subscribers: BTreeMap<u64, Subscriber>,
    pub next_subscriber_id: u64,
    pub decision_log: Vec<DecisionRecord>,
    pub finished: bool,
    pub stopped: bool,
    pub error: Option<String>,
    pub map_tiles: Vec<crate::protocol::view::BoardTileView>,
    pub galaxy_layout: crate::map::GalaxyLayout,
    pub event_log: Vec<GameEvent>,
    pub event_counter: u64,
    pub store: Option<Arc<FileGameStore>>,
    pub snapshot_decision_count: usize,
    pub redo_decisions: Vec<DecisionRecord>,
    pub history_active: bool,
    pub redo_events: Vec<GameEvent>,
    pub replay_complete: bool,
    pub history_generation: u64,
    pub batches: Vec<crate::storage::BatchRecord>,
}

/// Only explicitly public decisions receive prose. Unknown/private option IDs
/// remain generic rather than accidentally exposing a card or secret objective.
fn public_decision_detail(record: &DecisionRecord) -> Option<String> {
    let subtype = record.context.as_ref()?.subtype.as_str();
    let actor = &record.player;
    match subtype {
        "pay_resources" | "pay_influence" if record.chosen == "trade_good" => {
            Some(format!("{actor} spent a trade good"))
        }
        "pay_resources" | "pay_influence" => record
            .chosen
            .strip_prefix("exhaust|")
            .map(|planet| format!("{actor} exhausted {planet}")),
        "vote_exhaust_planet" if record.chosen == "decline" => Some("Done voting".into()),
        "vote_exhaust_planet" => Some(format!("{actor} exhausted {} to vote", record.chosen)),
        "produce_unit" if record.chosen == "done_producing" => Some("Done producing".into()),
        "produce_unit" => {
            let parts: Vec<_> = record.chosen.split('|').collect();
            if let ["build", unit, count] = parts.as_slice() {
                Some(format!("{actor} produced {count} {unit}"))
            } else {
                None
            }
        }
        "movement_step" if record.chosen == "done_moving" => Some("Done moving".into()),
        _ => None,
    }
}

fn public_movement_fact(shared: &SessionShared, record: &DecisionRecord) -> Option<MovementFact> {
    if record.context.as_ref()?.subtype != "movement_step" {
        return None;
    }
    let offered = shared
        .pending_decision
        .as_ref()?
        .choice
        .options
        .iter()
        .find(|option| option.id == record.chosen && option.kind == "move")?;
    Some(MovementFact {
        actor: record.player.clone(),
        origin: offered.payload.get("origin")?.as_str()?.to_owned(),
        destination: shared
            .latest_state
            .active_system
            .as_ref()?
            .as_str()
            .to_owned(),
        unit: offered.payload.get("unit")?.as_str()?.to_owned(),
    })
}

impl SessionShared {
    fn persist_history(&self) -> Result<(), String> {
        if self.history_active
            && let Some(store) = &self.store
        {
            store
                .save_history(
                    &self.game_id,
                    &crate::storage::GameHistory {
                        decisions: self.decision_log.clone(),
                        redo: self.redo_decisions.clone(),
                        events: self.event_log.clone(),
                        redo_events: self.redo_events.clone(),
                        event_counter: self.event_counter,
                        generation: self.history_generation,
                        revision: self.game_version.saturating_add(1),
                        batches: self.batches.clone(),
                    },
                )
                .map_err(|error| format!("failed to persist history: {error}"))?;
        }
        Ok(())
    }
    #[must_use]
    pub fn new(game_id: String, initial_state: GameState) -> Self {
        Self {
            game_id,
            game_version: 1,
            latest_state: initial_state,
            pending_decision: None,
            seat_inboxes: BTreeMap::new(),
            seat_tokens: BTreeMap::new(),
            player_ids: Vec::new(),
            seats: BTreeMap::new(),
            seed: None,
            subscribers: BTreeMap::new(),
            next_subscriber_id: 0,
            decision_log: Vec::new(),
            event_log: Vec::new(),
            event_counter: 0,
            finished: false,
            stopped: false,
            error: None,
            map_tiles: Vec::new(),
            galaxy_layout: crate::map::GalaxyLayout {
                version: 1,
                active_sources: Vec::new(),
                placements: Vec::new(),
                off_map_system_ids: Vec::new(),
            },
            store: None,
            snapshot_decision_count: 0,
            redo_decisions: Vec::new(),
            history_active: false,
            redo_events: Vec::new(),
            replay_complete: false,
            history_generation: 0,
            batches: Vec::new(),
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
        decision_count: usize,
        detail: Option<String>,
        movement: Option<MovementFact>,
    ) -> Result<(), String> {
        if self.history_active
            && !self.redo_decisions.is_empty()
            && matches!(
                event,
                GameEventKind::PhaseTransition { .. } | GameEventKind::GameFinished { .. }
            )
        {
            if self.event_log.last().is_some_and(|previous| {
                previous.event == event && previous.decision_count == Some(decision_count)
            }) {
                return Ok(());
            }
            // A replay may traverse an automatic phase step before it reaches the next
            // human choice. Reattach its original event instead of duplicating its ID.
            let before_next_decision = self
                .redo_events
                .iter()
                .take_while(|entry| !matches!(entry.event, GameEventKind::DecisionResolved))
                .count();
            if let Some(index) = self.redo_events[..before_next_decision]
                .iter()
                .position(|entry| {
                    entry.event == event
                        && entry.decision_count.unwrap_or(decision_count) == decision_count
                })
            {
                self.event_log.push(self.redo_events.remove(index));
                return Ok(());
            }
        }
        self.event_counter += 1;
        let id = format!("{}-{}", self.game_id, self.event_counter);
        let timestamp = current_utc_time_string();
        let action_id = (matches!(event, GameEventKind::DecisionResolved)
            && self.latest_state.phase == ti4_model::state::Phase::Action)
            .then(|| crate::protocol::server::action_id_for(&self.decision_log, decision_count))
            .flatten();
        let entry = GameEvent {
            id,
            timestamp,
            version,
            visibility,
            event,
            decision_count: Some(decision_count),
            batch_id: None,
            action_id,
            detail,
            movement,
        };
        if !self.history_active
            && let Some(store) = &self.store
        {
            store
                .append_event(&self.game_id, &entry)
                .map_err(|error| format!("failed to persist event: {error}"))?;
        }

        self.event_log.push(entry.clone());

        if self.history_active {
            // Publish only after the entire authoritative timeline is atomically saved.
            return Ok(());
        }

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

    fn publish_history_events_since(&mut self, index: usize) {
        if !self.history_active {
            return;
        }
        for entry in self.event_log[index..].iter().cloned() {
            let message = ServerMessage::Event(crate::protocol::server::GameEventMsg {
                protocol_version: PROTOCOL_VERSION,
                game_id: self.game_id.clone(),
                entry: entry.clone(),
            });
            self.subscribers.retain(|_, subscriber| {
                !entry.visibility.permits(&subscriber.viewer)
                    || subscriber.tx.try_send(message.clone()).is_ok()
            });
        }
    }

    /// Broadcasts a newly raised decision to all subscribers.
    ///
    /// The actor receives the full `PendingChoiceMsg` with legal options.
    /// Opponents and spectators receive a redacted `TurnStatusMsg::WaitingForDecision`.
    pub fn broadcast_pending_decision(&mut self, choice: &Choice, nonce: &str) {
        let status = project_turn_status(&self.latest_state, Some(choice));
        let game_id = self.game_id.clone();
        let game_version = self.game_version;
        let state = self.latest_state.clone();
        let galaxy_layout = self.galaxy_layout.clone();

        self.publish(|viewer| {
            if viewer.is_actor(&choice.player) {
                ServerMessage::PendingChoice(PendingChoiceMsg {
                    protocol_version: PROTOCOL_VERSION,
                    game_id: game_id.clone(),
                    game_version,
                    nonce: nonce.to_owned(),
                    choice: choice.clone(),
                    state: crate::projection::redacted_state(&state, viewer),
                    galaxy_layout: galaxy_layout.clone(),
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
        let galaxy_layout = self.galaxy_layout.clone();
        let self_decision_count = self.decision_log.len();
        let self_redo_count = self.redo_decisions.len();
        let self_generation = self.history_generation;
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
                &galaxy_layout,
            )
            .with_history(self_decision_count, self_redo_count, self_generation);
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
        let cursor = self.decision_log.len();
        self.record_and_broadcast_event(
            EventVisibility::Public,
            GameEventKind::GameFinished {
                winner: winner.clone(),
            },
            Some(self.game_version),
            cursor,
            None,
            None,
        )?;
        if !self.history_active {
            self.broadcast_game_over(winner, final_scores);
        }
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
    let prior_records = config.prior_decisions.clone();
    let prior_queue = Arc::new(Mutex::new(VecDeque::from(config.prior_decisions.clone())));

    let mut initial_shared = SessionShared::new(config.game_id.clone(), config.state.clone());
    initial_shared.seat_tokens.clone_from(&config.seat_tokens);
    initial_shared.player_ids.clone_from(&config.player_ids);
    initial_shared.seats.clone_from(&config.seats);
    initial_shared.seed = config.seed;
    initial_shared.map_tiles.clone_from(&config.map_tiles);
    initial_shared
        .galaxy_layout
        .clone_from(&config.galaxy_layout);
    initial_shared.store.clone_from(&config.store);
    initial_shared.game_version = config.initial_version.max(1);
    initial_shared
        .redo_decisions
        .clone_from(&config.redo_decisions);
    initial_shared.history_active = config.history_active;
    initial_shared.redo_events.clone_from(&config.redo_events);
    initial_shared.event_counter = config.event_counter;
    initial_shared.history_generation = config.history_generation;
    initial_shared.batches.clone_from(&config.batches);

    if !config.prior_events.is_empty() {
        initial_shared.event_counter = initial_shared
            .event_counter
            .max(config.prior_events.len() as u64);
        let last_version = config
            .prior_events
            .iter()
            .filter_map(|e| e.version)
            .max()
            .unwrap_or(1);
        initial_shared.game_version = initial_shared.game_version.max(last_version);
        initial_shared.event_log.clone_from(&config.prior_events);
    }
    if !config.prior_decisions.is_empty() {
        initial_shared
            .decision_log
            .clone_from(&config.prior_decisions);
    }

    let shared = Arc::new(Mutex::new(initial_shared));

    let worker_shared = shared.clone();
    let handle =
        thread::spawn(move || {
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

            if prior_count == 0 && config.prior_events.is_empty() {
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
                    0,
                    None,
                    None,
                ) {
                    lock.error = Some(error);
                    return;
                }
            } else if prior_count > 0 {
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
                if game.table.log.records != prior_records {
                    worker_shared.lock().expect("shared lock").error =
                        Some("recovery replay records diverged".to_owned());
                    return;
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
                    lock.replay_complete = true;
                }

                // Check if finished
                if game.state.finished {
                    let mut lock = worker_shared.lock().expect("shared lock");
                    if lock.event_log.last().is_some_and(|event| {
                        matches!(event.event, GameEventKind::GameFinished { .. })
                    }) {
                        lock.finished = true;
                        break;
                    }
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
                    // A rewind stops this worker at its last published decision. A step
                    // interrupted while waiting for a human may still let bots act before
                    // returning; those results belong to the discarded timeline.
                    if lock.stopped {
                        break 'worker;
                    }
                    let mut accepted_reply = None;
                    let event_start = lock.event_log.len();
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
                        let fork_cursor = prev_decision_count;
                        for (offset, record) in game.table.log.records[prev_decision_count..]
                            .iter()
                            .enumerate()
                        {
                            if !lock.history_active
                                && let Some(store) = &lock.store
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
                            let movement = public_movement_fact(&lock, record);
                            let event_error = lock
                                .record_and_broadcast_event(
                                    EventVisibility::Public,
                                    GameEventKind::DecisionResolved,
                                    Some(version),
                                    prev_decision_count + offset + 1,
                                    public_decision_detail(record),
                                    movement,
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
                        if !lock.redo_decisions.is_empty() {
                            // Only an accepted decision forks history; autonomous engine
                            // steps between decisions leave redo available.
                            lock.redo_decisions.clear();
                            lock.redo_events.clear();
                            lock.batches.retain(|b| b.end_cursor <= fork_cursor);
                        }
                    }
                    if !lock.history_active
                        && game.table.log.records.len() - lock.snapshot_decision_count >= 32
                    {
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
                        let cursor = lock.decision_log.len();
                        if let Err(error) = lock.record_and_broadcast_event(
                            EventVisibility::Public,
                            GameEventKind::PhaseTransition {
                                phase: game.state.phase,
                                round,
                            },
                            Some(version),
                            cursor,
                            None,
                            None,
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
                        if let Err(error) = lock.persist_history() {
                            lock.error = Some(error);
                            if let Some((reply_tx, _)) = accepted_reply {
                                let _ = reply_tx.send(Err(
                                    crate::protocol::status::RejectionReason::ValidationFailed {
                                        message: "history could not be persisted".to_owned(),
                                    },
                                ));
                            }
                            break;
                        }
                        lock.publish_history_events_since(event_start);
                        let winner = game
                            .state
                            .players
                            .iter()
                            .max_by_key(|p| p.victory_points)
                            .map(|p| p.id.clone());
                        let scores = game
                            .state
                            .players
                            .iter()
                            .map(|p| (p.id.clone(), p.victory_points.max(0).cast_unsigned()))
                            .collect();
                        if lock.history_active {
                            lock.broadcast_game_over(winner, scores);
                        }
                        if let Some((reply_tx, accepted)) = accepted_reply {
                            let _ = reply_tx.send(Ok(accepted));
                        }
                        break;
                    }

                    if let Err(error) = lock.persist_history() {
                        lock.error = Some(error);
                        if let Some((reply_tx, _)) = accepted_reply {
                            let _ = reply_tx.send(Err(
                                crate::protocol::status::RejectionReason::ValidationFailed {
                                    message: "history could not be persisted".to_owned(),
                                },
                            ));
                        }
                        break 'worker;
                    }
                    lock.publish_history_events_since(event_start);

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
