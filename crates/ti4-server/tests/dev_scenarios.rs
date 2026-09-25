use std::sync::Arc;
use std::thread;
use std::time::Duration;

use ti4_server::dev::{available_scenarios, execute_launch_scenario};
use ti4_server::protocol::server::ServerMessage;
use ti4_server::protocol::status::ViewerRole;
use ti4_server::session::{GameRegistry, MockClient};

#[test]
fn test_available_scenarios_listing() {
    let scenarios = available_scenarios();
    assert_eq!(scenarios.len(), 3);
    assert_eq!(scenarios[0].id, "tactical_action");
    assert_eq!(scenarios[1].id, "space_combat");
    assert_eq!(scenarios[2].id, "ongoing_combat");
}

#[test]
fn test_launch_tactical_scenario_and_flow() {
    let registry = Arc::new(GameRegistry::new());
    let response = execute_launch_scenario(&registry, "tactical_action", Some(42))
        .expect("launch tactical scenario");

    assert!(response.game_id.starts_with("dev_tactical_"));
    let session = registry
        .get_game(&response.game_id)
        .expect("session in registry");

    let p1 = ti4_model::id::PlayerId::new(&response.player_id);
    let client = MockClient::connect(session.clone(), ViewerRole::Player(p1.clone()));

    // Wait for the initial action phase choice
    let wait_for_choice = |client: &MockClient| {
        for _ in 0..100 {
            if let Ok(ServerMessage::PendingChoice(msg)) = client.try_recv() {
                return Some(msg.choice);
            }
            thread::sleep(Duration::from_millis(10));
        }
        None
    };

    let choice = wait_for_choice(&client).expect("choice for tactical action");
    assert_eq!(choice.player, p1);
    assert_eq!(choice.prompt, "action phase");

    // Should offer taking a tactical action
    let tactical_opt = choice
        .options
        .iter()
        .find(|o| o.id == "tactical")
        .expect("tactical action option");

    let (_, nonce_1, ver_1) = session.current_pending_decision().expect("pending decision");
    let res = client.submit(&nonce_1, ver_1, &tactical_opt.id);
    assert!(res.is_ok(), "submit tactical action: {res:?}");

    // Next decision should be activating a system
    let activate_choice = wait_for_choice(&client).expect("choice for system activation");
    assert_eq!(activate_choice.options[0].kind, "activate");
    assert!(!activate_choice.options.is_empty());
}

#[test]
fn test_launch_space_combat_scenario_has_hostile_units() {
    let registry = Arc::new(GameRegistry::new());
    let response = execute_launch_scenario(&registry, "space_combat", Some(12345))
        .expect("launch space combat scenario");

    assert!(response.game_id.starts_with("dev_combat_"));
    let session = registry
        .get_game(&response.game_id)
        .expect("session in registry");

    let p1 = ti4_model::id::PlayerId::new(&response.player_id);
    let client = MockClient::connect(session.clone(), ViewerRole::Player(p1.clone()));

    let wait_for_choice = |client: &MockClient| {
        for _ in 0..100 {
            if let Ok(ServerMessage::PendingChoice(msg)) = client.try_recv() {
                return Some(msg.choice);
            }
            thread::sleep(Duration::from_millis(10));
        }
        None
    };

    // 1. Initial action choice -> select "tactical"
    let choice = wait_for_choice(&client).expect("initial choice");
    assert_eq!(choice.player, p1);
    let (_, nonce_1, ver_1) = session.current_pending_decision().unwrap();
    client.submit(&nonce_1, ver_1, "tactical").unwrap();

    // 2. Activate choice -> activate border system where Letnev ships are
    let initial_snapshot = session.get_snapshot(&ViewerRole::Player(p1.clone()));
    let letnev_id = initial_snapshot
        .view
        .players
        .iter()
        .find(|p| p.faction.as_str().to_lowercase().contains("letnev"))
        .map(|p| p.id.clone())
        .expect("letnev player");
    let border_sys_id = initial_snapshot
        .view
        .board
        .systems
        .values()
        .find(|s| s.system_id.as_str() != "10" && s.system_id.as_str() != "01" && s.units.iter().any(|u| u.owner == letnev_id))
        .map(|s| s.system_id.clone())
        .expect("border system with Letnev units");

    let choice = wait_for_choice(&client).expect("activate choice");
    assert_eq!(choice.player, p1);
    let activate_opt = choice
        .options
        .iter()
        .find(|o| o.id == border_sys_id.as_str())
        .expect("activate border system option");
    let (_, nonce_2, ver_2) = session.current_pending_decision().unwrap();
    client.submit(&nonce_2, ver_2, &activate_opt.id).unwrap();

    // 3. Movement choice -> move ships then done_moving
    let mut choice = wait_for_choice(&client).expect("movement choice");
    // Advance movement and loading until space combat begins
    while !choice.options.iter().any(|o| o.kind == "retreat" || o.kind == "sustain" || o.kind == "casualty") {
        let (_, nonce, ver) = session.current_pending_decision().unwrap();
        if let Some(load_opt) = choice.options.iter().find(|o| o.id.starts_with("load|0")) {
            client.submit(&nonce, ver, &load_opt.id).unwrap();
        } else if let Some(done_load) = choice.options.iter().find(|o| o.id == "done_loading") {
            client.submit(&nonce, ver, &done_load.id).unwrap();
        } else if let Some(move_opt) = choice.options.iter().find(|o| o.id.starts_with("move|")) {
            client.submit(&nonce, ver, &move_opt.id).unwrap();
        } else if let Some(done_move) = choice.options.iter().find(|o| o.id == "done_moving") {
            client.submit(&nonce, ver, &done_move.id).unwrap();
        } else {
            panic!("unhandled choice in movement loop: {:?}", choice);
        }
        choice = wait_for_choice(&client).expect("next choice in combat setup");
    }

    let combat_choice = choice;
    assert_eq!(combat_choice.player, p1);
    let subtype = combat_choice
        .context
        .as_ref()
        .map(|c| c.subtype.as_str())
        .unwrap_or("");
    assert!(
        matches!(
            subtype,
            "announce_retreat" | "sustain_damage" | "assign_casualty"
        ),
        "unexpected combat choice subtype: {subtype}"
    );

    // Board projection should contain combat view
    let snapshot = session.get_snapshot(&ViewerRole::Player(p1.clone()));
    assert!(snapshot.view.board.combat.is_some(), "board combat must be projected");
    let combat = snapshot.view.board.combat.unwrap();
    assert_eq!(combat.attacker, p1);
    assert_eq!(combat.defender, letnev_id);
}

