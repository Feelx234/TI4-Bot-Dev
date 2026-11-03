//! Thunder's Edge content in the hosted game: the table switch brings the expedition into the
//! action menu, a game without it never offers it, and an expedition plays through to the planet.

use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use ti4_wasm::host_api;

use crate::common::{GAME, pending, send, serial, setup, start};

/// What a playthrough saw.
#[derive(Default)]
struct Seen {
    /// Every question of the human seat that offered an expedition slice: its option ids.
    expedition_menus: Vec<Vec<String>>,
    /// The subtype of each question after the first expedition pick, until the next action menu.
    follow_ups: Vec<String>,
    /// The expedition slices of the state after each answer, last wins.
    slices: Value,
    /// The placed planets of the last state.
    placed: Value,
    /// The last state's rules_version/thunders_edge flag.
    flag: Value,
    messages: Vec<Value>,
    taken: Option<String>,
    steps: usize,
    claimed_total: usize,
    /// How many of the slices seat `a` holds.
    mine: usize,
}

/// Plays the table with the rule bot, except that seat `a` takes an expedition slice whenever one
/// is offered, until `slices_wanted` slices are claimed by the game or `max_steps` questions ran.
fn play(setup: &Value, slices_wanted: usize, max_steps: usize) -> Seen {
    let seen = Arc::new(Mutex::new(Seen::default()));
    let sink = seen.clone();
    let seed = setup["seed"].as_u64().unwrap();
    ti4_wasm::set_native_host(move |text: &str| {
        let envelope: Value = serde_json::from_str(text).expect("JSON");
        if envelope["kind"] == "progress" {
            return host_api::CONTINUE;
        }
        let mut seen = sink.lock().unwrap();
        seen.messages
            .extend(envelope["messages"].as_array().unwrap().clone());
        let (nonce, version, choice, cursor) = pending(&seen.messages).expect("a question");
        let last = seen
            .messages
            .iter()
            .rev()
            .find(|m| m["state"].is_object())
            .cloned()
            .unwrap();
        seen.slices = last["state"]["expedition_slices"].clone();
        seen.placed = last["state"]["placed_planets"].clone();
        seen.flag = last["state"]["thunders_edge"].clone();
        seen.claimed_total = seen.slices.as_object().map_or(0, serde_json::Map::len);
        seen.mine = seen.slices.as_object().map_or(0, |slices| {
            slices.values().filter(|seat| *seat == "a").count()
        });
        seen.steps += 1;
        let place = usize::from(choice.player.as_str().as_bytes()[0] - b'a');
        let menu: Vec<String> = choice
            .options
            .iter()
            .filter(|o| o.id.starts_with("component|expedition|"))
            .map(|o| o.id.clone())
            .collect();
        if !menu.is_empty() {
            seen.expedition_menus.push(menu.clone());
        }
        let placed = seen
            .placed
            .as_object()
            .is_some_and(|p| p.contains_key("thundersedge"));
        if seen.steps >= max_steps
            || (if slices_wanted < 6 {
                seen.mine >= slices_wanted
            } else {
                seen.claimed_total >= 6 && placed
            })
        {
            return -1;
        }
        let ctx = choice.context.as_ref().map(|c| c.subtype.clone());
        if seen.claimed_total >= 6 || ctx.as_deref().is_some_and(|c| c.starts_with("expedition")) {
            seen.follow_ups
                .push(format!("{}:{}", choice.player, ctx.unwrap_or_default()));
        }
        let option = if choice.player.as_str() == "a" && !menu.is_empty() {
            seen.taken = Some(menu[0].clone());
            menu[0].clone()
        } else {
            let (picked, _) = ti4_rule_bot::choose(&choice, None, seed, place, cursor);
            choice.options[picked].id.clone()
        };
        let answered = send(&json!({
            "type": "submit_choice", "protocol_version": 4, "game_id": GAME,
            "nonce": nonce, "expected_version": version, "option_id": option,
        }));
        seen.messages
            .extend(answered["messages"].as_array().unwrap().clone());
        i32::try_from(answered["resume"].as_i64().expect("resume")).unwrap()
    });
    start(setup, None);
    let mut guard = seen.lock().unwrap();
    std::mem::take(&mut *guard)
}

#[test]
fn a_game_without_the_switch_never_offers_the_expedition() {
    let _serial = serial();
    let seen = play(&setup(11, 4), 1, 1500);
    assert!(seen.expedition_menus.is_empty());
    assert!(seen.flag.is_null(), "legacy states do not write the flag");
}

/// What `ti4_host_start` answers to `request`: the error text of a refusal.
fn refused(request: &Value) -> String {
    let len = host_api::put_input(&request.to_string());
    let status = ti4_wasm::ti4_host_start(u32::try_from(len).unwrap());
    let out = host_api::out_text();
    assert!(status < 0, "the start was not refused: {out}");
    out
}

/// Thunder's Edge content is switched off (owner decision 2026-11-03): the table switch and the
/// `expedition` preset are refused with the reason, before any game is built. The strategy card
/// set `te` is not content and still plays.
#[test]
fn thunders_edge_content_is_refused_at_the_start_with_its_reason() {
    let _serial = serial();
    let mut table = setup(13, 3);
    table["thunders_edge"] = json!(true);
    let text = refused(&json!({ "setup": table }));
    assert!(
        text.contains("Thunder's Edge content") && text.contains("switched off"),
        "{text}"
    );
    let mut table = setup(13, 3);
    table["preset"] = json!("expedition");
    let text = refused(&json!({ "setup": table }));
    assert!(text.contains("Thunder's Edge content"), "{text}");
    // The same table without the switch starts, with the `te` strategy cards.
    let mut table = setup(13, 3);
    table["card_set"] = json!("te");
    let seen = play(&table, 1, 20);
    assert!(seen.steps > 0, "the game asked nothing");
    assert!(seen.flag.is_null());
}
