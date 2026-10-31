//! Lane M1: the trade island as a native arm (`driver::windows::trade`, `rules::trade_window::TradeWindow` as a HELD window). The window is
//! rebuilt from its payload and the position at every call; these tests pin what the live loop writes, announces and refuses. The lockstep
//! against the string's `Game::step_trade` is `ti4-engine`'s `trade_live_tests`.

use std::rc::Rc;

use ti4_engine_core::choice::trade::{Ask as TAsk, Opt as TOpt, Shape};
use ti4_engine_core::containers::Cow;
use ti4_engine_core::driver::live::{advance, ask, is_open, Env, Live, LiveError};
use ti4_engine_core::driver::windows::trade::{advance_trade, ask_trade, begin};
use ti4_engine_core::driver::NoSeeds;
use ti4_engine_core::kernel::stage::AbilityTable;
use ti4_engine_core::kernel::Registry;
use ti4_engine_core::rules::trade_window::TradeWindow;
use ti4_engine_core::state::{Code, Emit, EventKind, FreeNote, FreeSpec, FreeTerms, Key, NoteKey, PlainKind, Trade, TransientFlags, Val, Window as OpenWindow, WindowKind};
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::{plain_systems, spiral};

const TRANSACTION: Code = Code(7);
const DARK_PACT: Code = Code(8);

fn env(k: &Kernel) -> Env<'_> {
    let mut table = AbilityTable::default();
    table.codes.turn.transaction = TRANSACTION;
    table.codes.turn.dark_pact = DARK_PACT;
    table.facts.staging = true;
    Env { kernel: k, registry: Cow::new(Registry::default()), table, seeds: Rc::new(NoSeeds) }
}

/// Hacan (seat 0, Arbiters, Guild Ships), Arborec (seat 1) next to it, Yin (seat 2) far away. Seat 0 has 2 goods and 2 commodities, seat 1 one of each.
fn table() -> (CoreState, Kernel) {
    let p = plain_systems();
    let ring = spiral(61);
    let hexes = [ring[0], ring[1], ring[60]];
    Scenario::new(&[Faction::TheEmiratesOfHacan, Faction::TheArborec, Faction::TheYinBrotherhood])
        .map_hexes(&[(p[0], hexes[0]), (p[1], hexes[1]), (p[2], hexes[2])])
        .unit(p[0], None, Unit::CruiserI, 0, 1)
        .unit(p[1], None, Unit::CruiserI, 1, 1)
        .unit(p[2], None, Unit::CruiserI, 2, 1)
        .goods(0, 2)
        .commodities(0, 2)
        .goods(1, 1)
        .commodities(1, 1)
        .goods(2, 3)
        .build()
}

fn goods(s: &CoreState, seat: u8) -> i16 {
    s.players().by_seat(Seat(seat)).unwrap().econ.trade_goods
}

fn deal(shape: Shape) -> Pick {
    Pick::Opt(Opt::Trade(TOpt::Deal(shape)))
}

fn accept() -> Pick {
    Pick::Opt(Opt::Trade(TOpt::Accept))
}

fn plain_kinds(s: &CoreState) -> Vec<PlainKind> {
    s.ledger.events.iter().filter_map(|e| e.plain().map(|p| p.kind)).collect()
}

fn typed_kinds(s: &CoreState) -> Vec<EventKind> {
    s.ledger.events.iter().filter_map(|e| if let Emit::Typed(t) = e { Some(t.kind) } else { None }).collect()
}

fn question(live: Live) -> ti4_engine_core::choice::Choice {
    let Live::Asks(a) = live else { panic!("the window asks") };
    a.choice
}

fn open_with_hacan_and_arborec() -> (CoreState, Kernel) {
    table()
}

