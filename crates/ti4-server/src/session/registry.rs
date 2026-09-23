//! Registry for pre-game lobbies and active game sessions.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use ti4_content::ContentStore;
use ti4_model::id::PlayerId;

use crate::session::{GameSession, SeatController, SessionConfig};
use crate::storage::{
    GameInitRecord, LobbyRecord, PersistedLobbyPhase, PersistedLobbySeat, StorageError,
};

use crate::storage::{
    LobbySlotId, PLAYER_RECORD_VERSION, PlayerGameInitRecord, PlayerLobbyMember, PlayerLobbyRecord,
    PlayerLobbySlot, PlayerSession, PlayerSessionsRecord, generate_player_id,
};

/// Public projection; deliberately cannot serialize the private persistence record.
#[derive(Debug, Clone, Serialize)]
pub struct PlayerLobbyView {
    pub game_id: String,
    pub phase: LobbyPhase,
    pub host_player_id: PlayerId,
    pub slots: Vec<PlayerSlotView>,
    pub lobby_version: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlayerSlotView {
    pub slot_id: LobbySlotId,
    pub position: usize,
    pub occupant: Option<PlayerId>,
    pub ready: bool,
}

impl PlayerLobbyRecord {
    /// Build a lobby with the creator in the first slot, leaving the rest open.
    pub fn create(
        game_id: String,
        slot_count: usize,
        seed: u64,
    ) -> Result<(Self, PlayerId, PlayerSession), StorageError> {
        crate::storage::validate_game_id(&game_id)?;
        if !(1..=8).contains(&slot_count) {
            return Err(StorageError::InvalidPlayerRecord("slot count"));
        }
        let mut players = BTreeMap::new();
        let host_player_id = generate_player_id(&players);
        let session = PlayerSession::generate();
        players.insert(
            host_player_id.clone(),
            PlayerLobbyMember {
                ready: false,
                session: session.clone(),
            },
        );
        let slots = (0..slot_count)
            .map(|index| PlayerLobbySlot {
                slot_id: LobbySlotId(format!("slot_{}", index + 1)),
                occupant: (index == 0).then(|| host_player_id.clone()),
            })
            .collect();
        let lobby = Self {
            schema_version: PLAYER_RECORD_VERSION,
            game_id,
            phase: PersistedLobbyPhase::Lobby,
            host_player_id: host_player_id.clone(),
            slots,
            players,
            seed,
            lobby_version: 1,
        };
        lobby.validate()?;
        Ok((lobby, host_player_id, session))
    }

    /// Only this explicit projection is suitable for public HTTP/WS output.
    #[must_use]
    pub fn public_view(&self) -> PlayerLobbyView {
        PlayerLobbyView {
            game_id: self.game_id.clone(),
            phase: match self.phase {
                PersistedLobbyPhase::Lobby => LobbyPhase::Lobby,
                PersistedLobbyPhase::Running => LobbyPhase::Running,
            },
            host_player_id: self.host_player_id.clone(),
            slots: self
                .slots
                .iter()
                .enumerate()
                .map(|(index, slot)| PlayerSlotView {
                    slot_id: slot.slot_id.clone(),
                    position: index + 1,
                    occupant: slot.occupant.clone(),
                    ready: slot
                        .occupant
                        .as_ref()
                        .is_some_and(|id| self.players[id].ready),
                })
                .collect(),
            lobby_version: self.lobby_version,
        }
    }
}

/// Summary of an active, completed, or unstarted game.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GameSummary {
    pub game_id: String,
    pub is_finished: bool,
    pub pending_seat: Option<PlayerId>,
}

/// Lifecycle phase exposed by the lobby API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LobbyPhase {
    Lobby,
    Running,
}

/// Server-owned lobby configuration.
#[derive(Debug, Clone)]
pub struct LobbyState {
    pub game_id: String,
    pub phase: LobbyPhase,
    pub host_seat: PlayerId,
    pub player_ids: Vec<PlayerId>,
    pub seats: BTreeMap<PlayerId, LobbySeat>,
    pub seed: u64,
    pub lobby_version: u64,
}

/// Lobby state for a configured seat. This type is never serialized directly.
#[derive(Clone)]
pub struct LobbySeat {
    pub controller: SeatController,
    pub ready: bool,
    pub seat_token: Option<String>,
    pub lease_expires_at_ms: Option<u64>,
}

impl std::fmt::Debug for LobbySeat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LobbySeat")
            .field("controller", &self.controller)
            .field("ready", &self.ready)
            .field("seat_token", &"[redacted]")
            .field("lease_expires_at_ms", &self.lease_expires_at_ms)
            .finish()
    }
}

/// Input used to create a durable pre-game lobby.
#[derive(Debug, Clone)]
pub struct LobbyConfig {
    pub game_id: String,
    pub host_seat: PlayerId,
    pub player_ids: Vec<PlayerId>,
    pub seats: BTreeMap<PlayerId, SeatController>,
    pub seed: u64,
}

