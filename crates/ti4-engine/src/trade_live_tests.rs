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
    // The opener announces the opening and runs the window to its first question (a window with nothing to propose is dropped there: the
    // string drops it one step later, with the same position).
    begin(&mut core, &env, proposer, partner).unwrap_or_else(|e| panic!("the native opener stopped: {e:?}"));
    for _ in 0..40 {
        if !ti4_engine_core::driver::live::is_open::<ti4_engine_core::rules::trade_window::TradeWindow>(&core) {
            break;
        }
        reconcile(&mut core, &lcx, &mut asker);
        let step = live::live_step(&mut core, Arm::Trade, &lcx, &mut asker).unwrap_or_else(|e| panic!("the native trade arm stopped: {e:?}"));
        if step.finished {
            break;
        }
    }
    assert_eq!(asker.left(), 0, "every answer the string was given was taken; the native side was asked {:?}", asker.seen.iter().map(|c| (&c.prompt, c.ids())).collect::<Vec<_>>());
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

const HACAN_SOL: [(&str, &str); 2] = [("a", "hacan"), ("b", "sol")];

/// Hacan's Arbiters let action cards change hands: `a` holds cards and the first by alias is sold for a good or swapped for a note.
#[test]
fn the_arbiters_trade_an_action_card_for_a_good_or_a_note_like_the_string() {
    for script in [&["acambush:1", "accept"][..], &["cnambush>cf:sol", "accept"][..]] {
        let mut t = table(&HACAN_SOL, &[]);
        let cards: Vec<&str> = <ti4_engine_core::generated::ActionCard as crate::generated::IdStr>::IDS.iter().copied().skip(10).take(2).collect();
        for card in &cards {
            t.state.player_mut(&pid("a")).unwrap().action_cards.push(ti4_model::id::ActionCardId::new(*card));
        }
        let script: Vec<String> = script.iter().map(|x| x.replace("ambush", cards[0])).collect();
        let script: Vec<&str> = script.iter().map(String::as_str).collect();
        let (string, native) = both(&t, "b", "sol", &script);
        assert_same(&string, &native);
        assert!(has(&string, "TRANSACTION_RESOLVED"), "{script:?}: {:?}", string.events);
    }
}

/// Black Market Dealings marks the negotiation: secrets and relic fragments are offered for a good, and the mark dies with the window.
#[test]
fn the_black_market_offers_secrets_and_fragments_and_its_mark_dies_with_the_negotiation_like_the_string() {
    for script in [&["decline"][..], &["frCULTURAL:1", "accept"][..], &["so0:1", "accept"][..]] {
        let mut t = table(&SOL_HACAN, &[]);
        t.state.transient_flags.set(TransientFlags::BLACK_MARKET);
        t.state.player_mut(&pid("a")).unwrap().relic_fragments.insert("CULTURAL".to_owned(), 1);
        let secret = t.state.player(&pid("a")).unwrap().secret_objectives.first().cloned();
        let id = secret.map(|s| format!("so{}:1", s.as_str()));
        let script: Vec<&str> = script.iter().map(|x| if *x == "so0:1" { id.as_deref().expect("a secret in hand") } else { *x }).collect();
        let (string, native) = both(&t, "b", "hacan", &script);
        assert_same(&string, &native);
    }
}

/// The free-form deals: every kind of term, the malformed id and the deal nobody can pay.
#[test]
fn free_form_deals_with_every_kind_of_term_are_priced_asked_and_struck_like_the_string() {
    let ids = [
        "x;gt=1;rt=1;",
        "x;gc=2;rt=1;",
        "x;gt=1;gc=1;rc=2;",
        "x;gn=cf:sol;rt=2;",
        "x;gt=1;rn=ta:hacan;",
        "x;gn=cf:sol;rn=ta:hacan;",
        "x;gn=support:sol;rn=support:hacan;",
        "x;rt=1;",
        "x;gt=2;",
    ];
    for id in ids {
        let t = table(&SOL_HACAN, &[]);
        let (string, native) = both(&t, "b", "hacan", &[id, "accept"]);
        assert_same(&string, &native);
        assert!(has(&string, "TRANSACTION_RESOLVED"), "{id}: {:?}", string.events);
    }
}