#[test]
fn opening_spends_the_pair_notes_the_negotiation_announces_the_opening_and_asks_the_proposer() {
    let (mut s, k) = open_with_hacan_and_arborec();
    let e = env(&k);
    let (live, out) = begin(&mut s, &e, Seat(0), Seat(1)).unwrap();
    let q = question(live);
    assert_eq!((q.actor, q.ask), (Seat(0), Ask::Trade(TAsk::Propose { proposer: Seat(0), partner: Seat(1), name: Faction::TheArborec })));
    assert!(is_open::<TradeWindow>(&s));
    // 94.1 is spent on opening: the pair is recorded for the turn, the negotiation counted.
    assert!(ti4_engine_core::query::trade::transacted_with(&s, Seat(0), Seat(1)) && ti4_engine_core::query::trade::transacted_with(&s, Seat(1), Seat(0)));
    assert_eq!(s.trade().negotiations_this_action.get(&Seat(0)), Some(&1));
    assert_eq!(s.trade().negotiations_this_round.get(&Seat(0)), Some(&1));
    // TRANSACTION_OPENED names both chairs and comes before the first question.
    assert_eq!(typed_kinds(&s), [EventKind::TransactionOpened]);
    assert_eq!(out.emitted[0].payload.get(Key::Player), Some(Val::Seat(Seat(0))));
    assert_eq!(out.emitted[0].payload.get(Key::Partner), Some(Val::Seat(Seat(1))));
    let Some(OpenWindow::Trade(t)) = s.pending().window(WindowKind::Trade) else { panic!("held") };
    assert!(t.announced && t.rounds_left == 2 && t.deal.is_none());
    // A second call asks the same question and announces nothing again.
    let again = ask_trade(&s, &e).unwrap().expect("the question is a pure read");
    assert_eq!(again.choice, q);
    assert_eq!(typed_kinds(&s).len(), 1);
}

#[test]
fn an_accepted_deal_stages_both_gains_emits_the_resolution_and_the_plain_event_and_leaves_the_stack() {
    let (mut s, k) = open_with_hacan_and_arborec();
    let e = env(&k);
    begin(&mut s, &e, Seat(0), Seat(1)).unwrap();
    let (live, out) = advance_trade(&mut s, &e, Some(deal(Shape::Goods { give: 2, want: 1 }))).unwrap();
    let q = question(live);
    assert_eq!((q.actor, q.opts.as_slice().to_vec()), (Seat(1), vec![Opt::Trade(TOpt::Accept), Opt::Trade(TOpt::Refuse), Opt::Trade(TOpt::Counter)]));
    assert!(out.emitted.is_empty(), "an offer emits nothing typed");
    assert_eq!(plain_kinds(&s), [PlainKind::TransactionOffered]);
    let Some(OpenWindow::Trade(t)) = s.pending().window(WindowKind::Trade) else { panic!("held") };
    assert_eq!(t.deal, Some(Shape::Goods { give: 2, want: 1 }));
    let (live, out) = advance_trade(&mut s, &e, Some(accept())).unwrap();
    assert_eq!(live, Live::Done);
    assert!(!is_open::<TradeWindow>(&s));
    assert_eq!((goods(&s, 0), goods(&s, 1)), (1, 2));
    assert_eq!(s.trade().this_round.as_slice(), &[(Seat(0), Seat(1))]);
    // The typed resolution, then the plain event; the two gains are staged (the next reconcile announces them), the partner's first.
    assert_eq!(out.emitted.len(), 1);
    assert_eq!(out.emitted[0].kind, EventKind::TransactionResolved);
    assert_eq!(out.emitted[0].payload.get(Key::Proposer), Some(Val::Seat(Seat(0))));
    assert_eq!(out.emitted[0].payload.get(Key::Partner), Some(Val::Seat(Seat(1))));
    assert_eq!(plain_kinds(&s), [PlainKind::TransactionOffered, PlainKind::Transaction]);
    let staged: Vec<_> = s.pending().staged.iter().map(|r| (r.kind, r.payload.get(Key::Player), r.payload.get(Key::Amount), r.payload.get(Key::Source))).collect();
    assert_eq!(
        staged,
        [
            (EventKind::TradeGoodsGained, Some(Val::Seat(Seat(1))), Some(Val::Int(2)), Some(Val::Code(TRANSACTION))),
            (EventKind::TradeGoodsGained, Some(Val::Seat(Seat(0))), Some(Val::Int(1)), Some(Val::Code(TRANSACTION))),
        ]
    );
}

