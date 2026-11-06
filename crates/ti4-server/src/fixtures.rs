//! Helpers to generate protocol golden fixtures for tests and documentation.

use serde_json::json;
use std::collections::BTreeMap;
use ti4_engine::choice::{Choice, ChoiceOption};
use ti4_engine::decision_context::{
    ConstraintKind, DecisionContext, DecisionSource, OutstandingConstraint,
};
use ti4_model::id::{
    ActionCardId, FactionId, ObjectiveId, PlayerId, SecretObjectiveId, StrategyCardId, SystemId,
    UnitTypeId,
};
use ti4_model::state::{GameState, Phase, SystemState};
use ti4_model::units::Unit;

use ti4_host::projection::project_initial_snapshot;
use ti4_host::protocol::PROTOCOL_VERSION;
use ti4_host::protocol::server::{ActionRejectedMsg, GameOverMsg, ServerMessage};
use ti4_host::protocol::status::{RejectionReason, ViewerRole};

/// Creates a standard sample game state for fixture generation and testing.
///
/// # Panics
///
/// Panics if the created player seats cannot be found in the fresh game state.
#[must_use]
pub fn create_sample_game() -> GameState {
    let seat_a = PlayerId::new("seat_a");
    let seat_b = PlayerId::new("seat_b");
    let seat_c = PlayerId::new("seat_c");
    let players = [seat_a.clone(), seat_b.clone(), seat_c.clone()];

    let mut state = GameState::new(&players, &[], BTreeMap::new(), None, 1);
    state.round = 2;
    state.phase = Phase::Action;
    state.active = Some(seat_a.clone());
    state.speaker = seat_a.clone();

    // Configure Player A
    {
        let p_a = state.player_mut(&seat_a).unwrap();
        p_a.faction = FactionId::new("sol");
        p_a.victory_points = 3;
        p_a.trade_goods = 4;
        p_a.commodities = 2;
        p_a.tactic_tokens = 3;
        p_a.fleet_tokens = 4;
        p_a.strategic_tokens = 2;
        p_a.strategy_cards = vec![StrategyCardId::new("pok1leadership")];
        p_a.action_cards = vec![
            ActionCardId::new("dh1"),
            ActionCardId::new("mb1"),
        ];
        p_a.secret_objectives = vec![SecretObjectiveId::new("dtgs")];
    }

    // Configure Player B
    {
        let p_b = state.player_mut(&seat_b).unwrap();
        p_b.faction = FactionId::new("hacan");
        p_b.victory_points = 2;
        p_b.trade_goods = 8;
        p_b.commodities = 6;
        p_b.tactic_tokens = 2;
        p_b.fleet_tokens = 3;
        p_b.strategic_tokens = 3;
        p_b.strategy_cards = vec![StrategyCardId::new("pok5trade")];
        p_b.action_cards = vec![ActionCardId::new("fs1")];
        p_b.secret_objectives = vec![SecretObjectiveId::new("btv")];
    }

    // Configure Player C
    {
        let p_c = state.player_mut(&seat_c).unwrap();
        p_c.faction = FactionId::new("letnev");
        p_c.victory_points = 1;
        p_c.trade_goods = 1;
        p_c.strategy_cards = vec![StrategyCardId::new("pok6warfare")];
        p_c.action_cards = vec![ActionCardId::new("sh1")];
        p_c.secret_objectives = vec![SecretObjectiveId::new("uf")];
    }

    // Add board systems and units
    let sys_18 = SystemId::new("18");
    let mut s18 = SystemState::default();
    s18.units.extend_from_slice(&[
        Unit::new(UnitTypeId::new("carrier"), seat_a.clone()),
        Unit::new(UnitTypeId::new("fighter"), seat_a.clone()),
    ]);
    s18.command_tokens.insert(seat_a.clone());
    state.board.insert(sys_18, s18);

    state.revealed_objectives = vec![
        ObjectiveId::new("corner"),
        ObjectiveId::new("develop"),
    ];

    state
}

