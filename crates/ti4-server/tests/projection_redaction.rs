use ti4_model::id::PlayerId;
use ti4_model::view::{HIDDEN, view_for};
use ti4_server::fixtures::{create_sample_game, create_sample_pending_choice};
use ti4_server::projection::{project_initial_snapshot, project_state_update};
use ti4_server::protocol::server::ServerMessage;
use ti4_server::protocol::status::ViewerRole;
use ti4_server::protocol::{EventVisibility, GameEvent, GameEventKind};

#[test]
fn actor_payload_includes_own_private_information_and_pending_choice() {
    let game = create_sample_game();
    let choice = create_sample_pending_choice();
    let seat_a = PlayerId::new("seat_a");
    let viewer_a = ViewerRole::Player(seat_a.clone());

    let snapshot = project_initial_snapshot(
        "test_game",
        1,
        &game,
        &viewer_a,
        Some((&choice, "nonce_123")),
    );
    let json = serde_json::to_string(&ServerMessage::InitialSnapshot(snapshot)).expect("serialize");

    // Actor sees own cards
    assert!(
        json.contains("direct_hit"),
        "Actor must see own action card"
    );
    assert!(
        json.contains("destroy_their_greatest_ship"),
        "Actor must see own secret objective"
    );

    // Actor sees pending choice and options
    assert!(
        json.contains("opt_carrier"),
        "Actor must see offered choice option"
    );
    assert!(
        json.contains("opt_infantry"),
        "Actor must see offered choice option"
    );
    assert!(
        json.contains("produce_units"),
        "Actor must see decision context subtype"
    );

    // Actor sees outstanding constraints
    assert!(
        json.contains("Resources"),
        "Actor must see outstanding resource constraint"
    );

    // Actor DOES NOT see opponent's private cards
    assert!(
        !json.contains("flank_speed"),
        "Actor must not see opponent's action card"
    );
    assert!(
        !json.contains("brave_the_void"),
        "Actor must not see opponent's secret objective"
    );
}

#[test]
fn opponent_payload_strictly_redacts_actor_private_cards_options_and_constraints() {
    let game = create_sample_game();
    let choice = create_sample_pending_choice();
    let seat_b = PlayerId::new("seat_b");
    let viewer_b = ViewerRole::Player(seat_b.clone());

    let snapshot = project_initial_snapshot(
        "test_game",
        1,
        &game,
        &viewer_b,
        Some((&choice, "nonce_123")),
    );
    let json = serde_json::to_string(&ServerMessage::InitialSnapshot(snapshot)).expect("serialize");

    // 1. Proves opponent payload contains NO action-card identity of actor
    assert!(
        !json.contains("direct_hit"),
        "Opponent payload leaked actor's direct_hit card!"
    );
    assert!(
        !json.contains("morale_boost"),
        "Opponent payload leaked actor's morale_boost card!"
    );

    // 2. Proves opponent payload contains NO secret-objective identity of actor
    assert!(
        !json.contains("destroy_their_greatest_ship"),
        "Opponent payload leaked actor's secret objective!"
    );

    // 3. Proves opponent payload contains NO legal options belonging to the actor
    assert!(
        !json.contains("opt_carrier"),
        "Opponent payload leaked actor's choice option id!"
    );
    assert!(
        !json.contains("opt_infantry"),
        "Opponent payload leaked actor's choice option id!"
    );
    assert!(
        !json.contains("Produce Carrier"),
        "Opponent payload leaked actor's choice label!"
    );

    // 4. Proves opponent payload contains NO pending_choice object
    assert!(
        !json.contains("\"pending_choice\""),
        "Opponent payload must not include pending_choice!"
    );

    // 5. Proves opponent payload contains NO actor-only outstanding constraints
    assert!(
        !json.contains("\"outstanding\""),
        "Opponent payload must not include outstanding constraints!"
    );

    // 6. Proves opponent sees accurate public counts
    assert!(
        json.contains("\"action_cards_count\":2"),
        "Public action card count must be visible"
    );
    assert!(
        json.contains("\"secret_objectives_count\":1"),
        "Public secret objective count must be visible"
    );

    // 7. Proves opponent sees public waiting turn status without leaking details
    assert!(
        json.contains("\"kind\":\"waiting_for_decision\""),
        "Opponent must see turn status as waiting_for_decision"
    );
    assert!(
        json.contains("\"seat\":\"seat_a\""),
        "Waiting seat must be seat_a"
    );
    assert!(
        json.contains("\"stage\":\"Waiting for player\""),
        "Public waiting status must not disclose an actor-only decision context"
    );
    assert!(
        !json.contains("Reaction Window"),
        "Public waiting status must not disclose a private reaction window"
    );
}

