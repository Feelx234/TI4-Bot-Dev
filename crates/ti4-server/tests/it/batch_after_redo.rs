//! Was: a movement batch confirmed on a timeline a turn redo produced (the forked check, the resumed session and the recovery must leave the game
//! the full replay leaves although the history carries forced rng positions and card reservations). Turn redo is switched off by decision
//! (docs/DISABLE_XF123.md), so no such timeline exists: the redo is refused, the history carries no marks, and a game that goes on after the
//! refusal is the ordinary one (`batch_commit_resume` covers the batch path). The old scenario is in the history of this file.

use std::time::{Duration, Instant};

use ti4_content::ContentStore;
use ti4_host::protocol::status::ViewerRole;
use ti4_model::id::PlayerId;
use ti4_server::session::registry::HistoryError;
use ti4_server::session::{GameRegistry, SeatController, SessionConfig};

#[test]
fn a_turn_redo_before_a_batch_is_refused_and_leaves_no_marks_or_reservations() {
    let registry = GameRegistry::new();
    let players = vec![PlayerId::new("p1"), PlayerId::new("p2")];
    let (state, galaxy) = ti4_host::map::create_game_with_template(ContentStore::embedded(), &players, 42, None).unwrap();
    let tiles = ti4_host::map::build_board_tiles(ContentStore::embedded(), &galaxy);
    let config = SessionConfig::new("batch_after_redo", state)
        .with_seed(42)
        .with_player_ids(players)
        .with_galaxy(galaxy, tiles)
        .with_seat(PlayerId::new("p1"), SeatController::Human)
        .with_seat(PlayerId::new("p2"), SeatController::Human);
    let session = registry.create_game(config).unwrap();
    let host_token = session.seat_tokens()[&PlayerId::new("p1")].clone();
    for _ in 0..20 {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            if let Some((seat, nonce, version)) = session.current_pending_decision() {
                let option = session.get_snapshot(&ViewerRole::Player(seat.clone())).pending_choice.expect("pending").choice.options[0].id.clone();
                if session.submit_choice(&seat, &nonce, version, &option).is_ok() {
                    break;
                }
            }
            assert!(Instant::now() < deadline, "no decision became pending");
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    let refused = registry.turn_redo_request("batch_after_redo", &host_token, session.game_version(), Some("p1"), Some(1));
    assert!(matches!(&refused, Err(HistoryError::Forbidden(why)) if why == ti4_host::disabled::TURN_REDO_OFF), "{refused:?}");
    assert!(session.rng_marks().is_empty(), "the history carries no marks or reservations");
}
