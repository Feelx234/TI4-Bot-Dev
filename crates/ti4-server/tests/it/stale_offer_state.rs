//! Nightly 2026-10-23, run s1-04-0102 (#243): after Nav Suite and Flank Speed the engine offered
//! the ships of system 24 for the move, but the facts the client was sent said those ships were
//! out of range. The client then planned a pickup on a route the ships did not take, and the
//! batch was refused (`option unavailable`, step 2).
//!
//! The cause: the facts were routed on the map the game started with. The engine's own map
//! changes as the game goes (wormhole tokens, edited tiles), so ships that a token
//! put in range were reported out of range, and the pickups the client derived from the facts
//! were not on the route the engine takes.
//!
//! The game is the recorded s1-04 game (the nightly reports of the development machine; the
//! test skips when they are gone), recovered to the position of its last question.

use crate::support;

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use ti4_host::protocol::status::ViewerRole;
use ti4_host::view::tactical::TacticalFacts;
use ti4_server::session::{GameSession, SeatController, SessionConfig};

const SAVED: &str = "/root/TI4-Bot-Dev/nightly-reports/2026-10-23/runs/s1-04-0102/server-data/game_5305be255c3f786196dc2ae9313b1d78";

#[test]
fn the_movement_facts_route_on_the_map_the_engine_has() {
    let dir = Path::new(SAVED);
    if !dir.exists() {
        eprintln!("skipping: {SAVED} is not on this machine");
        return;
    }
    let source = support::load_saved(dir).expect("the saved game loads");
    let mut config = SessionConfig::new("stale_offer", source.state.clone())
        .with_galaxy(source.galaxy.clone(), Vec::new());
    config.rng_marks = source.marks.clone();
    for player in &source.state.players {
        config = config.with_seat(player.id.clone(), SeatController::Human);
    }
    let session = Arc::new(GameSession::start_recovered(
        config,
        source.records.clone(),
        Vec::new(),
    ));
    let deadline = Instant::now() + Duration::from_secs(60);
    // The stored game was played at an older rules version, which the core refuses (only the newest version is played): the session says so
    // instead of offering a question. Re-record the position at version 36 to run the checks below again.
    if source.state.rules_version != ti4_model::state::RULES_VERSION {
        let error = loop {
            if let Some(error) = session.error() {
                break error;
            }
            assert!(Instant::now() < deadline, "the older game was neither played nor refused");
            std::thread::sleep(Duration::from_millis(20));
        };
        assert!(error.contains("rules version"), "{error}");
        return;
    }
    let (seat, _, _) = loop {
        if let Some(pending) = session.current_pending_decision() {
            break pending;
        }
        assert!(Instant::now() < deadline, "no question after the recovery");
        std::thread::sleep(Duration::from_millis(20));
    };
    let snapshot = session.get_snapshot(&ViewerRole::Player(seat));
    let choice = snapshot.pending_choice.expect("a question").choice;
    assert_eq!(choice.context.unwrap().subtype, "movement_step");
    let Some(TacticalFacts::Movement(facts)) = snapshot.tactical else {
        panic!("movement facts");
    };
    let dreadnought = facts
        .ships
        .iter()
        .find(|ship| ship.origin == "24" && ship.unit == "dreadnought")
        .expect("the dreadnought of system 24");
    assert!(
        dreadnought.r#move.is_some(),
        "the engine's map puts system 24 in range: {dreadnought:?}"
    );
    session.stop();
}
