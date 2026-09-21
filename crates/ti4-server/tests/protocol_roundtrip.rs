use std::collections::BTreeMap;
use ti4_engine::decision_context::{ConstraintKind, DecisionSource};
use ti4_model::id::PlayerId;
use ti4_model::state::Phase;
use ti4_server::protocol::choice::{
    ChoiceOptionDto, DecisionContextDto, OutstandingConstraintDto, PendingChoiceDto,
};
use ti4_server::protocol::client::ClientMessage;
use ti4_server::protocol::error::{ErrorKind, ProtocolError};
use ti4_server::protocol::server::{
    ActionAcceptedMsg, ActionRejectedMsg, GameOverMsg, PendingChoiceMsg, ProtocolErrorMsg,
    ServerMessage,
};
use ti4_server::protocol::status::RejectionReason;
use ti4_server::protocol::{
    PROTOCOL_VERSION, parse_client_message, parse_server_message, validate_protocol_version,
};

#[test]
fn client_subscribe_round_trips() {
    let msg = ClientMessage::Subscribe {
        protocol_version: PROTOCOL_VERSION,
        game_id: "game_abc".to_owned(),
        seat_token: Some("secret_token_123".to_owned()),
    };
    let json = serde_json::to_string_pretty(&msg).expect("serialize");
    assert!(json.contains("\"type\": \"subscribe\""));
    let deserialized: ClientMessage = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(msg, deserialized);
    assert_eq!(parse_client_message(&json).expect("parse"), msg);
}

#[test]
fn client_submit_choice_round_trips() {
    let msg = ClientMessage::SubmitChoice {
        protocol_version: PROTOCOL_VERSION,
        game_id: "game_abc".to_owned(),
        nonce: "nonce_456".to_owned(),
        expected_version: 12,
        option_id: "opt_tactical_activate".to_owned(),
    };
    let json = serde_json::to_string(&msg).expect("serialize");
    assert!(json.contains("\"type\":\"submit_choice\""));
    let deserialized: ClientMessage = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(msg, deserialized);
    assert_eq!(parse_client_message(&json).expect("parse"), msg);
}

#[test]
fn client_ping_round_trips() {
    let msg = ClientMessage::Ping {
        protocol_version: PROTOCOL_VERSION,
        sequence: 101,
    };
    let json = serde_json::to_string(&msg).expect("serialize");
    let deserialized = parse_client_message(&json).expect("parse");
    assert_eq!(msg, deserialized);
}

#[test]
fn server_action_accepted_round_trips() {
    let msg = ServerMessage::ActionAccepted(ActionAcceptedMsg {
        protocol_version: PROTOCOL_VERSION,
        game_id: "game_abc".to_owned(),
        game_version: 13,
        option_id: "opt_pass".to_owned(),
    });
    let json = serde_json::to_string(&msg).expect("serialize");
    assert!(json.contains("\"type\":\"action_accepted\""));
    let deserialized = parse_server_message(&json).expect("parse");
    assert_eq!(msg, deserialized);
}

#[test]
fn server_action_rejected_round_trips() {
    let msg = ServerMessage::ActionRejected(ActionRejectedMsg {
        protocol_version: PROTOCOL_VERSION,
        game_id: "game_abc".to_owned(),
        game_version: 12,
        reason: RejectionReason::StaleNonce,
    });
    let json = serde_json::to_string(&msg).expect("serialize");
    assert!(json.contains("\"type\":\"action_rejected\""));
    assert!(json.contains("\"reason\":\"stale_nonce\""));
    let deserialized = parse_server_message(&json).expect("parse");
    assert_eq!(msg, deserialized);
}

#[test]
fn server_error_round_trips() {
    let msg = ServerMessage::Error(ProtocolErrorMsg {
        protocol_version: PROTOCOL_VERSION,
        kind: ErrorKind::MalformedMessage,
        message: "Invalid payload formatting".to_owned(),
    });
    let json = serde_json::to_string(&msg).expect("serialize");
    let deserialized = parse_server_message(&json).expect("parse");
    assert_eq!(msg, deserialized);
}

