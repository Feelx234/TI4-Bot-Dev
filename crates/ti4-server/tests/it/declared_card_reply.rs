//! D2-8g: the reply to the answer that declares a card in a reaction window comes when the card
//! is announced, before the card's own question is put to its owner.
//!
//! With the Sabotage timing (rules version 3) a card declared in a window is announced when the
//! window closes, and the worker holds the declaring answer back until the position shows the
//! announcement (`on_observed_offer`). The first question after the announcement is the card's own
//! (a Rider asks its owner for the predicted outcome). The wrapper used to show that question the
//! position the step started from (`Asking::nested` is `None` for a question asked at the close of a
//! window, its position is `Asking::at`), so the play was missing, the answer was never acknowledged
//! and a client that waits for the acknowledgement before it answers the next question (this one)
//! deadlocked with the engine, which waited for the same seat's next answer.

use std::sync::Arc;
use std::time::{Duration, Instant};

use ti4_host::protocol::status::ViewerRole;
use ti4_model::id::PlayerId;
use ti4_server::session::{GameRegistry, GameSession};

/// A lobby game on the `agenda` start preset (the agenda phase from round one, riders and other
/// agenda cards dealt to every seat), the first seat a person, the others bots.
fn started(game_id: &str, seed: u64) -> Arc<GameSession> {
    ti4_server::session::random_bot::set_random_bot_delay_override(Some(0));
    let registry = Arc::new(GameRegistry::new());
    let template = ti4_host::maps::catalog::recommended_for(3);
    registry
        .create_player_game_with_random_seats(
            game_id.to_owned(),
            3,
            seed,
            "Host Name",
            template,
            Some(ti4_host::preset::AGENDA.to_owned()),
            ti4_host::setup::card_set::DEFAULT,
            ti4_host::setup::SpeakerOrder::Seat,
            None,
            false,
        )
        .expect("a started table");
    // The registry is dropped with the function; the session keeps what it needs.
    registry.get_game(game_id).expect("the game")
}

/// Answers the pending question with its first option on a thread (a client never blocks on the
/// reply: with the Sabotage timing it comes when the window closes) and returns the thread and
/// the prompt once the question is gone.
fn answer_first(
    session: &Arc<GameSession>,
) -> Option<(PlayerId, String, std::thread::JoinHandle<bool>)> {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if let Some((seat, nonce, version)) = session.current_pending_decision() {
            let Some(envelope) = session
                .get_snapshot(&ViewerRole::Player(seat.clone()))
                .pending_choice
            else {
                continue;
            };
            let prompt = envelope.choice.prompt.clone();
            let option = envelope.choice.options[0].id.clone();
            let (worker, who, asked) = (session.clone(), seat.clone(), nonce.clone());
            let handle = std::thread::spawn(move || {
                worker.submit_choice(&who, &asked, version, &option).is_ok()
            });
            let wait = Instant::now() + Duration::from_secs(5);
            while !handle.is_finished()
                && session
                    .current_pending_decision()
                    .is_some_and(|(_, now, _)| now == nonce)
                && Instant::now() < wait
            {
                std::thread::sleep(Duration::from_millis(2));
            }
            return Some((seat, prompt, handle));
        }
        if session.is_finished() {
            return None;
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
fn a_declared_rider_is_acknowledged_before_its_owner_is_asked_the_prediction() {
    let session = started("declared_card_reply", 18_432_220_952_314_264_515);
    let mut open: Vec<(PlayerId, String, std::thread::JoinHandle<bool>)> = Vec::new();
    let mut checked = 0;
    for _ in 0..400 {
        // The question about to be answered: the seat that is asked the prediction of its Rider
        // has had its declaration acknowledged (the card is announced first).
        let Some((seat, prompt, handle)) = answer_first(&session) else { break };
        if prompt.ends_with("predict the agenda outcome") {
            if let Some(index) = open.iter().rposition(|(who, declared, _)| who == &seat && declared.contains("AGENDA_REVEALED")) {
                let (_, declared, earlier) = open.remove(index);
                let wait = Instant::now() + Duration::from_secs(5);
                while !earlier.is_finished() && Instant::now() < wait {
                    std::thread::sleep(Duration::from_millis(2));
                }
                assert!(
                    earlier.is_finished(),
                    "the answer of \"{declared}\" was not acknowledged when \"{prompt}\" was asked"
                );
                assert!(earlier.join().expect("the submit"));
                checked += 1;
            }
            if checked > 0 {
                let _ = handle.join();
                break;
            }
        }
        open.push((seat, prompt, handle));
    }
    assert!(checked > 0, "no Rider was declared and predicted in the game");
    session.stop();
}