#[test]
fn refusing_and_declining_say_so_and_move_nothing() {
    let (mut s, k) = open_with_hacan_and_arborec();
    let e = env(&k);
    begin(&mut s, &e, Seat(0), Seat(1)).unwrap();
    advance_trade(&mut s, &e, Some(deal(Shape::Goods { give: 1, want: 1 }))).unwrap();
    let (live, out) = advance_trade(&mut s, &e, Some(Pick::Opt(Opt::Trade(TOpt::Refuse)))).unwrap();
    assert_eq!((live, out.emitted.len()), (Live::Done, 0));
    assert_eq!(plain_kinds(&s), [PlainKind::TransactionOffered, PlainKind::TransactionRefused]);
    assert_eq!((goods(&s, 0), goods(&s, 1)), (2, 1));
    assert!(s.pending().staged.is_empty());
    let (mut s, k) = open_with_hacan_and_arborec();
    let e = env(&k);
    begin(&mut s, &e, Seat(0), Seat(1)).unwrap();
    let (live, _) = advance_trade(&mut s, &e, Some(Pick::Opt(Opt::Decline))).unwrap();
    assert_eq!(live, Live::Done);
    assert_eq!(plain_kinds(&s), [PlainKind::TransactionAbandoned]);
}

#[test]
fn a_counter_swaps_the_chairs_in_the_payload_and_the_second_counter_ends_the_haggling() {
    let (mut s, k) = open_with_hacan_and_arborec();
    let e = env(&k);
    begin(&mut s, &e, Seat(0), Seat(1)).unwrap();
    advance_trade(&mut s, &e, Some(deal(Shape::Goods { give: 1, want: 1 }))).unwrap();
    let q = question(advance_trade(&mut s, &e, Some(Pick::Opt(Opt::Trade(TOpt::Counter)))).unwrap().0);
    assert_eq!((q.actor, q.ask), (Seat(1), Ask::Trade(TAsk::Propose { proposer: Seat(1), partner: Seat(0), name: Faction::TheEmiratesOfHacan })));
    let Some(OpenWindow::Trade(t)) = s.pending().window(WindowKind::Trade) else { panic!("held") };
    assert_eq!((t.proposer, t.partner, t.rounds_left, t.deal), (Seat(1), Seat(0), 1, None));
    advance_trade(&mut s, &e, Some(deal(Shape::Goods { give: 1, want: 0 }))).unwrap();
    let (live, _) = advance_trade(&mut s, &e, Some(Pick::Opt(Opt::Trade(TOpt::Counter)))).unwrap();
    assert_eq!(live, Live::Done);
    assert_eq!(plain_kinds(&s), [PlainKind::TransactionOffered, PlainKind::Counteroffer, PlainKind::TransactionOffered, PlainKind::TransactionAbandoned]);
    assert_eq!((goods(&s, 0), goods(&s, 1)), (2, 1));
}

#[test]
fn a_pair_that_cannot_reach_each_other_has_its_deal_rejected_unless_a_ship_ability_lets_it() {
    // Arborec (seat 1) and Yin (seat 2) are strangers: Arborec's deal is refused before the partner is troubled with it.
    let (mut s, k) = table();
    let e = env(&k);
    begin(&mut s, &e, Seat(1), Seat(2)).unwrap();
    let (live, _) = advance_trade(&mut s, &e, Some(deal(Shape::Goods { give: 1, want: 1 }))).unwrap();
    assert_eq!(live, Live::Done);
    assert_eq!(plain_kinds(&s), [PlainKind::TransactionRejected]);
    // Hacan's Guild Ships reach anyone: the same deal is put to the far Yin.
    let (mut s, k) = table();
    let e = env(&k);
    begin(&mut s, &e, Seat(0), Seat(2)).unwrap();
    let (live, _) = advance_trade(&mut s, &e, Some(deal(Shape::Goods { give: 1, want: 1 }))).unwrap();
    assert_eq!(question(live).actor, Seat(2));
    assert_eq!(plain_kinds(&s), [PlainKind::TransactionOffered]);
}

