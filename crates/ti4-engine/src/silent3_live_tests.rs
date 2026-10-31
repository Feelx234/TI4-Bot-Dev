//! Lane G3: the loud gap rows turned into native rules, run natively through `live::live_step` against the string bodies they replace, in the turn-flow harness
//! of lane B (`turn_live_tests`): the questions as rendered, the answers, the events and the position must agree. (The invasion's abilities are in
//! `invasion_live_g3_tests`.)

use ti4_model::state::GameState;

use super::turn_live_tests::{action_table, assert_same, based, prelude_both};

// -- T'ro's activation mark -------------------------------------------------------------------------------------------------------

/// A table in the action phase, seat `a` (Sardakk) about to begin its turn with its activation mark of `marked` still standing.
fn tro_table(marked: &[&str]) -> GameState {
    let mut state = action_table(&[("a", "sardakk"), ("b", "hacan"), ("c", "sol")]);
    for who in marked {
        state.faction_marks.insert(format!("sardakk:agent:tactical:{who}"), "27".to_owned());
    }
    based(state)
}

#[test]
fn a_turn_that_begins_forgets_the_activation_mark_of_its_player_only_like_the_string() {
    let state = tro_table(&["a", "b"]);
    let (string, native) = prelude_both(&state, &[]);
    assert_same(&string, &native);
    let marks = &string.state["faction_marks"];
    assert!(marks.get("sardakk:agent:tactical:a").is_none() && marks.get("sardakk:agent:tactical:b").is_some(), "{marks}");
    // Without a mark nothing happens (and nothing is asked).
    let plain = tro_table(&[]);
    let (string, native) = prelude_both(&plain, &[]);
    assert_same(&string, &native);
}

// -- Acidos (the Nekro commander) -------------------------------------------------------------------------------------------------

/// A table where seat `a` (Nekro, commander unlocked) has gained a technology the game has not announced yet.
fn gained_table() -> GameState {
    use ti4_content::ContentStore;
    use ti4_model::state::LeaderStatus;
    let mut state = action_table(&[("a", "nekro"), ("b", "hacan"), ("c", "sol")]);
    state.player_mut(&super::turn_live_tests::pid("a")).unwrap().leaders.insert(ti4_model::id::LeaderId::new("nekrocommander"), LeaderStatus::Unlocked);
    // The first look records the starting set; a technology gained after it is announced.
    let mut game = crate::game::Game::with_table(state.clone(), ContentStore::embedded(), crate::choice::Table::with_default(Box::new(crate::choice::FirstOption)));
    game.prepared_turn_seq = Some(game.state.turn_seq);
    let _ = game.step();
    let mut state = game.state.clone();
    state.player_mut(&super::turn_live_tests::pid("a")).unwrap().technologies.insert(ti4_model::id::TechnologyId::new("st"));
    state
}

#[test]
fn the_nekro_commander_may_draw_a_card_for_a_technology_gained_like_the_string() {
    for answer in ["leader:nekro:nekrocommander:TECHNOLOGY_GAINED:after", "decline"] {
        let state = gained_table();
        let (string, native) = super::turn_live_tests::reconcile_both(&state, &[answer]);
        assert_same(&string, &native);
        assert!(string.events.iter().any(|e| e == "TECHNOLOGY_GAINED"), "{:?}", string.events);
        assert_eq!(string.questions.len(), 1, "{answer}: the window is asked once");
        let hand = |run: &super::turn_live_tests::Run| run.state["players"].as_array().map(|p| p[0]["action_cards"].as_array().map_or(0, Vec::len));
        let before = hand(&super::turn_live_tests::string_run(&state, &[], |_| {}));
        let after = hand(&string);
        assert_eq!(after.zip(before).map(|(a, b)| a - b), Some(usize::from(answer != "decline")), "{answer}");
    }
}