#[test]
fn a_free_form_deal_that_is_malformed_or_cannot_be_paid_ends_like_the_string() {
    for (id, want) in [("x;bogus", "TRANSACTION_ABANDONED"), ("x;gt=9;rt=1;", "TRANSACTION_REJECTED"), ("x;gn=ps:sol;gn=ps:sol;", "TRANSACTION_ABANDONED")] {
        let t = table(&SOL_HACAN, &[]);
        let (string, native) = both(&t, "b", "hacan", &[id]);
        assert_same(&string, &native);
        assert!(has(&string, want), "{id}: {:?}", string.events);
    }
}

#[test]
fn a_free_form_counter_is_named_from_the_other_chair_like_the_string() {
    let t = table(&SOL_HACAN, &[]);
    let (string, native) = both(&t, "b", "hacan", &["x;gt=2;rt=1;", "counter", "x;gt=1;rt=1;", "accept"]);
    assert_same(&string, &native);
    assert!(has(&string, "COUNTEROFFER") && has(&string, "TRANSACTION_RESOLVED"), "{:?}", string.events);
}

/// Strangers cannot deal; Hacan's Guild Ships reach anyone.
#[test]
fn guild_ships_reach_a_stranger_and_a_stranger_is_turned_away_like_the_string() {
    let seats = [("a", "hacan"), ("b", "sol"), ("c", "arborec")];
    let t = table(&seats, &["b", "c"]);
    let (string, native) = both(&t, "b", "sol", &["2:1", "accept"]);
    assert_same(&string, &native);
    assert!(has(&string, "TRANSACTION_RESOLVED"), "{:?}", string.events);
}

/// A window with nothing to propose is dropped without a word (the string asks nothing and loses the window).
#[test]
fn a_window_with_nothing_to_propose_is_dropped_like_the_string() {
    let mut t = table(&SOL_HACAN, &[]);
    for p in &mut t.state.players {
        p.trade_goods = 0;
        p.commodities = 0;
    }
    t.state.support_holders.insert(pid("a"), pid("b"));
    t.state.support_holders.insert(pid("b"), pid("a"));
    for key in t.state.promissory_notes.keys().cloned().collect::<Vec<_>>() {
        t.state.promissory_notes.remove(&key);
    }
    let (string, native) = both(&t, "b", "hacan", &[]);
    assert_same(&string, &native);
    assert_eq!(string.events, ["TRANSACTION_OPENED"], "the opening is announced, nothing else is said");
}

/// Lie in Wait, held by a third seat, is played after the neighbours' transaction: the core reads the neighbours who traded from the position
/// (`lie_in_wait_traders`) and the two sides take the same cards.
#[test]
fn lie_in_wait_takes_a_card_from_each_neighbour_who_traded_like_the_string() {
    let seats = [("a", "sol"), ("b", "hacan"), ("c", "arborec")];
    let mut t = table(&seats, &[]);
    let cards: Vec<&str> = <ti4_engine_core::generated::ActionCard as crate::generated::IdStr>::IDS.iter().copied().skip(30).take(5).collect();
    let give = |state: &mut GameState, who: &str, card: &str| state.player_mut(&pid(who)).unwrap().action_cards.push(ti4_model::id::ActionCardId::new(card));
    give(&mut t.state, "a", cards[0]);
    give(&mut t.state, "b", cards[1]);
    give(&mut t.state, "c", "lieinwait");
    let (string, native) = both(&t, "b", "hacan", &["2:1", "accept"]);
    assert_same(&string, &native);
    assert!(has(&string, "TRANSACTION_RESOLVED") && has(&string, "ACTION_CARD_PLAYED"), "{:?}", string.events);
    let held = string.state["players"][2]["action_cards"].as_array().map_or(0, Vec::len);
    assert_eq!(held, 2, "the holder took one card from each of the two neighbours: {:?}", string.state["players"][2]["action_cards"]);
}