#[test]
fn server_game_over_round_trips() {
    let mut scores = BTreeMap::new();
    scores.insert(PlayerId::new("player_1"), 10);
    scores.insert(PlayerId::new("player_2"), 8);

    let msg = ServerMessage::GameOver(GameOverMsg {
        protocol_version: PROTOCOL_VERSION,
        game_id: "game_abc".to_owned(),
        game_version: 120,
        winner: Some(PlayerId::new("player_1")),
        final_scores: scores,
    });
    let json = serde_json::to_string(&msg).expect("serialize");
    let deserialized = parse_server_message(&json).expect("parse");
    assert_eq!(msg, deserialized);
}

#[test]
fn server_pending_choice_round_trips() {
    let msg = ServerMessage::PendingChoice(PendingChoiceMsg {
        protocol_version: PROTOCOL_VERSION,
        game_id: "game_abc".to_owned(),
        game_version: 14,
        choice: PendingChoiceDto {
            nonce: "nonce_abc".to_owned(),
            actor: PlayerId::new("player_1"),
            prompt: "Select strategy card".to_owned(),
            options: vec![
                ChoiceOptionDto {
                    id: "leadership".to_owned(),
                    kind: "pick_strategy_card".to_owned(),
                    label: "1 - Leadership".to_owned(),
                    payload: BTreeMap::new(),
                },
                ChoiceOptionDto {
                    id: "diplomacy".to_owned(),
                    kind: "pick_strategy_card".to_owned(),
                    label: "2 - Diplomacy".to_owned(),
                    payload: BTreeMap::new(),
                },
            ],
            context: Some(DecisionContextDto {
                version: 1,
                actor: PlayerId::new("player_1"),
                source: DecisionSource::Rule("83.2".to_owned()),
                subtype: "pick_strategy_card".to_owned(),
                phase: Phase::Strategy,
                round: 1,
                optional: false,
                target: None,
                outstanding: vec![OutstandingConstraintDto {
                    kind: ConstraintKind::CommandTokens,
                    amount: 3,
                    paid: 0,
                }],
            }),
        },
    });
    let json = serde_json::to_string(&msg).expect("serialize");
    let deserialized = parse_server_message(&json).expect("parse");
    assert_eq!(msg, deserialized);
}

#[test]
fn server_event_round_trips() {
    let msg = ServerMessage::Event(ti4_server::protocol::server::GameEventMsg {
        protocol_version: PROTOCOL_VERSION,
        game_id: "game_abc".to_owned(),
        entry: ti4_server::protocol::server::GameEvent {
            id: "game_abc-1".to_owned(),
            timestamp: "12:34:56".to_owned(),
            version: Some(2),
            visibility: ti4_server::protocol::server::EventVisibility::Public,
            event: ti4_server::protocol::server::GameEventKind::DecisionResolved,
        },
    });
    let json = serde_json::to_string(&msg).expect("serialize");
    assert!(json.contains("\"type\":\"event\""));
    assert!(json.contains("\"kind\":\"decision_resolved\""));
    let deserialized = parse_server_message(&json).expect("parse");
    assert_eq!(msg, deserialized);
}

#[test]
fn unknown_protocol_version_is_rejected() {
    let res = validate_protocol_version(99);
    assert_eq!(
        res,
        Err(ProtocolError::UnsupportedVersion {
            found: 99,
            expected: PROTOCOL_VERSION
        })
    );

    let client_json = r#"{
        "type": "ping",
        "protocol_version": 99,
        "sequence": 1
    }"#;
    assert!(matches!(
        parse_client_message(client_json),
        Err(ProtocolError::UnsupportedVersion { found: 99, .. })
    ));
}

#[test]
fn unknown_wire_fields_are_rejected() {
    let client_json_with_extra = r#"{
        "type": "ping",
        "protocol_version": 2,
        "sequence": 1,
        "extra_field": "unexpected"
    }"#;
    assert!(matches!(
        parse_client_message(client_json_with_extra),
        Err(ProtocolError::Json(_))
    ));

    let server_json_with_extra = r#"{
        "type": "pong",
        "protocol_version": 2,
        "sequence": 1,
        "unexpected": true
    }"#;
    assert!(matches!(
        parse_server_message(server_json_with_extra),
        Err(ProtocolError::Json(_))
    ));
}
