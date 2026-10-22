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

/// The switch brings the expedition into the closing menu of a turn (the end of the turn, after
/// the action), never into the action menu, and a slice claimed there uses no action.
#[test]
fn the_expedition_is_offered_at_the_end_of_the_turn_and_plays_through_to_the_planet() {
    let _serial = serial();
    let mut table = setup(13, 3);
    table["thunders_edge"] = json!(true);
    table["preset"] = json!("expedition");
    let seen = play(&table, 6, 4000);
    assert_eq!(seen.flag, json!(true));
    assert!(
        seen.expedition_menus.iter().all(|menu| menu.len() <= 2),
        "only the two slices the seat can pay are offered: {:?}",
        seen.expedition_menus
    );
    assert_eq!(seen.claimed_total, 6, "slices: {}", seen.slices);
    assert_eq!(
        seen.mine, 2,
        "the first seat claimed the last two: {}",
        seen.slices
    );
    assert!(
        seen.placed
            .as_object()
            .is_some_and(|placed| placed.contains_key("thundersedge")),
        "the planet was placed: {} (follow-ups {:?})",
        seen.placed,
        seen.follow_ups
    );
    // The completion asks the finisher where to place it and which tied seat places the infantry.
    assert!(
        seen.follow_ups
            .iter()
            .any(|f| f.ends_with("expedition_place_system")),
        "{:?}",
        seen.follow_ups
    );
    assert!(
        seen.follow_ups
            .iter()
            .any(|f| f.ends_with("expedition_placement_tiebreak")),
        "{:?}",
        seen.follow_ups
    );
}

#[test]
fn slow_thunders_edge_games_play_to_the_end() {
    let _serial = serial();
    for (players, seed) in [(3, 21), (4, 22), (5, 23), (6, 24), (4, 25), (3, 26)] {
        let mut table = setup(seed, players);
        table["thunders_edge"] = json!(true);
        let seen = play(&table, 99, 40000);
        assert!(seen.steps < 40000, "{players}p seed {seed} did not end");
        assert_eq!(seen.flag, json!(true));
    }
}