/// A Pride of Kenara on a planet `who` controls (and a second planet to move the units to); the planets it controls.
fn with_kenara(state: &mut GameState, who: &PlayerId) -> Vec<ti4_model::id::PlanetId> {
    if state.controlled_planets(who).len() < 2 {
        let (system, planet) = crate::fixtures::a_placed_planet();
        state.system_mut(&system).set_control(planet, who.clone());
    }
    let (system, planet) = {
        let owned = state.controlled_planets(who);
        let (system, planet) = owned.first().map(|(s, p)| ((*s).clone(), (*p).clone())).expect("a seat holds a planet");
        (system, planet)
    };
    state.system_mut(&system).planet_units.entry(planet).or_default().push(ti4_model::units::Unit::new(ti4_model::id::UnitTypeId::new("hacan_mech"), who.clone()));
    assert!(!crate::transactions::pride_of_kenara_planets(state, who).is_empty(), "the position gives {who} a Pride of Kenara");
    state.controlled_planets(who).into_iter().map(|(_, p)| p.clone()).collect()
}

fn planet_price(planet: &ti4_model::id::PlanetId) -> u8 {
    crate::transactions::planet_value(ContentStore::embedded(), planet) as u8
}

/// The planet cards a Pride of Kenara lets change hands: a gift, a sale, a purchase, each moving the units to the planet it names.
#[test]
fn a_planet_card_is_given_sold_and_bought_with_its_units_moving_like_the_string() {
    // The proposer's own card: the gift and the sale.
    for sale in [false, true] {
        let mut t = table(&HACAN_SOL, &[]);
        let planets = with_kenara(&mut t.state, &pid("a"));
        let price = if sale { planet_price(&planets[0]) } else { 0 };
        let id = format!("kp{}>{}:{price}", planets[0], planets[1]);
        let (string, native) = both(&t, "b", "sol", &[id.as_str(), "accept"]);
        assert_same(&string, &native);
        assert!(has(&string, "TRANSACTION_RESOLVED"), "{id}: {:?}", string.events);
    }
    // The partner's card, bought for its worth.
    let mut t = table(&SOL_HACAN, &[]);
    let planets = with_kenara(&mut t.state, &pid("b"));
    t.state.player_mut(&pid("a")).unwrap().trade_goods = 6;
    let id = format!("kq{}>{}:{}", planets[0], planets[1], planet_price(&planets[0]));
    let (string, native) = both(&t, "b", "hacan", &[id.as_str(), "accept"]);
    assert_same(&string, &native);
    assert!(has(&string, "TRANSACTION_RESOLVED"), "{id}: {:?}", string.events);
}

#[test]
fn a_free_form_planet_card_and_a_card_whose_mech_left_are_handled_like_the_string() {
    let mut t = table(&HACAN_SOL, &[]);
    let planets = with_kenara(&mut t.state, &pid("a"));
    let id = format!("x;gp={}>{};rt=1;", planets[0], planets[1]);
    let (string, native) = both(&t, "b", "sol", &[id.as_str(), "accept"]);
    assert_same(&string, &native);
    assert!(has(&string, "TRANSACTION_RESOLVED"), "{:?}", string.events);
    // The mech is not on the card: the free-form deal is rejected at the proposal.
    let mut t = table(&HACAN_SOL, &[]);
    let planets = with_kenara(&mut t.state, &pid("a"));
    for sys in t.state.board.values_mut() {
        for units in sys.planet_units.values_mut() {
            units.retain(|u| u.type_id.as_str() != "hacan_mech");
        }
    }
    let id = format!("x;gp={}>{};rt=1;", planets[0], planets[1]);
    let (string, native) = both(&t, "b", "sol", &[id.as_str()]);
    assert_same(&string, &native);
    assert!(has(&string, "TRANSACTION_REJECTED"), "{:?}", string.events);
}