/// Result returned only at creation time, when handoff capabilities are allowed.
#[derive(Clone)]
pub struct CreatedLobby {
    pub lobby: LobbyState,
    pub creator_token: String,
    /// Internal creation result; HTTP deliberately returns only `creator_token`.
    pub seat_tokens: BTreeMap<PlayerId, String>,
}

impl std::fmt::Debug for CreatedLobby {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CreatedLobby")
            .field("lobby", &self.lobby)
            .field("creator_token", &"[redacted]")
            .field("seat_tokens", &"[redacted]")
            .finish()
    }
}

/// Snapshot of a lobby and the caller authenticated by a capability, if any.
#[derive(Debug, Clone)]
pub struct LobbyStatus {
    pub lobby: LobbyState,
    pub viewer: Option<PlayerId>,
}

/// A deterministic lifecycle or authorization failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LobbyError {
    NotFound,
    NotInLobby,
    AlreadyRunning,
    InvalidCapability,
    HumanSeatRequired,
    HostRequired,
    HumansNotReady,
    SeatUnavailable,
    TakeoverUnavailable,
    Map(String),
    Storage(String),
}

impl LobbyError {
    #[must_use]
    pub fn message(&self) -> String {
        match self {
            Self::NotFound => "Game not found".to_owned(),
            Self::NotInLobby => "Game has no lobby".to_owned(),
            Self::AlreadyRunning => "Game has already started".to_owned(),
            Self::InvalidCapability => "Invalid seat capability".to_owned(),
            Self::HumanSeatRequired => "Only human seats may update readiness".to_owned(),
            Self::HostRequired => "Only the lobby host may start the game".to_owned(),
            Self::HumansNotReady => "Every human seat must be ready before starting".to_owned(),
            Self::SeatUnavailable => "Seat is unavailable".to_owned(),
            Self::TakeoverUnavailable => "Takeover is not available yet".to_owned(),
            Self::Map(error) => format!("Failed to start game with map: {error}"),
            Self::Storage(error) => format!("Failed to persist lobby lifecycle: {error}"),
        }
    }
}

#[derive(Default)]
struct RegistryState {
    sessions: BTreeMap<String, Arc<GameSession>>,
    lobbies: BTreeMap<String, LobbyState>,
    player_lobbies: BTreeMap<String, PlayerLobbyRecord>,
}

/// Thread-safe registry that serializes each lobby's transition into an active session.
pub struct GameRegistry {
    state: Mutex<RegistryState>,
    store: Option<Arc<crate::storage::FileGameStore>>,
    lease_duration: Duration,
}

impl Default for GameRegistry {
    fn default() -> Self {
        Self {
            state: Mutex::new(RegistryState::default()),
            store: None,
            lease_duration: Duration::from_secs(30),
        }
    }
}

