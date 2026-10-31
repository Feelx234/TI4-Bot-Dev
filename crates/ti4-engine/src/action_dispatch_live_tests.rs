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

/// A Winnu seat that holds Mecatol Rex and a ready Lazax Gate Folding.
fn lazax_table() -> GameState {
    let mut state = action_table(&[("a", "winnu"), ("b", "sol"), ("c", "hacan")]);
    state.player_mut(&pid("a")).unwrap().technologies.insert(ti4_model::id::TechnologyId::new("lgf"));
    let mecatol = ti4_model::id::SystemId::new(crate::seating::mecatol_on(&state));
    let planet = ti4_model::id::PlanetId::new(ti4_engine_core::generated::Planet::MecatolRex.as_str());
    state.board.entry(mecatol).or_default().set_control(planet, pid("a"));
    super::turn_live_tests::based(state)
}

#[test]
fn lazax_gate_folding_chosen_from_the_menu_places_the_infantry_like_the_string() {
    let state = lazax_table();
    let (string, native) = both(&state, &["faction|winnu|lgf"]);
    assert_same(&string, &native);
    assert!(has(&string, "COMPONENT_ACTION_RESOLVED"), "{:?}", string.events);
}

fn fragments_table(deck: &[&str]) -> GameState {
    let mut state = action_table(&THREE);
    state.player_mut(&pid("a")).unwrap().relic_fragments.insert("CULTURAL".to_owned(), 3);
    state.relic_deck = deck.iter().map(|r| ti4_model::id::RelicId::new(*r)).collect();
    super::turn_live_tests::based(state)
}

#[test]
fn purging_three_fragments_for_a_relic_chosen_from_the_menu_gains_it_like_the_string() {
    let state = fragments_table(&["codex", "shard"]);
    let (string, native) = both(&state, &["purge|CULTURAL"]);
    assert_same(&string, &native);
    assert!(has(&string, "COMPONENT_ACTION_RESOLVED"), "{:?}", string.events);
}

#[test]
fn the_shard_of_the_throne_scores_at_once_when_purged_for_like_the_string() {
    let state = fragments_table(&["shard", "codex"]);
    let (string, native) = both(&state, &["purge|CULTURAL"]);
    assert_same(&string, &native);
    assert!(has(&string, "COMPONENT_ACTION_RESOLVED"), "{:?}", string.events);
}

#[test]
fn a_purge_that_cannot_resolve_fails_and_the_menu_withholds_it_this_turn_like_the_string() {
    let state = fragments_table(&[]);
    let (string, taken) = string_run_first(&state, &["purge|CULTURAL"], |g| {
        let _ = g.step();
        let _ = g.step();
    });
    let native = native_marked(&state, &[(true, Arm::TurnPrelude, None), (false, Arm::Ask, None), (true, Arm::Ask, None)], &taken.iter().map(String::as_str).collect::<Vec<_>>());
    assert_same(&string, &native);
    assert!(has(&string, "COMPONENT_ACTION_FAILED"), "{:?}", string.events);
    let second = string.questions.last().expect("the menu again");
    assert!(!second.options.iter().any(|o| o.id.starts_with("purge|")), "the failed purge is withheld: {:?}", second.ids());
}

#[test]
fn the_trade_primary_at_a_cabal_table_stages_the_goods_before_the_replenish_like_the_string() {
    let mut state = action_table(&[("a", "sol"), ("b", "hacan"), ("c", "cabal")]);
    state.player_mut(&pid("a")).unwrap().strategy_cards.push(ti4_model::id::StrategyCardId::new("pok5trade"));
    state.player_mut(&pid("a")).unwrap().commodities = 0;
    let state = super::turn_live_tests::based(state);
    let (string, native) = both(&state, &["strategic", "done"]);
    assert_same(&string, &native);
    assert_eq!(string.staged, ["TRADE_GOODS_GAINED", "COMMODITIES_REPLENISHED"], "the string's order");
}

fn orbital_table() -> GameState {
    let mut state = action_table(&THREE);
    state.player_mut(&pid("a")).unwrap().strategic_tokens = 2;
    super::turn_live_tests::based(state)
}

#[test]
fn orbital_drop_chosen_from_the_menu_drops_the_infantry_like_the_string() {
    let state = orbital_table();
    // The planet (when the seat has more than one), then the offered mech is declined.
    let (string, taken) = string_run_first(&state, &["faction|orbital_drop"], |g| {
        let _ = g.step();
    });
    let native = native_marked(&state, &[(true, Arm::TurnPrelude, None), (false, Arm::Ask, None)], &taken.iter().map(String::as_str).collect::<Vec<_>>());
    assert_same(&string, &native);
    assert!(has(&string, "COMPONENT_ACTION_RESOLVED"), "{:?} / {:?}", string.events, taken);
}

#[test]
fn orbital_drop_at_a_table_without_a_module_announces_nothing_like_the_string() {
    // sol, letnev, xxcha, jolnar: no seated faction has a module, so the string's token event is never made.
    let mut state = action_table(&[("a", "sol"), ("b", "letnev"), ("c", "xxcha"), ("d", "jolnar")]);
    state.player_mut(&pid("a")).unwrap().strategic_tokens = 2;
    let state = super::turn_live_tests::based(state);
    let (string, taken) = string_run_first(&state, &["faction|orbital_drop"], |g| {
        let _ = g.step();
    });
    let native = native_marked(&state, &[(true, Arm::TurnPrelude, None), (false, Arm::Ask, None)], &taken.iter().map(String::as_str).collect::<Vec<_>>());
    assert_same(&string, &native);
    assert!(!has(&string, "STRATEGY_TOKEN_SPENT"), "{:?}", string.events);
}
