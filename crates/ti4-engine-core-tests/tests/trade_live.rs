//! Lane M1: the trade island as a native arm (`driver::windows::trade`, `rules::trade_window::TradeWindow` as a HELD window). The window is
//! rebuilt from its payload and the position at every call; these tests pin what the live loop writes, announces and refuses. The lockstep
//! against the string's `Game::step_trade` is `ti4-engine`'s `trade_live_tests`.


use ti4_engine_core::choice::trade::{Ask as TAsk, Opt as TOpt, Shape};
use ti4_engine_core::containers::Cow;
use ti4_engine_core::driver::live::{advance, ask, is_open, Env, Live, LiveError};
use ti4_engine_core::driver::windows::trade::{advance_trade, ask_trade, begin};
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
    Env { kernel: k, registry: Cow::new(Registry::default()), table }
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

#[test]
fn the_gains_are_staged_only_while_staging_is_on_and_the_goods_move_either_way() {
    let (mut s, k) = table();
    let mut e = env(&k);
    e.table.facts.staging = false;
    begin(&mut s, &e, Seat(0), Seat(1)).unwrap();
    advance_trade(&mut s, &e, Some(deal(Shape::Goods { give: 2, want: 1 }))).unwrap();
    advance_trade(&mut s, &e, Some(accept())).unwrap();
    assert_eq!((goods(&s, 0), goods(&s, 1)), (1, 2), "the deal is struck");
    assert!(s.pending().staged.is_empty(), "no seated faction has a module: the string stages nothing");
    assert_eq!(plain_kinds(&s), [PlainKind::TransactionOffered, PlainKind::Transaction]);
}

/// The reaction window of TRANSACTION_RESOLVED is offered to every seat in the initiative order of the position, not only to the active seat
/// (the live resolver carried an empty initiative order: only seat 0 was asked).
#[test]
fn the_resolution_opens_its_reaction_window_to_a_seat_that_is_not_the_active_one() {
    use ti4_engine_core::choice::reaction::{AbilityId, AbilityKind, Ask as RAsk, Opt as ROpt, Relation};
    use ti4_engine_core::kernel::resolver::AbilityDef;
    use ti4_engine_core::kernel::stage::{Slot, Spec};
    let (mut s, k) = table();
    let mut e = env(&k);
    let id = AbilityId { kind: AbilityKind::Ability, key: Code(1) };
    let mut resolver = ti4_engine_core::kernel::Resolver::new(&[Seat(0), Seat(1), Seat(2)], Some(Seat(0)));
    resolver.register([AbilityDef::new(id, Seat(1), EventKind::TransactionResolved, Relation::After).with_optional(true).with_arg(1)]);
    e.registry = Cow::new(resolver.registry().clone());
    e.table.add(Spec::new(Slot(1), AbilityKind::Ability, EventKind::TransactionResolved, Relation::After, |eff| {
        ti4_engine_core::kernel::fx::gain_trade_goods(eff.state(), Seat(1), 5);
        Ok(())
    })
    .optional());
    begin(&mut s, &e, Seat(0), Seat(1)).unwrap();
    advance_trade(&mut s, &e, Some(deal(Shape::Goods { give: 1, want: 1 }))).unwrap();
    let (live, _) = advance_trade(&mut s, &e, Some(accept())).unwrap();
    let q = question(live);
    assert_eq!((q.actor, q.ask), (Seat(1), Ask::Reaction(RAsk::Window { event: EventKind::TransactionResolved, relation: Relation::After })));
    assert!(is_open::<TradeWindow>(&s), "the window waits on the reaction question, suspended in the position");
    let (live, _) = advance_trade(&mut s, &e, Some(Pick::Opt(Opt::Reaction(ROpt::Ability(id))))).unwrap();
    assert_eq!(live, Live::Done);
    assert_eq!(goods(&s, 1), 1 + 5, "one good given, one received, and the ability ran once");
}

