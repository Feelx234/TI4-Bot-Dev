//! Client to server messages.

use serde::{Deserialize, Serialize};
pub use ti4_model::state::ReactionMode;

/// Largest externally supplied protocol fields accepted by the server.
pub const MAX_GAME_ID_BYTES: usize = 64;
pub const MAX_PLAYER_SESSION_BYTES: usize = 128;
pub const MAX_NONCE_BYTES: usize = 128;
pub const MAX_OPTION_ID_BYTES: usize = 1024;
pub const MAX_CARD_NAME_BYTES: usize = 128;

/// Messages submitted from a client to the authoritative server.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ClientMessage {
    /// Subscribe to live updates for a game session.
    Subscribe {
        protocol_version: u16,
        game_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        player_session: Option<String>,
    },
    /// Submit an engine-offered option ID for an outstanding decision.
    SubmitChoice {
        protocol_version: u16,
        game_id: String,
        nonce: String,
        expected_version: u64,
        option_id: String,
    },
    /// Set how this connection's own seat wants one action card handled in reaction windows.
    ///
    /// The seat is the connection's, never named by the message, so a player can only change
    /// their own modes and a spectator cannot change any. `card` is the printed card name: every
    /// copy of it is covered, for the rest of the game.
    SetReactionMode {
        protocol_version: u16,
        game_id: String,
        card: String,
        mode: ReactionMode,
    },
    /// Keep-alive ping message.
    Ping {
        protocol_version: u16,
        sequence: u64,
    },
}

impl std::fmt::Debug for ClientMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Subscribe {
                protocol_version,
                game_id,
                player_session,
            } => f
                .debug_struct("Subscribe")
                .field("protocol_version", protocol_version)
                .field("game_id", game_id)
                .field(
                    "player_session",
                    &player_session.as_ref().map(|_| "[redacted]"),
                )
                .finish(),
            Self::SubmitChoice {
                protocol_version,
                game_id,
                nonce,
                expected_version,
                option_id,
            } => f
                .debug_struct("SubmitChoice")
                .field("protocol_version", protocol_version)
                .field("game_id", game_id)
                .field("nonce", nonce)
                .field("expected_version", expected_version)
                .field("option_id", option_id)
                .finish(),
            Self::SetReactionMode {
                protocol_version,
                game_id,
                card,
                mode,
            } => f
                .debug_struct("SetReactionMode")
                .field("protocol_version", protocol_version)
                .field("game_id", game_id)
                .field("card", card)
                .field("mode", mode)
                .finish(),
            Self::Ping {
                protocol_version,
                sequence,
            } => f
                .debug_struct("Ping")
                .field("protocol_version", protocol_version)
                .field("sequence", sequence)
                .finish(),
        }
    }
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
            | Self::SetReactionMode {
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
                player_session,
                ..
            } => {
                bounded(game_id, MAX_GAME_ID_BYTES, "game_id")?;
                if let Some(token) = player_session {
                    bounded(token, MAX_PLAYER_SESSION_BYTES, "player_session")?;
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
            Self::SetReactionMode { game_id, card, .. } => {
                bounded(game_id, MAX_GAME_ID_BYTES, "game_id")?;
                bounded(card, MAX_CARD_NAME_BYTES, "card")?;
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

    #[test]
    fn set_reaction_mode_round_trips_and_is_bounded() {
        let text = r#"{"type":"set_reaction_mode","protocol_version":1,"game_id":"g","card":"Sabotage","mode":"never"}"#;
        let message: ClientMessage = serde_json::from_str(text).expect("decodes");
        assert_eq!(
            message,
            ClientMessage::SetReactionMode {
                protocol_version: 1,
                game_id: "g".to_owned(),
                card: "Sabotage".to_owned(),
                mode: ReactionMode::Never,
            }
        );
        assert_eq!(serde_json::to_string(&message).expect("encodes"), text);
        assert_eq!(message.validate_bounds(), Ok(()));
        let long = ClientMessage::SetReactionMode {
            protocol_version: 1,
            game_id: "g".to_owned(),
            card: "x".repeat(MAX_CARD_NAME_BYTES + 1),
            mode: ReactionMode::Always,
        };
        assert_eq!(long.validate_bounds(), Err("card"));
        // An unknown mode or an extra seat field is not accepted.
        assert!(serde_json::from_str::<ClientMessage>(
            r#"{"type":"set_reaction_mode","protocol_version":1,"game_id":"g","card":"Sabotage","mode":"sometimes"}"#
        ).is_err());
        assert!(serde_json::from_str::<ClientMessage>(
            r#"{"type":"set_reaction_mode","protocol_version":1,"game_id":"g","card":"Sabotage","mode":"never","seat":"p2"}"#
        ).is_err());
    }
}