/// Dark Pact: the holder gives the Empyrean exactly its maximum commodity value; both gain a good, staged after the deal's own gains.
#[test]
fn dark_pact_pays_both_sides_a_good_like_the_string() {
    let mut t = table(&[("a", "sol"), ("b", "empyrean")], &[]);
    let max = ti4_content::factions::get(ContentStore::embedded(), "sol").map(|f| f.commodities()).expect("sol");
    t.state.player_mut(&pid("a")).unwrap().commodities = max;
    t.state.promissory_notes.insert("dark_pact:empyrean".to_owned(), pid("a"));
    t.state.promissory_faceup.insert("dark_pact:empyrean".to_owned());
    let id = format!("c{max}:0");
    let (string, native) = both(&t, "b", "empyrean", &[id.as_str(), "accept"]);
    assert_same(&string, &native);
    assert_eq!(string.staged, ["TRADE_GOODS_GAINED"; 3], "the deal's gain and the two Dark Pact goods");
}

/// Black Market Dealings, played into the window the opening opens, marks the negotiation: the first question already offers the secrets.
#[test]
fn black_market_dealings_played_at_the_opening_widens_the_first_question_like_the_string() {
    let mut t = table(&SOL_HACAN, &[]);
    t.state.player_mut(&pid("a")).unwrap().action_cards.push(ti4_model::id::ActionCardId::new("blackmarketdealing"));
    t.state.player_mut(&pid("a")).unwrap().relic_fragments.insert("CULTURAL".to_owned(), 1);
    // The reaction window of the opening (the option is `reaction:...`), then the fragment sale.
    let (string, native) = {
        let open = "component|trade|hacan".to_owned();
        let (probe, taken, start) = {
            let mut ids = vec![open.as_str(), "reaction:sol:TRANSACTION_OPENED:when"];
            ids.extend(["frCULTURAL:1", "accept"]);
            string_side(&t, &ids)
        };
        let native = native_side(&t, &start, "b", &taken);
        (probe, native)
    };
    assert_same(&string, &native);
    assert!(string.questions.iter().any(|q| q.options.iter().any(|o| o.id == "frCULTURAL:1")), "the marked table is offered the fragments");
    assert!(has(&string, "TRANSACTION_RESOLVED"));
}

/// The menu source of the action menu: the seats that may be asked to a transaction now, from the core's position alone, are the string's
/// `open_partners` over strangers, the pair limit, the negotiation limits, Guild Ships and Trade Convoys.
#[test]
fn the_partners_the_menu_offers_are_the_strings_over_strangers_limits_and_reach() {
    /// A named change to the position the menu is judged in.
    type Tweak = Box<dyn Fn(&mut GameState)>;
    let content = ContentStore::embedded();
    let seats = [("a", "hacan"), ("b", "sol"), ("c", "arborec"), ("d", "yin")];
    let positions: Vec<(&str, Tweak)> = vec![
        ("plain", Box::new(|_| {})),
        ("pair spent", Box::new(|s| s.record_transaction(&pid("a"), &pid("b")))),
        ("two negotiations this action", Box::new(|s| {
            s.note_negotiation(&pid("a"));
            s.note_negotiation(&pid("a"));
        })),
        ("six negotiations this round", Box::new(|s| {
            for _ in 0..6 {
                s.note_negotiation(&pid("a"));
            }
            s.negotiations_this_action.clear();
        })),
    ];
    for far in [&[][..], &["c", "d"][..], &["b", "c", "d"][..]] {
        for (label, edit) in &positions {
            for proposer in ["a", "b", "c"] {
                let mut t = table(&seats, far);
                edit(&mut t.state);
                let mut cx = Ctx::of(&t.state).expect("owners");
                let game = super::Game::with_table(t.state.clone(), content, Table::with_default(Box::new(crate::choice::FirstOption)));
                let core = Codec::load(&mut cx, &t.state, Sources { galaxy: Some(&t.hub.galaxy), rng: Some(&game.rng_positions()) }, SliceSet::ALL).expect("loads");
                let kernel = live::kernel::kernel_for(DEFAULT);
                let who = cx.owners.seat(proposer).expect("seated");
                let native: Vec<PlayerId> = ti4_engine_core::rules::trade::open_partners_live(&core, kernel, who).as_slice().iter().map(|s| pid(cx.owners.id(*s).expect("id"))).collect();
                let string = crate::transactions::open_partners_text(&t.state, content, &t.hub.galaxy, &pid(proposer));
                assert_eq!(native, string, "{label}, far {far:?}, proposer {proposer}");
            }
        }
    }
}