#[test]
fn a_deal_that_is_no_option_changes_nothing_and_the_pair_stays_recorded() {
    let (mut s, k) = open_with_hacan_and_arborec();
    let e = env(&k);
    begin(&mut s, &e, Seat(0), Seat(1)).unwrap();
    let held = s.clone();
    assert_eq!(advance_trade(&mut s, &e, Some(deal(Shape::Goods { give: 3, want: 3 }))), Err(LiveError::Illegal));
    assert_eq!(s, held, "the rollback restores the position");
    assert!(ti4_engine_core::query::trade::transacted_with(&s, Seat(0), Seat(1)), "94.1 was spent on opening, not on the first answer");
    assert_eq!(advance_trade(&mut s, &e, Some(accept())), Err(LiveError::Illegal), "an answer before the proposal");
    assert_eq!(s, held);
}

#[test]
fn a_clone_in_the_middle_of_the_window_continues_identically() {
    let (mut s, k) = open_with_hacan_and_arborec();
    let e = env(&k);
    begin(&mut s, &e, Seat(0), Seat(1)).unwrap();
    advance_trade(&mut s, &e, Some(deal(Shape::Swap { n: 1 }))).unwrap();
    let mut fork = s.clone();
    let mut other_env = env(&k);
    other_env.registry = Cow::new(Registry::default());
    for (state, env) in [(&mut s, &e), (&mut fork, &other_env)] {
        advance_trade(state, env, Some(Pick::Opt(Opt::Trade(TOpt::Counter)))).unwrap();
        advance_trade(state, env, Some(deal(Shape::GiftCommodities { n: 1 }))).unwrap();
        advance_trade(state, env, Some(accept())).unwrap();
    }
    assert_eq!(s, fork);
    assert!(!is_open::<TradeWindow>(&s));
}

#[test]
fn a_window_with_no_deal_to_propose_is_dropped_silently_and_keeps_the_black_market_mark() {
    // Seat 2 has three trade goods and nothing else; the partner (seat 1) has goods and commodities, so there are deals. Take everything away.
    let (mut s, k) = table();
    for seat in [0, 1] {
        let p = s.players_mut().by_seat_mut(Seat(seat)).unwrap();
        p.econ.trade_goods = 0;
        p.econ.commodities = 0;
    }
    // Both Supports for the Throne are lent out: no swap either.
    s.trade_mut().support_holders.insert(Seat(0), Seat(2));
    s.trade_mut().support_holders.insert(Seat(1), Seat(2));
    s.transient_mut().misc.flags.set(TransientFlags::BLACK_MARKET);
    let e = env(&k);
    let (live, _) = begin(&mut s, &e, Seat(0), Seat(1)).unwrap();
    assert_eq!(live, Live::Done, "the string drops a window that has nothing to propose");
    assert!(plain_kinds(&s).is_empty());
    assert_eq!(typed_kinds(&s), [EventKind::TransactionOpened], "the opening was announced");
    assert!(s.transient().misc.flags.has(TransientFlags::BLACK_MARKET), "the mark is cleared after an answer only");
}

#[test]
fn the_black_market_mark_dies_with_the_negotiation_that_an_answer_ended() {
    let (mut s, k) = open_with_hacan_and_arborec();
    s.transient_mut().misc.flags.set(TransientFlags::BLACK_MARKET);
    let e = env(&k);
    begin(&mut s, &e, Seat(0), Seat(1)).unwrap();
    assert!(s.transient().misc.flags.has(TransientFlags::BLACK_MARKET), "it lives while the negotiation does");
    advance_trade(&mut s, &e, Some(Pick::Opt(Opt::Decline))).unwrap();
    assert!(!s.transient().misc.flags.has(TransientFlags::BLACK_MARKET));
}

#[test]
fn a_position_without_a_map_drops_the_window_without_a_word() {
    let (mut s, k) = table();
    s.map = ti4_engine_core::state::MapSlice::default();
    let e = env(&k);
    let (live, _) = begin(&mut s, &e, Seat(0), Seat(1)).unwrap();
    assert_eq!(live, Live::Done);
    assert!(plain_kinds(&s).is_empty() && !is_open::<TradeWindow>(&s));
}

fn free(given: FreeTerms, received: FreeTerms) -> FreeSpec {
    FreeSpec { given, received }
}

