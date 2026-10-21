//! A card played in a reaction window and the question it leads to (Skilled Retreat's
//! destination): a save taken at that question must play back to it.

use std::sync::{Arc, Mutex};

use crate::common::{Act, GAME, Page, install, last_question, pending, resume};
use serde_json::{Value, json};

/// The save of a nightly run (o3-04-2341, seed 2276135692): the position is the reaction window
/// after the combat round started, where the seat of Sol may play Skilled Retreat.
const SAVE: &str = include_str!("fixtures/skilled_retreat_save.json");

/// Answers the current question with `option` (the step of a person).
fn answer(option: &str) -> Act {
    let option = option.to_owned();
    Act::Probe(Box::new(move |page: &mut Page| {
        let (nonce, version, choice, _) = pending(&page.messages).unwrap();
        assert!(
            choice.options.iter().any(|o| o.id == option),
            "{option} not offered at {}: {:?}",
            choice.prompt,
            choice.options.iter().map(|o| &o.id).collect::<Vec<_>>()
        );
        page.script.push_front(Act::Cmd(json!({
            "type": "submit_choice", "protocol_version": 4, "game_id": GAME,
            "nonce": nonce, "expected_version": version, "option_id": option,
        })));
    }))
}

/// Plays on from the saved position with `steps`, saves at the question for the destination of
/// Skilled Retreat, plays that save again and checks that the replay came back to the saved game.
fn save_at_destination_and_resume(reaction: &str) {
    let base: Value = serde_json::from_str(SAVE).unwrap();
    let seed = base["seed"].as_u64().unwrap();
    let first = Arc::new(Mutex::new(Page {
        script: vec![
            answer("s_retreat1"),
            answer(reaction),
            Act::Probe(Box::new(|page| {
                let q = last_question(&page.messages);
                assert_eq!(
                    q["pending_choice"]["choice"]["context"]["subtype"],
                    "skilled_retreat_choose_system",
                    "{}",
                    q["pending_choice"]["choice"]
                );
            })),
            Act::Cmd(json!({ "type": "save_state" })),
            Act::Leave,
        ]
        .into(),
        ..Page::default()
    }));
    install(&first, seed, "d");
    resume(&base);
    let save = first.lock().unwrap().replies.last().unwrap()["save"].clone();

    let second = Arc::new(Mutex::new(Page {
        script: vec![Act::Leave].into(),
        ..Page::default()
    }));
    install(&second, seed, "d");
    resume(&save);
    let second = second.lock().unwrap();
    assert_eq!(
        second.asks[0]["resume_check"]["ok"], true,
        "{}",
        second.asks[0]["resume_check"]
    );
}

#[test]
fn a_save_at_the_destination_of_skilled_retreat_plays_back() {
    let _serial = crate::common::serial();
    // The reaction window opens again after the card; the person plays its ability (the window
    // that is still open holds the announcement of the answer back) ...
    save_at_destination_and_resume("reaction:sol:COMBAT_ROUND_STARTED:after");
}

#[test]
fn a_save_at_the_destination_after_declining_the_window_plays_back() {
    let _serial = crate::common::serial();
    // ... or declines it.
    save_at_destination_and_resume("decline");
}
