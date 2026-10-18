//! Replays an offline save natively, with the same host crate the browser runs, to reproduce the
//! position of a feedback report.
//!
//! ```text
//! server feedback-save --data-dir /var/lib/ti4-public --out save.json fb_1760227200_ab12cd34
//! cargo run -p ti4-wasm --example replay_save -- save.json [--snapshot snapshot.json]
//! ```
//!
//! The file may be the save blob itself, a stored report (`context.local.save`) or a request body,
//! or a replay export of an online game (`GET /api/games/{id}/replay`, format `ti4-replay`, see
//! `docs/REPLAY_FORMAT.md`): that carries its own setup, so it needs no settings and is replayed
//! natively from its factions, seats, card set and seed through every journaled decision.
//! The game is played again from its seed and every saved decision until the saved position;
//! the tool prints whether the decisions came back the same (count and hash of the save) and what
//! the person was asked there. A save of a finished game has no question: the whole game's decisions
//! (count and hash) are compared instead. `--snapshot FILE` also writes the full `initial_snapshot` message
//! (the board as the person saw it) for inspection. Exit code 1 when the replay differs.

use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use ti4_wasm::host_api;

fn save_of(file: &Value) -> Option<Value> {
    [
        file.pointer("/context/local/save"),
        file.pointer("/local/save"),
        file.get("save"),
        Some(file),
    ]
    .into_iter()
    .flatten()
    .find(|v| v.get("game").is_some() && v.get("decisions").is_some())
    .cloned()
}

/// A replay export in `file`: the file itself, or under `replay` (a report that carries one).
fn export_of(file: &Value) -> Option<Value> {
    [file.get("replay"), Some(file)]
        .into_iter()
        .flatten()
        .find(|v| v["format"] == "ti4-replay")
        .cloned()
}

/// Replays an export from its own setup and says what it found. Exit code 1 when the journal does
/// not come back the same.
fn replay_export(value: Value) -> Result<(), String> {
    let export = ti4_host::export::ReplayExport::from_value(value)?;
    println!(
        "export: format {} of {}, seed {}, {} seats, {} decisions",
        export.format_version,
        export.game_id,
        export.seed.map_or("-".to_owned(), |s| s.to_string()),
        export.player_ids.len(),
        export.history.decisions.len(),
    );
    if let Some(build) = &export.build {
        println!(
            "written by: build {}, engine {}, content {}, rules version {}",
            build.build_commit.as_deref().unwrap_or("?"),
            build.engine_version.as_deref().unwrap_or("?"),
            build.content_digest.as_deref().unwrap_or("?"),
            build.rules_version,
        );
    }
    if let Some(setup) = &export.setup {
        let table = &setup.table;
        println!(
            "table: map {:?}, cards {}, speaker {:?}, starting tech {}, thunders edge {}",
            table.map,
            table.strategy_cards,
            table.speaker_order,
            table.starting_tech,
            table.thunders_edge,
        );
    }
    for seat in &export.seats {
        println!(
            "  seat {} {} ({}) {}",
            seat.seat,
            seat.nickname.as_deref().unwrap_or("-"),
            seat.kind,
            seat.faction.as_str(),
        );
    }
    let outcome = export.replay()?;
    let state = &outcome.final_state;
    println!(
        "replayed {} decisions: round {} phase {:?}, state hash {}",
        outcome.decisions, state.round, state.phase, outcome.final_state_hash,
    );
    let mut ok = outcome.hashes_match;
    if outcome.setup_matches == Some(false) {
        println!(
            "DIFFERENT: the setup builds another opening state than the one the game began with"
        );
        ok = false;
    }
    if let Some(check) = &export.check {
        println!(
            "saved: decisions {} hash {}; found: decisions {} hash {}",
            check.decisions, check.decisions_hash, outcome.decisions, outcome.decisions_hash,
        );
        ok &=
            check.decisions == outcome.decisions && check.decisions_hash == outcome.decisions_hash;
    }
    if ok {
        println!("OK: the game is reproduced");
        Ok(())
    } else {
        println!("DIFFERENT: this build decides otherwise than the one that made the file");
        std::process::exit(1);
    }
}

