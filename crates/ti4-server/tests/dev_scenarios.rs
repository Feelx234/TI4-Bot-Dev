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
    assert_eq!(scenarios.len(), 2);
    assert_eq!(scenarios[0].id, "tactical_action");
    assert_eq!(scenarios[1].id, "space_combat");
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

    // Wait for the initial action choice
    for _ in 0..100 {
        if let Ok(ServerMessage::PendingChoice(msg)) = client.try_recv() {
            assert_eq!(msg.choice.player, p1);
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("Did not receive initial choice in space combat scenario");
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
    assert_eq!(list.len(), 2);

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

