//! Lane M1: the native trade island (`ti4_engine_core::driver::windows::trade`, `rules::trade_window::TradeWindow` held on the stack)
//! run through `live::live_step` against the string's own `Game::step` (`apply_choice`'s opening, `step_trade`): the questions as
//! rendered (prompt, option ids, kinds, labels, display facts, context), the events, the position, the events left staged and the dice
//! streams, from the action menu's "open a transaction" to the close of the window.
//!
//! The string plays `Game::step` on a table with a map: the first scripted id opens the transaction from the action menu, the rest are
//! the negotiation. The native side opens the same transaction with `trade::begin` (what the dispatch of the menu calls) and takes one
//! `live_step` of `Arm::Trade` per string step with the same ids.

use std::rc::Rc;

use ti4_content::ContentStore;
use ti4_engine_core::driver::session::Arm;
use ti4_engine_core::driver::windows::trade::begin;
use ti4_engine_core::state::SliceSet;
use ti4_model::content_types::DEFAULT;
use ti4_model::id::{PlayerId, SystemId};
use ti4_model::state::{GameState, Phase, TransientFlags};

use super::turn_live_tests::{assert_same, based, pid, string_run_first, without_staged, Run};
use crate::choice::{Choice, Table};
use crate::core_sync::codec::rng::TextSeeds;
use crate::core_sync::{materialize, Codec, Ctx, Sources};
use crate::live::{self, Scripted};
use crate::render::Render;

/// Seats `a` and `b` side by side in the centre system, the rest in the ring: the seats listed in `strangers` stand on opposite ring
/// systems (two apart), so they are neighbours of the centre's seats only.
pub(super) struct Table2 {
    pub state: GameState,
    pub hub: crate::fixtures::Hub,
}

/// A table of real factions in the action phase, seat `a` active with its turn prepared, every seat holding two trade goods and three
/// commodities; `a` and `b` (and `c`) stand in the centre, the seats of `far` on ring systems opposite each other.
pub(super) fn table(seats: &[(&str, &str)], far: &[&str]) -> Table2 {
    let mut state = crate::fixtures::seated_dealt(seats, DEFAULT);
    let hub = crate::fixtures::plain_hub();
    let centre = SystemId::new(hub.centre.clone());
    let mut ring = hub.outer.iter();
    let mut opposite: Option<String> = None;
    for (name, _) in seats {
        let who = PlayerId::new(*name);
        if far.contains(name) {
            let at = match opposite.take() {
                None => {
                    let first = ring.next().expect("a ring system").clone();
                    opposite = Some(hub.across(&first));
                    first
                }
                Some(o) => o,
            };
            crate::fixtures::put(&mut state, &SystemId::new(at), "cruiser", &who, 1);
        } else {
            crate::fixtures::put(&mut state, &centre, "cruiser", &who, 1);
        }
    }
    for seat in &mut state.players {
        seat.trade_goods = 2;
        seat.commodities = 3;
    }
    state.phase = Phase::Action;
    state.active = Some(pid(seats[0].0));
    Table2 { state: based(state), hub }
}

/// The string: `Game::step` until the transaction window is closed again. `ids` are the menu pick and then the negotiation.
fn string_side(t: &Table2, ids: &[&str]) -> (Run, Vec<String>, GameState) {
    let mut start = t.state.clone();
    // The prelude of the turn is not under test: the turn is prepared.
    let probe = super::Game::with_table(start.clone(), ContentStore::embedded(), Table::with_default(Box::new(crate::choice::FirstOption))).with_galaxy(t.hub.galaxy.clone());
    start = probe.state.clone();
    let seq = start.turn_seq;
    let (mut run, taken) = string_run_first(&start, ids, |g| {
        g.galaxy = Some(t.hub.galaxy.clone());
        g.prepared_turn_seq = Some(seq);
        // The menu step opens the window; the next steps are the window's.
        let _ = g.step();
        assert!(g.trade.is_some(), "the menu pick opened a transaction: {:?}", g.events);
        for _ in 0..40 {
            if g.trade.is_none() {
                break;
            }
            let _ = g.step();
        }
        assert!(g.trade.is_none(), "the negotiation ended");
    });
    // The first question is the action menu's.
    assert_eq!(run.questions.first().map(|q| q.options.iter().any(|o| o.id == ids[0])), Some(true), "the menu offers {:?}", ids[0]);
    run.questions.remove(0);
    (run, taken, start)
}

