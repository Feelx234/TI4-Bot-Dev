//! Client to server messages.

use serde::{Deserialize, Serialize};
pub use ti4_model::state::ReactionMode;

/// Largest externally supplied protocol fields accepted by the server.
pub const MAX_GAME_ID_BYTES: usize = 64;
pub const MAX_PLAYER_SESSION_BYTES: usize = 128;
pub const MAX_NONCE_BYTES: usize = 128;
pub const MAX_OPTION_ID_BYTES: usize = 1024;
pub const MAX_CARD_NAME_BYTES: usize = 128;
/// The most entries a script may have. A whole tactical action is some tens of entries.
pub const MAX_SCRIPT_ENTRIES: usize = 512;

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
    /// Start an inactive authenticated player's tactical draft.
    StartPlanning {
        protocol_version: u16,
        game_id: String,
    },
    ResetPlanning {
        protocol_version: u16,
        game_id: String,
        identity: crate::planning::runner::AttemptIdentity,
    },
    EditPlanningMovement {
        protocol_version: u16,
        game_id: String,
        identity: crate::planning::runner::AttemptIdentity,
    },
    ApplyPlanning {
        protocol_version: u16,
        game_id: String,
        identity: crate::planning::runner::AttemptIdentity,
        nonce: String,
        expected_version: u64,
    },
    /// Answer an offer from the player's current planning attempt.
    SubmitPlanningChoice {
        protocol_version: u16,
        game_id: String,
        identity: crate::planning::runner::AttemptIdentity,
        option_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        request_id: Option<String>,
    },
    /// Draft this seat's secondary of the strategy card being resolved, on its own
    /// disposable fork and independently of any tactical draft.
    SecondaryPlanning {
        protocol_version: u16,
        game_id: String,
        request: SecondaryPlanningRequest,
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
    /// Play `script` as the next turn of this connection's seat on a copy of the game, and
    /// tell where the run ended (`ServerMessage::DraftOutcome` with this `request_id`). The game
    /// is not changed and the server keeps nothing of the script. Of the requests that wait on
    /// one connection only the newest is run: an older one gets no answer.
    ///
    /// With `scope` the script is of the secondary of this seat, for the strategic action that
    /// is resolved now (`secondary` of the update names it, and says whether the seat may draft).
    RunDraft {
        protocol_version: u16,
        game_id: String,
        request_id: String,
        script: Vec<ti4_view::planning::draft::DraftStep>,
        /// Left out: the next tactical action.
        #[serde(default)]
        scope: ti4_view::planning::audit::PlanScope,
    },
    /// Answer the pending choice of this connection's seat, and the choices after it, with
    /// `script`: a draft or a plan that is sent to the live game. A script that the game would
    /// refuse changes nothing; where a script stopped, or that it was played whole, is told
    /// with `ServerMessage::DraftApplied`.
    ApplyDraft {
        protocol_version: u16,
        game_id: String,
        nonce: String,
        expected_version: u64,
        script: Vec<ti4_view::planning::draft::DraftStep>,
    },
    /// Take back the last decision of this connection's seat, when the last update said that
    /// it may (`HistoryStatus::can_undo`): a movement, the landings and a purchase go back
    /// whole. The game is then another session: the connection is closed as replaced, and the
    /// next snapshot of the seat has the steps that were taken back. A refusal is
    /// `ServerMessage::ActionRejected`, with the barrier when one is the cause.
    Undo {
        protocol_version: u16,
        game_id: String,
        expected_version: u64,
    },
    /// Keep-alive ping message.
    Ping {
        protocol_version: u16,
        sequence: u64,
    },
}

/// One operation on the connection's own secondary draft.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum SecondaryPlanningRequest {
    Start {},
    Reset {
        identity: crate::planning::runner::AttemptIdentity,
    },
    Answer {
        identity: crate::planning::runner::AttemptIdentity,
        option_id: String,
    },
    /// Ready: submit the draft for this seat when the live window reaches it.
    SetReady {
        identity: crate::planning::runner::AttemptIdentity,
        ready: bool,
    },
}

impl SecondaryPlanningRequest {
    #[must_use]
    pub const fn identity(&self) -> Option<crate::planning::runner::AttemptIdentity> {
        match self {
            Self::Start {} => None,
            Self::Reset { identity }
            | Self::Answer { identity, .. }
            | Self::SetReady { identity, .. } => Some(*identity),
        }
    }
}

