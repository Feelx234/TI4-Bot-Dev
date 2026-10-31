//! Lane F2b (package KM2): the options of the action menu whose bodies the core now runs (`ti4_engine_core::driver::turn::{action, component}`)
//! chosen from the menu question and run through `live::live_step` against the string's own `Game::step`: the questions as rendered, the events,
//! the position and the dice streams. The string step is the turn prelude, the menu asked, and the body of the answer.

use ti4_engine_core::driver::session::Arm;
use ti4_model::state::GameState;

use super::turn_live_tests::{action_table, assert_same, native_marked, pid, string_run_first, Run, THREE};
use crate::generated::IdStr;

/// The string's step of a position, the menu answered by `ids` (then the first option of what the body asks), against the native prelude and
/// menu steps answered the same.
fn both(state: &GameState, ids: &[&str]) -> (Run, Run) {
    let (string, taken) = string_run_first(state, ids, |g| {
        let _ = g.step();
    });
    let native = native_marked(state, &[(true, Arm::TurnPrelude, None), (false, Arm::Ask, None)], &taken.iter().map(String::as_str).collect::<Vec<_>>());
    (string, native)
}

fn has(run: &Run, event: &str) -> bool {
    run.events.iter().any(|e| e == event)
}

#[test]
fn a_strategic_action_chosen_from_the_menu_begins_like_the_string() {
    for card in ["pok5trade", "pok3politics", "pok1leadership", "pok8imperial"] {
        let mut state = action_table(&THREE);
        state.player_mut(&pid("a")).unwrap().strategy_cards.push(ti4_model::id::StrategyCardId::new(card));
        for p in ["b", "c"] {
            let seat = state.player_mut(&pid(p)).unwrap();
            seat.strategic_tokens = 2;
            seat.trade_goods = 6;
        }
        let state = super::turn_live_tests::based(state);
        let (string, native) = both(&state, &["strategic"]);
        assert_same(&string, &native);
        assert!(has(&string, "STRATEGIC_ACTION_BEGAN"), "{card}: {:?}", string.events);
    }
}

#[test]
fn an_action_card_played_as_the_action_resolves_and_ends_the_action_like_the_string() {
    use ti4_engine_core::generated::ActionCard;
    let mut state = action_table(&THREE);
    state.player_mut(&pid("a")).unwrap().action_cards = vec![ti4_model::id::ActionCardId::new(ActionCard::EconomicInitiative.as_str())];
    let state = super::turn_live_tests::based(state);
    let (string, native) = both(&state, &["action_card|0"]);
    assert_same(&string, &native);
    assert!(has(&string, "ACTION_CARD_PLAYED") && has(&string, "COMPONENT_ACTION_RESOLVED") && has(&string, "ACTION_COMPLETED"), "{:?}", string.events);
}

#[test]
fn extreme_duress_on_the_acting_seat_punishes_a_card_action_like_the_string() {
    use ti4_engine_core::generated::ActionCard;
    let mut state = action_table(&THREE);
    let seat = state.player_mut(&pid("a")).unwrap();
    seat.action_cards = vec![ti4_model::id::ActionCardId::new(ActionCard::EconomicInitiative.as_str()), ti4_model::id::ActionCardId::new(ActionCard::ALL[0].as_str())];
    seat.trade_goods = 3;
    seat.duress_by = Some(pid("b"));
    let state = super::turn_live_tests::based(state);
    let (string, native) = both(&state, &["action_card|0"]);
    assert_same(&string, &native);
    assert!(string.events.iter().any(|e| e.starts_with("EXTREME_DURESS:")), "{:?}", string.events);
}
