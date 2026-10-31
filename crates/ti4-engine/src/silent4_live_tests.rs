//! Lane G4: the last loud gap rows turned into native rules, run natively through `live::live_step` against the string bodies they replace, in the turn-flow harness
//! of lane B (`turn_live_tests`): the questions as rendered, the answers, the events and the position must agree.

use ti4_model::state::GameState;

use super::turn_live_tests::{action_table, assert_same, based, pid, reconcile_both};

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

#[allow(dead_code)]
fn _unused(_: &GameState) {
    let _ = pid("a");
}