impl std::fmt::Debug for ClientMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SecondaryPlanning {
                game_id, request, ..
            } => f
                .debug_struct("SecondaryPlanning")
                .field("game_id", game_id)
                .field("request", request)
                .finish(),
            Self::ApplyPlanning {
                game_id,
                identity,
                nonce,
                expected_version,
                ..
            } => f
                .debug_struct("ApplyPlanning")
                .field("game_id", game_id)
                .field("identity", identity)
                .field("nonce", nonce)
                .field("expected_version", expected_version)
                .finish(),
            Self::ResetPlanning {
                game_id, identity, ..
            } => f
                .debug_struct("ResetPlanning")
                .field("game_id", game_id)
                .field("identity", identity)
                .finish(),
            Self::EditPlanningMovement {
                game_id, identity, ..
            } => f
                .debug_struct("EditPlanningMovement")
                .field("game_id", game_id)
                .field("identity", identity)
                .finish(),
            Self::StartPlanning { game_id, .. } => f
                .debug_struct("StartPlanning")
                .field("game_id", game_id)
                .finish(),
            Self::SubmitPlanningChoice {
                game_id,
                identity,
                option_id,
                ..
            } => f
                .debug_struct("SubmitPlanningChoice")
                .field("game_id", game_id)
                .field("identity", identity)
                .field("option_id", option_id)
                .finish(),
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
            Self::RunDraft {
                game_id,
                request_id,
                script,
                ..
            } => f
                .debug_struct("RunDraft")
                .field("game_id", game_id)
                .field("request_id", request_id)
                .field("entries", &script.len())
                .finish(),
            Self::ApplyDraft {
                game_id,
                nonce,
                expected_version,
                script,
                ..
            } => f
                .debug_struct("ApplyDraft")
                .field("game_id", game_id)
                .field("nonce", nonce)
                .field("expected_version", expected_version)
                .field("entries", &script.len())
                .finish(),
            Self::Undo {
                game_id,
                expected_version,
                ..
            } => f
                .debug_struct("Undo")
                .field("game_id", game_id)
                .field("expected_version", expected_version)
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
            }
            | Self::RunDraft {
                protocol_version, ..
            }
            | Self::ApplyDraft {
                protocol_version, ..
            }
            | Self::Undo {
                protocol_version, ..
            }
            | Self::StartPlanning {
                protocol_version, ..
            }
            | Self::ResetPlanning {
                protocol_version, ..
            }
            | Self::EditPlanningMovement {
                protocol_version, ..
            }
            | Self::ApplyPlanning {
                protocol_version, ..
            }
            | Self::SubmitPlanningChoice {
                protocol_version, ..
            }
            | Self::SecondaryPlanning {
                protocol_version, ..
            } => *protocol_version,
        }
    }

    /// Rejects variable-length client fields before they reach session state.
    pub fn validate_bounds(&self) -> Result<(), &'static str> {
        match self {
            Self::ApplyPlanning { game_id, nonce, .. } => {
                bounded(game_id, MAX_GAME_ID_BYTES, "game_id")?;
                bounded(nonce, MAX_NONCE_BYTES, "nonce")?;
            }
            Self::SecondaryPlanning {
                game_id, request, ..
            } => {
                bounded(game_id, MAX_GAME_ID_BYTES, "game_id")?;
                if let SecondaryPlanningRequest::Answer { option_id, .. } = request {
                    bounded(option_id, MAX_OPTION_ID_BYTES, "option_id")?;
                }
            }
            Self::StartPlanning { game_id, .. }
            | Self::ResetPlanning { game_id, .. }
            | Self::EditPlanningMovement { game_id, .. } => {
                bounded(game_id, MAX_GAME_ID_BYTES, "game_id")?
            }
            Self::SubmitPlanningChoice {
                game_id,
                option_id,
                request_id,
                ..
            } => {
                bounded(game_id, MAX_GAME_ID_BYTES, "game_id")?;
                bounded(option_id, MAX_OPTION_ID_BYTES, "option_id")?;
                if let Some(request_id) = request_id {
                    bounded(request_id, MAX_NONCE_BYTES, "request_id")?;
                }
            }
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
            Self::RunDraft {
                game_id,
                request_id,
                script,
                ..
            } => {
                bounded(game_id, MAX_GAME_ID_BYTES, "game_id")?;
                bounded(request_id, MAX_NONCE_BYTES, "request_id")?;
                bounded_script(script)?;
            }
            Self::ApplyDraft {
                game_id,
                nonce,
                script,
                ..
            } => {
                bounded(game_id, MAX_GAME_ID_BYTES, "game_id")?;
                bounded(nonce, MAX_NONCE_BYTES, "nonce")?;
                bounded_script(script)?;
            }
            Self::Undo { game_id, .. } => bounded(game_id, MAX_GAME_ID_BYTES, "game_id")?,
            Self::Ping { .. } => {}
        }
        Ok(())
    }
}

