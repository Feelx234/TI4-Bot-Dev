//! Client to server messages.

use serde::{Deserialize, Serialize};
pub use ti4_model::state::ReactionMode;

/// Largest externally supplied protocol fields accepted by the server.
pub const MAX_GAME_ID_BYTES: usize = 64;
pub const MAX_PLAYER_SESSION_BYTES: usize = 128;
pub const MAX_NONCE_BYTES: usize = 128;
pub const MAX_OPTION_ID_BYTES: usize = 1024;
pub const MAX_CARD_NAME_BYTES: usize = 128;
/// Most trigger ids one `SetReactionIntent` may carry (the server then allows fewer, see
/// `session::bluff::MAX_DECLARED_TRIGGERS`) and the longest id.
pub const MAX_INTENT_TRIGGERS: usize = 16;
pub const MAX_TRIGGER_ID_BYTES: usize = 32;

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
    /// Declare which kinds of reaction window this connection's seat wants to bluff about.
    ///
    /// The whole declaration replaces the previous one; re-sending the current one is a no-op.
    /// Ephemeral and private to the seat: the server never persists or logs it, and other
    /// viewers never see it. Older servers answer an unknown message type with a malformed
    /// message error (the protocol version is not bumped, as for `SetReactionMode`), so clients
    /// send it only when the player has something declared.
    SetReactionIntent {
        protocol_version: u16,
        game_id: String,
        triggers: Vec<String>,
    },
    /// End the seat's running bluff hold early.
    PassReactionHold {
        protocol_version: u16,
        game_id: String,
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
            Self::SetReactionIntent {
                protocol_version,
                game_id,
                triggers,
            } => f
                .debug_struct("SetReactionIntent")
                .field("protocol_version", protocol_version)
                .field("game_id", game_id)
                .field("triggers", &format_args!("[{} declared]", triggers.len()))
                .finish(),
            Self::PassReactionHold {
                protocol_version,
                game_id,
            } => f
                .debug_struct("PassReactionHold")
                .field("protocol_version", protocol_version)
                .field("game_id", game_id)
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
            | Self::SetReactionIntent {
                protocol_version, ..
            }
            | Self::PassReactionHold {
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
            Self::SetReactionIntent {
                game_id, triggers, ..
            } => {
                bounded(game_id, MAX_GAME_ID_BYTES, "game_id")?;
                if triggers.len() > MAX_INTENT_TRIGGERS {
                    return Err("triggers");
                }
                for trigger in triggers {
                    bounded(trigger, MAX_TRIGGER_ID_BYTES, "triggers")?;
                }
            }
            Self::PassReactionHold { game_id, .. } => {
                bounded(game_id, MAX_GAME_ID_BYTES, "game_id")?;
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
    fn reaction_intent_messages_round_trip_and_are_bounded() {
        let text = r#"{"type":"set_reaction_intent","protocol_version":3,"game_id":"g","triggers":["agenda","movement"]}"#;
        let message: ClientMessage = serde_json::from_str(text).expect("decodes");
        assert_eq!(
            message,
            ClientMessage::SetReactionIntent {
                protocol_version: 3,
                game_id: "g".to_owned(),
                triggers: vec!["agenda".to_owned(), "movement".to_owned()],
            }
        );
        assert_eq!(serde_json::to_string(&message).expect("encodes"), text);
        assert_eq!(message.validate_bounds(), Ok(()));
        // The seat is the connection's, never named by the message.
        assert!(serde_json::from_str::<ClientMessage>(
            r#"{"type":"set_reaction_intent","protocol_version":3,"game_id":"g","triggers":[],"seat":"p2"}"#
        ).is_err());
        let too_many = ClientMessage::SetReactionIntent {
            protocol_version: 3,
            game_id: "g".to_owned(),
            triggers: vec!["agenda".to_owned(); MAX_INTENT_TRIGGERS + 1],
        };
        assert_eq!(too_many.validate_bounds(), Err("triggers"));
        let long = ClientMessage::SetReactionIntent {
            protocol_version: 3,
            game_id: "g".to_owned(),
            triggers: vec!["x".repeat(MAX_TRIGGER_ID_BYTES + 1)],
        };
        assert_eq!(long.validate_bounds(), Err("triggers"));
        let empty_id = ClientMessage::SetReactionIntent {
            protocol_version: 3,
            game_id: "g".to_owned(),
            triggers: vec![String::new()],
        };
        assert_eq!(empty_id.validate_bounds(), Err("triggers"));
        // Clearing is a valid, empty declaration.
        let clear = ClientMessage::SetReactionIntent {
            protocol_version: 3,
            game_id: "g".to_owned(),
            triggers: vec![],
        };
        assert_eq!(clear.validate_bounds(), Ok(()));
        let pass = r#"{"type":"pass_reaction_hold","protocol_version":3,"game_id":"g"}"#;
        let message: ClientMessage = serde_json::from_str(pass).expect("decodes");
        assert_eq!(serde_json::to_string(&message).expect("encodes"), pass);
        assert_eq!(message.validate_bounds(), Ok(()));
        // The declaration never shows up in debug output either.
        let shown = format!("{:?}", ClientMessage::SetReactionIntent {
            protocol_version: 3,
            game_id: "g".to_owned(),
            triggers: vec!["agenda".to_owned()],
        });
        assert!(!shown.contains("agenda"));
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
