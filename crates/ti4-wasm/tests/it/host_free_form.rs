//! A deal composed on the trade desk (a free-form answer, `x;...`) is answered in an offline
//! game: the engine takes it by its id, the rule bot on the other side accepts or refuses it by
//! the value it prices, and the game goes on either way.

use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use ti4_engine::transactions::OPEN_KIND;
use ti4_wasm::host_api;

use crate::common::{GAME, Page, last_question, pending, send, serial, setup, start};

/// What the person did at the table, for the assertions.
#[derive(Default)]
struct Table {
    /// The free-form id that was sent.
    sent: Option<String>,
    /// The notes in hand (note id to holder) at the first question after the deal.
    notes: Option<Value>,
}

/// Plays seat `a` with the rule bot, except that the first transaction it can open is opened
/// and answered with `deal` (a free-form id).
fn install(page: &Arc<Mutex<Page>>, table: &Arc<Mutex<Table>>, seed: u64, deal: &'static str) {
    let page = page.clone();
    let table = table.clone();
    ti4_wasm::set_native_host(move |text: &str| {
        let envelope: Value = serde_json::from_str(text).expect("the output is JSON");
        let mut page = page.lock().unwrap();
        if envelope["kind"] != "ask" {
            return host_api::CONTINUE;
        }
        page.messages
            .extend(envelope["messages"].as_array().unwrap().clone());
        let (nonce, version, choice, cursor) =
            pending(&page.messages).expect("an ask has a pending choice");
        let mut table = table.lock().unwrap();
        if table.sent.is_some() && table.notes.is_none() {
            let state = last_question(&page.messages);
            table.notes = Some(state["state"]["promissory_notes"].clone());
        }
        let option = if table.sent.is_none() && choice.free_form.is_some() {
            table.sent = Some(deal.to_owned());
            deal.to_owned()
        } else if table.sent.is_none()
            && let Some(open) = choice.options.iter().find(|o| o.kind == OPEN_KIND)
        {
            open.id.clone()
        } else {
            let (picked, _) = ti4_rule_bot::choose(&choice, None, seed, 0, cursor);
            choice.options[picked].id.clone()
        };
        let answered = send(&json!({
            "type": "submit_choice", "protocol_version": 4, "game_id": GAME,
            "nonce": nonce, "expected_version": version, "option_id": option,
        }));
        page.messages
            .extend(answered["messages"].as_array().unwrap().clone());
        i32::try_from(answered["resume"].as_i64().expect("an answer resumes")).unwrap()
    });
}

/// Plays a game in which the person puts `deal` to the first neighbour it can trade with.
/// Returns the table and whether the game was played to its end.
fn play(deal: &'static str) -> (Table, bool) {
    let page = Arc::new(Mutex::new(Page::default()));
    let table = Arc::new(Mutex::new(Table::default()));
    install(&page, &table, 11, deal);
    let end = start(&setup(11, 4), None);
    let finished = end["end"]["finished"] == true && end["end"]["error"].is_null();
    let table = std::mem::take(&mut *table.lock().unwrap());
    (table, finished)
}

#[test]
fn a_free_form_gift_is_taken_by_the_rule_bot() {
    let _serial = serial();
    let (table, finished) = play("x;gn=cf:saar;");
    assert!(finished, "the game was played to its end");
    assert_eq!(table.sent.as_deref(), Some("x;gn=cf:saar;"));
    let notes = table.notes.expect("a question followed the deal");
    assert!(notes["cf:saar"] == "b", "the ghost took the note: {notes}");
}

#[test]
fn a_free_form_demand_is_refused_by_the_rule_bot() {
    let _serial = serial();
    let (table, finished) = play("x;rn=cf:ghost;");
    assert!(finished, "the game was played to its end");
    let notes = table.notes.expect("a question followed the deal");
    assert!(notes["cf:ghost"] == "b", "the ghost kept its note: {notes}");
}

/// A deal that cannot be paid (the person holds no 50 trade goods) is answered all the same: the
/// negotiation ends with nothing exchanged and the game goes on, with no answer left hanging.
#[test]
fn a_free_form_deal_that_cannot_be_paid_ends_the_negotiation() {
    let _serial = serial();
    let (table, finished) = play("x;gt=50;rn=cf:ghost;");
    assert!(finished, "the game was played to its end");
    let notes = table.notes.expect("a question followed the deal");
    assert!(notes["cf:ghost"] == "b", "nothing changed hands: {notes}");
}