#[test]
fn the_menu_partners_and_the_neighbours_who_traded_are_read_from_the_position() {
    use ti4_engine_core::kernel::trade_env::{lie_in_wait_traders, open_partners_live};
    let (mut s, k) = table();
    // Seats 0 and 1 are neighbours; seat 2 is a stranger to both, but Hacan's Guild Ships reach it.
    assert_eq!(open_partners_live(&s, &k, Seat(0)).as_slice(), &[Seat(1), Seat(2)]);
    assert_eq!(open_partners_live(&s, &k, Seat(1)).as_slice(), &[Seat(0)], "Arborec reaches its neighbour only; Yin is a stranger to it");
    assert!(open_partners_live(&s, &k, Seat(2)).as_slice() == [Seat(0)], "the stranger reaches only the seat that reaches it");
    // The pair spent for the turn is not offered again; the negotiation limit of the action closes the menu.
    ti4_engine_core::rules::trade::record_transaction(&mut s, Seat(0), Seat(1));
    assert_eq!(open_partners_live(&s, &k, Seat(0)).as_slice(), &[Seat(2)]);
    ti4_engine_core::rules::trade::note_negotiation(&mut s, Seat(0));
    ti4_engine_core::rules::trade::note_negotiation(&mut s, Seat(0));
    assert!(open_partners_live(&s, &k, Seat(0)).is_empty());
    // Lie in Wait: nobody traded yet; then the pair did, and the holder's neighbours who traded come in the order of the transactions.
    assert_eq!(lie_in_wait_traders(&s, &k, Seat(1)), [None, None]);
    ti4_engine_core::rules::trade::record_resolved(&mut s, Seat(1), Seat(0));
    assert_eq!(lie_in_wait_traders(&s, &k, Seat(2)), [None, None], "a stranger has no neighbours");
    ti4_engine_core::rules::trade::record_resolved(&mut s, Seat(0), Seat(1));
    assert_eq!(lie_in_wait_traders(&s, &k, Seat(0)), [Some(Seat(1)), None], "a neighbour who traded twice counts once");
    // A position without a map has no neighbours.
    s.map = ti4_engine_core::state::MapSlice::default();
    assert!(open_partners_live(&s, &k, Seat(0)).is_empty());
    assert_eq!(lie_in_wait_traders(&s, &k, Seat(0)), [None, None]);
}

#[test]
fn a_module_that_exempts_the_pair_from_the_limit_leaves_it_unrecorded() {
    use ti4_engine_core::kernel::HookTable;
    let (mut s, mut k) = table();
    let mut hooks: HookTable = k.hooks.clone();
    hooks.cards.transaction_limit_exempt.push(|_, _, _, _| true);
    k.hooks = hooks;
    let e = env(&k);
    begin(&mut s, &e, Seat(0), Seat(1)).unwrap();
    assert!(!ti4_engine_core::query::trade::transacted_with(&s, Seat(0), Seat(1)), "an exempt pair is not recorded");
    assert_eq!(s.trade().negotiations_this_action.get(&Seat(0)), Some(&1), "the negotiation is counted all the same");
}

#[test]
fn what_a_window_accepts_unlisted_is_only_a_named_free_form_deal_at_the_proposal() {
    use ti4_engine_core::query::trade_deals::{DealFacts, FreeDeal};
    use ti4_engine_core::query::trade::Terms;
    let facts = |named: bool| DealFacts {
        note_order: |a, b| a.cmp(&b),
        card_order: |a, b| a.cmp(&b),
        unlocked: 0,
        pair: (Seat(0), Seat(1)),
        may: [true, true],
        arbiters: [false, false],
        planet_values: &[],
        free: [named.then_some(FreeDeal { given: Terms::default(), received: Terms::default() }), None],
    };
    let named = Pick::Opt(Opt::Trade(TOpt::Deal(Shape::Free(0))));
    assert!(TradeWindow::open(Seat(0), Seat(1), facts(true)).accepts_unlisted(&named));
    assert!(!TradeWindow::open(Seat(0), Seat(1), facts(false)).accepts_unlisted(&named), "no deal in that slot");
    assert!(!TradeWindow::open(Seat(0), Seat(1), facts(true)).accepts_unlisted(&Pick::Opt(Opt::Decline)));
    assert!(!TradeWindow::open(Seat(0), Seat(1), facts(true)).accepts_unlisted(&Pick::Opt(Opt::Trade(TOpt::Deal(Shape::SwapOne)))));
    assert!(!TradeWindow::at(Seat(0), Seat(1), Some(Shape::SwapOne), 2, facts(true)).accepts_unlisted(&named), "the partner's question takes no named deal");
}

