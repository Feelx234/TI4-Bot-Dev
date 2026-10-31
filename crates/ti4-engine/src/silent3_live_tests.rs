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