/// The native side from the same position: `begin` and then `live_step(Arm::Trade)` until the window leaves the stack.
fn native_side(t: &Table2, start: &GameState, partner: &str, ids: &[String]) -> Run {
    let mut cx = Ctx::of(start).expect("a table the codecs carry");
    let render = Render::new(cx.owners.ids().to_vec());
    let game = super::Game::with_table(start.clone(), ContentStore::embedded(), Table::with_default(Box::new(crate::choice::FirstOption)));
    let positions = game.rng_positions();
    let mut core = Codec::load(&mut cx, start, Sources { galaxy: Some(&t.hub.galaxy), rng: Some(&positions) }, SliceSet::ALL).expect("the position loads");
    let sources = DEFAULT;
    let kernel = live::kernel::kernel_for(sources);
    let env = live::env::env_with_plays(&render, &core, kernel, Rc::new(TextSeeds::of(&cx)), sources);
    let lcx = live::Cx::new(&render, &env);
    let proposer = cx.owners.seat(start.active.as_ref().expect("an active seat").as_str()).expect("seated");
    let partner = cx.owners.seat(partner).expect("seated");
    let mut asker = Scripted::new(ids[1..].iter().cloned());
    // The menu step of the string starts with the reconcile hooks, as every step does.
    reconcile(&mut core, &lcx, &mut asker);
    begin(&mut core, &env, proposer, partner).unwrap_or_else(|e| panic!("the native opener stopped: {e:?}"));
    for _ in 0..40 {
        reconcile(&mut core, &lcx, &mut asker);
        let step = live::live_step(&mut core, Arm::Trade, &lcx, &mut asker).unwrap_or_else(|e| panic!("the native trade arm stopped: {e:?}"));
        if step.finished {
            break;
        }
    }
    assert_eq!(asker.left(), 0, "every answer the string was given was taken");
    let questions = asker.seen.clone();
    let events = crate::render::events::events_text(&render, &cx, &core).expect("every event has a text");
    let after = materialize::materialize(&core, &cx, start).expect("materializes");
    let rng = materialize::rng_positions(&core).expect("the streams have names");
    let staged = core.pending().staged.iter().map(|e| crate::kernel_sync::text::event_text(&render, e.kind)).collect();
    let (state, _) = without_staged(serde_json::to_value(&after).expect("serializes"));
    Run { questions, events, state, rng, staged }
}

/// The reconcile hooks every string step starts with.
fn reconcile(core: &mut ti4_engine_core::state::CoreState, lcx: &live::Cx<'_>, asker: &mut Scripted) {
    loop {
        let step = live::live_reconcile(core, lcx, asker).unwrap_or_else(|e| panic!("the native reconcile stopped: {e:?}"));
        if step.finished {
            break;
        }
    }
}

/// Both sides on one table: the menu id of the partner is `component|trade|<faction>`.
pub(super) fn both(t: &Table2, partner: &str, faction: &str, script: &[&str]) -> (Run, Run) {
    let open = format!("component|trade|{faction}");
    let mut ids = vec![open.as_str()];
    ids.extend_from_slice(script);
    let (string, taken, start) = string_side(t, &ids);
    // The string took the first option after the script: the native side takes the same.
    let native = native_side(t, &start, partner, &taken);
    (string, native)
}

fn texts(run: &Run) -> Vec<&str> {
    run.questions.iter().map(|q: &Choice| q.prompt.as_str()).collect()
}

const SOL_HACAN: [(&str, &str); 2] = [("a", "sol"), ("b", "hacan")];
/// Arborec has a module: the gains of a deal are staged.
const ARBOREC_HACAN: [(&str, &str); 2] = [("a", "arborec"), ("b", "hacan")];

fn has(run: &Run, event: &str) -> bool {
    run.events.iter().any(|x| x.starts_with(event))
}

