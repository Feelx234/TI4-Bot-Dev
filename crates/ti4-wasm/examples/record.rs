//! Writes the update of one choice of a native game to stdout, as a fixture for `web2/`.
//! Usage: cargo run -p ti4-wasm --example record -- <seed> <players> <human mask> <choice number> [option id ...]
//! With the id of a scenario in place of the seed, the game is that scenario (the players are
//! then not read): cargo run -p ti4-wasm --example record -- late-game 8 1 3 tactical 72
//! Each asked choice is listed on stderr. The option ids answer the first choices, in order; a
//! choice without one is answered by a fixed rule.
//!
//! The game is played through the hosted exports, as the page plays it. The fixture is the part
//! of the `state_update` that the page reads (`updateOf` in `web2/src/session/serverMessages.ts`).

use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use ti4_wasm::host_api;

/// The fields of a `state_update` that are the update of the page.
const UPDATE: [&str; 13] = [
    "viewer",
    "view",
    "pending_choice",
    "turn_status",
    "tactical",
    "exploration",
    "action_cards",
    "secret_objectives",
    "score",
    "agendas",
    "secondary",
    "reaction",
    "seats",
];

fn command(command: &Value) -> Value {
    let len = host_api::put_input(&command.to_string());
    let status = ti4_wasm::ti4_host_command(u32::try_from(len).expect("a short command"));
    let out = host_api::out_text();
    assert!(status >= 0, "{command}: {out}");
    serde_json::from_str(&out).expect("an envelope is JSON")
}

fn main() {
    let mut args = std::env::args().skip(1).peekable();
    let scenario = args.next_if(|arg| arg.parse::<u32>().is_err());
    let mut next = |default| {
        args.next()
            .and_then(|arg| arg.parse::<u32>().ok())
            .unwrap_or(default)
    };
    let seed = if scenario.is_some() { 0 } else { next(3) };
    let (players, mask, wanted) = (next(8), next(1), next(40));
    let mut script: std::collections::VecDeque<String> = args.collect();
    let recorded = Arc::new(Mutex::new(None));
    let sink = recorded.clone();
    let mut asked = 0_u32;
    ti4_wasm::set_native_host(move |output| {
        let envelope: Value = serde_json::from_str(output).expect("an envelope is JSON");
        if envelope["kind"] == "progress" {
            return host_api::CONTINUE;
        }
        let Some(message) = envelope["messages"].as_array().and_then(|messages| {
            messages.iter().rev().find(|message| {
                matches!(
                    message["type"].as_str(),
                    Some("state_update" | "initial_snapshot")
                )
            })
        }) else {
            eprintln!("an ask without an update: {output}");
            return -1;
        };
        let update: serde_json::Map<String, Value> = UPDATE
            .iter()
            .filter_map(|key| {
                let value = message.get(*key).filter(|value| !value.is_null())?;
                Some(((*key).to_owned(), value.clone()))
            })
            .collect();
        let update = Value::Object(update);
        let text = update.to_string();
        asked += 1;
        let pending = &update["pending_choice"];
        let choice = &pending["choice"];
        let options = choice["options"].as_array().map_or(0, Vec::len);
        eprintln!(
            "{asked:4} {} {} [{options}] {} bytes",
            choice["player"].as_str().unwrap_or("?"),
            choice["prompt"].as_str().unwrap_or("?"),
            text.len()
        );
        if asked == wanted {
            *sink.lock().expect("sink lock") = Some(text);
            return -1;
        }
        let id = match script.pop_front() {
            Some(id) => id,
            // Not always the first option, so that the game moves on.
            None => choice["options"][asked as usize * 7 % options.max(1)]["id"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
        };
        let answered = command(&json!({
            "type": "submit_choice",
            "protocol_version": ti4_host::PROTOCOL_VERSION,
            "game_id": "local",
            "nonce": pending["nonce"],
            "expected_version": message["game_version"],
            "option_id": id,
        }));
        match answered["resume"].as_i64() {
            Some(resume) => i32::try_from(resume).expect("a small code"),
            None => {
                eprintln!("choice {asked} does not offer {id:?}: {answered}");
                -1
            }
        }
    });
    let local = match &scenario {
        Some(id) => json!({ "scenario": id, "humans": mask }),
        None => json!({ "seed": seed, "players": players, "humans": mask }),
    };
    let len = host_api::put_input(&json!({ "local": local }).to_string());
    let status = ti4_wasm::ti4_host_start(u32::try_from(len).expect("a short request"));
    if status < 0 {
        eprintln!("{}", host_api::out_text());
        std::process::exit(1);
    }
    match recorded.lock().expect("sink lock").take() {
        Some(update) => println!("{update}"),
        None => {
            eprintln!("the game ended before choice {wanted}");
            std::process::exit(1);
        }
    }
}