/// [`create_sample_game`] as a table the typed core can play: the same three seats (Sol, Hacan, Letnev) in the action phase of round 2 with the same
/// economy, but real strategy card, action card and objective ids and a deployed board (the sample's ids are real too since integration 22). For the sessions that actually step.
///
/// # Panics
/// If setup refuses the three seats.
#[must_use]
pub fn create_playable_sample_game() -> GameState {
    let seat_a = PlayerId::new("seat_a");
    let mut state = ti4_engine::fixtures::seated_dealt(
        &[("seat_a", "sol"), ("seat_b", "hacan"), ("seat_c", "letnev")],
        ti4_model::content_types::POK,
    );
    state.round = 2;
    state.speaker = seat_a.clone();
    for (seat, cards, tg, tactic, fleet, strategic, card) in [
        ("seat_a", 2, 4, 3, 4, 2, "pok1leadership"),
        ("seat_b", 1, 8, 2, 3, 3, "pok5trade"),
        ("seat_c", 1, 1, 3, 3, 2, "pok6warfare"),
    ] {
        let id = PlayerId::new(seat);
        let p = state.player_mut(&id).unwrap();
        p.trade_goods = tg;
        p.tactic_tokens = tactic;
        p.fleet_tokens = fleet;
        p.strategic_tokens = strategic;
        p.strategy_cards = vec![StrategyCardId::new(card)];
        p.action_cards = ["abs", "assassin"].iter().take(cards).map(|c| ActionCardId::new(*c)).collect();
        state.unclaimed_strategy_cards.retain(|c| c.as_str() != card);
    }
    ti4_engine::builders::phase::advance_phase(&mut state);
    ti4_engine::builders::phase::begin_action_turn(&mut state, &seat_a);
    state.round = 2;
    state
}

/// Creates a sample pending choice with decision context and an outstanding constraint.
#[must_use]
pub fn create_sample_pending_choice() -> Choice {
    let seat_a = PlayerId::new("seat_a");
    let options = vec![
        ChoiceOption::labelled("opt_carrier", "produce_unit", "Produce Carrier (Cost: 3)")
            .with("cost", json!(3))
            .with("unit", json!("carrier")),
        ChoiceOption::labelled(
            "opt_infantry",
            "produce_unit",
            "Produce 2 Infantry (Cost: 1)",
        )
        .with("cost", json!(1))
        .with("unit", json!("infantry")),
        ChoiceOption::decline(),
    ];

    let context = DecisionContext::new(
        seat_a.clone(),
        DecisionSource::Rule("68".to_owned()),
        "produce_units",
        Phase::Action,
        2,
    )
    .optional(true)
    .owing(OutstandingConstraint::new(ConstraintKind::Resources, 6, 2));

    Choice {
        player: seat_a,
        prompt: "Choose units to produce in Mecatol Rex".to_owned(),
        options,
        context: Some(context),
        details: serde_json::Map::new(),
        free_form: None,
        multi_pick: None,
    }
}

/// Generates the sample actor snapshot message.
#[must_use]
pub fn sample_actor_snapshot() -> ServerMessage {
    let game = create_sample_game();
    let choice = create_sample_pending_choice();
    let msg = project_initial_snapshot(
        "game_12345",
        42,
        &game,
        &ViewerRole::Player(PlayerId::new("seat_a")),
        Some((&choice, "nonce_xyz789")),
    );
    ServerMessage::InitialSnapshot(msg)
}

/// Generates the sample opponent snapshot message.
#[must_use]
pub fn sample_opponent_snapshot() -> ServerMessage {
    let game = create_sample_game();
    let choice = create_sample_pending_choice();
    let msg = project_initial_snapshot(
        "game_12345",
        42,
        &game,
        &ViewerRole::Player(PlayerId::new("seat_b")),
        Some((&choice, "nonce_xyz789")),
    );
    ServerMessage::InitialSnapshot(msg)
}

/// Generates the sample spectator snapshot message.
#[must_use]
pub fn sample_spectator_snapshot() -> ServerMessage {
    let game = create_sample_game();
    let choice = create_sample_pending_choice();
    let msg = project_initial_snapshot(
        "game_12345",
        42,
        &game,
        &ViewerRole::Spectator,
        Some((&choice, "nonce_xyz789")),
    );
    ServerMessage::InitialSnapshot(msg)
}

/// Generates a sample rejected action response due to stale version.
#[must_use]
pub fn sample_stale_submission_rejected() -> ServerMessage {
    ServerMessage::ActionRejected(ActionRejectedMsg {
        protocol_version: PROTOCOL_VERSION,
        game_id: "game_12345".to_owned(),
        game_version: 42,
        reason: RejectionReason::StaleVersion {
            expected: 40,
            current: 42,
        },
    })
}

/// Generates a sample terminal game over message.
#[must_use]
pub fn sample_terminal_game_over() -> ServerMessage {
    let mut scores = BTreeMap::new();
    scores.insert(PlayerId::new("seat_a"), 10);
    scores.insert(PlayerId::new("seat_b"), 8);
    scores.insert(PlayerId::new("seat_c"), 7);

    ServerMessage::GameOver(GameOverMsg {
        protocol_version: PROTOCOL_VERSION,
        game_id: "game_12345".to_owned(),
        game_version: 156,
        winner: Some(PlayerId::new("seat_a")),
        final_scores: scores,
    })
}
