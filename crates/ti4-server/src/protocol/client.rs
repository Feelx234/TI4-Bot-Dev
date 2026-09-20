//! Client to server messages.

use serde::{Deserialize, Serialize};

/// Messages submitted from a client to the authoritative server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ClientMessage {
    /// Subscribe to live updates for a game session.
    Subscribe {
        protocol_version: u16,
        game_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        seat_token: Option<String>,
    },
    /// Submit an engine-offered option ID for an outstanding decision.
    SubmitChoice {
        protocol_version: u16,
        game_id: String,
        nonce: String,
        expected_version: u64,
        option_id: String,
    },
    /// Keep-alive ping message.
    Ping {
        protocol_version: u16,
        sequence: u64,
    },
}

impl ClientMessage {
    /// Returns the protocol version advertised by this message.
    #[must_use]
    pub fn protocol_version(&self) -> u16 {
        match self {
            Self::Subscribe {
                protocol_version, ..
            }
            | Self::SubmitChoice {
                protocol_version, ..
            }
            | Self::Ping {
                protocol_version, ..
            } => *protocol_version,
        }
    }
}
