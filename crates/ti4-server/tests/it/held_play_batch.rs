//! A batch planned while a card played inside another card's window is still held back from
//! the published log (it is announced only when its own effect starts): the batch replays from
//! every settled answer, the held play too (run s2-01-2331: Summit's two pools, with Manipulate
//! Investments played inside Summit's window, were refused as "workflow interrupted").

use std::sync::Arc;
use std::time::{Duration, Instant};

use ti4_content::ContentStore;
use ti4_engine::choice::Choice;
use ti4_host::batch::{BatchKind, BatchRequest, MovementPlan, MovementStep};
use ti4_host::protocol::server::GameEventKind;
use ti4_model::id::PlayerId;
use ti4_server::session::{GameRegistry, GameSession, SeatController, SessionConfig};
use ti4_server::storage::FileGameStore;

type Pending = (PlayerId, String, u64, Choice);

fn current(session: &GameSession) -> Pending {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some((seat, nonce, version)) = session.current_pending_decision() {
            let snapshot = session.get_snapshot(&ti4_host::protocol::status::ViewerRole::Player(
                seat.clone(),
            ));
            return (
                seat,
                nonce,
                version,
                snapshot.pending_choice.unwrap().choice,
            );
        }
        assert!(
            Instant::now() < deadline,
            "timed out: {:?}",
            session.error()
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn a_token_batch_after_a_play_held_inside_summits_window_applies() {
    let game_id = "held_play_batch";
    let path = std::env::temp_dir().join(format!("ti4_held_{:032x}", rand::random::<u128>()));
    let store = Arc::new(FileGameStore::new(&path).unwrap());
    let registry = GameRegistry::new().with_store(store);
    let players = vec![PlayerId::new("p1"), PlayerId::new("p2")];
    let (mut state, galaxy) =
        ti4_host::map::create_game_with_template(ContentStore::embedded(), &players, 42, None)
            .unwrap();
    for (seat, card) in [(0, "summit"), (1, "investments")] {
        let index = state
            .action_card_deck
            .iter()
            .position(|c| c.as_str() == card)
            .expect("card in the deck");
        let card = state.action_card_deck.remove(index);
        state.players[seat].action_cards.push(card);
    }
    let tiles = ti4_host::map::build_board_tiles(ContentStore::embedded(), &galaxy);
    let config = SessionConfig::new(game_id, state)
        .with_seed(42)
        .with_player_ids(players.clone())
        .with_galaxy(galaxy, tiles)
        .with_seat(players[0].clone(), SeatController::Human)
        .with_seat(players[1].clone(), SeatController::Human);
    let session = registry.create_game(config).unwrap();
    let p1_token = session.seat_tokens()[&players[0]].clone();

    // p1 plays Summit, p2 plays Manipulate Investments inside its window, then Summit asks p1
    // for its two pools while p2's play is not announced yet.
    let mut played = 0;
    let mut asked = 0;
    let mut held_answer = None;
    let pending = loop {
        let pending = current(&session);
        asked += 1;
        assert!(
            asked < 200,
            "Summit's pools were never asked: {:?}",
            pending.3.prompt
        );
        let (seat, nonce, version, choice) = &pending;
        // Summit's two tokens: ONE gain question (`{tactic}|{fleet}|{strategic}`).
        if choice.prompt == "gain 2 command tokens into which pools" {
            break pending;
        }
        let summit = choice.options.iter().find(|o| o.label.contains("Summit"));
        let investments = choice
            .options
            .iter()
            .find(|o| played == 1 && o.label.contains("Manipulate Investments"));
        let decline = choice
            .options
            .iter()
            .find(|o| choice.prompt.starts_with("after ") && o.id == "decline");
        let pick = summit
            .or(investments)
            .or(decline)
            .unwrap_or(&choice.options[0]);
        if summit.is_some() || investments.is_some() {
            played += 1;
        }
        // Answered from a thread: an answer inside a card's window is accepted only when the
        // play is announced (Manipulate Investments: after Summit's pools).
        let (live, seat, nonce, version, id) = (
            session.clone(),
            seat.clone(),
            nonce.clone(),
            *version,
            pick.id.clone(),
        );
        let answer = std::thread::spawn(move || live.submit_choice(&seat, &nonce, version, &id));
        if investments.is_some() {
            held_answer = Some(answer);
        }
        let deadline = Instant::now() + Duration::from_secs(10);
        while session
            .current_pending_decision()
            .is_none_or(|p| p.2 == version)
        {
            assert!(Instant::now() < deadline, "the answer was not taken");
            std::thread::sleep(Duration::from_millis(5));
        }
    };
    assert_eq!(played, 2, "both cards were played");
    assert_eq!(pending.0, players[0]);

    let result = registry
        .submit_batch(
            game_id,
            &p1_token,
            BatchRequest {
                request_id: "summit".into(),
                expected_version: pending.2,
                nonce: pending.1.clone(),
                plan: MovementPlan {
                    kind: BatchKind::Tokens,
                    destination: String::new(),
                    steps: vec![
                        MovementStep::Pool {
                            pool: "strategic_tokens".into(),
                        },
                        MovementStep::Pool {
                            pool: "strategic_tokens".into(),
                        },
                    ],
                },
            },
        )
        .expect("the batch replays the held play and applies both pools");

    // p2's play was waiting for its acceptance; the commit tells it the play was taken.
    let accepted = held_answer.expect("p2 played").join().unwrap();
    assert!(accepted.is_ok(), "{accepted:?}");

    let session = registry.get_game(game_id).unwrap();
    session.wait_replayed().unwrap();
    let log = session.decision_log();
    // Both pool steps answer the one gain question.
    assert_eq!(result.end_cursor, result.start_cursor + 1);
    assert_eq!(log[result.start_cursor].chosen, "0|0|2");
    assert!(
        log[..result.start_cursor]
            .iter()
            .any(|r| r.player == players[1] && r.chosen.starts_with("reaction:")),
        "the held play is part of the committed timeline"
    );
    // Every decision of the committed timeline has its history line, the held one included.
    let resolved: Vec<_> = session
        .event_log()
        .iter()
        .filter(|e| matches!(e.event, GameEventKind::DecisionResolved))
        .filter_map(|e| e.decision_count)
        .collect();
    for count in 1..=result.end_cursor {
        assert!(
            resolved.contains(&count),
            "decision {count} has no history line"
        );
    }
    let _ = std::fs::remove_dir_all(&path);
}
