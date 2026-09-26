//! Server to client messages.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use ti4_model::id::PlayerId;

use super::error::ErrorKind;
use super::status::{PublicTurnStatus, RejectionReason, ViewerRole};
use super::view::GameView;
use crate::map::GalaxyLayout;
pub use ti4_engine::choice::{Choice, ChoiceOption};
use ti4_model::state::GameState;
use ti4_model::state::Phase;

/// Server submission metadata around the engine's wire-serialized choice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PendingChoiceEnvelope {
    pub nonce: String,
    pub choice: Choice,
}

/// Initial per-viewer snapshot sent upon subscription or reconnection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InitialSnapshotMsg {
    pub protocol_version: u16,
    pub game_id: String,
    pub game_version: u64,
    pub viewer: ViewerRole,
    pub view: GameView,
    pub state: GameState,
    pub galaxy_layout: GalaxyLayout,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending_choice: Option<PendingChoiceEnvelope>,
    pub turn_status: PublicTurnStatus,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<GameEvent>,
    #[serde(default, skip_serializing_if = "is_default_history")]
    pub history: HistoryStatus,
}

/// Public cursor counts decisions, not engine steps or wall-clock events.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HistoryStatus {
    pub cursor: usize,
    pub redo_count: usize,
    #[serde(default)]
    pub generation: u64,
}

fn is_default_history(value: &HistoryStatus) -> bool {
    *value == HistoryStatus::default()
}

impl InitialSnapshotMsg {
    #[must_use]
    pub fn with_history(mut self, cursor: usize, redo_count: usize, generation: u64) -> Self {
        self.history = HistoryStatus {
            cursor,
            redo_count,
            generation,
        };
        self
    }
}

/// Explicit audience for an authoritative event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(tag = "visibility", content = "seat", rename_all = "snake_case")]
pub enum EventVisibility {
    Public,
    Seat(PlayerId),
    Referee,
}

impl EventVisibility {
    #[must_use]
    pub fn permits(&self, viewer: &ViewerRole) -> bool {
        match self {
            Self::Public => true,
            Self::Seat(seat) => viewer.is_actor(seat),
            Self::Referee => false,
        }
    }
}

/// Typed event payload. Visibility is never inferred from presentation text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum GameEventKind {
    GameInitialized {
        round: u32,
        phase: Phase,
        speaker: PlayerId,
    },
    DecisionResolved,
    PhaseTransition {
        phase: Phase,
        round: u32,
    },
    GameFinished {
        winner: Option<PlayerId>,
    },
}

/// Authoritative, auditable event log entry recorded during game execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GameEvent {
    pub id: String,
    pub timestamp: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<u64>,
    pub visibility: EventVisibility,
    pub event: GameEventKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub batch_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub movement: Option<MovementFact>,
}

/// Stable decision-cursor identity of the most recent engine action selection.
/// Histories predating action IDs still use the prompt boundary as a fallback.
#[must_use]
pub fn action_id_for(
    records: &[ti4_engine::choice::DecisionRecord],
    cursor: usize,
) -> Option<String> {
    records
        .get(..cursor)?
        .iter()
        .rposition(|record| record.prompt == "action phase")
        .map(|start| format!("action_{}", start + 1))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MovementFact {
    pub actor: PlayerId,
    pub origin: String,
    pub destination: String,
    pub unit: String,
}

/// Server message carrying a new game event to all subscribers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GameEventMsg {
    pub protocol_version: u16,
    pub game_id: String,
    pub entry: GameEvent,
}

/// Versioned state update or replacement snapshot after state transition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StateUpdateMsg {
    pub protocol_version: u16,
    pub game_id: String,
    pub game_version: u64,
    pub viewer: ViewerRole,
    pub view: GameView,
    pub state: GameState,
    pub galaxy_layout: GalaxyLayout,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending_choice: Option<PendingChoiceEnvelope>,
    pub turn_status: PublicTurnStatus,
    #[serde(default, skip_serializing_if = "is_default_history")]
    pub history: HistoryStatus,
}