#[test]
fn spectator_payload_redacts_all_private_cards_and_pending_choices() {
    let game = create_sample_game();
    let choice = create_sample_pending_choice();
    let viewer_spec = ViewerRole::Spectator;

    let snapshot = project_initial_snapshot(
        "test_game",
        1,
        &game,
        &viewer_spec,
        Some((&choice, "nonce_123")),
    );
    let json = serde_json::to_string(&ServerMessage::InitialSnapshot(snapshot)).expect("serialize");

    // Spectator sees NO private cards from ANY player
    assert!(!json.contains("direct_hit"));
    assert!(!json.contains("morale_boost"));
    assert!(!json.contains("destroy_their_greatest_ship"));
    assert!(!json.contains("flank_speed"));
    assert!(!json.contains("brave_the_void"));
    assert!(!json.contains("shields_holding"));
    assert!(!json.contains("unveil_flagship"));

    // Spectator sees NO pending choice or legal options
    assert!(!json.contains("\"pending_choice\""));
    assert!(!json.contains("opt_carrier"));

    // Spectator sees public counts
    assert!(json.contains("\"action_cards_count\":2"));
    assert!(json.contains("\"action_cards_count\":1"));
}

#[test]
fn state_update_preserves_identical_redaction_guarantees() {
    let game = create_sample_game();
    let choice = create_sample_pending_choice();
    let viewer_b = ViewerRole::Player(PlayerId::new("seat_b"));

    let update = project_state_update(
        "test_game",
        2,
        &game,
        &viewer_b,
        Some((&choice, "nonce_123")),
    );
    let json = serde_json::to_string(&ServerMessage::StateUpdate(update)).expect("serialize");

    assert!(!json.contains("direct_hit"));
    assert!(!json.contains("destroy_their_greatest_ship"));
    assert!(!json.contains("opt_carrier"));
    assert!(!json.contains("\"pending_choice\""));
}

#[test]
fn player_view_uses_the_model_redaction_result_including_search_warrant() {
    let mut game = create_sample_game();
    game.laws.insert("warrant".to_owned(), "seat_a".to_owned());
    let viewer = PlayerId::new("seat_b");
    let expected = view_for(&game, &viewer);

    let snapshot =
        project_initial_snapshot("test_game", 1, &game, &ViewerRole::Player(viewer), None);
    let expected_a = expected.player(&PlayerId::new("seat_a")).unwrap();
    let projected_a = snapshot
        .view
        .players
        .iter()
        .find(|player| player.id == PlayerId::new("seat_a"))
        .unwrap();

    assert!(
        expected_a
            .action_cards
            .iter()
            .all(|card| card.as_str() == HIDDEN)
    );
    assert!(projected_a.held_action_cards.is_empty());
    assert_eq!(
        projected_a.held_secret_objectives, expected_a.secret_objectives,
        "Search Warrant's engine-owned exception must reach the server projection"
    );
}

#[test]
fn spectator_excludes_hidden_markers_from_held_card_lists() {
    let game = create_sample_game();
    let snapshot = project_initial_snapshot("test_game", 1, &game, &ViewerRole::Spectator, None);

    for player in snapshot.view.players {
        assert!(player.held_action_cards.is_empty(), "{}", player.id);
        assert!(player.held_secret_objectives.is_empty(), "{}", player.id);
        assert!(player.action_cards_count > 0 || player.secret_objectives_count > 0);
    }
}

#[test]
fn event_history_is_projected_to_its_explicit_audience() {
    let game = create_sample_game();
    let choice = create_sample_pending_choice();
    let events = vec![
        GameEvent {
            id: "test-1".to_owned(),
            timestamp: "00:00:00".to_owned(),
            version: Some(1),
            visibility: EventVisibility::Public,
            event: GameEventKind::DecisionResolved,
        },
        GameEvent {
            id: "test-2".to_owned(),
            timestamp: "00:00:01".to_owned(),
            version: Some(1),
            visibility: EventVisibility::Seat(PlayerId::new("seat_a")),
            event: GameEventKind::DecisionResolved,
        },
    ];

    let actor = ti4_server::projection::project_initial_snapshot_with_map(
        "test_game",
        1,
        &game,
        &ViewerRole::Player(PlayerId::new("seat_a")),
        Some((&choice, "nonce_123")),
        &[],
        &events,
    );
    let opponent = ti4_server::projection::project_initial_snapshot_with_map(
        "test_game",
        1,
        &game,
        &ViewerRole::Player(PlayerId::new("seat_b")),
        Some((&choice, "nonce_123")),
        &[],
        &events,
    );

    assert_eq!(actor.events.len(), 2);
    assert_eq!(opponent.events.len(), 1);
    assert_eq!(opponent.events[0].id, "test-1");
}