/// An Alliance conveys nothing until its owner's commander is unlocked (`promissory::commander_unlocked`): the core reads the leader status
/// of the owner from the position. Mahact's Hubris: nobody gives the Mahact seat an Alliance.
#[test]
fn an_alliance_is_offered_when_its_owners_commander_is_unlocked_and_never_to_mahact_like_the_string() {
    // Locked: no `an:` option; unlocked: the Alliance is sold at its posted price.
    let locked = table(&SOL_HACAN, &[]);
    let (string, native) = both(&locked, "b", "hacan", &["decline"]);
    assert_same(&string, &native);
    assert!(string.questions[0].options.iter().all(|o| !o.id.contains("an:sol")), "{:?}", string.questions[0].ids());
    let mut t = table(&SOL_HACAN, &[]);
    t.state.player_mut(&pid("a")).unwrap().leaders.insert(ti4_model::id::LeaderId::new("solcommander"), ti4_model::state::LeaderStatus::Unlocked);
    let (string, native) = both(&t, "b", "hacan", &["pnan:sol:0", "accept"]);
    assert_same(&string, &native);
    assert!(has(&string, "TRANSACTION_RESOLVED"), "{:?}", string.events);
    // Mahact as the partner: the Alliance is not among the deals.
    let mut t = table(&[("a", "sol"), ("b", "mahact")], &[]);
    t.state.player_mut(&pid("a")).unwrap().leaders.insert(ti4_model::id::LeaderId::new("solcommander"), ti4_model::state::LeaderStatus::Unlocked);
    let (string, native) = both(&t, "b", "mahact", &["decline"]);
    assert_same(&string, &native);
    assert!(string.questions[0].options.iter().all(|o| !o.id.contains("an:sol")), "{:?}", string.questions[0].ids());
}