impl StateUpdateMsg {
    #[must_use]
    pub fn with_history(mut self, cursor: usize, redo_count: usize, generation: u64) -> Self {
        self.history = HistoryStatus {
            cursor,
            redo_count,
            generation,
        };
        self
    }
}

/// Pending choice sent only to the acting seat.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PendingChoiceMsg {
    pub protocol_version: u16,
    pub game_id: String,
    pub game_version: u64,
    pub nonce: String,
    pub choice: Choice,
    pub state: GameState,
    pub galaxy_layout: GalaxyLayout,
}

/// Public turn status update.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TurnStatusMsg {
    pub protocol_version: u16,
    pub game_id: String,
    pub game_version: u64,
    pub status: PublicTurnStatus,
}

/// Confirmation that an action was validated and accepted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionAcceptedMsg {
    pub protocol_version: u16,
    pub game_id: String,
    pub game_version: u64,
    pub option_id: String,
}

/// Notification that an action submission was rejected.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionRejectedMsg {
    pub protocol_version: u16,
    pub game_id: String,
    pub game_version: u64,
    pub reason: RejectionReason,
}

/// Protocol error notification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProtocolErrorMsg {
    pub protocol_version: u16,
    pub kind: ErrorKind,
    pub message: String,
}

/// Notification of game completion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GameOverMsg {
    pub protocol_version: u16,
    pub game_id: String,
    pub game_version: u64,
    pub winner: Option<PlayerId>,
    pub final_scores: BTreeMap<PlayerId, u32>,
}

/// Keep-alive pong response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PongMsg {
    pub protocol_version: u16,
    pub sequence: u64,
}

/// Messages emitted from server to client.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ServerMessage {
    InitialSnapshot(InitialSnapshotMsg),
    StateUpdate(StateUpdateMsg),
    PendingChoice(PendingChoiceMsg),
    TurnStatus(TurnStatusMsg),
    ActionAccepted(ActionAcceptedMsg),
    ActionRejected(ActionRejectedMsg),
    Error(ProtocolErrorMsg),
    GameOver(GameOverMsg),
    Pong(PongMsg),
    Event(GameEventMsg),
}

impl ServerMessage {
    /// Returns the protocol version of this server message.
    #[must_use]
    pub fn protocol_version(&self) -> u16 {
        match self {
            Self::InitialSnapshot(m) => m.protocol_version,
            Self::StateUpdate(m) => m.protocol_version,
            Self::PendingChoice(m) => m.protocol_version,
            Self::TurnStatus(m) => m.protocol_version,
            Self::ActionAccepted(m) => m.protocol_version,
            Self::ActionRejected(m) => m.protocol_version,
            Self::Error(m) => m.protocol_version,
            Self::GameOver(m) => m.protocol_version,
            Self::Pong(m) => m.protocol_version,
            Self::Event(m) => m.protocol_version,
        }
    }

    /// Returns the game ID associated with this message, if applicable.
    #[must_use]
    pub fn game_id(&self) -> Option<&str> {
        match self {
            Self::InitialSnapshot(m) => Some(&m.game_id),
            Self::StateUpdate(m) => Some(&m.game_id),
            Self::PendingChoice(m) => Some(&m.game_id),
            Self::TurnStatus(m) => Some(&m.game_id),
            Self::ActionAccepted(m) => Some(&m.game_id),
            Self::ActionRejected(m) => Some(&m.game_id),
            Self::GameOver(m) => Some(&m.game_id),
            Self::Event(m) => Some(&m.game_id),
            Self::Error(_) | Self::Pong(_) => None,
        }
    }

    /// Returns the monotonic game version, if applicable.
    #[must_use]
    pub fn game_version(&self) -> Option<u64> {
        match self {
            Self::InitialSnapshot(m) => Some(m.game_version),
            Self::StateUpdate(m) => Some(m.game_version),
            Self::PendingChoice(m) => Some(m.game_version),
            Self::TurnStatus(m) => Some(m.game_version),
            Self::ActionAccepted(m) => Some(m.game_version),
            Self::ActionRejected(m) => Some(m.game_version),
            Self::GameOver(m) => Some(m.game_version),
            Self::Event(m) => m.entry.version,
            Self::Error(_) | Self::Pong(_) => None,
        }
    }
}