impl GameRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_store(mut self, store: Arc<crate::storage::FileGameStore>) -> Self {
        self.store = Some(store);
        self
    }

    #[must_use]
    pub fn with_lease_duration(mut self, lease_duration: Duration) -> Self {
        self.lease_duration =
            lease_duration.clamp(Duration::from_secs(1), Duration::from_secs(300));
        self
    }

    /// Returns the optional underlying game store.
    #[must_use]
    pub fn store(&self) -> Option<Arc<crate::storage::FileGameStore>> {
        self.store.clone()
    }

    /// Recovers both unstarted lobbies and started sessions from durable storage.
    pub fn recover_all_games(&self) -> Result<Vec<String>, StorageError> {
        let Some(store) = &self.store else {
            return Ok(Vec::new());
        };
        let game_ids: BTreeSet<_> = store.list_saved_games()?.into_iter().collect();
        let lobby_ids = store.list_saved_lobbies()?;
        let mut state = self.state.lock().expect("registry lock");
        let mut recovered = Vec::new();

        for game_id in lobby_ids {
            if state.lobbies.contains_key(&game_id)
                || state.sessions.contains_key(&game_id)
                || state.player_lobbies.contains_key(&game_id)
            {
                continue;
            }
            // A v2 lobby is a private record; never deserialize it as a v1 seat lobby.
            let path = store.game_dir(&game_id)?.join("lobby.json");
            if std::fs::metadata(&path)?.len() > 64 * 1024 {
                return Err(StorageError::Oversized {
                    path,
                    limit: 64 * 1024,
                });
            }
            let marker: serde_json::Value = serde_json::from_slice(&std::fs::read(&path)?)?;
            if marker
                .get("payload")
                .and_then(|v| v.get("schema_version"))
                .is_some()
            {
                let mut record = store
                    .load_player_lobby(&game_id)?
                    .expect("listed lobby exists");
                if game_ids.contains(&game_id) {
                    let init = store.load_player_init(&game_id)?;
                    let credentials = store.load_player_sessions(&game_id)?;
                    if init.player_ids
                        != record
                            .slots
                            .iter()
                            .filter_map(|slot| slot.occupant.clone())
                            .collect::<Vec<_>>()
                        || credentials.sessions.len() != record.players.len()
                        || credentials.sessions.keys().ne(record.players.keys())
                    {
                        return Err(StorageError::InvalidPlayerRecord("started roster"));
                    }
                    state.sessions.insert(
                        game_id.clone(),
                        Arc::new(store.recover_player_session(&game_id)?),
                    );
                    record.phase = PersistedLobbyPhase::Running;
                } else {
                    record.phase = PersistedLobbyPhase::Lobby;
                }
                state.player_lobbies.insert(game_id.clone(), record);
                recovered.push(game_id);
                continue;
            }
            let record = store.load_lobby(&game_id)?.expect("listed lobby exists");
            let mut lobby = lobby_from_record(record);
            if game_ids.contains(&game_id) {
                lobby.phase = LobbyPhase::Running;
                state
                    .sessions
                    .insert(game_id.clone(), Arc::new(store.recover_session(&game_id)?));
            } else {
                // A crash between marking the lobby running and writing init.json remains a lobby.
                lobby.phase = LobbyPhase::Lobby;
            }
            state.lobbies.insert(game_id.clone(), lobby);
            recovered.push(game_id);
        }

        for game_id in game_ids {
            if state.sessions.contains_key(&game_id) {
                continue;
            }
            let session = Arc::new(store.recover_session(&game_id)?);
            let init = store.load_init(&game_id)?;
            state
                .lobbies
                .insert(game_id.clone(), legacy_running_lobby(&init));
            state.sessions.insert(game_id.clone(), session);
            recovered.push(game_id);
        }
        recovered.sort();
        Ok(recovered)
    }

    /// Creates the private v2 lobby before delivering its first credential.
    pub fn create_player_lobby(
        &self,
        game_id: String,
        count: usize,
        seed: u64,
    ) -> Result<(PlayerLobbyView, PlayerId, PlayerSession), LobbyError> {
        let mut state = self.state.lock().expect("registry lock");
        if state.lobbies.contains_key(&game_id)
            || state.player_lobbies.contains_key(&game_id)
            || state.sessions.contains_key(&game_id)
            || self
                .store
                .as_ref()
                .is_some_and(|store| store.game_dir(&game_id).is_ok_and(|dir| dir.exists()))
        {
            return Err(LobbyError::SeatUnavailable);
        }
        let (record, player, credential) = PlayerLobbyRecord::create(game_id.clone(), count, seed)
            .map_err(|error| LobbyError::Storage(error.to_string()))?;
        self.save_player_lobby(&record)?;
        let view = record.public_view();
        state.player_lobbies.insert(game_id, record);
        Ok((view, player, credential))
    }

    fn save_player_lobby(&self, record: &PlayerLobbyRecord) -> Result<(), LobbyError> {
        if let Some(store) = &self.store {
            store
                .save_player_lobby(record)
                .map_err(|e| LobbyError::Storage(e.to_string()))?;
        }
        Ok(())
    }

    /// Read-only spectator projection; an invalid supplied credential fails closed.
    pub fn player_lobby_status(
        &self,
        game_id: &str,
        credential: Option<&str>,
    ) -> Result<(PlayerLobbyView, Option<PlayerId>), LobbyError> {
        let state = self.state.lock().expect("registry lock");
        let record = state
            .player_lobbies
            .get(game_id)
            .ok_or(LobbyError::NotFound)?;
        let viewer = credential
            .map(|value| authenticate_player(record, value))
            .transpose()?;
        Ok((record.public_view(), viewer))
    }

    /// Serializes first-open admission and credential-bearing reconnects.
    pub fn join_player_lobby(
        &self,
        game_id: &str,
        credential: Option<&str>,
    ) -> Result<(PlayerLobbyView, PlayerId, Option<PlayerSession>), LobbyError> {
        let mut state = self.state.lock().expect("registry lock");
        let lobby = state
            .player_lobbies
            .get_mut(game_id)
            .ok_or(LobbyError::NotFound)?;
        if let Some(credential) = credential {
            let player = authenticate_player(lobby, credential)?;
            return Ok((lobby.public_view(), player, None));
        }
        if !matches!(lobby.phase, PersistedLobbyPhase::Lobby) {
            return Err(LobbyError::AlreadyRunning);
        }
        let slot = lobby
            .slots
            .iter()
            .position(|slot| slot.occupant.is_none())
            .ok_or(LobbyError::SeatUnavailable)?;
        let mut updated = lobby.clone();
        let player = generate_player_id(&updated.players);
        let session = loop {
            let candidate = PlayerSession::generate();
            if updated
                .players
                .values()
                .all(|member| member.session != candidate)
            {
                break candidate;
            }
        };
        updated.slots[slot].occupant = Some(player.clone());
        updated.players.insert(
            player.clone(),
            PlayerLobbyMember {
                ready: false,
                session: session.clone(),
            },
        );
        updated.lobby_version += 1;
        self.save_player_lobby(&updated)?;
        *lobby = updated;
        Ok((lobby.public_view(), player, Some(session)))
    }

    /// A leaving player retires their identity and credential, never the host's.
    pub fn leave_player_lobby(
        &self,
        game_id: &str,
        credential: &str,
    ) -> Result<PlayerLobbyView, LobbyError> {
        let mut state = self.state.lock().expect("registry lock");
        let lobby = state
            .player_lobbies
            .get_mut(game_id)
            .ok_or(LobbyError::NotFound)?;
        if !matches!(lobby.phase, PersistedLobbyPhase::Lobby) {
            return Err(LobbyError::AlreadyRunning);
        }
        let player = authenticate_player(lobby, credential)?;
        if player == lobby.host_player_id {
            return Err(LobbyError::HostRequired);
        }
        let mut updated = lobby.clone();
        updated.players.remove(&player);
        updated
            .slots
            .iter_mut()
            .find(|slot| slot.occupant.as_ref() == Some(&player))
            .expect("validated occupant")
            .occupant = None;
        updated.lobby_version += 1;
        self.save_player_lobby(&updated)?;
        *lobby = updated;
        Ok(lobby.public_view())
    }

    pub fn set_player_ready(
        &self,
        game_id: &str,
        credential: &str,
        ready: bool,
    ) -> Result<(PlayerLobbyView, PlayerId), LobbyError> {
        let mut state = self.state.lock().expect("registry lock");
        let lobby = state
            .player_lobbies
            .get_mut(game_id)
            .ok_or(LobbyError::NotFound)?;
        if !matches!(lobby.phase, PersistedLobbyPhase::Lobby) {
            return Err(LobbyError::AlreadyRunning);
        }
        let player = authenticate_player(lobby, credential)?;
        if lobby.players[&player].ready != ready {
            let mut updated = lobby.clone();
            updated
                .players
                .get_mut(&player)
                .expect("authenticated player")
                .ready = ready;
            updated.lobby_version += 1;
            self.save_player_lobby(&updated)?;
            *lobby = updated;
        }
        Ok((lobby.public_view(), player))
    }

    pub fn start_player_lobby(
        &self,
        game_id: &str,
        credential: &str,
    ) -> Result<PlayerLobbyView, LobbyError> {
        let mut state = self.state.lock().expect("registry lock");
        let lobby = state
            .player_lobbies
            .get_mut(game_id)
            .ok_or(LobbyError::NotFound)?;
        if !matches!(lobby.phase, PersistedLobbyPhase::Lobby) {
            return Err(LobbyError::AlreadyRunning);
        }
        if authenticate_player(lobby, credential)? != lobby.host_player_id {
            return Err(LobbyError::HostRequired);
        }
        if lobby.slots.iter().any(|slot| slot.occupant.is_none())
            || lobby.players.values().any(|member| !member.ready)
        {
            return Err(LobbyError::HumansNotReady);
        }
        let players: Vec<_> = lobby
            .slots
            .iter()
            .map(|slot| slot.occupant.clone().expect("full lobby"))
            .collect();
        let content = ContentStore::embedded();
        let (initial_state, galaxy) =
            crate::map::create_game_with_map(content, &players, lobby.seed)
                .map_err(|e| LobbyError::Map(e.to_string()))?;
        let map_tiles = crate::map::build_board_tiles(content, &galaxy);
        let mut config = SessionConfig::new(game_id, initial_state.clone())
            .with_seed(lobby.seed)
            .with_player_ids(players.clone())
            .with_galaxy(galaxy, map_tiles.clone());
        config.seats = players
            .iter()
            .map(|p| (p.clone(), SeatController::Human))
            .collect();
        config.seat_tokens = lobby
            .players
            .iter()
            .map(|(p, m)| (p.clone(), m.session.as_str().to_owned()))
            .collect();
        config.store.clone_from(&self.store);
        let mut running = lobby.clone();
        running.phase = PersistedLobbyPhase::Running;
        running.lobby_version += 1;
        if let Some(store) = &self.store {
            store
                .save_player_sessions(&PlayerSessionsRecord {
                    schema_version: PLAYER_RECORD_VERSION,
                    game_id: game_id.to_owned(),
                    sessions: lobby
                        .players
                        .iter()
                        .map(|(p, m)| (p.clone(), m.session.clone()))
                        .collect(),
                })
                .map_err(|e| LobbyError::Storage(e.to_string()))?;
            self.save_player_lobby(&running)?;
            if let Err(e) = store.save_player_init(&PlayerGameInitRecord {
                schema_version: PLAYER_RECORD_VERSION,
                game_id: game_id.to_owned(),
                seed: lobby.seed,
                player_ids: players,
                initial_state,
                map_tiles,
            }) {
                // No valid init: recovery treats the lobby as unstarted.
                let _ = self.save_player_lobby(lobby);
                return Err(LobbyError::Storage(e.to_string()));
            }
        }
        *lobby = running;
        let view = lobby.public_view();
        state
            .sessions
            .insert(game_id.to_owned(), Arc::new(GameSession::start(config)));
        Ok(view)
    }

    /// Creates a pre-game lobby and persists it before returning capabilities.
    pub fn create_lobby(&self, config: LobbyConfig) -> Result<CreatedLobby, String> {
        crate::storage::validate_game_id(&config.game_id).map_err(|error| error.to_string())?;
        let mut state = self.state.lock().expect("registry lock");
        if state.sessions.contains_key(&config.game_id)
            || state.lobbies.contains_key(&config.game_id)
        {
            return Err(format!("Game session '{}' already exists", config.game_id));
        }

        let mut seats = BTreeMap::new();
        let creator_token = new_token();
        let lease_expires_at_ms = Some(self.lease_expiry_ms());
        for player_id in &config.player_ids {
            let controller = config
                .seats
                .get(player_id)
                .expect("validated lobby seat")
                .clone();
            let seat_token = (player_id == &config.host_seat).then(|| creator_token.clone());
            seats.insert(
                player_id.clone(),
                LobbySeat {
                    controller,
                    ready: false,
                    seat_token,
                    lease_expires_at_ms: if player_id == &config.host_seat {
                        lease_expires_at_ms
                    } else {
                        None
                    },
                },
            );
        }
        let lobby = LobbyState {
            game_id: config.game_id.clone(),
            phase: LobbyPhase::Lobby,
            host_seat: config.host_seat.clone(),
            player_ids: config.player_ids,
            seats,
            seed: config.seed,
            lobby_version: 1,
        };
        if let Some(store) = &self.store {
            store
                .save_lobby(&lobby_to_record(&lobby))
                .map_err(|error| format!("Failed to save lobby: {error}"))?;
        }
        state.lobbies.insert(lobby.game_id.clone(), lobby.clone());
        Ok(CreatedLobby {
            lobby,
            creator_token: creator_token.clone(),
            seat_tokens: BTreeMap::from([(config.host_seat, creator_token)]),
        })
    }

    /// Returns public lobby state and authenticates an optional capability.
    pub fn lobby_status(
        &self,
        game_id: &str,
        token: Option<&str>,
    ) -> Result<LobbyStatus, LobbyError> {
        let mut state = self.state.lock().expect("registry lock");
        let lobby = state.lobbies.get_mut(game_id).ok_or(LobbyError::NotFound)?;
        self.expire_claims(lobby)?;
        let viewer = match token {
            Some(token) => lobby
                .seats
                .iter()
                .find_map(|(seat, lobby_seat)| {
                    (lobby_seat.seat_token.as_deref() == Some(token)).then(|| seat.clone())
                })
                .ok_or(LobbyError::InvalidCapability)?,
            None => {
                return Ok(LobbyStatus {
                    lobby: lobby.clone(),
                    viewer: None,
                });
            }
        };
        Ok(LobbyStatus {
            lobby: lobby.clone(),
            viewer: Some(viewer),
        })
    }

    /// Atomically claims an available human seat and returns its new bearer credential.
    pub fn claim_seat(
        &self,
        game_id: &str,
        requested_seat: &str,
    ) -> Result<(LobbyStatus, String), LobbyError> {
        let mut state = self.state.lock().expect("registry lock");
        let lobby = state.lobbies.get_mut(game_id).ok_or(LobbyError::NotFound)?;
        self.expire_claims(lobby)?;
        let seat_id = PlayerId::new(requested_seat);
        let seat = lobby
            .seats
            .get(&seat_id)
            .ok_or(LobbyError::SeatUnavailable)?;
        if seat.controller != SeatController::Human || seat.seat_token.is_some() {
            return Err(LobbyError::SeatUnavailable);
        }
        let token = new_token();
        let mut updated = lobby.clone();
        let claimed = updated.seats.get_mut(&seat_id).expect("validated seat");
        claimed.seat_token = Some(token.clone());
        claimed.lease_expires_at_ms = Some(self.lease_expiry_ms());
        claimed.ready = false;
        updated.lobby_version += 1;
        self.save_lobby(&updated)?;
        *lobby = updated.clone();
        Ok((
            LobbyStatus {
                lobby: updated,
                viewer: Some(seat_id),
            },
            token,
        ))
    }

    /// Authenticates and renews a current claim. All HTTP and WebSocket authorization enters here.
    pub fn authenticate_and_renew(
        &self,
        game_id: &str,
        token: &str,
    ) -> Result<PlayerId, LobbyError> {
        let mut state = self.state.lock().expect("registry lock");
        if let Some(lobby) = state.player_lobbies.get(game_id) {
            // Until PIL-03 replaces the WebSocket field, the WS authentication
            // boundary still calls this method. Never renew or expire v2 credentials.
            return authenticate_player(lobby, token);
        }
        let lobby = state.lobbies.get_mut(game_id).ok_or(LobbyError::NotFound)?;
        self.expire_claims(lobby)?;
        let viewer = authenticated_seat(lobby, token)?;
        let mut updated = lobby.clone();
        updated
            .seats
            .get_mut(&viewer)
            .expect("authenticated seat")
            .lease_expires_at_ms = Some(self.lease_expiry_ms());
        self.save_lobby(&updated)?;
        *lobby = updated;
        Ok(viewer)
    }

    /// Updates only the authenticated human seat's readiness while still in the lobby.
    pub fn set_ready(
        &self,
        game_id: &str,
        token: &str,
        ready: bool,
    ) -> Result<LobbyStatus, LobbyError> {
        let mut state = self.state.lock().expect("registry lock");
        let lobby = state.lobbies.get_mut(game_id).ok_or(LobbyError::NotFound)?;
        self.expire_claims(lobby)?;
        if lobby.phase != LobbyPhase::Lobby {
            return Err(LobbyError::AlreadyRunning);
        }
        let viewer = authenticated_seat(lobby, token)?;
        let seat = lobby.seats.get(&viewer).expect("authenticated seat exists");
        if seat.controller != SeatController::Human {
            return Err(LobbyError::HumanSeatRequired);
        }
        if seat.ready != ready {
            let mut updated = lobby.clone();
            updated
                .seats
                .get_mut(&viewer)
                .expect("authenticated seat exists")
                .ready = ready;
            updated.lobby_version += 1;
            self.save_lobby(&updated)?;
            *lobby = updated;
        }
        Ok(LobbyStatus {
            lobby: lobby.clone(),
            viewer: Some(viewer),
        })
    }

    /// Starts exactly one game after the authenticated host has all human seats ready.
    pub fn start_lobby(&self, game_id: &str, token: &str) -> Result<Arc<GameSession>, LobbyError> {
        let mut state = self.state.lock().expect("registry lock");
        let lobby = state.lobbies.get_mut(game_id).ok_or(LobbyError::NotFound)?;
        self.expire_claims(lobby)?;
        if lobby.phase != LobbyPhase::Lobby {
            return Err(LobbyError::AlreadyRunning);
        }
        let viewer = authenticated_seat(lobby, token)?;
        if viewer != lobby.host_seat {
            return Err(LobbyError::HostRequired);
        }
        if lobby
            .seats
            .values()
            .any(|seat| seat.controller == SeatController::Human && !seat.ready)
        {
            return Err(LobbyError::HumansNotReady);
        }

        let content = ContentStore::embedded();
        let (initial_state, galaxy) =
            crate::map::create_game_with_map(content, &lobby.player_ids, lobby.seed)
                .map_err(|error| LobbyError::Map(error.to_string()))?;
        let map_tiles = crate::map::build_board_tiles(content, &galaxy);
        let mut config = SessionConfig::new(&lobby.game_id, initial_state.clone())
            .with_seed(lobby.seed)
            .with_player_ids(lobby.player_ids.clone())
            .with_galaxy(galaxy, map_tiles);
        config.seats = lobby
            .seats
            .iter()
            .map(|(seat, lobby_seat)| (seat.clone(), lobby_seat.controller.clone()))
            .collect();
        config.seat_tokens = lobby
            .seats
            .iter()
            .filter_map(|(seat, lobby_seat)| {
                lobby_seat
                    .seat_token
                    .as_ref()
                    .map(|token| (seat.clone(), token.clone()))
            })
            .collect();
        config.store.clone_from(&self.store);

        // Record Running before init; recovery treats a Running record without init as Lobby.
        lobby.phase = LobbyPhase::Running;
        lobby.lobby_version += 1;
        if let Some(store) = &self.store {
            if let Err(error) = store.save_lobby(&lobby_to_record(lobby)) {
                lobby.phase = LobbyPhase::Lobby;
                lobby.lobby_version -= 1;
                return Err(LobbyError::Storage(error.to_string()));
            }
            let init = GameInitRecord {
                game_id: config.game_id.clone(),
                seed: config.seed,
                player_ids: config.player_ids.clone(),
                initial_state,
                seats: config.seats.clone(),
                seat_tokens: config.seat_tokens.clone(),
                map_tiles: config.map_tiles.clone(),
            };
            if let Err(error) = store.save_init(&init) {
                lobby.phase = LobbyPhase::Lobby;
                lobby.lobby_version -= 1;
                let _ = store.save_lobby(&lobby_to_record(lobby));
                return Err(LobbyError::Storage(error.to_string()));
            }
        }
        let session = Arc::new(GameSession::start(config));
        state.sessions.insert(game_id.to_owned(), session.clone());
        Ok(session)
    }

    fn lease_expiry_ms(&self) -> u64 {
        now_ms().saturating_add(
            self.lease_duration
                .as_millis()
                .try_into()
                .unwrap_or(u64::MAX),
        )
    }

    fn save_lobby(&self, lobby: &LobbyState) -> Result<(), LobbyError> {
        if let Some(store) = &self.store {
            store
                .save_lobby(&lobby_to_record(lobby))
                .map_err(|error| LobbyError::Storage(error.to_string()))?;
        }
        Ok(())
    }

    fn expire_claims(&self, lobby: &mut LobbyState) -> Result<(), LobbyError> {
        let now = now_ms();
        let mut updated = lobby.clone();
        let mut changed = false;
        for seat in updated.seats.values_mut() {
            if seat.controller == SeatController::Human
                && seat.lease_expires_at_ms.is_some_and(|expiry| expiry <= now)
            {
                seat.seat_token = None;
                seat.lease_expires_at_ms = None;
                seat.ready = false;
                changed = true;
            }
        }
        if changed {
            updated.lobby_version += 1;
            self.save_lobby(&updated)?;
            *lobby = updated;
        }
        Ok(())
    }

    /// Creates and starts a legacy active session. New HTTP games use [`Self::create_lobby`].
    pub fn create_game(&self, mut config: SessionConfig) -> Result<Arc<GameSession>, String> {
        crate::storage::validate_game_id(&config.game_id).map_err(|error| error.to_string())?;
        let mut state = self.state.lock().expect("registry lock");
        if state.sessions.contains_key(&config.game_id)
            || state.lobbies.contains_key(&config.game_id)
        {
            return Err(format!("Game session '{}' already exists", config.game_id));
        }
        if config.store.is_none() {
            config.store.clone_from(&self.store);
        }
        if let Some(store) = &config.store {
            if config.player_ids.is_empty() {
                return Err("durable sessions require an explicit player order".to_owned());
            }
            store
                .save_init(&GameInitRecord {
                    game_id: config.game_id.clone(),
                    seed: config.seed,
                    player_ids: config.player_ids.clone(),
                    initial_state: config.state.clone(),
                    seats: config.seats.clone(),
                    seat_tokens: config.seat_tokens.clone(),
                    map_tiles: config.map_tiles.clone(),
                })
                .map_err(|error| format!("Failed to save initial game configuration: {error}"))?;
        }
        let lobby = running_lobby_from_session_config(&config);
        let session = Arc::new(GameSession::start(config));
        state.lobbies.insert(lobby.game_id.clone(), lobby);
        state
            .sessions
            .insert(session.id().to_owned(), session.clone());
        Ok(session)
    }

    pub fn register_recovered(&self, session: Arc<GameSession>) -> Result<(), String> {
        let mut state = self.state.lock().expect("registry lock");
        if state.sessions.contains_key(session.id()) || state.lobbies.contains_key(session.id()) {
            return Err(format!("Game session '{}' already exists", session.id()));
        }
        let lobby = running_lobby_from_session(&session);
        state.lobbies.insert(lobby.game_id.clone(), lobby);
        state.sessions.insert(session.id().to_owned(), session);
        Ok(())
    }

    #[must_use]
    pub fn get_game(&self, game_id: &str) -> Option<Arc<GameSession>> {
        self.state
            .lock()
            .expect("registry lock")
            .sessions
            .get(game_id)
            .cloned()
    }

    #[must_use]
    pub fn contains_game(&self, game_id: &str) -> bool {
        let state = self.state.lock().expect("registry lock");
        state.sessions.contains_key(game_id)
            || state.lobbies.contains_key(game_id)
            || state.player_lobbies.contains_key(game_id)
    }

    #[must_use]
    pub fn list_games(&self) -> Vec<GameSummary> {
        let state = self.state.lock().expect("registry lock");
        let mut games: Vec<_> = state
            .sessions
            .values()
            .map(|session| GameSummary {
                game_id: session.id().to_owned(),
                is_finished: session.is_finished(),
                pending_seat: session.current_pending_decision().map(|(seat, _, _)| seat),
            })
            .collect();
        games.extend(
            state
                .lobbies
                .values()
                .filter(|lobby| lobby.phase == LobbyPhase::Lobby)
                .map(|lobby| GameSummary {
                    game_id: lobby.game_id.clone(),
                    is_finished: false,
                    pending_seat: None,
                }),
        );
        games.extend(
            state
                .player_lobbies
                .values()
                .filter(|lobby| matches!(lobby.phase, PersistedLobbyPhase::Lobby))
                .map(|lobby| GameSummary {
                    game_id: lobby.game_id.clone(),
                    is_finished: false,
                    pending_seat: None,
                }),
        );
        games.sort_by(|left, right| left.game_id.cmp(&right.game_id));
        games
    }

    pub fn remove_game(&self, game_id: &str) -> Option<Arc<GameSession>> {
        let mut state = self.state.lock().expect("registry lock");
        state.lobbies.remove(game_id);
        state.player_lobbies.remove(game_id);
        let session = state.sessions.remove(game_id);
        if let Some(session) = &session {
            session.stop();
        }
        session
    }
}