/// Temporal Command Suite: a Nomad (seat `a`, strangers with `b` by the map) hears an agent of `b` become exhausted (the event is staged, the
/// step's reconcile announces it), readies it and deals with `b` through the questions of the effect, with the reach the card grants.
#[test]
fn temporal_command_suite_readies_the_agent_and_runs_the_transaction_like_the_string() {
    for (script, resolved) in [
        (&["technology:nomad:tcs:AGENT_EXHAUSTED:after", "transact", "2:1", "accept"][..], true),
        (&["technology:nomad:tcs:AGENT_EXHAUSTED:after", "transact", "2:1", "refuse"][..], false),
        (&["technology:nomad:tcs:AGENT_EXHAUSTED:after", "decline"][..], false),
        (&["decline"][..], false),
    ] {
        let mut t = table(&[("a", "nomad"), ("b", "arborec")], &["a", "b"]);
        t.state.player_mut(&pid("a")).unwrap().technologies.insert(ti4_model::id::TechnologyId::new("tcs"));
        t.state.player_mut(&pid("b")).unwrap().leaders.insert(ti4_model::id::LeaderId::new("arborecagent"), ti4_model::state::LeaderStatus::Exhausted);
        let probe = super::Game::with_table(t.state.clone(), ContentStore::embedded(), Table::with_default(Box::new(crate::choice::FirstOption))).with_galaxy(t.hub.galaxy.clone());
        let start = probe.state.clone();
        let mut staged = start.clone();
        let payload = std::collections::BTreeMap::from([("player".to_owned(), serde_json::Value::from("b")), ("leader".to_owned(), serde_json::Value::from("arborecagent"))]);
        assert!(crate::supply::stage_event(&mut staged, "AGENT_EXHAUSTED", &payload), "a module is seated: staging is on");
        let seq = staged.turn_seq;
        let mut string = super::turn_live_tests::string_run(&staged, script, |g| {
            g.galaxy = Some(t.hub.galaxy.clone());
            g.prepared_turn_seq = Some(seq);
            let _ = g.step();
        });
        // The step goes on into the action menu, which the script does not answer: that question is not the island's.
        string.questions.retain(|q| q.prompt != "choose an action" && !q.options.iter().any(|o| o.id.starts_with("component|")));
        // Native: the position without the staged mark, the event staged in the core, and the reconcile that announces it.
        let mut cx = Ctx::of(&start).expect("owners");
        let render = Render::new(cx.owners.ids().to_vec());
        let game = super::Game::with_table(start.clone(), ContentStore::embedded(), Table::with_default(Box::new(crate::choice::FirstOption)));
        let mut core = Codec::load(&mut cx, &start, Sources { galaxy: Some(&t.hub.galaxy), rng: Some(&game.rng_positions()) }, SliceSet::ALL).expect("loads");
        let leader = <ti4_engine_core::generated::Leader as crate::generated::IdStr>::from_id("arborecagent").expect("a leader");
        let seat_b = cx.owners.seat("b").expect("seated");
        ti4_engine_core::kernel::fx::stage(&mut core, ti4_engine_core::kernel::fx::event(ti4_engine_core::state::EventKind::AgentExhausted, ti4_engine_core::state::Payload::new().with(ti4_engine_core::state::Key::Player, ti4_engine_core::state::Val::Seat(seat_b)).with(ti4_engine_core::state::Key::Leader, ti4_engine_core::state::Val::Leader(leader))));
        let kernel = live::kernel::kernel_for(DEFAULT);
        let env = live::env::env_with_plays(&render, &core, kernel, Rc::new(TextSeeds::of(&cx)), DEFAULT);
        let lcx = live::Cx::new(&render, &env);
        let mut asker = Scripted::new(script.iter().copied());
        reconcile(&mut core, &lcx, &mut asker);
        assert_eq!(asker.left(), 0, "every answer was taken; asked {:?}", asker.seen.iter().map(|c| (&c.prompt, c.ids())).collect::<Vec<_>>());
        let questions = asker.seen.clone();
        let events = crate::render::events::events_text(&render, &cx, &core).expect("every event has a text");
        let after = materialize::materialize(&core, &cx, &start).expect("materializes");
        let rng = materialize::rng_positions(&core).expect("rng");
        let staged_names = core.pending().staged.iter().map(|e| crate::kernel_sync::text::event_text(&render, e.kind)).collect();
        let (state, _) = without_staged(serde_json::to_value(&after).expect("serializes"));
        let native = Run { questions, events, state, rng, staged: staged_names };
        // The string's reconcile also announces things this scenario does not touch (baselines): compare what the island produced.
        assert_eq!(string.questions.iter().map(|q| (q.prompt.clone(), q.ids().into_iter().map(str::to_owned).collect::<Vec<_>>())).collect::<Vec<_>>(), native.questions.iter().map(|q| (q.prompt.clone(), q.ids().into_iter().map(str::to_owned).collect::<Vec<_>>())).collect::<Vec<_>>(), "{script:?}");
        let island: Vec<&String> = string.events.iter().filter(|e| e.contains("TRANSACTION") || e.contains("AGENT_EXHAUSTED")).collect();
        let theirs: Vec<&String> = native.events.iter().filter(|e| e.contains("TRANSACTION") || e.contains("AGENT_EXHAUSTED")).collect();
        assert_eq!(island, theirs, "{script:?}\nstring {:?}\nnative {:?}", string.events, native.events);
        assert_eq!(string.staged, native.staged, "{script:?}");
        let mut s_state = string.state.clone();
        let mut n_state = native.state.clone();
        for v in [&mut s_state, &mut n_state] {
            if let Some(m) = v.get_mut("faction_marks").and_then(serde_json::Value::as_object_mut) {
                m.retain(|k, _| !k.starts_with("private:#seen"));
            }
        }
        let diffs = crate::dual::diff_json(&s_state["players"], &n_state["players"], 12);
        assert!(diffs.is_empty(), "{script:?}: {diffs:?}");
        assert_eq!(island.iter().any(|e| e.starts_with("TRANSACTION_RESOLVED")), resolved, "{script:?}: {island:?}");
    }
}
