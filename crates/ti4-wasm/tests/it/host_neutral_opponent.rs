//! A space combat won against the neutral ships of Pirate Contract asks no seat of the neutral
//! owner.

use std::sync::{Arc, Mutex};

use crate::common::{Act, GAME, Page, install, pending, resume};
use serde_json::{Value, json};

/// The save of a nightly run (o3-01-2330, seed 3880286144): the Ghosts are asked what to produce
/// at home; next the Mentaks (with their commander) beat the neutral destroyer of Pirate Contract
/// in system 28, and the commander asked the neutral owner for a note (the host panicked).
const SAVE: &str = include_str!("fixtures/neutral_opponent_save.json");

#[test]
#[ignore = "the fixture is a Thunder's Edge content game saved at rules version 27; both are refused (docs/D28E_RESUME.md): re-record the position without the content at version 35"]
fn winning_against_pirate_contract_ships_asks_the_neutral_owner_nothing() {
    let _serial = crate::common::serial();
    let base: Value = serde_json::from_str(SAVE).unwrap();
    let seed = base["seed"].as_u64().unwrap();
    let page = Arc::new(Mutex::new(Page {
        script: vec![
            Act::Probe(Box::new(|page: &mut Page| {
                let (nonce, version, choice, _) = pending(&page.messages).unwrap();
                assert!(
                    choice.options.iter().any(|o| o.id == "done_producing"),
                    "{}",
                    choice.prompt
                );
                page.script.push_front(Act::Cmd(json!({
                    "type": "submit_choice", "protocol_version": 4, "game_id": GAME,
                    "nonce": nonce, "expected_version": version, "option_id": "done_producing",
                })));
            })),
            Act::Answer(10),
            Act::Leave,
        ]
        .into(),
        ..Page::default()
    }));
    install(&page, seed, "c");
    resume(&base);
    assert!(page.lock().unwrap().asks.len() >= 2);
}
