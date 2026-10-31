//! Lane G4: the last loud gap rows turned into native rules, run natively through `live::live_step` against the string bodies they replace, in the turn-flow harness
//! of lane B (`turn_live_tests`): the questions as rendered, the answers, the events and the position must agree.

use ti4_model::state::GameState;

use super::turn_live_tests::{action_table, assert_same, based, pid, reconcile_both, Run};

/// The turn prelude of the string against the native prelude. A card move an ability stages (`ACTION_CARD_TAKEN`) is announced by the string's step before it stops at the action menu;
/// the native side leaves the staged `cards:staged:*` row in the position for the host's next step (the rows are `hooks_cards::StagedCardEvent`s, announced by
/// `reactions::announce_staged_card_events`): the rows are taken out of the native position and counted against the announced events of the string.
fn prelude_both(state: &GameState, ids: &[&str]) -> (Run, Run) {
    let (mut string, mut native) = super::turn_live_tests::prelude_both(state, ids);
    let rows: Vec<String> = native.state["faction_marks"].as_object_mut().map(|marks| {
        let keys: Vec<String> = marks.keys().filter(|k| k.starts_with("cards:staged:")).cloned().collect();
        keys.iter().filter_map(|k| marks.remove(k)).filter_map(|v| v.as_str().map(str::to_owned)).collect()
    }).unwrap_or_default();
    let announced = string.events.iter().filter(|e| *e == "ACTION_CARD_TAKEN").count();
    assert_eq!(rows.iter().filter(|r| r.starts_with("taken|")).count(), announced, "{rows:?}: each card taken is staged by the native side and announced by the string");
    string.events.retain(|e| e != "ACTION_CARD_TAKEN");
    (string, native)
}

// -- Propagation (Nekro) ----------------------------------------------------------------------------------------------------------

/// A table where seat `a` (Nekro) has had `pending` researches replaced by command tokens that the game has not announced yet.
fn propagation_table(pending: u32) -> GameState {
    let mut state = action_table(&[("a", "nekro"), ("b", "hacan"), ("c", "sol")]);
    state.faction_marks.insert("nekro:propagation:a".to_owned(), pending.to_string());
    based(state)
}

#[test]
fn a_replaced_research_gives_three_command_tokens_into_the_pools_of_the_nekro_like_the_string() {
    for (pending, answers) in [
        (1_u32, vec!["tactic_tokens", "fleet_tokens", "strategic_tokens"]),
        (1, vec!["tactic_tokens", "tactic_tokens", "tactic_tokens"]),
        (2, vec!["fleet_tokens", "fleet_tokens", "fleet_tokens", "strategic_tokens", "tactic_tokens", "tactic_tokens"]),
    ] {
        let state = propagation_table(pending);
        let (string, native) = reconcile_both(&state, &answers);
        assert_same(&string, &native);
        assert!(string.events.iter().any(|e| e == "PROPAGATION_RESEARCH"), "{:?}", string.events);
        assert_eq!(string.questions.len(), 3 * pending as usize, "three pool questions per replaced research");
        assert!(string.state["faction_marks"].get("nekro:propagation:a").is_none(), "the pending count is settled");
        let seat = |run: &super::turn_live_tests::Run| run.state["players"][0].clone();
        let (before, after) = (seat(&super::turn_live_tests::string_run(&state, &[], |_| {})), seat(&string));
        let total = |p: &serde_json::Value| ["tactic_tokens", "fleet_tokens", "strategic_tokens"].iter().map(|k| p[k].as_u64().unwrap_or(0)).sum::<u64>();
        assert_eq!(total(&after) - total(&before), 3 * u64::from(pending), "{answers:?}");
    }
    // Nothing owed: nothing is announced.
    let (string, native) = reconcile_both(&propagation_table(0), &[]);
    assert_same(&string, &native);
    assert!(string.events.iter().all(|e| e != "PROPAGATION_RESEARCH"));
}

// -- Spy Net (Yssaril) ------------------------------------------------------------------------------------------------------------

/// A table where seat `a` (Hacan) holds the Yssaril's Spy Net, seat `b` is the Yssaril holding `hand`, and `a` holds `own` action cards.
fn spy_table(hand: &[&str], own: &[&str]) -> GameState {
    use ti4_model::id::ActionCardId;
    let mut state = action_table(&[("a", "hacan"), ("b", "yssaril"), ("c", "sol")]);
    state.promissory_notes.insert("spynet:yssaril".to_owned(), pid("a"));
    state.player_mut(&pid("b")).unwrap().action_cards = hand.iter().map(|c| ActionCardId::new(*c)).collect();
    state.player_mut(&pid("a")).unwrap().action_cards = own.iter().map(|c| ActionCardId::new(*c)).collect();
    based(state)
}

#[test]
fn the_holder_of_spy_net_takes_a_card_of_the_yssaril_hand_at_the_start_of_the_turn_like_the_string() {
    const WINDOW: &str = "promissory:hacan:spynet:TURN_BEGAN:after";
    // Several printed cards (a question), one (taken unasked), a duplicate pair (one option), a decline of the window.
    for (hand, answers) in [
        (vec!["sabo1", "insub", "reverse_engineer"], vec![WINDOW, "insub"]),
        (vec!["sabo1", "insub", "reverse_engineer"], vec![WINDOW, "sabo1"]),
        (vec!["sabo1"], vec![WINDOW]),
        (vec!["sabo1", "sabo2"], vec![WINDOW]),
        (vec!["sabo1", "insub"], vec!["decline"]),
    ] {
        let state = spy_table(&hand, &[]);
        let (string, native) = prelude_both(&state, &answers);
        assert_same(&string, &native);
        let taken = answers.first() == Some(&WINDOW);
        let hand_of = |run: &super::turn_live_tests::Run, seat: usize| run.state["players"][seat]["action_cards"].as_array().map_or(0, Vec::len);
        assert_eq!(hand_of(&string, 0), usize::from(taken), "{hand:?} {answers:?}: one card is taken only by a use");
        assert_eq!(string.state["promissory_notes"].get("spynet:yssaril").is_none_or(|holder| holder != "a"), taken, "{hand:?} {answers:?}: the note goes home only after a use");
    }
    // Nothing to look at: the window is not offered.
    let (string, native) = prelude_both(&spy_table(&[], &[]), &[]);
    assert_same(&string, &native);
}

#[test]
fn spy_net_over_the_hand_limit_asks_the_discard_like_the_string() {
    const WINDOW: &str = "promissory:hacan:spynet:TURN_BEGAN:after";
    let full = ["coup", "dh1", "industrial_initiative", "investments", "reverse_engineer", "sabo1", "sabo2"];
    let state = spy_table(&["insub", "sabo1"], &full);
    let (string, native) = prelude_both(&state, &[WINDOW, "insub", "0"]);
    assert_same(&string, &native);
    assert!(string.questions.iter().any(|q| q.prompt.starts_with("over the hand limit")), "{:?}", string.questions.iter().map(|q| &q.prompt).collect::<Vec<_>>());
}
