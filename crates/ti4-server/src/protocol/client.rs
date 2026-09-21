//! Client to server messages.

use serde::{Deserialize, Serialize};

/// Largest externally supplied protocol fields accepted by the server.
pub const MAX_GAME_ID_BYTES: usize = 64;
pub const MAX_SEAT_TOKEN_BYTES: usize = 128;
pub const MAX_NONCE_BYTES: usize = 128;
pub const MAX_OPTION_ID_BYTES: usize = 1024;

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

    /// Rejects variable-length client fields before they reach session state.
    pub fn validate_bounds(&self) -> Result<(), &'static str> {
        match self {
            Self::Subscribe {
                game_id,
                seat_token,
                ..
            } => {
                bounded(game_id, MAX_GAME_ID_BYTES, "game_id")?;
                if let Some(token) = seat_token {
                    bounded(token, MAX_SEAT_TOKEN_BYTES, "seat_token")?;
                }
            }
            Self::SubmitChoice {
                game_id,
                nonce,
                option_id,
                ..
            } => {
                bounded(game_id, MAX_GAME_ID_BYTES, "game_id")?;
                bounded(nonce, MAX_NONCE_BYTES, "nonce")?;
                bounded(option_id, MAX_OPTION_ID_BYTES, "option_id")?;
            }
            Self::Ping { .. } => {}
        }
        Ok(())
    }
}

fn bounded(value: &str, limit: usize, field: &'static str) -> Result<(), &'static str> {
    if value.is_empty() || value.len() > limit {
        Err(field)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_oversized_client_fields() {
        let message = ClientMessage::SubmitChoice {
            protocol_version: 1,
            game_id: "game".to_owned(),
            nonce: "n".repeat(MAX_NONCE_BYTES + 1),
            expected_version: 1,
            option_id: "option".to_owned(),
        };

        assert_eq!(message.validate_bounds(), Err("nonce"));
    }
}
