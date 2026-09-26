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

/// Public facts derived from the engine's offered option, never from a submitted plan.
/// Unrecognized choices stay generic so private option payloads cannot enter the public log.
#[must_use]
pub fn public_decision_facts(
    record: &ti4_engine::choice::DecisionRecord,
    offered: Option<&ChoiceOption>,
    destination: Option<&str>,
) -> (Option<String>, Option<MovementFact>) {
    let Some(context) = record.context.as_ref() else {
        return (None, None);
    };
    let Some(option) = offered.filter(|option| option.id == record.chosen) else {
        return (None, None);
    };
    let actor = &record.player;
    let detail = match (
        context.subtype.as_str(),
        option.kind.as_str(),
        option.id.as_str(),
    ) {
        ("pay_resources" | "pay_influence", "pay", "trade_good") => {
            Some(format!("{actor} spent a trade good"))
        }
        ("pay_resources" | "pay_influence", "pay", id) => id
            .strip_prefix("exhaust|")
            .filter(|planet| !planet.is_empty())
            .map(|planet| format!("{actor} exhausted {planet}")),
        ("vote_exhaust_planet", _, "decline") => Some("Done voting".into()),
        ("vote_exhaust_planet", "vote_planet", _) => {
            Some(format!("{actor} exhausted {} to vote", option.id))
        }
        ("produce_unit", _, "done_producing") => Some("Done producing".into()),
        ("produce_unit", "produce", _) => {
            let unit = option
                .payload
                .get("unit")
                .and_then(serde_json::Value::as_str);
            let count = option
                .payload
                .get("count")
                .and_then(serde_json::Value::as_u64);
            unit.zip(count)
                .map(|(unit, count)| format!("{actor} produced {count} {unit}"))
        }
        ("load_cargo", _, "done_loading") => Some("Done loading".into()),
        ("load_cargo", "load", _) => option
            .payload
            .get("unit")
            .and_then(serde_json::Value::as_str)
            .map(|unit| format!("{actor} loaded {unit}")),
        ("movement_step", _, "done_moving") => Some("Done moving".into()),
        ("movement_step", "move", _) => {
            let origin = option
                .payload
                .get("origin")
                .and_then(serde_json::Value::as_str);
            let unit = option
                .payload
                .get("unit")
                .and_then(serde_json::Value::as_str);
            origin
                .zip(unit)
                .zip(destination)
                .map(|((origin, unit), destination)| {
                    format!("{actor} moved {unit} from #{origin} to #{destination}")
                })
        }
        _ => None,
    };
    let movement = if context.subtype == "movement_step" && option.kind == "move" {
        option
            .payload
            .get("origin")
            .and_then(serde_json::Value::as_str)
            .zip(
                option
                    .payload
                    .get("unit")
                    .and_then(serde_json::Value::as_str),
            )
            .zip(destination)
            .map(|((origin, unit), destination)| MovementFact {
                actor: actor.clone(),
                origin: origin.into(),
                destination: destination.into(),
                unit: unit.into(),
            })
    } else {
        None
    };
    (detail, movement)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MovementFact {
    pub actor: PlayerId,
    pub origin: String,
    pub destination: String,
    pub unit: String,
}

#[cfg(test)]
mod fact_tests {
    use super::*;
    use ti4_engine::choice::DecisionRecord;
    use ti4_engine::decision_context::{DecisionContext, DecisionSource};

    fn record(subtype: &str, option: &ChoiceOption) -> DecisionRecord {
        let player = PlayerId::new("p1");
        DecisionRecord {
            player: player.clone(),
            prompt: "test".into(),
            chosen: option.id.clone(),
            offered: vec![option.id.clone()],
            context: Some(DecisionContext::new(
                player,
                DecisionSource::Rule("test".into()),
                subtype,
                Phase::Action,
                1,
            )),
        }
    }

    #[test]
    fn movement_facts_require_a_matching_offered_option_and_destination() {
        let move_option = ChoiceOption::new("move|16|fighter", "move")
            .with("origin", "16")
            .with("unit", "fighter");
        let record = record("movement_step", &move_option);
        let (detail, movement) = public_decision_facts(&record, Some(&move_option), Some("22"));
        assert_eq!(detail.as_deref(), Some("p1 moved fighter from #16 to #22"));
        assert_eq!(movement.unwrap().destination, "22");
        assert_eq!(
            public_decision_facts(&record, Some(&move_option), None),
            (None, None)
        );
        assert_eq!(
            public_decision_facts(
                &record,
                Some(&ChoiceOption::new("other", "move")),
                Some("22")
            ),
            (None, None)
        );
    }

    #[test]
    fn public_facts_do_not_echo_unknown_or_private_option_payloads() {
        let secret = ChoiceOption::new("secret_card", "play_card").with("card", "private");
        assert_eq!(
            public_decision_facts(&record("play_card", &secret), Some(&secret), None),
            (None, None)
        );
        let production = ChoiceOption::new("build|fighter|2", "produce")
            .with("unit", "fighter")
            .with("count", 2);
        assert_eq!(
            public_decision_facts(
                &record("produce_unit", &production),
                Some(&production),
                None
            )
            .0
            .as_deref(),
            Some("p1 produced 2 fighter")
        );
    }

    #[test]
    fn action_selection_is_included_in_its_own_action_id() {
        let action = DecisionRecord {
            prompt: "action phase".into(),
            ..record("select_action", &ChoiceOption::new("tactical", "action"))
        };
        assert_eq!(
            action_id_for(&[action.clone()], 1).as_deref(),
            Some("action_1")
        );
        assert_eq!(
            action_id_for(&[action.clone(), action], 2).as_deref(),
            Some("action_2")
        );
    }
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