fn authenticate_player(
    lobby: &PlayerLobbyRecord,
    credential: &str,
) -> Result<PlayerId, LobbyError> {
    lobby
        .players
        .iter()
        .find_map(|(id, member)| (member.session.as_str() == credential).then(|| id.clone()))
        .ok_or(LobbyError::InvalidCapability)
}

fn running_lobby_from_session_config(config: &SessionConfig) -> LobbyState {
    let host_seat = config
        .player_ids
        .first()
        .cloned()
        .unwrap_or_else(|| PlayerId::new("host"));
    LobbyState {
        game_id: config.game_id.clone(),
        phase: LobbyPhase::Running,
        host_seat,
        player_ids: config.player_ids.clone(),
        seed: config.seed.unwrap_or_default(),
        lobby_version: 1,
        seats: config
            .seats
            .iter()
            .map(|(seat, controller)| {
                (
                    seat.clone(),
                    LobbySeat {
                        controller: controller.clone(),
                        ready: true,
                        seat_token: config.seat_tokens.get(seat).cloned(),
                        lease_expires_at_ms: None,
                    },
                )
            })
            .collect(),
    }
}

fn running_lobby_from_session(session: &GameSession) -> LobbyState {
    let (player_ids, seats, seat_tokens, seed) = session.lobby_details();
    running_lobby_from_session_config(&SessionConfig {
        game_id: session.id().to_owned(),
        state: session.current_state(),
        seats,
        seat_tokens,
        galaxy: None,
        map_tiles: Vec::new(),
        galaxy_layout: crate::map::GalaxyLayout {
            version: 1,
            active_sources: Vec::new(),
            placements: Vec::new(),
            off_map_system_ids: Vec::new(),
        },
        seed,
        player_ids,
        store: None,
        prior_decisions: Vec::new(),
        prior_events: Vec::new(),
    })
}

