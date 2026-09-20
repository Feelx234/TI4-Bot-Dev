//! Server to client messages.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use ti4_model::id::PlayerId;

use super::choice::PendingChoiceDto;
use super::error::ErrorKind;
use super::status::{PublicTurnStatus, RejectionReason, ViewerRole};
use super::view::GameView;

/// Initial per-viewer snapshot sent upon subscription or reconnection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InitialSnapshotMsg {
    pub protocol_version: u16,
    pub game_id: String,
    pub game_version: u64,
    pub viewer: ViewerRole,
    pub view: GameView,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending_choice: Option<PendingChoiceDto>,
    pub turn_status: PublicTurnStatus,
}

/// Versioned state update or replacement snapshot after state transition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StateUpdateMsg {
    pub protocol_version: u16,
    pub game_id: String,
    pub game_version: u64,
    pub viewer: ViewerRole,
    pub view: GameView,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending_choice: Option<PendingChoiceDto>,
    pub turn_status: PublicTurnStatus,
}

/// Pending choice sent only to the acting seat.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PendingChoiceMsg {
    pub protocol_version: u16,
    pub game_id: String,
    pub game_version: u64,
    pub choice: PendingChoiceDto,
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
            Self::Error(_) | Self::Pong(_) => None,
        }
    }
}
