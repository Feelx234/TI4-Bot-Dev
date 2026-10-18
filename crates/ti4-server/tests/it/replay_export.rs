//! `GET /api/games/{id}/replay`: any seated player may copy the game's history out; everyone
//! else is refused.

use std::sync::Arc;
use std::time::{Duration, Instant};

use reqwest::StatusCode;
use serde_json::Value;
use tokio::net::TcpListener;

use ti4_content::ContentStore;
use ti4_host::protocol::status::ViewerRole;
use ti4_model::id::PlayerId;
use ti4_server::create_app;
use ti4_server::session::{GameRegistry, GameSession, SeatController, SessionConfig};

async fn spawn(registry: Arc<GameRegistry>) -> String {
    let app = create_app(registry);
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let address = listener.local_addr().expect("addr");
    tokio::spawn(async move { axum::serve(listener, app).await.expect("serve") });
    format!("http://{address}")
}

fn answer_first_choice(session: &GameSession) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some((seat, nonce, version)) = session.current_pending_decision() {
            let option = session
                .get_snapshot(&ViewerRole::Player(seat.clone()))
                .pending_choice
                .expect("pending choice")
                .choice
                .options[0]
                .id
                .clone();
            session
                .submit_choice(&seat, &nonce, version, &option)
                .expect("submit");
            return;
        }
        assert!(
            Instant::now() < deadline,
            "timed out: {:?}",
            session.error()
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

async fn replay(base: &str, game: &str, token: Option<&str>) -> (StatusCode, Value) {
    let mut request = reqwest::Client::new().get(format!("{base}/api/games/{game}/replay"));
    if let Some(token) = token {
        request = request.header("x-ti4-player-session", token);
    }
    let response = request.send().await.expect("replay request");
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    let json = serde_json::from_str(&body).unwrap_or(Value::String(body));
    (status, json)
}

#[tokio::test(flavor = "multi_thread")]
async fn seated_players_get_the_replay_and_everyone_else_is_refused() {
    let registry = Arc::new(GameRegistry::new());
    let host = PlayerId::new("p1");
    let guest = PlayerId::new("p2");
    let players = vec![host.clone(), guest.clone()];
    let (state, galaxy) =
        ti4_host::map::create_game_with_template(ContentStore::embedded(), &players, 42, None)
            .unwrap();
    let tiles = ti4_host::map::build_board_tiles(ContentStore::embedded(), &galaxy);
    let config = SessionConfig::new("replay_probe", state)
        .with_seed(42)
        .with_player_ids(players)
        .with_galaxy(galaxy, tiles)
        .with_seat(host.clone(), SeatController::Human)
        .with_seat(guest.clone(), SeatController::Human);
    let session = registry.create_game(config).unwrap();
    let tokens = session.seat_tokens();
    for _ in 0..3 {
        answer_first_choice(&session);
    }
    let base = spawn(registry).await;

    // No credential, and a credential that belongs to nobody.
    let (status, _) = replay(&base, "replay_probe", None).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "missing token");
    let (status, _) = replay(&base, "replay_probe", Some("not-a-token")).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "unknown token");
    // A known game id is not needed to be refused, and an unknown game is a 404 for a real token.
    let (status, _) = replay(&base, "no_such_game", Some(&tokens[&host])).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Every seated player, not only the host, gets the same replay.
    let (status, from_host) = replay(&base, "replay_probe", Some(&tokens[&host])).await;
    assert_eq!(status, StatusCode::OK, "{from_host}");
    let (status, from_guest) = replay(&base, "replay_probe", Some(&tokens[&guest])).await;
    assert_eq!(status, StatusCode::OK, "{from_guest}");
    assert_eq!(from_host, from_guest);

    assert_eq!(from_host["format"], "ti4-replay");
    assert_eq!(from_host["version"], 2);
    assert_eq!(from_host["format_version"], 2);
    assert_eq!(from_host["game_id"], "replay_probe");
    // Big integers are strings, always.
    assert_eq!(from_host["seed"], "42");
    // A game that was not made from a lobby has no setup to tell, but the seats are named.
    assert!(from_host.get("setup").is_none());
    assert_eq!(from_host["seats"].as_array().unwrap().len(), 2);
    assert_eq!(from_host["player_ids"].as_array().unwrap().len(), 2);
    let history = &from_host["history"];
    assert_eq!(
        history["decisions"].as_array().unwrap().len(),
        session.decision_log().len(),
        "the live decisions, not a stale saved file"
    );
    assert!(history["decisions"].as_array().unwrap().len() >= 3);
    assert!(!history["events"].as_array().unwrap().is_empty());
    assert!(history["revision"].as_u64().unwrap() > 1);
}

/// A table started from a lobby: the export carries its setup, and alone replays to the live game.
#[tokio::test(flavor = "multi_thread")]
async fn a_lobby_game_exports_its_setup_and_replays_from_the_file_alone() {
    lobby_case("lobby_export", 400).await;
}

/// The same, at a question in the middle of the game.
#[tokio::test(flavor = "multi_thread")]
async fn a_game_in_progress_replays_to_its_open_question() {
    lobby_case("lobby_midgame", 60).await;
}