fn authenticated_seat(lobby: &LobbyState, token: &str) -> Result<PlayerId, LobbyError> {
    lobby
        .seats
        .iter()
        .find_map(|(seat, lobby_seat)| {
            (lobby_seat.seat_token.as_deref() == Some(token)).then(|| seat.clone())
        })
        .ok_or(LobbyError::InvalidCapability)
}

fn lobby_to_record(lobby: &LobbyState) -> LobbyRecord {
    LobbyRecord {
        game_id: lobby.game_id.clone(),
        phase: match lobby.phase {
            LobbyPhase::Lobby => PersistedLobbyPhase::Lobby,
            LobbyPhase::Running => PersistedLobbyPhase::Running,
        },
        host_seat: lobby.host_seat.clone(),
        player_ids: lobby.player_ids.clone(),
        seed: lobby.seed,
        lobby_version: lobby.lobby_version,
        seats: lobby
            .seats
            .iter()
            .map(|(seat, lobby_seat)| {
                (
                    seat.clone(),
                    PersistedLobbySeat {
                        controller: lobby_seat.controller.clone(),
                        ready: lobby_seat.ready,
                        seat_token: lobby_seat.seat_token.clone(),
                        lease_expires_at_ms: lobby_seat.lease_expires_at_ms,
                    },
                )
            })
            .collect(),
    }
}