#[test]
fn a_free_form_deal_the_proposer_names_is_taken_from_slot_one_priced_and_executed() {
    let (mut s, k) = open_with_hacan_and_arborec();
    let e = env(&k);
    begin(&mut s, &e, Seat(0), Seat(1)).unwrap();
    // The live loop writes the terms of the named deal to slot 1 before the pick that names it. Seat 0 gives one trade good and one
    // commodity for one commodity of seat 1's.
    let spec = free(FreeTerms { trade_goods: 1, commodities: 1, ..FreeTerms::default() }, FreeTerms { commodities: 1, ..FreeTerms::default() });
    let Some(OpenWindow::Trade(t)) = s.pending_mut().window_mut(WindowKind::Trade) else { panic!("held") };
    t.free[1] = Some(spec);
    let q = question(advance_trade(&mut s, &e, Some(deal(Shape::Free(1)))).unwrap().0);
    assert_eq!((q.actor, q.ask), (Seat(1), Ask::Trade(TAsk::Answer { responder: Seat(1), proposer: Seat(0), from: Faction::TheEmiratesOfHacan, deal: Shape::Free(0) })));
    let Some(OpenWindow::Trade(t)) = s.pending().window(WindowKind::Trade) else { panic!("held") };
    assert_eq!((t.deal, t.free), (Some(Shape::Free(0)), [Some(spec), None]), "the deal on the table is slot 0");
    // A clone answers the same way.
    let mut fork = s.clone();
    for state in [&mut s, &mut fork] {
        assert_eq!(advance_trade(state, &e, Some(accept())).unwrap().0, Live::Done);
    }
    assert_eq!(s, fork);
    assert_eq!((goods(&s, 0), goods(&s, 1)), (2 - 1 + 1, 1 + 1 + 1), "seat 0: -1 good, +1 (the commodity it received); seat 1: +1 good, +1 (the commodity from seat 0)");
    let p0 = s.players().by_seat(Seat(0)).unwrap();
    assert_eq!(p0.econ.commodities, 1);
}

#[test]
fn a_free_form_deal_nobody_can_pay_is_rejected_and_an_unnamed_slot_is_no_option() {
    let (mut s, k) = open_with_hacan_and_arborec();
    let e = env(&k);
    begin(&mut s, &e, Seat(0), Seat(1)).unwrap();
    // Slot 1 is empty: naming it is no option.
    let held = s.clone();
    assert_eq!(advance_trade(&mut s, &e, Some(deal(Shape::Free(1)))), Err(LiveError::Illegal));
    assert_eq!(s, held);
    // Seat 0 offers more trade goods than it has.
    let spec = free(FreeTerms { trade_goods: 5, ..FreeTerms::default() }, FreeTerms::default());
    let Some(OpenWindow::Trade(t)) = s.pending_mut().window_mut(WindowKind::Trade) else { panic!("held") };
    t.free[1] = Some(spec);
    let (live, _) = advance_trade(&mut s, &e, Some(deal(Shape::Free(1)))).unwrap();
    assert_eq!(live, Live::Done);
    assert_eq!(plain_kinds(&s), [PlainKind::TransactionRejected]);
}

#[test]
fn a_free_form_note_leg_is_read_against_the_position_every_time() {
    // The proposer gives a Ceasefire of its own faction (held in hand), the partner gives a trade good.
    let (mut s, k) = open_with_hacan_and_arborec();
    let e = env(&k);
    let key = NoteKey { note: PromissoryNote::Ceasefire, faction: Faction::TheEmiratesOfHacan };
    s.cards_mut().promissory.edit().insert(key, Seat(0));
    begin(&mut s, &e, Seat(0), Seat(1)).unwrap();
    let spec = free(FreeTerms { note: Some(FreeNote::Held(key)), ..FreeTerms::default() }, FreeTerms { trade_goods: 1, ..FreeTerms::default() });
    let Some(OpenWindow::Trade(t)) = s.pending_mut().window_mut(WindowKind::Trade) else { panic!("held") };
    t.free[1] = Some(spec);
    advance_trade(&mut s, &e, Some(deal(Shape::Free(1)))).unwrap();
    // The facts follow the position: the note is lent away behind the window's back, and the acceptance is rejected (the string recomputes).
    s.cards_mut().promissory.edit().insert(key, Seat(2));
    let (live, _) = advance_trade(&mut s, &e, Some(accept())).unwrap();
    assert_eq!(live, Live::Done);
    assert_eq!(plain_kinds(&s), [PlainKind::TransactionOffered, PlainKind::TransactionRejected]);
    assert_eq!((goods(&s, 0), goods(&s, 1)), (2, 1));
}

