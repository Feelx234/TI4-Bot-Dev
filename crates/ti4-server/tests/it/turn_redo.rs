//! H7 turn redo (rewind a seat's last turn, play a new one, auto-play the others' recorded decisions with forced dice). The feature is switched
//! off by decision (`ti4_host::disabled::TURN_REDO_ENABLED`, docs/DISABLE_XF123.md): the redone timeline runs under a deck reserve
//! (`RngForce::attach`), which the typed core does not carry. The tests of the redo itself (turn spans, conflicts, forced dice, card reservations,
//! restart recovery) described unreachable behaviour and are replaced by the refusal: the reserve is a `GameError::Disabled`, the host's checks
//! name it, and a state that carries one is not playable. The old scenarios are in the history of this file (before the integration-21 commit).

use ti4_content::ContentStore;
use ti4_engine::choice::{Choice, ChoiceOption, Decider, IllegalChoice, Table};
use ti4_engine::native::{Game, GameError};
use ti4_host::disabled::{Disabled, check_playable};
use ti4_host::rng_force::{RngForce, RngMarks};
use ti4_model::deck_reserve::DeckReserve;

struct First;

impl Decider for First {
    fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
        Ok(choice.options[0].clone())
    }
}

fn marks_with_a_plan() -> RngMarks {
    let mut marks = RngMarks::new();
    marks.insert(0, ti4_engine::rng::RngPositions::default());
    marks
}

#[test]
fn attaching_a_deck_reserve_to_a_game_is_refused_with_disabled_at_its_first_step() {
    let state = ti4_engine::fixtures::seated_dealt(&[("p1", "sol"), ("p2", "hacan"), ("p3", "letnev")], ti4_model::content_types::POK);
    let mut game = Game::with_table(state, ContentStore::embedded(), Table::with_default(Box::new(First)));
    RngForce::always(&marks_with_a_plan()).attach(&mut game);
    assert!(game.state.deck_reserve.is_some(), "the force gave the game a reserve");
    let error = game.step().error.expect("refused");
    assert!(matches!(error, GameError::Disabled(_)), "{error:?}");
    assert_eq!(game.table.log.records.len(), 0, "nothing was played");
}

#[test]
fn a_state_that_carries_a_reserve_is_not_playable() {
    let mut state = ti4_engine::fixtures::seated_dealt(&[("p1", "sol"), ("p2", "hacan"), ("p3", "letnev")], ti4_model::content_types::POK);
    assert_eq!(check_playable(&state), Ok(()));
    let cursor = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    state.deck_reserve = Some(DeckReserve::new(cursor, Vec::new()));
    assert_eq!(check_playable(&state), Err(Disabled::TurnRedo));
}
