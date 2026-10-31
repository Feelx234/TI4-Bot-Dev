//! Lane F (package KM): the action menu and the closing menu the core assembles (`ti4_engine_core::driver::turn::{action_menu, action}`)
//! run through `live::live_step` against the string's own `turn_options` / `closing_options` and the pass of `apply_choice`, question by
//! question: prompt, option ids, kinds, labels, the display facts of `with_turn_menu` (`details`), the context, the events and the position.

use ti4_model::state::GameState;

use super::turn_live_tests::{action_table, assert_same, native_marked, pid, string_run, Run, THREE};
use ti4_engine_core::driver::session::Arm;
use ti4_engine_core::generated::ActionCard;
use crate::generated::IdStr;

/// The string's step of a position (prelude, the action menu asked, `ids` answering) against the native prelude and menu steps.
fn step_both(state: &GameState, ids: &[&str]) -> (Run, Run) {
    let string = string_run(state, ids, |g| {
        let _ = g.step();
    });
    let native = native_marked(state, &[(true, Arm::TurnPrelude, None), (false, Arm::Ask, None)], ids);
    (string, native)
}

#[test]
fn the_action_menu_of_a_plain_seat_is_asked_with_its_display_facts_and_a_pass_ends_the_turn_like_the_string() {
    let state = action_table(&THREE);
    let (string, native) = step_both(&state, &["pass"]);
    assert_same(&string, &native);
    let menu = string.questions.last().expect("the menu was asked");
    assert_eq!(menu.details.get("kind").and_then(|v| v.as_str()), Some("turn_menu"));
    assert!(string.events.iter().any(|e| e == "PLAYER_PASSED"), "{:?}", string.events);
}

#[test]
fn a_seat_with_cards_relics_and_a_hand_is_offered_every_source_like_the_string() {
    let mut state = action_table(&THREE);
    let cards: Vec<_> = state.unclaimed_strategy_cards.clone();
    // Seats b and c hold cards (seat a, the active one, has none: with an unused card "pass" is not offered).
    for (seat, card) in ["b", "c"].iter().zip(cards.iter()) {
        state.deal_strategy_card(&pid(seat), card.clone());
    }
    state.player_mut(&pid("a")).unwrap().relic_fragments.insert("CULTURAL".to_owned(), 3);
    state.player_mut(&pid("a")).unwrap().action_cards = vec![ti4_model::id::ActionCardId::new(ActionCard::EconomicInitiative.as_str()), ti4_model::id::ActionCardId::new(ActionCard::ALL[0].as_str())];
    let (string, native) = step_both(&based_state(state), &["pass"]);
    assert_same(&string, &native);
    let menu = string.questions.last().expect("the menu was asked");
    assert!(menu.options.iter().any(|o| o.id.starts_with("purge|")), "{:?}", menu.ids());
}

fn based_state(state: GameState) -> GameState {
    super::turn_live_tests::based(state)
}

/// The string's `finish_action` and the closing question it leads to (answered by `ids`), against the native finish job and closing step.
fn closing_both(state: &GameState, ids: &[&str]) -> (Run, Run) {
    let string = string_run(state, ids, |g| {
        g.prepared_turn_seq = Some(g.state.turn_seq);
        g.finish_action().expect("the action finishes");
        let _ = g.step();
    });
    let native = native_marked(state, &[(false, Arm::Ask, Some(ti4_engine_core::state::TurnJob::Finish)), (true, Arm::Ask, None)], ids);
    (string, native)
}

#[test]
fn fleet_logistics_offers_the_second_action_and_the_closing_menu_is_the_cores_like_the_string() {
    let mut state = action_table(&THREE);
    state.player_mut(&pid("a")).unwrap().technologies.insert(ti4_model::id::TechnologyId::new("fl"));
    let (string, native) = closing_both(&state, &["end_turn"]);
    assert_same(&string, &native);
    let closing = string.questions.last().expect("the closing menu was asked");
    assert_eq!(closing.prompt, "end your turn, or take your second action");
    assert_eq!(closing.details.get("closing").and_then(|v| v.as_bool()), Some(true));
}

#[test]
fn a_leader_printed_during_the_action_phase_is_a_free_option_of_the_closing_menu_like_the_string() {
    let mut state = action_table(&[("a", "hacan"), ("b", "sol"), ("c", "arborec")]);
    // Carth of Golden Sands: "During the action phase".
    state.player_mut(&pid("a")).unwrap().leaders.insert(ti4_model::id::LeaderId::new("hacanagent"), ti4_model::state::LeaderStatus::Readied);
    let (string, native) = closing_both(&state, &["end_turn"]);
    assert_same(&string, &native);
    let closing = string.questions.last().expect("the closing menu was asked");
    assert!(closing.options.iter().any(|o| o.id == "component|leader|hacanagent"), "{:?}", closing.ids());
}