#[test]
fn dark_pact_pays_both_sides_a_good_when_the_maximum_commodity_value_is_given() {
    // Empyrean is not seated in `table()`: seat the Dark Pact of Hacan's holder-giver instead: seat 1 holds Hacan's Dark Pact?
    // Dark Pact is the Empyrean's note: use an Empyrean at seat 1.
    let p = plain_systems();
    let (mut s, k) = Scenario::new(&[Faction::TheArborec, Faction::TheEmpyrean])
        .map_hexes(&[(p[0], spiral(2)[0]), (p[1], spiral(2)[1])])
        .unit(p[0], None, Unit::CruiserI, 0, 1)
        .unit(p[1], None, Unit::CruiserI, 1, 1)
        .commodities(0, 3)
        .build();
    let max = i16::from(Faction::TheArborec.data().commodities);
    s.players_mut().by_seat_mut(Seat(0)).unwrap().econ.commodities = max;
    let key = NoteKey { note: PromissoryNote::DarkPact, faction: Faction::TheEmpyrean };
    s.cards_mut().promissory.edit().insert(key, Seat(0));
    s.cards_mut().promissory_faceup.edit().insert(key, ());
    let e = env(&k);
    begin(&mut s, &e, Seat(0), Seat(1)).unwrap();
    let spec = free(FreeTerms { commodities: i32::from(max), ..FreeTerms::default() }, FreeTerms::default());
    let Some(OpenWindow::Trade(t)) = s.pending_mut().window_mut(WindowKind::Trade) else { panic!("held") };
    t.free[1] = Some(spec);
    advance_trade(&mut s, &e, Some(deal(Shape::Free(1)))).unwrap();
    advance_trade(&mut s, &e, Some(accept())).unwrap();
    let staged: Vec<_> = s.pending().staged.iter().map(|r| (r.payload.get(Key::Player), r.payload.get(Key::Amount), r.payload.get(Key::Source))).collect();
    assert_eq!(
        staged,
        [
            (Some(Val::Seat(Seat(1))), Some(Val::Int(i64::from(max))), Some(Val::Code(TRANSACTION))),
            (Some(Val::Seat(Seat(0))), Some(Val::Int(1)), Some(Val::Code(DARK_PACT))),
            (Some(Val::Seat(Seat(1))), Some(Val::Int(1)), Some(Val::Code(DARK_PACT))),
        ],
        "the deal's gain, then the giver's and the Empyrean's Dark Pact goods"
    );
    assert_eq!(goods(&s, 0), 1);
    assert_eq!(goods(&s, 1), i16::from(max) + 1);
}

#[test]
fn the_question_is_a_pure_read_and_a_payload_roundtrips_through_the_machine() {
    let (mut s, k) = open_with_hacan_and_arborec();
    let e = env(&k);
    begin(&mut s, &e, Seat(0), Seat(1)).unwrap();
    advance_trade(&mut s, &e, Some(deal(Shape::Goods { give: 1, want: 1 }))).unwrap();
    let before = s.clone();
    let a = ask::<TradeWindow>(&s, &e).unwrap().unwrap();
    assert_eq!(s, before, "asking writes nothing");
    let Some(OpenWindow::Trade(t)) = s.pending().window(WindowKind::Trade) else { panic!("held") };
    use ti4_engine_core::kernel::stage::Held;
    let m = TradeWindow::from_pending(&s, &k).unwrap();
    let mut again = s.clone();
    m.to_pending(&mut again);
    let Some(OpenWindow::Trade(back)) = again.pending().window(WindowKind::Trade) else { panic!("held") };
    assert_eq!(back, t, "from_pending then to_pending is the identity on the payload");
    assert_eq!(a.choice.actor, Seat(1));
    let _ = Trade::opened(Seat(0), Seat(1));
    let _ = advance::<TradeWindow>;
}