/// A script may be empty: a draft with nothing chosen opens the turn menu. Its texts are
/// bounded by the size of a message.
fn bounded_script(script: &[ti4_view::planning::draft::DraftStep]) -> Result<(), &'static str> {
    if script.len() > MAX_SCRIPT_ENTRIES {
        Err("script")
    } else {
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
    fn planning_messages_round_trip_and_validate_bounds() {
        let identity = crate::planning::runner::AttemptIdentity {
            checkpoint_id: 12,
            plan_revision: 3,
            generation_id: 2,
        };
        for message in [
            ClientMessage::StartPlanning {
                protocol_version: 1,
                game_id: "game".into(),
            },
            ClientMessage::ResetPlanning {
                protocol_version: 1,
                game_id: "game".into(),
                identity,
            },
            ClientMessage::ApplyPlanning {
                protocol_version: 1,
                game_id: "game".into(),
                identity,
                nonce: "current-choice".into(),
                expected_version: 9,
            },
            ClientMessage::SubmitPlanningChoice {
                protocol_version: 1,
                game_id: "game".into(),
                identity,
                option_id: "tactical".into(),
                request_id: Some("answer-request".into()),
            },
        ] {
            let encoded = serde_json::to_string(&message).unwrap();
            assert_eq!(
                serde_json::from_str::<ClientMessage>(&encoded).unwrap(),
                message
            );
            assert_eq!(message.validate_bounds(), Ok(()));
            assert_eq!(message.protocol_version(), 1);
        }
        assert_eq!(
            ClientMessage::StartPlanning {
                protocol_version: 1,
                game_id: String::new()
            }
            .validate_bounds(),
            Err("game_id")
        );
        assert_eq!(
            ClientMessage::SubmitPlanningChoice {
                protocol_version: 1,
                game_id: "game".into(),
                identity,
                option_id: "x".repeat(MAX_OPTION_ID_BYTES + 1),
                request_id: None,
            }
            .validate_bounds(),
            Err("option_id")
        );
        for request_id in [String::new(), "x".repeat(MAX_NONCE_BYTES + 1)] {
            assert_eq!(
                ClientMessage::SubmitPlanningChoice {
                    protocol_version: 1,
                    game_id: "game".into(),
                    identity,
                    option_id: "tactical".into(),
                    request_id: Some(request_id),
                }
                .validate_bounds(),
                Err("request_id")
            );
        }
        assert!(serde_json::from_value::<ClientMessage>(serde_json::json!({
            "type": "start_planning", "protocol_version": 1, "game_id": "game", "seat": "someone_else"
        })).is_err());
    }

    #[test]
    fn secondary_planning_messages_round_trip_and_validate_bounds() {
        let identity = crate::planning::runner::AttemptIdentity {
            checkpoint_id: 12,
            plan_revision: 3,
            generation_id: 2,
        };
        for request in [
            SecondaryPlanningRequest::Start {},
            SecondaryPlanningRequest::Reset { identity },
            SecondaryPlanningRequest::Answer {
                identity,
                option_id: "yes".into(),
            },
            SecondaryPlanningRequest::SetReady {
                identity,
                ready: true,
            },
        ] {
            let message = ClientMessage::SecondaryPlanning {
                protocol_version: 1,
                game_id: "game".into(),
                request: request.clone(),
            };
            let encoded = serde_json::to_string(&message).unwrap();
            assert_eq!(
                serde_json::from_str::<ClientMessage>(&encoded).unwrap(),
                message
            );
            assert_eq!(message.validate_bounds(), Ok(()));
            assert_eq!(
                request.identity().is_none(),
                request == SecondaryPlanningRequest::Start {}
            );
        }
        assert_eq!(
            ClientMessage::SecondaryPlanning {
                protocol_version: 1,
                game_id: "game".into(),
                request: SecondaryPlanningRequest::Answer {
                    identity,
                    option_id: String::new(),
                },
            }
            .validate_bounds(),
            Err("option_id")
        );
        // The seat is the connection's: a request cannot name another.
        assert!(
            serde_json::from_value::<ClientMessage>(serde_json::json!({
                "type": "secondary_planning", "protocol_version": 1, "game_id": "game",
                "request": {"action": "start", "seat": "someone_else"}
            }))
            .is_err()
        );
    }

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