fn lobby_from_record(record: LobbyRecord) -> LobbyState {
    LobbyState {
        game_id: record.game_id,
        phase: match record.phase {
            PersistedLobbyPhase::Lobby => LobbyPhase::Lobby,
            PersistedLobbyPhase::Running => LobbyPhase::Running,
        },
        host_seat: record.host_seat,
        player_ids: record.player_ids,
        seed: record.seed,
        lobby_version: record.lobby_version,
        seats: record
            .seats
            .into_iter()
            .map(|(seat, lobby_seat)| {
                (
                    seat,
                    LobbySeat {
                        controller: lobby_seat.controller,
                        ready: lobby_seat.ready,
                        // Pre-lease records have no proof of a live claim, so do not resurrect credentials.
                        seat_token: lobby_seat.lease_expires_at_ms.and(lobby_seat.seat_token),
                        lease_expires_at_ms: lobby_seat.lease_expires_at_ms,
                    },
                )
            })
            .collect(),
    }
}

fn new_token() -> String {
    format!("{:032x}", rand::random::<u128>())
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

fn legacy_running_lobby(init: &GameInitRecord) -> LobbyState {
    running_lobby_from_session_config(&SessionConfig {
        game_id: init.game_id.clone(),
        state: init.initial_state.clone(),
        seed: init.seed,
        galaxy: None,
        map_tiles: init.map_tiles.clone(),
        galaxy_layout: crate::map::GalaxyLayout {
            version: 1,
            active_sources: Vec::new(),
            placements: Vec::new(),
            off_map_system_ids: Vec::new(),
        },
        seats: init.seats.clone(),
        seat_tokens: init.seat_tokens.clone(),
        store: None,
        player_ids: init.player_ids.clone(),
        prior_decisions: Vec::new(),
        prior_events: Vec::new(),
    })
}