#[tokio::test]
async fn test_dev_scenarios_http_api() {
    let registry = Arc::new(GameRegistry::new());
    let app = ti4_server::create_app(registry);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener");
    let addr = listener.local_addr().expect("local addr");
    tokio::spawn(async move { axum::serve(listener, app).await.expect("serve app") });

    let client = reqwest::Client::new();
    let res = client
        .get(format!("http://{addr}/api/dev/scenarios"))
        .send()
        .await
        .expect("get dev scenarios");
    assert_eq!(res.status(), reqwest::StatusCode::OK);
    let list: Vec<ti4_server::dev::ScenarioSummary> =
        res.json().await.expect("json scenario list");
    assert_eq!(list.len(), 3);

    let launch_res = client
        .post(format!("http://{addr}/api/dev/scenarios/launch"))
        .json(&serde_json::json!({
            "scenario_id": "tactical_action",
            "seed": 999
        }))
        .send()
        .await
        .expect("post launch scenario");
    assert_eq!(launch_res.status(), reqwest::StatusCode::OK);
    let launched: ti4_server::dev::LaunchScenarioResponse =
        launch_res.json().await.expect("json launch response");
    assert!(launched.game_id.starts_with("dev_tactical_"));

    // Verify snapshot endpoint works with token
    let snapshot_res = client
        .get(format!("http://{addr}/api/games/{}/snapshot", launched.game_id))
        .header("x-ti4-player-session", &launched.player_session)
        .send()
        .await
        .expect("get snapshot");
    assert_eq!(snapshot_res.status(), reqwest::StatusCode::OK);

    // Verify lobby endpoint works with token
    let lobby_res = client
        .get(format!("http://{addr}/api/games/{}/lobby", launched.game_id))
        .header("x-ti4-player-session", &launched.player_session)
        .send()
        .await
        .expect("get lobby");
    assert_eq!(lobby_res.status(), reqwest::StatusCode::OK);
    let lobby_json: serde_json::Value = lobby_res.json().await.expect("lobby json");
    assert_eq!(lobby_json["phase"], "running");
}

#[test]
fn test_launch_ongoing_combat_scenario_starts_in_combat() {
    let registry = Arc::new(GameRegistry::new());
    let response = execute_launch_scenario(&registry, "ongoing_combat", Some(42))
        .expect("launch ongoing combat scenario");

    assert!(response.game_id.starts_with("dev_combat_"));
    let session = registry
        .get_game(&response.game_id)
        .expect("session in registry");

    let p1 = ti4_model::id::PlayerId::new(&response.player_id);
    let snapshot = session.get_snapshot(&ViewerRole::Player(p1.clone()));

    // Combat is active immediately on launch!
    assert!(
        snapshot.view.board.combat.is_some(),
        "board combat must be active on scenario launch"
    );
    let combat = snapshot.view.board.combat.unwrap();
    assert_eq!(combat.attacker, p1);

    // Sol holds direct_hit action card
    let p1_player = snapshot
        .view
        .players
        .iter()
        .find(|p| p.id == p1)
        .expect("p1 player in snapshot");
    assert!(
        p1_player
            .held_action_cards
            .iter()
            .any(|c| c.as_str() == "direct_hit"),
        "Player 1 (Sol) must hold direct_hit action card"
    );

    // Both sides fielded a Dreadnought in the battle system
    let sys = snapshot
        .view
        .board
        .systems
        .get(&combat.system_id)
        .expect("combat system");
    assert!(
        sys.units
            .iter()
            .any(|u| u.owner == p1 && u.unit_type.as_str() == "dreadnought"),
        "Attacker (Sol) must have dreadnought in combat"
    );
    assert!(
        sys.units
            .iter()
            .any(|u| u.owner == combat.defender && u.unit_type.as_str() == "dreadnought"),
        "Defender must have dreadnought in combat"
    );

    // Pending choice is combat choice
    assert!(snapshot.pending_choice.is_some());
    let pending = snapshot.pending_choice.unwrap();
    assert_eq!(pending.choice.player, p1);
    let subtype = pending
        .choice
        .context
        .as_ref()
        .map(|c| c.subtype.as_str())
        .unwrap_or("");
    assert!(
        matches!(
            subtype,
            "sustain_damage" | "assign_casualty"
        ),
        "expected sustain_damage or assign_casualty, got: {subtype}"
    );

    // Hits or rolls are populated
    assert!(
        combat.attacker_hits.is_some()
            || combat.defender_hits.is_some()
            || combat.hits_to_assign.is_some()
            || !combat.dice_rolls.is_empty(),
        "combat hits or dice rolls must be present"
    );
}

