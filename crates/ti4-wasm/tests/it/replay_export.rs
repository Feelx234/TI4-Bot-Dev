//! The save of a game of the page is self-contained (the table, the seats with their names and
//! factions, the card set, the seed...) and survives a JSON that has only floats for numbers; a
//! save made before the seats were listed still loads.

use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use ti4_wasm::host_api;

use crate::common::{Act, Page, install, resume, serial};

const SEED: u64 = 4_503_599_627_370_000;

fn table() -> Value {
    json!({ "table": {
        "setup": { "player_count": 3, "seed": SEED, "strategy_cards": "pok", "speaker_order": "seat",
                   "starting_tech": true, "thunders_edge": true },
        "seats": [
            { "kind": "human", "name": "Ada", "faction": "sol" },
            { "kind": "smart", "name": "Rule Bot", "faction": "hacan" },
            { "kind": "random", "name": "Dice Bot" },
        ],
    } })
}

/// Plays the table for `answers` questions of seat `a` (the rule bot answers for the page) and
/// leaves; returns the save.
fn played(answers: usize) -> Value {
    let page = Arc::new(Mutex::new(Page::default()));
    page.lock()
        .unwrap()
        .script
        .extend([Act::Answer(answers), Act::Leave]);
    install(&page, SEED, "a");
    let len = host_api::put_input(&table().to_string());
    let status = ti4_wasm::ti4_host_start(u32::try_from(len).unwrap());
    assert!(status >= 0, "{}", host_api::out_text());
    saved()
}

fn saved() -> Value {
    host_api::with(|s| s.save_state(json!({ "build": "test", "content_digest": "none" })))
        .expect("a session")
}

/// The save played again from the file alone; returns the save it makes.
fn replayed(save: &Value) -> Value {
    let page = Arc::new(Mutex::new(Page::default()));
    page.lock().unwrap().script.push_back(Act::Leave);
    install(&page, SEED, "a");
    resume(save);
    saved()
}

/// `JSON.parse` then `JSON.stringify` in Node, when Node is there.
fn node_round_trip(text: &str) -> Option<String> {
    use std::io::Write;
    let mut child = std::process::Command::new("node")
        .args([
            "-e",
            "let s='';process.stdin.on('data',d=>s+=d).on('end',()=>process.stdout.write(JSON.stringify(JSON.parse(s))))",
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .ok()?;
    child.stdin.take()?.write_all(text.as_bytes()).ok()?;
    let out = child.wait_with_output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8(out.stdout).ok())?
}

#[test]
fn a_save_lists_the_seats_and_replays_to_the_same_hash_after_a_hostile_trip() {
    let _serial = serial();
    let save = played(60);
    assert!(save["decisions"].as_array().unwrap().len() > 60);
    // The reader of the file sees who sat where, without decoding the table.
    let seats = save["seats"].as_array().unwrap();
    assert_eq!(seats.len(), 3);
    assert_eq!(seats[0]["name"], "Ada");
    assert_eq!(seats[0]["kind"], "human");
    assert_eq!(seats[0]["faction"], "sol");
    assert_eq!(seats[1]["name"], "Rule Bot");
    assert_eq!(seats[1]["kind"], "smart");
    assert_eq!(seats[1]["faction"], "hacan");
    assert_eq!(seats[2]["kind"], "random");
    assert!(seats[2]["faction"].is_string());
    // The setup is all in the save: map, card set, speaker order, switches, seed.
    let setup = &save["game"]["table"]["setup"];
    assert_eq!(setup["strategy_cards"], "pok");
    assert_eq!(setup["speaker_order"], "seat");
    assert_eq!(setup["thunders_edge"], true);
    assert_eq!(setup["seed"], SEED);
    assert_eq!(
        save["rules_version"].as_u64().unwrap_or(0) as u32,
        ti4_model::state::RULES_VERSION
    );
    assert_eq!(save["bot_version"], ti4_rule_bot::tuning::BOT_VERSION);

    let text = save.to_string();
    let through_value =
        serde_json::to_string(&serde_json::from_str::<Value>(&text).unwrap()).unwrap();
    let mut trips = vec![text, through_value];
    if let Some(node) = node_round_trip(&trips[0]) {
        trips.push(node);
    }
    for trip in trips {
        let again: Value = serde_json::from_str(&trip).unwrap();
        let back = replayed(&again);
        assert_eq!(back["decisions_hash"], save["decisions_hash"]);
        assert_eq!(back["decisions"], save["decisions"]);
    }
}

#[test]
fn a_save_made_before_the_seats_were_listed_still_loads_and_a_string_seed_is_read() {
    let _serial = serial();
    let save = played(40);
    let mut old = save.clone();
    old.as_object_mut().unwrap().remove("seats");
    // And a seed that a newer export wrote as a string.
    old["game"]["table"]["setup"]["seed"] = json!(SEED.to_string());
    let back = replayed(&old);
    assert_eq!(back["decisions_hash"], save["decisions_hash"]);
}