#[test]
fn a_window_that_has_not_announced_its_opening_or_has_no_map_asks_nothing() {
    let (mut s, k) = table();
    let e = env(&k);
    s.pending_mut().open(OpenWindow::Trade(Trade::opened(Seat(0), Seat(1))));
    assert_eq!(ask_trade(&s, &e).unwrap(), None, "the first call announces before it asks");
    s.pending_mut().open(OpenWindow::Trade(Trade { announced: true, ..Trade::opened(Seat(0), Seat(1)) }));
    assert!(ask_trade(&s, &e).unwrap().is_some());
    s.map = ti4_engine_core::state::MapSlice::default();
    assert_eq!(ask_trade(&s, &e).unwrap(), None, "without a map the string drops the window");
}

#[test]
fn the_question_of_a_proposal_is_mandatory_and_not_a_space_battle() {
    let (mut s, k) = table();
    let e = env(&k);
    let q = question(begin(&mut s, &e, Seat(0), Seat(1)).unwrap().0);
    assert!(!q.ctx.optional && !q.ctx.space_battle && q.ctx.invasion_seq.is_none());
    let q = question(advance_trade(&mut s, &e, Some(deal(Shape::Goods { give: 1, want: 1 }))).unwrap().0);
    assert!(!q.ctx.optional && !q.ctx.space_battle);
}

#[test]
fn both_gains_of_a_free_form_deal_count_goods_and_commodities_together() {
    let (mut s, k) = table();
    let e = env(&k);
    begin(&mut s, &e, Seat(0), Seat(1)).unwrap();
    // Seat 1 gives one good and one commodity for one good: seat 0 receives 2, seat 1 receives 1.
    let spec = free(FreeTerms { trade_goods: 1, ..FreeTerms::default() }, FreeTerms { trade_goods: 1, commodities: 1, ..FreeTerms::default() });
    let Some(OpenWindow::Trade(t)) = s.pending_mut().window_mut(WindowKind::Trade) else { panic!("held") };
    t.free[1] = Some(spec);
    advance_trade(&mut s, &e, Some(deal(Shape::Free(1)))).unwrap();
    advance_trade(&mut s, &e, Some(accept())).unwrap();
    let amounts: Vec<_> = s.pending().staged.iter().map(|r| (r.payload.get(Key::Player), r.payload.get(Key::Amount))).collect();
    assert_eq!(amounts, [(Some(Val::Seat(Seat(1))), Some(Val::Int(1))), (Some(Val::Seat(Seat(0))), Some(Val::Int(2)))]);
}

#[test]
fn dark_pact_needs_the_faceup_note_in_the_givers_hand_and_exactly_the_maximum_value() {
    let p = plain_systems();
    let ring = spiral(2);
    let build = || {
        let (mut s, k) = Scenario::new(&[Faction::TheArborec, Faction::TheEmpyrean]).map_hexes(&[(p[0], ring[0]), (p[1], ring[1])]).unit(p[0], None, Unit::CruiserI, 0, 1).unit(p[1], None, Unit::CruiserI, 1, 1).build();
        let max = i16::from(Faction::TheArborec.data().commodities);
        s.players_mut().by_seat_mut(Seat(0)).unwrap().econ.commodities = max + 1;
        let key = NoteKey { note: PromissoryNote::DarkPact, faction: Faction::TheEmpyrean };
        s.cards_mut().promissory.edit().insert(key, Seat(0));
        s.cards_mut().promissory_faceup.edit().insert(key, ());
        (s, k, max, key)
    };
    let run = |s: &mut CoreState, k: &Kernel, give: i32, staging: bool| {
        let mut e = env(k);
        e.table.facts.staging = staging;
        begin(s, &e, Seat(0), Seat(1)).unwrap();
        let spec = free(FreeTerms { commodities: give, ..FreeTerms::default() }, FreeTerms::default());
        let Some(OpenWindow::Trade(t)) = s.pending_mut().window_mut(WindowKind::Trade) else { panic!("held") };
        t.free[1] = Some(spec);
        advance_trade(s, &e, Some(deal(Shape::Free(1)))).unwrap();
        advance_trade(s, &e, Some(accept())).unwrap();
    };
    let dark = |s: &CoreState| s.pending().staged.iter().filter(|r| r.payload.get(Key::Source) == Some(Val::Code(DARK_PACT))).count();
    // The exact value, staging off: the goods move all the same (one each), nothing is staged.
    let (mut s, k, max, _) = build();
    run(&mut s, &k, i32::from(max), false);
    assert_eq!((goods(&s, 0), goods(&s, 1)), (1, i16::from(max) + 1));
    assert!(s.pending().staged.is_empty());
    // One short, one over, zero: no Dark Pact.
    for give in [i32::from(max) - 1, i32::from(max) + 1] {
        let (mut s, k, _, _) = build();
        run(&mut s, &k, give, true);
        assert_eq!(dark(&s), 0, "{give}");
    }
    // The note not faceup, or held by the other seat: no Dark Pact.
    let (mut s, k, max, key) = build();
    s.cards_mut().promissory_faceup.edit().remove(&key);
    run(&mut s, &k, i32::from(max), true);
    assert_eq!(dark(&s), 0);
    let (mut s, k, max, key) = build();
    s.cards_mut().promissory.edit().insert(key, Seat(1));
    run(&mut s, &k, i32::from(max), true);
    assert_eq!(dark(&s), 0);
}