/// Both sides agree on a deal between `a` and `b` and the window leaves the stack.
fn agree(seats: &[(&str, &str)], script: &[&str]) -> Run {
    let t = table(seats, &[]);
    let (string, native) = both(&t, "b", seats[1].1, script);
    assert_same(&string, &native);
    string
}

#[test]
fn a_commodity_swap_is_opened_proposed_priced_and_struck_like_the_string() {
    let string = agree(&SOL_HACAN, &["cc3", "accept"]);
    for e in ["TRANSACTION_OPENED", "TRANSACTION_OFFERED", "TRANSACTION_RESOLVED", "TRANSACTION"] {
        assert!(has(&string, e), "{e} in {:?}", string.events);
    }
    assert!(string.staged.is_empty(), "no seated faction has a module: the string stages nothing (`supply::staging_enabled`)");
    assert_eq!(texts(&string).len(), 2, "{:?}", texts(&string));
}

#[test]
fn the_gains_of_a_deal_are_staged_while_a_seated_faction_has_a_module() {
    let string = agree(&ARBOREC_HACAN, &["cc3", "accept"]);
    assert_eq!(string.staged, ["TRADE_GOODS_GAINED", "TRADE_GOODS_GAINED"]);
}

#[test]
fn the_grid_of_goods_and_commodity_deals_is_listed_and_struck_in_the_strings_order() {
    for script in [["2:1", "accept"], ["ct3:2", "accept"], ["tc2:3", "accept"], ["c3:0", "accept"], ["cc1", "accept"], ["0:2", "accept"]] {
        let string = agree(&SOL_HACAN, &script);
        assert!(has(&string, "TRANSACTION_RESOLVED"), "{script:?}: {:?}", string.events);
    }
}

#[test]
fn refusing_declining_and_haggling_end_like_the_strings() {
    let refused = agree(&SOL_HACAN, &["2:1", "refuse"]);
    assert!(has(&refused, "TRANSACTION_REFUSED") && !has(&refused, "TRANSACTION_RESOLVED"));
    let declined = agree(&SOL_HACAN, &["decline"]);
    assert!(has(&declined, "TRANSACTION_ABANDONED"), "{:?}", declined.events);
    // One counter: the other chair proposes and the first accepts.
    let countered = agree(&SOL_HACAN, &["2:1", "counter", "1:1", "accept"]);
    assert!(has(&countered, "COUNTEROFFER") && has(&countered, "TRANSACTION_RESOLVED"), "{:?}", countered.events);
    // Two counters end the haggling.
    let bounded = agree(&SOL_HACAN, &["2:1", "counter", "1:1", "counter"]);
    assert!(has(&bounded, "COUNTEROFFER") && has(&bounded, "TRANSACTION_ABANDONED") && !has(&bounded, "TRANSACTION_RESOLVED"), "{:?}", bounded.events);
}

#[test]
fn a_note_is_sold_given_bought_and_swapped_like_the_string() {
    for script in [
        &["pnps:sol:2", "accept"][..],
        &["pncf:sol:2", "accept"][..],
        &["pnta:sol:2", "accept"][..],
        &["ss", "accept"][..],
        &["npcf:hacan:2", "accept"][..],
        &["cpcf:hacan:2", "accept"][..],
        &["cpconvoys:hacan:3", "accept"][..],
        &["pcps:sol:2", "accept"][..],
        &["nncf:sol>ta:hacan", "accept"][..],
        &["nnps:sol>ta:hacan", "refuse"][..],
    ] {
        let t = table(&SOL_HACAN, &[]);
        let (string, native) = both(&t, "b", "hacan", script);
        assert_same(&string, &native);
    }
}

#[test]
fn a_note_is_given_to_a_partner_who_cannot_pay_like_the_string() {
    let mut t = table(&SOL_HACAN, &[]);
    t.state.player_mut(&pid("b")).unwrap().trade_goods = 0;
    let (string, native) = both(&t, "b", "hacan", &["pnps:sol:0", "accept"]);
    assert_same(&string, &native);
    assert!(has(&string, "TRANSACTION_RESOLVED"));
}
