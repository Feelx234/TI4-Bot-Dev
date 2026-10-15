//! Offline games through the native host, and the bot version of their saves.
//!
//! ```text
//! cargo run -p ti4-wasm --release --example bot_version_check -- [games] [players] [asks]
//! ```
//!
//! For each seed: (1) a new game (no `bot_version` in the setup) is played to the end, the person's
//! seat answering the first option of every question, and must finish; (2) the same game is
//! stopped after `asks` questions, saved, and resumed from the save: the replay must reach the
//! saved position (`resume_check.ok`) and the save must record the current bot version; (3) the
//! same with a version-1 game whose save has the field removed (a save made before the field
//! existed): resumed, it must replay as version 1. Exit code 1 on any failure.

use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use ti4_wasm::host_api;

/// Play `request` with the first option for every question of the person; stop after `stop`
/// questions (`None`: to the end). Returns (finished, asks, save at the stop, resume check).
fn play(request: &Value, stop: Option<u32>) -> Result<(bool, u32, Value, Value), String> {
    let asks = Arc::new(Mutex::new(0_u32));
    let check: Arc<Mutex<Value>> = Arc::default();
    let (a, c) = (asks.clone(), check.clone());
    ti4_wasm::set_native_host(move |text: &str| {
        let envelope: Value = serde_json::from_str(text).expect("the output is JSON");
        if envelope["kind"] != "ask" {
            return host_api::CONTINUE;
        }
        let mut n = a.lock().expect("asks");
        if *n == 0 {
            *c.lock().expect("check") = envelope["resume_check"].clone();
        }
        *n += 1;
        if stop.is_some_and(|stop| *n > stop) {
            return -1;
        }
        0
    });
    let len = host_api::put_input(&request.to_string());
    let status = ti4_wasm::ti4_host_start(u32::try_from(len).map_err(|e| e.to_string())?);
    let end = host_api::out_text();
    if status < 0 {
        return Err(format!("host failed: {end}"));
    }
    let saved = host_api::with(|session| session.save_state(Value::Null)).unwrap_or(Value::Null);
    let finished = saved["finished"] == true;
    let n = *asks.lock().expect("asks");
    let checked = check.lock().expect("check").clone();
    Ok((finished, n, saved, checked))
}

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let games: u64 = args.next().map_or(Ok(3), |t| t.parse()).map_err(|e| format!("{e}"))?;
    let players: u64 = args.next().map_or(Ok(4), |t| t.parse()).map_err(|e| format!("{e}"))?;
    let asks: u32 = args.next().map_or(Ok(120), |t| t.parse()).map_err(|e| format!("{e}"))?;
    let mut failed = false;
    for seed in 1..=games {
        let setup = json!({ "game_id": format!("check{seed}"), "seed": seed, "players": players });
        let (finished, n, _, _) = play(&json!({ "setup": setup }), None)?;
        let decisions = host_api::with(|s| s.decision_log().len()).unwrap_or(0);
        println!("seed {seed}: full game finished={finished} after {n} questions of the person, {decisions} decisions");
        failed |= !finished;

        let (_, _, save, _) = play(&json!({ "setup": setup }), Some(asks))?;
        let version = save.pointer("/setup/bot_version").cloned().unwrap_or(Value::Null);
        let (_, _, _, check) = play(&json!({ "setup": save["setup"], "resume": save }), Some(0))?;
        let ok = check["ok"] == true && version == json!(ti4_rule_bot::tuning::BOT_VERSION);
        println!("seed {seed}: new game saved with bot_version {version}, resumed: {check} -> {}", if ok { "OK" } else { "FAIL" });
        failed |= !ok;

        let mut old = setup.clone();
        old["bot_version"] = json!(1);
        let (_, _, mut save, _) = play(&json!({ "setup": old }), Some(asks))?;
        if let Some(setup) = save.get_mut("setup").and_then(Value::as_object_mut) {
            setup.remove("bot_version");
        }
        let (_, _, _, check) = play(&json!({ "setup": save["setup"], "resume": save }), Some(0))?;
        let ok = check["ok"] == true;
        println!("seed {seed}: version-1 save without the field, resumed: {check} -> {}", if ok { "OK" } else { "FAIL" });
        failed |= !ok;
        // For information: the same save resumed with the current bot (what every old save
        // would do without the field), usually a different game.
        let mut forced = save.clone();
        forced["setup"]["bot_version"] = json!(ti4_rule_bot::tuning::BOT_VERSION);
        let (_, _, _, check) = play(&json!({ "setup": forced["setup"], "resume": forced }), Some(0))?;
        println!("seed {seed}: the same save resumed with the current bot: ok={}", check["ok"]);
    }
    if failed {
        std::process::exit(1);
    }
    Ok(())
}