#[test]
fn the_reach_the_exemption_the_arbiters_and_the_black_market_come_from_the_hooks_and_the_position() {
    use ti4_engine_core::kernel::trade_env::KernelEnv;
    use ti4_engine_core::query::trade::Env;
    let (mut s, mut k) = table();
    let env_of = |s: &CoreState, k: &Kernel| {
        // Arborec (1) has no Arbiters or Guild Ships of its own.
        (KernelEnv { core: s, kernel: k }.transaction_reach(Seat(1), Seat(2)), KernelEnv { core: s, kernel: k }.exempt_from_limit(Seat(1), Seat(2)), KernelEnv { core: s, kernel: k }.trades_action_cards(Seat(1)), KernelEnv { core: s, kernel: k }.black_market(), KernelEnv { core: s, kernel: k }.trades_action_cards(Seat(0)), KernelEnv { core: s, kernel: k }.wide(Seat(0)), KernelEnv { core: s, kernel: k }.wide(Seat(1)))
    };
    assert_eq!(env_of(&s, &k), (false, false, false, false, true, true, false));
    k.hooks.cards.transaction_reach.push(|_, _, _, _| true);
    k.hooks.cards.transaction_limit_exempt.push(|_, _, _, _| true);
    k.hooks.general.trades_action_cards.push(|_, _, _| true);
    k.hooks.general.ignores_neighbours.push(|_, _, who| who == Seat(1));
    s.transient_mut().misc.flags.set(TransientFlags::BLACK_MARKET);
    assert_eq!(env_of(&s, &k), (true, true, true, true, true, true, true));
    // A faceup Trade Convoys reaches anyone.
    let (mut s, k) = table();
    let key = NoteKey { note: PromissoryNote::TradeConvoys, faction: Faction::TheArborec };
    s.cards_mut().promissory.edit().insert(key, Seat(1));
    s.cards_mut().promissory_faceup.edit().insert(key, ());
    assert!(KernelEnv { core: &s, kernel: &k }.wide(Seat(1)));
}