fn main() -> Result<(), String> {
    let mut path = None;
    let mut snapshot_path = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--snapshot" => snapshot_path = Some(args.next().ok_or("--snapshot needs a file")?),
            _ if path.is_none() => path = Some(arg),
            _ => return Err(format!("unexpected argument {arg:?}")),
        }
    }
    let path = path.ok_or("usage: replay_save SAVE.json [--snapshot FILE]")?;
    let file: Value =
        serde_json::from_slice(&std::fs::read(&path).map_err(|e| format!("{path}: {e}"))?)
            .map_err(|e| format!("{path}: {e}"))?;
    if let Some(export) = export_of(&file) {
        return replay_export(export);
    }
    let save = save_of(&file).ok_or("no save found (need game and decisions)")?;
    let seed = ti4_host::wire::read_u64(&save["seed"]).ok_or("save.seed")?;
    let humans = save["humans"].as_u64().unwrap_or(0);
    if let Some(local) = file.pointer("/context/local").or_else(|| file.get("local")) {
        println!(
            "report: seed {seed}, engine build {}, content {}, wasm {}",
            local.pointer("/engine/build").unwrap_or(&Value::Null),
            local
                .pointer("/engine/content_digest")
                .unwrap_or(&Value::Null),
            local.get("wasm_sha256").unwrap_or(&Value::Null),
        );
    }

    // The first question after the replay ends the run; keep what it showed.
    let seen: Arc<Mutex<Option<Value>>> = Arc::default();
    let sink = seen.clone();
    ti4_wasm::set_native_host(move |text: &str| {
        let envelope: Value = serde_json::from_str(text).expect("the output is JSON");
        match envelope["kind"].as_str() {
            Some("progress") => return host_api::CONTINUE,
            Some("ask") => *sink.lock().expect("sink") = Some(envelope),
            _ => {}
        }
        -1
    });
    let request = json!({ "resume": save });
    let len = host_api::put_input(&request.to_string());
    let status = ti4_wasm::ti4_host_start(u32::try_from(len).map_err(|e| e.to_string())?);
    let end = host_api::out_text();
    if status < 0 {
        return Err(format!("the replay failed: {end}"));
    }
    let asked = seen.lock().expect("seen").take();
    if asked.is_none() && save["finished"] == true {
        // A finished game has no question to compare at: compare everything it decided instead.
        let (found, hash) = host_api::with(|session| {
            (
                session.decision_log().len(),
                ti4_wasm::host_session::decisions_hash(session.decision_log()),
            )
        })
        .ok_or("the replay left no session")?;
        println!(
            "replayed {} answers of the page's seats {humans:#b}: the game ended",
            save["answers"].as_array().map_or(0, Vec::len),
        );
        println!(
            "saved: decisions {} hash {}",
            save["decisions"], save["decisions_hash"]
        );
        println!("found: decisions {found} hash {hash}");
        if save["decisions"] == json!(found) && save["decisions_hash"] == json!(hash) {
            println!("OK: the finished game is reproduced");
            return Ok(());
        }
        println!("DIFFERENT: this build decides otherwise than the one that made the save");
        std::process::exit(1);
    }
    let ask = asked.ok_or_else(|| format!("the game ended before a question: {end}"))?;
    let snapshot = ask["messages"]
        .as_array()
        .and_then(|m| m.iter().find(|m| m["type"] == "initial_snapshot"))
        .cloned()
        .unwrap_or(Value::Null);
    let check = &ask["resume_check"];
    println!(
        "replayed {} decisions, the page's seats {humans:#b}: round {} phase {} version {}",
        save["decisions"].as_array().map_or(0, Vec::len),
        snapshot.pointer("/state/round").unwrap_or(&Value::Null),
        snapshot.pointer("/state/phase").unwrap_or(&Value::Null),
        snapshot.get("game_version").unwrap_or(&Value::Null),
    );
    println!(
        "saved: format {} hash {}",
        save["format"], save["decisions_hash"]
    );
    println!("check: {check}");
    println!(
        "question: {}",
        snapshot
            .pointer("/pending_choice/choice/prompt")
            .unwrap_or(&Value::Null)
    );
    if let Some(out) = snapshot_path {
        std::fs::write(
            &out,
            serde_json::to_string_pretty(&snapshot).map_err(|e| e.to_string())?,
        )
        .map_err(|e| format!("{out}: {e}"))?;
        println!("wrote {out}");
    }
    if check["ok"] == true {
        println!("OK: the position is reproduced");
        Ok(())
    } else {
        println!("DIFFERENT: this build decides otherwise than the one that made the save");
        std::process::exit(1);
    }
}
