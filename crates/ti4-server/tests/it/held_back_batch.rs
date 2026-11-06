//! Nightly 2026-10-23, runs s2-01-2331 (#1145) and s1-11-0441 (#860): a batch sent for the
//! question on the table was refused `409 workflow interrupted at step 0`, because the check
//! met an older question first.
//!
//! The cause: a card played in a reaction window is only declared there and announced later.
//! The worker does not publish such an answer (or anything after it) before the announcement,
//! so while the next question was already on the table the published log ended before it. The
//! batch check replayed that log and met the window again. The session now keeps the answers it
//! has recorded but not published (`GameSession::batch_basis`), and the batch replays them.
//!
//! The game: a human seat holds Summit and Manipulate Investments at the start of the strategy
//! phase and plays both into the window; the bots decline. The seat then answers with its first
//! option until Summit asks into which pools the tokens go (one question for both tokens).

use crate::support;

use std::sync::Arc;
use std::time::{Duration, Instant};

use ti4_host::batch::{BatchKind, MovementPlan, MovementStep, simulate};
use ti4_host::protocol::status::ViewerRole;
use ti4_model::id::{ActionCardId, PlayerId};
use ti4_server::session::{GameSession, SeatController, SessionConfig};

fn subtype_of(session: &GameSession, seat: &PlayerId) -> Option<String> {
    session
        .get_snapshot(&ViewerRole::Player(seat.clone()))
        .pending_choice
        .and_then(|envelope| envelope.choice.context.map(|c| c.subtype))
}

#[test]
fn a_batch_replays_the_answers_the_log_has_not_published_yet() {
    for seed in [3_u64, 4, 5] {
        let (mut state, galaxy) =
            support::factions_start(seed, ["sol", "hacan", "letnev", "xxcha"]);
        let a = PlayerId::new("a");
        for card in ["summit", "investments"] {
            state
                .player_mut(&a)
                .unwrap()
                .action_cards
                .push(ActionCardId::new(card));
        }
        state
            .player_mut(&PlayerId::new("b"))
            .unwrap()
            .action_cards
            .push(ActionCardId::new("sabo1"));
        let mut config = SessionConfig::new("held_back", state)
            .with_galaxy(galaxy, Vec::new())
            .with_seat(a.clone(), SeatController::Human);
        for bot in ["b", "c", "d"] {
            config = config.with_seat(PlayerId::new(bot), SeatController::BotAlwaysDecline);
        }
        let restart = config.clone();
        let session = Arc::new(GameSession::start(config));
        let deadline = Instant::now() + Duration::from_secs(20);
        let mut reached = false;
        while Instant::now() < deadline {
            let Some((seat, nonce, version)) = session.current_pending_decision() else {
                std::thread::sleep(Duration::from_millis(5));
                continue;
            };
            let Some(subtype) = subtype_of(&session, &seat) else {
                continue;
            };
            if subtype == "gain_command_token" {
                reached = true;
                break;
            }
            let options = session
                .get_snapshot(&ViewerRole::Player(seat.clone()))
                .pending_choice
                .unwrap()
                .choice
                .options;
            // Summit first: its announcement (and its question) then comes before the other
            // card's.
            let option = options
                .iter()
                .find(|o| o.id == "summit")
                .unwrap_or(&options[0])
                .id
                .clone();
            let handle = {
                let (session, seat, nonce) = (session.clone(), seat.clone(), nonce.clone());
                std::thread::spawn(move || session.submit_choice(&seat, &nonce, version, &option))
            };
            // The reply to a declared card comes when its window closes; do not wait for it.
            let waited = Instant::now() + Duration::from_secs(5);
            while !handle.is_finished()
                && session
                    .current_pending_decision()
                    .is_some_and(|(_, now, _)| now == nonce)
                && Instant::now() < waited
            {
                std::thread::sleep(Duration::from_millis(2));
            }
        }
        assert!(reached, "seed {seed}: the seat was asked for a token pool");
        let published = session.decision_log();
        let (prior, held, snapshot) = session.batch_basis();
        eprintln!(
            "seed {seed}: {} published, {} held back",
            published.len(),
            held.len()
        );
        assert_eq!(prior.len(), published.len() + held.len());
        let plan = MovementPlan {
            kind: BatchKind::Tokens,
            destination: String::new(),
            steps: vec![
                MovementStep::Pool {
                    pool: "fleet_tokens".into(),
                },
                MovementStep::Pool {
                    pool: "strategic_tokens".into(),
                },
            ],
        };
        let full = simulate(&restart, &prior, &a, &plan, snapshot.as_deref())
            .unwrap_or_else(|failure| panic!("seed {seed}: the full log replays: {failure:?}"));
        // Summit's two tokens are ONE gain question (`{tactic}|{fleet}|{strategic}`): both pool
        // steps answer it.
        assert_eq!(full.selected.len(), 1);
        assert_eq!(full.selected[0].id, "0|1|1");
        if !held.is_empty() {
            assert!(
                held.iter().all(Option::is_some),
                "every held answer keeps the option it selected"
            );
            let short = simulate(&restart, &published, &a, &plan, None);
            assert!(
                short.is_err(),
                "seed {seed}: the published log alone does not lead to the question"
            );
        }
        session.stop();
    }
}