#[test]
fn the_free_form_facts_and_prices_come_from_the_position() {
    use ti4_engine_core::query::trade_deals::{commander_unlocked_mask, free_nets, planet_value, planet_values};
    let (mut s, _) = table();
    assert_eq!(commander_unlocked_mask(&s), 0);
    let commander = Leader::ALL.iter().copied().find(|l| l.data().kind == LeaderType::Commander).expect("a commander");
    let agent = Leader::ALL.iter().copied().find(|l| l.data().kind == LeaderType::Agent).expect("an agent");
    s.players_mut().by_seat_mut(Seat(0)).unwrap().leaders.status.insert(commander, ti4_engine_core::state::LeaderStatus::Unlocked);
    s.players_mut().by_seat_mut(Seat(1)).unwrap().leaders.status.insert(agent, ti4_engine_core::state::LeaderStatus::Unlocked);
    s.players_mut().by_seat_mut(Seat(2)).unwrap().leaders.status.insert(commander, ti4_engine_core::state::LeaderStatus::Unlocked);
    s.players_mut().by_seat_mut(Seat(1)).unwrap().leaders.status.insert(commander, ti4_engine_core::state::LeaderStatus::Locked);
    assert_eq!(commander_unlocked_mask(&s), 0b101, "an unlocked commander of seats 0 and 2 (an unlocked agent and a locked commander of seat 1 do not count)");
    // Resources plus influence, at least one.
    for p in Planet::ALL {
        assert_eq!(planet_value(p), (p.data().resources + p.data().influence).max(1));
    }
    assert!(Planet::ALL.iter().any(|p| p.data().resources + p.data().influence == 0) && planet_values().iter().all(|v| *v >= 1));
    // goods: 1 + 0.2 * 3 commodities cost against 2 + 1 worth.
    let spec = free(FreeTerms { trade_goods: 1, commodities: 3, ..FreeTerms::default() }, FreeTerms { trade_goods: 2, fragments: [1, 0, 0, 0], ..FreeTerms::default() });
    let (net, theirs) = free_nets(&spec);
    assert_eq!(net, (2.0 + 1.0) - (1.0 + 0.2 * 3.0));
    assert_eq!(theirs, (1.0 + 3.0) - (2.0 + 1.0));
}

#[test]
fn a_support_leg_of_a_free_form_deal_is_payable_only_while_the_givers_support_is_not_lent() {
    let (mut s, k) = table();
    let e = env(&k);
    begin(&mut s, &e, Seat(0), Seat(1)).unwrap();
    let spec = free(FreeTerms { note: Some(FreeNote::Support(Faction::TheEmiratesOfHacan)), ..FreeTerms::default() }, FreeTerms { trade_goods: 1, ..FreeTerms::default() });
    let Some(OpenWindow::Trade(t)) = s.pending_mut().window_mut(WindowKind::Trade) else { panic!("held") };
    t.free[1] = Some(spec);
    let mut lent = s.clone();
    lent.trade_mut().support_holders.insert(Seat(0), Seat(2));
    assert_eq!(advance_trade(&mut lent, &e, Some(deal(Shape::Free(1)))).unwrap().0, Live::Done, "a lent Support cannot be given");
    assert_eq!(plain_kinds(&lent), [PlainKind::TransactionRejected]);
    let (live, _) = advance_trade(&mut s, &e, Some(deal(Shape::Free(1)))).unwrap();
    assert_eq!(question(live).actor, Seat(1));
}

/// A window of the dual (`open`, `at`) and the inline one differ from the live one: they stage, announce and log nothing of their own.
#[test]
fn only_a_live_window_announces_stages_emits_and_logs() {
    use ti4_engine_core::query::trade_deals::DealFacts;
    let facts = || DealFacts { note_order: |a, b| a.cmp(&b), card_order: |a, b| a.cmp(&b), unlocked: 0, pair: (Seat(0), Seat(1)), may: [true, true], arbiters: [false, false], planet_values: &[], free: [None, None] };
    let scenario = || Scenario::new(&[Faction::TheEmiratesOfHacan, Faction::TheArborec]).goods(0, 2).goods(1, 1);
    let windows: [Box<dyn Fn() -> TradeWindow>; 2] = [Box::new(move || TradeWindow::open(Seat(0), Seat(1), facts())), Box::new(move || TradeWindow::at(Seat(0), Seat(1), Some(Shape::Goods { give: 1, want: 1 }), 2, facts()))];
    for make in windows {
        let mut rig = Rig::new(scenario(), make());
        rig.run();
        if rig.opts().contains(&Opt::Decline) {
            rig.pick_opt(Opt::Trade(TOpt::Deal(Shape::Goods { give: 1, want: 1 })));
        }
        rig.pick_opt(Opt::Trade(TOpt::Accept));
        assert!(rig.state.pending().staged.is_empty(), "the string stages the gains of a dual window");
        assert!(rig.state.ledger.events.is_empty(), "no typed event, no plain event, no TRANSACTION_OPENED");
    }
}