async fn lobby_case(game_id: &str, max_answers: usize) {
    use ti4_host::export::{ReplayExport, state_hash};
    ti4_server::session::random_bot::set_random_bot_delay_override(Some(0));
    let registry = Arc::new(GameRegistry::new());
    // A seed that no JavaScript number holds.
    let seed = 18_432_220_952_314_264_515_u64;
    let template = ti4_host::maps::catalog::recommended_for(3);
    let (_, host, credential) = registry
        .create_player_game_with_random_seats(
            game_id.to_owned(),
            3,
            seed,
            "Host Name",
            template.clone(),
            None,
            ti4_host::setup::card_set::DEFAULT,
            ti4_host::setup::SpeakerOrder::Seat,
            None,
            true,
        )
        .expect("a started table");
    let session = registry.get_game(game_id).expect("the game");
    // Far enough to cross the status phase; the game may also end before.
    let mut answered = 0;
    while answered < max_answers && !session.is_finished() {
        if session.current_pending_decision().is_some() {
            answer_first_choice(&session);
            answered += 1;
        } else {
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    let base = spawn(registry).await;
    let (status, body) = replay(&base, game_id, Some(credential.as_str())).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["seed"], "18432220952314264515");
    assert_eq!(body["setup"]["table"]["seed"], "18432220952314264515");
    assert_eq!(body["setup"]["table"]["thunders_edge"], true);
    assert_eq!(body["setup"]["table"]["speaker_order"], "seat");
    assert_eq!(body["seats"][0]["nickname"], "Host Name");
    assert_eq!(body["seats"][0]["kind"], "human");
    assert_ne!(body["seats"][1]["kind"], "human");
    assert!(body["seats"][1]["faction"].is_string());
    assert!(body["map_layout"]["placements"].as_array().unwrap().len() > 3);
    assert_eq!(
        body["build"]["rules_version"],
        ti4_model::state::RULES_VERSION
    );
    let _ = (host, template);

    // The text a page would save, parsed and printed again by a JSON that has only floats for
    // integers (here: a Value round trip, and Node when it is installed), is the same file.
    let text = reqwest::Client::new()
        .get(format!("{base}/api/games/{game_id}/replay"))
        .header("x-ti4-player-session", credential.as_str())
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    // `TI4_EXPORT_DIR=/some/dir cargo test ...` keeps the file, to try `replay_save` on it.
    if let Some(dir) = std::env::var_os("TI4_EXPORT_DIR") {
        let path = std::path::Path::new(&dir).join(format!("{game_id}.json"));
        std::fs::write(path, &text).expect("the export is written");
    }
    let through_value =
        serde_json::to_string(&serde_json::from_str::<Value>(&text).unwrap()).unwrap();
    let mut texts = vec![text.clone(), through_value];
    if let Some(node) = node_round_trip(&text) {
        texts.push(node);
    }
    let live = session.current_state();
    for text in texts {
        let export = ReplayExport::parse(&text).expect("the file reads");
        assert_eq!(export.seed, Some(seed));
        let outcome = export.replay().expect("the file replays by itself");
        assert_eq!(
            outcome.setup_matches,
            Some(true),
            "the setup rebuilds the opening state"
        );
        assert!(outcome.hashes_match);
        assert_eq!(outcome.decisions, session.decision_log().len());
        assert_eq!(
            Some(outcome.decisions_hash.clone()),
            export.check.as_ref().map(|c| c.decisions_hash.clone())
        );
        if state_hash(&outcome.final_state) != state_hash(&live) {
            let a = serde_json::to_value(&outcome.final_state).unwrap();
            let b = serde_json::to_value(&live).unwrap();
            for (key, value) in a.as_object().unwrap() {
                if b.get(key) != Some(value) {
                    eprintln!("DIFF {key}: replay {value} live {}", b[key]);
                }
            }
            panic!("final state differs; finished={}", session.is_finished());
        }
    }
}

/// `JSON.parse` then `JSON.stringify` in Node, when Node is there.
fn node_round_trip(text: &str) -> Option<String> {
    use std::io::Write;
    let mut child = std::process::Command::new("node")
        .args([
            "-e",
            "let s='';process.stdin.on('data',d=>s+=d).on('end',()=>process.stdout.write(JSON.stringify(JSON.parse(s))))",
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .ok()?;
    child.stdin.take()?.write_all(text.as_bytes()).ok()?;
    let out = child.wait_with_output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8(out.stdout).ok())?
}

/// A report of an online game: the export is made from the game's files, as the live one is.
#[test]
fn the_export_of_the_files_in_the_data_directory_replays_like_the_live_one() {
    use ti4_host::export::{ReplayExport, state_hash};
    use ti4_server::storage::FileGameStore;
    ti4_server::session::random_bot::set_random_bot_delay_override(Some(0));
    let dir = std::env::temp_dir().join(format!("ti4_export_{:016x}", rand::random::<u64>()));
    let store = Arc::new(FileGameStore::new(&dir).unwrap());
    let registry = GameRegistry::new().with_store(store.clone());
    registry
        .create_player_game_with_random_seats(
            "stored_export".to_owned(),
            3,
            77,
            "Host",
            None,
            None,
            ti4_host::setup::card_set::DEFAULT,
            ti4_host::setup::SpeakerOrder::Random,
            None,
            false,
        )
        .expect("a started table");
    let session = registry.get_game("stored_export").expect("the game");
    for _ in 0..30 {
        answer_first_choice(&session);
    }
    // Let the worker save what it has decided.
    std::thread::sleep(Duration::from_millis(300));
    let export = ti4_server::replay_export::from_store(&store, "stored_export", "abc").unwrap();
    let text = serde_json::to_string_pretty(&export).unwrap();
    let export = ReplayExport::parse(&text).unwrap();
    assert_eq!(export.seats[0].nickname.as_deref(), Some("Host"));
    assert!(export.map_layout.is_some());
    let outcome = export.replay().expect("the stored export replays");
    assert_eq!(outcome.setup_matches, Some(true));
    assert!(outcome.hashes_match);
    // The files hold everything decided when they were read; the live game may be a step on.
    assert!(outcome.decisions <= session.decision_log().len());
    assert!(outcome.decisions >= 30);
    let _ = (
        state_hash(&outcome.final_state),
        std::fs::remove_dir_all(&dir),
    );
}
