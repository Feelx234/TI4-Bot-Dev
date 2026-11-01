//! Lane B, second batch: the end of an action, the closing menu, the fleet sweep's facts, the gates at the end of the draft, the gains the
//! reconcile announces and the answers the arms refuse. Written against the survivors of `tools/engine_core/mutants.sh` on `driver/turn/*`.

use std::rc::Rc;

use ti4_engine_core::choice::turn::{Ask as TuAsk, Opt as TuOpt};
use ti4_engine_core::containers::Cow;
use ti4_engine_core::driver::live::{Env, Live, LiveError, Out};
use ti4_engine_core::driver::turn::ask::{advance_ask, ask_ask};
use ti4_engine_core::driver::turn::closing;
use ti4_engine_core::driver::turn::facts;
use ti4_engine_core::driver::turn::{advance_job, advance_reconcile};
use ti4_engine_core::driver::NoSeeds;
use ti4_engine_core::kernel::ability::Unknown;
use ti4_engine_core::kernel::stage::AbilityTable;
use ti4_engine_core::kernel::turn_codes::TurnGap;
use ti4_engine_core::kernel::Registry;
use ti4_engine_core::keys::{Atom, MarkKey, MarkVal, Outcome, Word};
use ti4_engine_core::state::{Emit, EventKind, Key, PlainArg, PlainKind, PlainRec, StartingTechPick, StartingTechSource, TurnJob, Val};
use ti4_engine_core_tests::prelude::*;

fn env(k: &Kernel) -> Env<'_> {
    let mut table = AbilityTable::default();
    ti4_engine_core::driver::children::register(&mut table);
    Env { kernel: k, registry: Cow::new(Registry::default()), table, seeds: Rc::new(NoSeeds) }
}

fn gap(why: TurnGap) -> Result<(Live, Out), LiveError> {
    Err(LiveError::Unknown(Unknown::TurnGap(why)))
}

fn plain(s: &CoreState) -> Vec<PlainRec> {
    s.ledger.events.iter().filter_map(|e| e.plain().copied()).collect()
}

fn typed(s: &CoreState, kind: EventKind) -> Vec<ti4_engine_core::state::EventRec> {
    s.ledger.events.iter().filter_map(|e| if let Emit::Typed(t) = e { Some(t.clone()) } else { None }).filter(|t| t.kind == kind).collect()
}

fn finish(s: &mut CoreState, k: &Kernel) -> Result<(Live, Out), LiveError> {
    advance_job(TurnJob::Finish, s, &env(k), None)
}

// -- the end of an action ---------------------------------------------------------------------------------------------------------

#[test]
fn the_end_of_an_action_announces_it_names_the_seat_and_passes_the_turn_when_nothing_else_is_offered() {
    let (mut s, k) = Scenario::lobby(3).phase(Phase::Action).active(Some(0)).build();
    s.pending_mut().actions_this_turn = 3;
    assert_eq!(finish(&mut s, &k).unwrap().0, Live::Done);
    let done = typed(&s, EventKind::ActionCompleted);
    assert_eq!(done.len(), 1);
    assert_eq!(done[0].payload.get(Key::Player), Some(Val::Seat(Seat(0))));
    assert_eq!(done[0].payload.get(Key::Component), Some(Val::Flag(false)));
    assert_eq!(s.meta().active, Some(Seat(1)), "the turn passed on");
    assert_eq!(s.pending().actions_this_turn, 0, "the next turn starts its count again");
    assert_eq!(s.pending().turn_closing, None);
    assert_eq!(plain(&s), [PlainRec::bare(PlainKind::TurnPassed)]);
}

#[test]
fn an_action_that_was_a_component_action_says_so_in_its_event() {
    let (mut s, k) = Scenario::lobby(3).phase(Phase::Action).active(Some(0)).build();
    s.ledger.events.push(Emit::Plain(PlainRec::bare(PlainKind::ComponentActionResolved)));
    finish(&mut s, &k).unwrap();
    assert_eq!(typed(&s, EventKind::ActionCompleted)[0].payload.get(Key::Component), Some(Val::Flag(true)));
    // Another line after it: the action was not one.
    let (mut s, k) = Scenario::lobby(3).phase(Phase::Action).active(Some(0)).build();
    s.ledger.events.push(Emit::Plain(PlainRec::bare(PlainKind::ComponentActionResolved)));
    s.ledger.events.push(Emit::Plain(PlainRec::bare(PlainKind::ProductionResolved)));
    finish(&mut s, &k).unwrap();
    assert_eq!(typed(&s, EventKind::ActionCompleted)[0].payload.get(Key::Component), Some(Val::Flag(false)));
}

#[test]
fn the_cards_shown_for_this_action_stop_being_visible_with_it() {
    let (mut s, k) = Scenario::lobby(3).phase(Phase::Action).active(Some(0)).build();
    let reveal = |kind| MarkKey::CardsReveal(kind, Word::Open, Seat(0), Seat(1), Word::ActionCards);
    for kind in [Word::Action, Word::Choice] {
        s.marks_mut().tail.edit().insert(reveal(kind), MarkVal::List(Vec::new()));
    }
    finish(&mut s, &k).unwrap();
    assert!(!s.marks().tail.contains_key(&reveal(Word::Action)), "the action's reveal ended");
    assert!(s.marks().tail.contains_key(&reveal(Word::Choice)), "another scope's stays");
}

#[test]
fn fleet_logistics_counts_the_action_before_the_menu_is_judged() {
    let (mut s, k) = Scenario::lobby(3).phase(Phase::Action).active(Some(0)).tech(0, Technology::FleetLogistics).build();
    // The first action: a second one is offered, which the core does not assemble yet.
    let held = s.clone();
    assert_eq!(finish(&mut s, &k), gap(TurnGap::MenuSources));
    assert_eq!(s, held, "the action is not counted in a position the job left");
    // The second action is the last: nothing but "end your turn" is left.
    s.pending_mut().actions_this_turn = 1;
    assert_eq!(finish(&mut s, &k).unwrap().0, Live::Done);
    assert_eq!(s.meta().active, Some(Seat(1)));
    // Outside the action phase an action takes nothing and closes nothing.
    let (mut s, k) = Scenario::lobby(3).phase(Phase::Agenda).active(Some(0)).tech(0, Technology::FleetLogistics).build();
    assert_eq!(finish(&mut s, &k).unwrap().0, Live::Done);
}

#[test]
fn an_additional_action_skips_the_closing_menu_and_keeps_the_turn() {
    let (mut s, k) = Scenario::lobby(3).phase(Phase::Action).active(Some(0)).tech(0, Technology::FleetLogistics).build();
    s.transient_mut().misc.flags.set(ti4_engine_core::state::TransientFlags::ADDITIONAL_ACTION);
    assert_eq!(finish(&mut s, &k).unwrap().0, Live::Done);
    assert_eq!(s.meta().active, Some(Seat(0)));
    assert_eq!(plain(&s), [PlainRec::bare(PlainKind::TurnRetained)]);
}

#[test]
fn a_turn_that_ends_outside_the_action_phase_asks_no_end_of_turn_windows() {
    let planet_system = ti4_engine_core_tests::scenario::first_planet().0;
    let (mut s, k) = Scenario::lobby(3).phase(Phase::Agenda).active(Some(0)).command_token(planet_system, 0).build();
    s.agenda_mut().laws.insert(Agenda::MinisterOfWar, Outcome::Seat(Seat(0)));
    s.players_mut().edit(0).econ.tactic_tokens = 2;
    s.pending_mut().actions_this_turn = 2;
    assert_eq!(advance_job(TurnJob::End, &mut s, &env(&k), None).unwrap().0, Live::Done, "the Minister of War is a law of the action phase");
    assert_eq!(s.pending().actions_this_turn, 0);
}

// -- the closing menu -------------------------------------------------------------------------------------------------------------

#[test]
fn the_closing_menu_holds_only_the_end_of_the_turn_when_no_source_can_offer_a_free_option() {
    let (s, k) = Scenario::lobby(3).phase(Phase::Action).active(Some(0)).build();
    assert_eq!(closing::options(&s, &k, Seat(0)), Ok(vec![TuOpt::EndTurn]));
    // Fleet Logistics: a second action while fewer than two were taken.
    let (mut s, k) = Scenario::lobby(3).phase(Phase::Action).active(Some(0)).tech(0, Technology::FleetLogistics).build();
    s.pending_mut().actions_this_turn = 1;
    assert_eq!(closing::options(&s, &k, Seat(0)), Err(TurnGap::MenuSources));
    s.pending_mut().actions_this_turn = 2;
    assert_eq!(closing::options(&s, &k, Seat(0)), Ok(vec![TuOpt::EndTurn]));
}

/// The position as a lend without its map leaves it: Hacan's Guild Ships reach every seat only over a map, so no transaction partner is a source of
/// the closing menu (with one, the menu stops with `TurnGap::MenuSources`: the typed partner options are lane F's).
fn without_map(s: &mut ti4_engine_core::state::CoreState) {
    s.loaded = ti4_engine_core::state::SliceSet(ti4_engine_core::state::SliceSet::ALL.0 & !(1 << ti4_engine_core::state::Slice::Map as u32));
}

#[test]
fn a_leader_printed_during_the_action_phase_is_a_free_option_of_the_closing_menu() {
    let agent = Leader::ALL.iter().copied().find(|l| l.data().kind == LeaderType::Agent && l.data().window == LeaderWindow::ActionPhase).unwrap();
    let faction = Faction::ALL.iter().copied().find(|f| ti4_engine_core::query::tech::tag_is(agent.data().faction, *f)).unwrap();
    // (One seat: Hacan's Guild Ships would trade with anybody, and a partner is a free option of its own.)
    let (mut s, k) = Scenario::new(&[faction]).phase(Phase::Action).active(Some(0)).build();
    s.players_mut().edit(0).leaders.status.insert(agent, ti4_engine_core::state::LeaderStatus::Readied);
    // The menu lists exactly what the action menu's leaders source lists and calls free: "end your turn" first, then those.
    let free: Vec<TuOpt> = ti4_engine_core::driver::menu_sources::leaders(&s, &k, Seat(0)).into_iter().filter(|o| ti4_engine_core::driver::menu_sources::leader_is_free(*o) == Some(true)).collect();
    let mut expected = vec![TuOpt::EndTurn];
    expected.extend(free.iter().copied());
    assert_eq!(closing::options(&s, &k, Seat(0)), Ok(expected));
    s.players_mut().edit(0).leaders.status.insert(agent, ti4_engine_core::state::LeaderStatus::Exhausted);
    assert_eq!(closing::options(&s, &k, Seat(0)), Ok(vec![TuOpt::EndTurn]), "an exhausted agent offers nothing");
}

#[test]
fn a_free_leader_of_the_closing_menu_is_asked_and_its_dispatch_is_a_counted_gap() {
    use ti4_engine_core::generated::Leader as L;
    let faction = Faction::ALL.iter().copied().find(|f| ti4_engine_core::query::tech::tag_is(L::CarthOfGoldenSands.data().faction, *f)).unwrap();
    let (mut s, k) = Scenario::new(&[faction, Faction::TheFederationOfSol]).phase(Phase::Action).active(Some(0)).build();
    without_map(&mut s);
    s.players_mut().edit(0).leaders.status.insert(L::CarthOfGoldenSands, ti4_engine_core::state::LeaderStatus::Readied);
    s.pending_mut().turn_closing = Some(Seat(0));
    let e = env(&k);
    let q = ask_ask(&s, &e).unwrap().unwrap().choice;
    let carth = Opt::Turn(TuOpt::Leader(L::CarthOfGoldenSands));
    assert_eq!(q.opts.as_slice(), &[Opt::Turn(TuOpt::EndTurn), carth][..], "end your turn first, then the free leader");
    // Taking it is the string's dispatch: a counted gap, the position as it was.
    let held = s.clone();
    assert_eq!(advance_ask(&mut s, &e, Some(Pick::Opt(carth))), Err(LiveError::Unknown(Unknown::TurnGap(TurnGap::MenuDispatch))));
    assert_eq!(s, held);
    // Ending the turn still ends it.
    assert_eq!(advance_ask(&mut s, &e, Some(Pick::Opt(Opt::Turn(TuOpt::EndTurn)))).unwrap().0, Live::Done);
    assert_eq!(s.pending().turn_closing, None);
}

#[test]
fn a_seat_with_a_neighbour_on_the_map_could_open_a_transaction() {
    let (system, _) = ti4_engine_core_tests::scenario::first_planet();
    let (s, k) = Scenario::lobby(3).phase(Phase::Action).active(Some(0)).map_ring(1).unit(system, None, Unit::CruiserI, 0, 1).unit(system, None, Unit::CruiserI, 1, 1).build();
    assert_eq!(closing::options(&s, &k, Seat(0)), Err(TurnGap::MenuSources), "seats that share a system are neighbours");
    let (alone, k) = Scenario::lobby(3).phase(Phase::Action).active(Some(0)).map_ring(1).unit(system, None, Unit::CruiserI, 0, 1).build();
    assert_eq!(closing::options(&alone, &k, Seat(0)), Ok(vec![TuOpt::EndTurn]), "nobody is near");
}

#[test]
fn the_closing_question_is_asked_of_the_seat_in_it_and_its_end_ends_the_turn() {
    let (mut s, k) = Scenario::lobby(3).phase(Phase::Action).active(Some(0)).build();
    s.pending_mut().turn_closing = Some(Seat(0));
    let e = env(&k);
    let q = ask_ask(&s, &e).unwrap().unwrap().choice;
    assert_eq!((q.actor, q.ask, q.opts.as_slice()), (Seat(0), Ask::Turn(TuAsk::Closing { second: false }), &[Opt::Turn(TuOpt::EndTurn)][..]));
    let held = s.clone();
    assert_eq!(advance_ask(&mut s, &e, Some(Pick::Opt(Opt::Decline))), Err(LiveError::Illegal));
    assert_eq!(advance_ask(&mut s, &e, Some(Pick::Number(1))), Err(LiveError::Illegal));
    assert_eq!(s, held);
    assert_eq!(advance_ask(&mut s, &e, Some(Pick::Opt(Opt::Turn(TuOpt::EndTurn)))).unwrap().0, Live::Done);
    assert_eq!((s.meta().active, s.pending().turn_closing), (Some(Seat(1)), None));
    // Where the menu is not assembled yet the question stops with the gap.
    let (mut s, k) = Scenario::lobby(3).phase(Phase::Action).active(Some(0)).tech(0, Technology::FleetLogistics).build();
    s.pending_mut().turn_closing = Some(Seat(0));
    assert_eq!(ask_ask(&s, &env(&k)).map(|_| ()), Err(LiveError::Unknown(Unknown::TurnGap(TurnGap::MenuSources))));
    // An ask arm nobody registered is a gap that names it.
    let (s, k) = Scenario::lobby(3).phase(Phase::Action).active(Some(0)).build();
    assert!(matches!(ask_ask(&s, &env(&k)), Err(LiveError::Unknown(Unknown::TurnGap(TurnGap::AskArm(_))))));
}

// -- the fleet sweep's facts -------------------------------------------------------------------------------------------------------

#[test]
fn the_sweep_judges_every_seat_with_ships_in_table_order_by_the_fleet_supply_it_has() {
    let planets: Vec<System> = ti4_engine_core_tests::scenario::plain_systems();
    let (mut s, k) = Scenario::new(&[Faction::TheEmiratesOfHacan, Faction::TheFederationOfSol, Faction::TheNaaluCollective])
        .phase(Phase::Action)
        .tokens(1, 3, 5, 2)
        .unit(planets[0], None, Unit::CruiserI, 1, 2)
        .unit(planets[1], None, Unit::CruiserI, 2, 1)
        .unit(planets[1], None, Unit::CruiserI, 1, 1)
        .build();
    let pairs = facts::enforce_pairs(&s, &k);
    let seats: Vec<(Seat, System)> = pairs.iter().map(|(seat, system, _)| (*seat, *system)).collect();
    assert_eq!(seats, [(Seat(1), planets[0]), (Seat(1), planets[1]), (Seat(2), planets[1])], "table order, then board order; seat 0 has no ship");
    assert_eq!(facts::fleet_limit(&s, &k, Seat(1)), 5, "the fleet pool");
    assert_eq!((pairs[0].2.enforce, pairs[0].2.shown, pairs[0].2.halves), (5, 5, [None, None]));
    // An unlimited fleet supply (Darktalon Treilla) shows the bill, and lifts the enforcement from rules version 17.
    s.players_mut().edit(1).production_timers.fleet_supply_unlimited_until = ti4_engine_core::Opt32::some(s.meta().round);
    let l = facts::limits(&s, &k, Seat(1), planets[0]);
    assert_eq!((l.enforce, l.shown), (10_000, 10_000));
    s.meta_mut().rules_version = 16;
    let l = facts::limits(&s, &k, Seat(1), planets[0]);
    assert_eq!((l.enforce, l.shown), (5, 10_000), "before version 17 the lift is only shown");
}

#[test]
fn naalus_hybrid_crystal_fighters_weigh_half_a_ship_in_the_sweep() {
    let system = ti4_engine_core_tests::scenario::plain_systems()[0];
    let (s, k) = Scenario::new(&[Faction::TheNaaluCollective, Faction::TheEmiratesOfHacan])
        .phase(Phase::Action)
        .unit(system, None, Unit::HybridCrystalFighterII, 0, 1)
        .unit(system, None, Unit::CruiserI, 0, 1)
        .build();
    assert_eq!(facts::limits(&s, &k, Seat(0), system).halves, [Some(Unit::HybridCrystalFighterII), None]);
    let (s, k) = Scenario::new(&[Faction::TheEmiratesOfHacan, Faction::TheNaaluCollective]).phase(Phase::Action).unit(system, None, Unit::HybridCrystalFighterII, 0, 1).build();
    assert_eq!(facts::limits(&s, &k, Seat(0), system).halves, [None, None], "only Naalu's own fighters");
}

// -- the end of the draft ---------------------------------------------------------------------------------------------------------

fn last_pick(seats: &[Faction]) -> (CoreState, Kernel) {
    let (mut s, k) = Scenario::new(seats).phase(Phase::Strategy).active(None).build();
    s.meta_mut().strategy_cards_per_player = 1;
    s.strategy_mut().unclaimed = [StrategyCard::Imperial].into_iter().collect();
    ti4_engine_core::rules::strategy_flow::deal(&mut s, Seat(0), StrategyCard::Leadership);
    (s, k)
}

fn take_last(s: &mut CoreState, k: &Kernel) -> Result<(Live, Out), LiveError> {
    advance_ask(s, &env(k), Some(Pick::Opt(Opt::Strategy(ti4_engine_core::choice::strategy::Opt::Card(StrategyCard::Imperial)))))
}

/// The question a call stopped at.
fn asked(r: Result<(Live, Out), LiveError>) -> ti4_engine_core::choice::Choice {
    match r.expect("the job runs") {
        (Live::Asks(a), _) => a.choice,
        (Live::Done, _) => panic!("a question was expected"),
    }
}

fn offers_the_arbiter(r: Result<(Live, Out), LiveError>) -> bool {
    use ti4_engine_core::choice::agenda::Ask as AAsk;
    matches!(r.expect("the job runs"), (Live::Asks(a), _) if matches!(a.choice.ask, Ask::Agenda(AAsk::DiscardLaw { law: Agenda::ImperialArbiter, .. })))
}

#[test]
fn imperial_arbiter_is_offered_only_when_its_owner_and_another_seat_both_hold_a_card() {
    let seats = [Faction::TheEmiratesOfHacan, Faction::TheFederationOfSol, Faction::TheNaaluCollective];
    // Seat 0 holds Leadership and seat 1 takes the last card: with the law elected for seat 0 a swap is possible.
    let (mut s, k) = last_pick(&seats);
    s.agenda_mut().laws.insert(Agenda::ImperialArbiter, Outcome::Seat(Seat(0)));
    assert!(offers_the_arbiter(take_last(&mut s, &k)));
    // The owner (seat 2) holds no card: nothing to give.
    let (mut s, k) = last_pick(&seats);
    s.agenda_mut().laws.insert(Agenda::ImperialArbiter, Outcome::Seat(Seat(2)));
    assert_eq!(take_last(&mut s, &k).unwrap().0, Live::Done);
    // The law is elected for the picker, who now holds the last card, and seat 0 holds one: a swap.
    let (mut s, k) = last_pick(&seats);
    s.agenda_mut().laws.insert(Agenda::ImperialArbiter, Outcome::Seat(Seat(1)));
    assert!(offers_the_arbiter(take_last(&mut s, &k)));
    // Nobody else holds a card: the owner takes the only card dealt.
    let (mut s, k) = Scenario::new(&seats[..2]).phase(Phase::Strategy).active(None).build();
    s.meta_mut().strategy_cards_per_player = 1;
    s.strategy_mut().unclaimed = [StrategyCard::Imperial].into_iter().collect();
    s.agenda_mut().laws.insert(Agenda::ImperialArbiter, Outcome::Seat(Seat(0)));
    assert_eq!(take_last(&mut s, &k).unwrap().0, Live::Done);
    // Without the law the draft simply ends.
    let (mut s, k) = last_pick(&seats);
    assert_eq!(take_last(&mut s, &k).unwrap().0, Live::Done);
}

/// Seat 0 holds Technology and takes Diplomacy, the last card on the mat; seat 1 holds Leadership and Politics; the Arbiter is seat 0's.
/// The initiative of the eight cards of the Prophecy of Kings (the holdings are kept in this order).
fn initiative(s: &mut CoreState) {
    let cards = [StrategyCard::Leadership, StrategyCard::DiplomacyCodex, StrategyCard::Politics, StrategyCard::Construction, StrategyCard::Trade, StrategyCard::Warfare, StrategyCard::Technology, StrategyCard::Imperial];
    for (i, c) in cards.iter().enumerate() {
        s.strategy_mut().initiative.insert(*c, i as i32 + 1);
    }
}

fn take_diplomacy(s: &mut CoreState, k: &Kernel) -> Result<(Live, Out), LiveError> {
    advance_ask(s, &env(k), Some(Pick::Opt(Opt::Strategy(ti4_engine_core::choice::strategy::Opt::Card(StrategyCard::DiplomacyCodex)))))
}

fn arbiter_draft() -> (CoreState, Kernel) {
    let (mut s, k) = Scenario::new(&[Faction::TheEmiratesOfHacan, Faction::TheFederationOfSol]).phase(Phase::Strategy).active(None).build();
    s.meta_mut().strategy_cards_per_player = 2;
    initiative(&mut s);
    s.strategy_mut().unclaimed = [StrategyCard::DiplomacyCodex].into_iter().collect();
    for (seat, card) in [(0, StrategyCard::Technology), (1, StrategyCard::Leadership), (1, StrategyCard::Politics)] {
        ti4_engine_core::rules::strategy_flow::deal(&mut s, Seat(seat), card);
    }
    s.agenda_mut().laws.insert(Agenda::ImperialArbiter, Outcome::Seat(Seat(0)));
    (s, k)
}

fn holding(s: &CoreState, seat: u8) -> Vec<StrategyCard> {
    s.players().by_seat(Seat(seat)).unwrap().strat.as_slice().to_vec()
}

fn pick_of(o: ti4_engine_core::choice::strategy::Opt) -> Pick {
    Pick::Opt(Opt::Strategy(o))
}

#[test]
fn the_arbiter_swap_asks_what_to_give_then_what_to_take_and_moves_the_cards_by_hand() {
    use ti4_engine_core::choice::agenda::Opt as AOpt;
    use ti4_engine_core::choice::strategy::{Ask as SAsk, Opt as SOpt};
    let (mut s, k) = arbiter_draft();
    let e = env(&k);
    let offer = asked(take_diplomacy(&mut s, &k));
    assert_eq!(offer.actor, Seat(0));
    assert_eq!(offer.opts.as_slice(), [Opt::Agenda(AOpt::DiscardLaw), Opt::Decline]);
    // The owner (who took the last card) discards the law and is asked what to give: its two cards, in the order it holds them, with no way to decline.
    let give = asked(advance_ask(&mut s, &e, Some(Pick::Opt(Opt::Agenda(AOpt::DiscardLaw)))));
    assert_eq!((give.actor, give.ask), (Seat(0), Ask::Strategy(SAsk::ArbiterGive)));
    assert_eq!(give.opts.as_slice(), [Opt::Strategy(SOpt::Card(StrategyCard::DiplomacyCodex)), Opt::Strategy(SOpt::Card(StrategyCard::Technology))]);
    // Then what to take: every card of the others with its holder.
    let take = asked(advance_ask(&mut s, &e, Some(pick_of(SOpt::Card(StrategyCard::DiplomacyCodex)))));
    assert_eq!((take.actor, take.ask), (Seat(0), Ask::Strategy(SAsk::ArbiterTake)));
    assert_eq!(take.opts.as_slice(), [Opt::Strategy(SOpt::Take(Seat(1), StrategyCard::Leadership)), Opt::Strategy(SOpt::Take(Seat(1), StrategyCard::Politics))]);
    assert_eq!(advance_ask(&mut s, &e, Some(pick_of(SOpt::Take(Seat(1), StrategyCard::Leadership)))).unwrap().0, Live::Done);
    // The taken card goes on the END of the holding and the card given on the end of the other's: no initiative order.
    assert_eq!(holding(&s, 0), [StrategyCard::Technology, StrategyCard::Leadership]);
    assert_eq!(holding(&s, 1), [StrategyCard::Politics, StrategyCard::DiplomacyCodex]);
    assert!(!s.agenda().laws.contains_key(&Agenda::ImperialArbiter), "the law was discarded");
    assert_eq!(plain(&s).last(), Some(&PlainRec::two(PlainKind::StrategyCardsSwapped, PlainArg::Seat(Seat(0)), PlainArg::Seat(Seat(1)))));
}

#[test]
fn a_declined_arbiter_keeps_the_law_and_the_cards_and_a_card_not_offered_is_refused() {
    use ti4_engine_core::choice::agenda::Opt as AOpt;
    use ti4_engine_core::choice::strategy::Opt as SOpt;
    let (mut s, k) = arbiter_draft();
    let e = env(&k);
    asked(take_diplomacy(&mut s, &k));
    assert_eq!(advance_ask(&mut s, &e, Some(Pick::Opt(Opt::Decline))).unwrap().0, Live::Done);
    assert!(s.agenda().laws.contains_key(&Agenda::ImperialArbiter));
    assert_eq!(holding(&s, 0), [StrategyCard::DiplomacyCodex, StrategyCard::Technology], "the last card is the owner's, nothing was swapped");
    assert_eq!(holding(&s, 1), [StrategyCard::Leadership, StrategyCard::Politics]);
    assert!(!plain(&s).iter().any(|p| p.kind == PlainKind::StrategyCardsSwapped));
    // A card the owner does not hold cannot be given.
    let (mut s, k) = arbiter_draft();
    let e = env(&k);
    asked(take_diplomacy(&mut s, &k));
    asked(advance_ask(&mut s, &e, Some(Pick::Opt(Opt::Agenda(AOpt::DiscardLaw)))));
    assert_eq!(advance_ask(&mut s, &e, Some(pick_of(SOpt::Card(StrategyCard::Leadership)))), Err(LiveError::Illegal));
}

/// The datahub draft: seat 1 (Sol) owns the node and holds Leadership and Politics; seat 0 holds Technology and takes the last card, Diplomacy.
fn datahub_draft(tokens: i16, goods: i16) -> (CoreState, Kernel) {
    let (mut s, k) = Scenario::new(&[Faction::TheEmiratesOfHacan, Faction::TheFederationOfSol]).phase(Phase::Strategy).active(None).build();
    s.meta_mut().strategy_cards_per_player = 2;
    initiative(&mut s);
    s.strategy_mut().unclaimed = [StrategyCard::DiplomacyCodex].into_iter().collect();
    for (seat, card) in [(0, StrategyCard::Technology), (1, StrategyCard::Leadership), (1, StrategyCard::Politics)] {
        ti4_engine_core::rules::strategy_flow::deal(&mut s, Seat(seat), card);
    }
    s.players_mut().edit(1).tech.owned.insert(Technology::QuantumDatahubNode);
    s.players_mut().edit(1).econ.strategic_tokens = tokens;
    s.players_mut().edit(1).econ.trade_goods = goods;
    (s, k)
}

#[test]
fn the_datahub_offers_every_partner_card_pair_and_pays_a_token_and_three_goods() {
    use ti4_engine_core::choice::technology::{Ask as TAsk, Opt as TOpt};
    let (mut s, k) = datahub_draft(2, 4);
    let e = env(&k);
    let q = asked(take_diplomacy(&mut s, &k));
    assert_eq!((q.actor, q.ask), (Seat(1), Ask::Technology(TAsk::QuantumDatahub)));
    // The partner's cards in order, each against each card the holder has (Leadership, Politics), then the decline.
    let swap = |give, take| Opt::Technology(TOpt::Swap { partner: Seat(0), give, take });
    assert_eq!(
        q.opts.as_slice(),
        [
            swap(StrategyCard::Leadership, StrategyCard::DiplomacyCodex),
            swap(StrategyCard::Leadership, StrategyCard::Technology),
            swap(StrategyCard::Politics, StrategyCard::DiplomacyCodex),
            swap(StrategyCard::Politics, StrategyCard::Technology),
            Opt::Decline
        ]
    );
    let staged_before = s.pending().staged.len();
    assert_eq!(advance_ask(&mut s, &e, Some(Pick::Opt(swap(StrategyCard::Leadership, StrategyCard::Technology)))).unwrap().0, Live::Done);
    // Both holdings stay in initiative order and the trade goods and the token moved.
    assert_eq!(holding(&s, 0), [StrategyCard::Leadership, StrategyCard::DiplomacyCodex]);
    assert_eq!(holding(&s, 1), [StrategyCard::Politics, StrategyCard::Technology]);
    assert_eq!((s.players().by_seat(Seat(1)).unwrap().econ.trade_goods, s.players().by_seat(Seat(1)).unwrap().econ.strategic_tokens), (1, 1));
    assert_eq!(s.players().by_seat(Seat(0)).unwrap().econ.trade_goods, 3);
    assert_eq!(s.pending().staged.len() - staged_before, 2, "the token spent and the goods gained wait for the next step");
    assert_eq!(plain(&s).last(), Some(&PlainRec::two(PlainKind::QuantumDatahubSwap, PlainArg::Seat(Seat(1)), PlainArg::Seat(Seat(0)))));
    // A decline changes nothing but the draft's own record.
    let (mut s, k) = datahub_draft(2, 4);
    let e = env(&k);
    asked(take_diplomacy(&mut s, &k));
    assert_eq!(advance_ask(&mut s, &e, Some(Pick::Opt(Opt::Decline))).unwrap().0, Live::Done);
    assert_eq!(s.players().by_seat(Seat(1)).unwrap().econ.trade_goods, 4);
    assert!(!plain(&s).iter().any(|p| p.kind == PlainKind::QuantumDatahubSwap));
}

#[test]
fn the_datahub_is_offered_only_with_the_technology_a_token_and_three_trade_goods_and_a_partner_with_a_card() {
    for (tokens, goods, offered) in [(1, 3, true), (0, 3, false), (1, 2, false)] {
        let (mut s, k) = datahub_draft(tokens, goods);
        let r = take_diplomacy(&mut s, &k);
        assert_eq!(matches!(r, Ok((Live::Asks(_), _))), offered, "{tokens} {goods}");
    }
    // Without the technology's text.
    let (mut s, k) = datahub_draft(1, 3);
    s.players_mut().edit(1).tech.owned.remove(Technology::QuantumDatahubNode);
    assert_eq!(take_diplomacy(&mut s, &k).unwrap().0, Live::Done);
    // The first seat with the node and the means is the one asked: a second owner is never offered the swap.
    let (mut s, k) = datahub_draft(1, 3);
    s.players_mut().edit(0).tech.owned.insert(Technology::QuantumDatahubNode);
    s.players_mut().edit(0).econ.strategic_tokens = 1;
    s.players_mut().edit(0).econ.trade_goods = 3;
    assert_eq!(asked(take_diplomacy(&mut s, &k)).actor, Seat(0));
    // The owner is the picker and no other seat holds a card: nobody to swap with.
    let (mut s, k) = Scenario::new(&[Faction::TheEmiratesOfHacan, Faction::TheFederationOfSol]).phase(Phase::Strategy).active(None).tech(0, Technology::QuantumDatahubNode).tokens(0, 1, 1, 1).goods(0, 5).build();
    s.meta_mut().strategy_cards_per_player = 1;
    s.strategy_mut().unclaimed = [StrategyCard::Imperial].into_iter().collect();
    assert_eq!(take_last(&mut s, &k).unwrap().0, Live::Done);
}

#[test]
fn the_last_card_is_dealt_and_its_trade_goods_are_staged_only_when_there_are_some() {
    let (mut s, k) = last_pick(&[Faction::TheEmiratesOfHacan, Faction::TheFederationOfSol]);
    s.strategy_mut().goods.insert(StrategyCard::Imperial, 2);
    assert_eq!(take_last(&mut s, &k).unwrap().0, Live::Done);
    assert_eq!(s.players().by_seat(Seat(1)).unwrap().econ.trade_goods, 2);
    assert_eq!(s.pending().staged.len(), 1);
    let (mut s, k) = last_pick(&[Faction::TheEmiratesOfHacan, Faction::TheFederationOfSol]);
    take_last(&mut s, &k).unwrap();
    assert!(s.pending().staged.is_empty(), "no goods, no gain to announce");
}

// -- the starting technology ------------------------------------------------------------------------------------------------------

#[test]
fn a_pick_from_the_technologies_others_own_offers_only_non_faction_cards_the_seat_lacks() {
    use ti4_engine_core::driver::turn::starting::{options, question};
    let faction_tech = Technology::ALL.iter().copied().find(|t| t.data().faction.is_some()).unwrap();
    let (mut s, k) = Scenario::new(&[Faction::TheEmiratesOfHacan, Faction::TheFederationOfSol, Faction::TheNaaluCollective])
        .phase(Phase::Strategy)
        .tech(1, Technology::SarweenTools)
        .tech(1, faction_tech)
        .tech(2, Technology::PlasmaScoring)
        .tech(0, Technology::PlasmaScoring)
        .build();
    let pick = StartingTechPick { player: Seat(0), of: 2, remaining: 2, source: StartingTechSource::OwnedByOthers };
    s.transient_mut().misc.starting_tech_picks.push(pick);
    assert_eq!(options(&s, &k, &pick), [Technology::SarweenTools], "the faction card and the one the seat owns are left out");
    let q = question(&s, &k, ti4_engine_core::state::Code(0)).unwrap();
    assert_eq!(q.ask, Ask::Technology(ti4_engine_core::choice::technology::Ask::StartingTech { number: 1, of: 2, rule: ti4_engine_core::choice::technology::StartRule::OwnedByOthers }));
    // A pick of the seat that is not seated offers nothing.
    let ghost = StartingTechPick { player: Seat(7), ..pick };
    assert!(options(&s, &k, &ghost).is_empty());
}

// -- the reconcile ----------------------------------------------------------------------------------------------------------------

#[test]
fn a_relic_and_a_breakthrough_nobody_announced_are_announced_once_each() {
    let (mut s, k) = Scenario::lobby(2).phase(Phase::Action).build();
    let mut e = env(&k);
    e.table.facts.staging = true;
    advance_reconcile(&mut s, &e, None).unwrap();
    s.players_mut().edit(0).cards.relics.push(Relic::TheCodex);
    advance_reconcile(&mut s, &e, None).unwrap();
    let relics: Vec<_> = typed(&s, EventKind::RelicGained);
    assert_eq!(relics.len(), 1);
    assert_eq!(relics[0].payload.get(Key::Player), Some(Val::Seat(Seat(0))));
    advance_reconcile(&mut s, &e, None).unwrap();
    assert_eq!(typed(&s, EventKind::RelicGained).len(), 1, "announced once");
    // A second copy of the same relic is a new gain; one that stays is not.
    s.players_mut().edit(0).cards.relics.push(Relic::TheCodex);
    advance_reconcile(&mut s, &e, None).unwrap();
    assert_eq!(typed(&s, EventKind::RelicGained).len(), 2);
    assert_eq!(s.marks().tail.get(&MarkKey::PrivateSeen(Word::Relics, Seat(0))), Some(&MarkVal::List(vec![MarkVal::Atom(Atom::Relic(Relic::TheCodex)); 2])));
    s.players_mut().edit(1).cards.breakthrough = Some(Breakthrough::Psychospore);
    advance_reconcile(&mut s, &e, None).unwrap();
    assert_eq!(typed(&s, EventKind::BreakthroughGained).len(), 1);
    advance_reconcile(&mut s, &e, None).unwrap();
    assert_eq!(typed(&s, EventKind::BreakthroughGained).len(), 1, "announced once");
}

#[test]
fn a_spec_ops_die_survives_on_a_five_or_more() {
    let (mut s, k) = Scenario::new(&[Faction::TheFederationOfSol, Faction::TheEmiratesOfHacan]).phase(Phase::Action).build();
    s.players_mut().edit(0).misc.spec_ops_destroyed = 60;
    s.rng = ti4_engine_core::state::Rng::new(9);
    let mut e = env(&k);
    e.seeds = Rc::new(ti4_engine_core_tests::rig::KitSeeds(3));
    advance_reconcile(&mut s, &e, None).unwrap();
    let faces: Vec<u8> = s.ledger.dice.get(0).unwrap().faces.as_slice().to_vec();
    assert_eq!(faces.len(), 60);
    assert!(faces.contains(&5) && faces.iter().any(|f| *f < 5) && faces.iter().all(|f| (1..=10).contains(f)), "{faces:?}");
    let expected: Vec<PlainKind> = faces.iter().map(|f| if *f >= 5 { PlainKind::SpecOpsSurvived } else { PlainKind::SpecOpsLost }).collect();
    assert_eq!(plain(&s).iter().map(|p| p.kind).collect::<Vec<_>>(), expected, "one line per die, in the order rolled");
    assert_eq!(s.players().by_seat(Seat(0)).unwrap().misc.spec_ops_card as usize, faces.iter().filter(|f| **f >= 5).count());
    assert_eq!(s.ledger.dice.get(0).unwrap().hits_on, Some(5));
    assert_eq!(s.ledger.dice.get(0).unwrap().owner, Some(Seat(0)));
}

#[test]
fn the_end_of_the_round_resets_both_flags_and_the_levy_needs_the_revolution_against() {
    use ti4_engine_core::driver::turn::advance_phase;
    use ti4_engine_core::state::{DriverFlags, PlanetFlags};
    let planets: Vec<Planet> = Planet::ALL.iter().copied().filter(|p| p.data().system.is_some()).take(2).collect();
    let (mut s, k) = Scenario::lobby(2).phase(Phase::Status).active(None).control(planets[0], 0).control(planets[1], 0).tech(0, Technology::Psychoarchaeology).tech(0, Technology::AntimassDeflectors).build();
    for c in [StrategyCard::Leadership, StrategyCard::Imperial] {
        s.strategy_mut().initiative.insert(c, 1);
    }
    s.pending_mut().driver.set(DriverFlags::STATUS_RESOLVED, true);
    s.pending_mut().driver.set(DriverFlags::AGENDA_RESOLVED, true);
    s.agenda_mut().laws.insert(Agenda::AntiIntellectualRevolution, Outcome::For);
    advance_phase(&mut s, &env(&k), None).unwrap();
    assert!(!s.pending().driver.has(DriverFlags::STATUS_RESOLVED) && !s.pending().driver.has(DriverFlags::AGENDA_RESOLVED));
    assert!(planets.iter().all(|p| !s.board().planet(*p).flags.has(PlanetFlags::EXHAUSTED)), "a revolution in favour levies nothing");
}

// -- third batch: gating of the prelude, the datahub's conditions one by one, the job window's own functions ----------------------------


#[test]
fn a_card_the_mat_does_not_hold_is_refused_by_the_job_and_leaves_the_position() {
    use ti4_engine_core::driver::turn::advance_job;
    let (mut s, k) = last_pick(&[Faction::TheEmiratesOfHacan, Faction::TheFederationOfSol]);
    let held = s.clone();
    assert_eq!(advance_job(TurnJob::Draft(StrategyCard::Trade), &mut s, &env(&k), None), gap(TurnGap::ReplayDiverged));
    assert_eq!(s, held);
}

#[test]
fn military_support_returns_home_with_a_token_and_is_announced() {
    use ti4_engine_core::driver::turn::advance_prelude;
    use ti4_engine_core::kernel::ability::{PlayFacts, PlaysProvider, SharedPlay};
    let (mut s, k) = Scenario::new(&[Faction::TheFederationOfSol, Faction::TheEmiratesOfHacan]).phase(Phase::Action).active(Some(0)).tokens(0, 3, 3, 2).build();
    let key = ti4_engine_core::state::NoteKey { note: PromissoryNote::MilitarySupport, faction: Faction::TheFederationOfSol };
    s.cards_mut().promissory.edit().insert(key, Seat(1));
    let roster: &'static ti4_engine_core::query::tech::UnitRoster = Box::leak(Box::default());
    let mut e = env(&k);
    e.table.facts.plays_provider = Some(PlaysProvider(std::sync::Arc::new(move |core, _| {
        Some(PlayFacts {
            scope: 0,
            blanked: false,
            action_card: ti4_engine_core::state::Code(0),
            supply_base: ti4_engine_core::state::Code(0),
            note_order: |a, b| a.cmp(&b),
            seats: Vec::new(),
            name_group: Vec::new(),
            has_effect: Vec::new(),
            skimmer: false,
            roster: Some(roster),
            rules_version: core.meta().rules_version,
            tech_order: |a, b| a.cmp(&b),
            shared: SharedPlay::default(),
        })
    })));
    let held = s.clone();
    // Without the play facts the job cannot know the form the holder places.
    assert_eq!(advance_prelude(&mut s, &env(&k), None), gap(TurnGap::NoRoster));
    assert_eq!(s, held);
    advance_prelude(&mut s, &e, None).unwrap();
    assert_eq!(plain(&s), [PlainRec::bare(PlainKind::MilitarySupportUsed)]);
    assert_eq!(s.players().by_seat(Seat(0)).unwrap().econ.strategic_tokens, 1, "the strategy token is spent");
    assert_eq!(s.cards().promissory.get(&key).copied(), Some(Seat(0)), "the note goes home");
    // A note nobody holds elsewhere does nothing.
    let (mut s, k) = Scenario::new(&[Faction::TheFederationOfSol, Faction::TheEmiratesOfHacan]).phase(Phase::Action).active(Some(0)).build();
    advance_prelude(&mut s, &env(&k), None).unwrap();
    assert!(plain(&s).is_empty());
}

#[test]
fn only_a_locked_commander_without_a_hook_stops_the_prelude() {
    use ti4_engine_core::driver::turn::advance_prelude;
    use ti4_engine_core::state::LeaderStatus;
    let (mut s, k) = Scenario::new(&[Faction::TheArborec, Faction::TheEmiratesOfHacan]).phase(Phase::Action).active(Some(0)).build();
    let of = |kind| Leader::ALL.iter().copied().find(|l| l.data().faction == FactionTag::Arborec && l.data().kind == kind).unwrap();
    // The commander is unlocked already and the hero is locked: neither is a locked commander.
    s.players_mut().edit(0).leaders.status.insert(of(LeaderType::Commander), LeaderStatus::Unlocked);
    s.players_mut().edit(0).leaders.status.insert(of(LeaderType::Hero), LeaderStatus::Locked);
    assert_eq!(advance_prelude(&mut s, &env(&k), None).unwrap().0, Live::Done);
    // A faction with a hook (Hacan's Gila the Silvertongue) is judged, not stopped.
    let (mut s, k) = Scenario::new(&[Faction::TheEmiratesOfHacan, Faction::TheFederationOfSol]).phase(Phase::Action).active(Some(0)).build();
    let gila = Leader::GilaTheSilvertongue;
    s.players_mut().edit(0).leaders.status.insert(gila, LeaderStatus::Locked);
    s.players_mut().edit(0).econ.trade_goods = 10;
    advance_prelude(&mut s, &env(&k), None).unwrap();
    assert_eq!(s.players().by_seat(Seat(0)).unwrap().leaders.status.get(&gila), Some(&LeaderStatus::Unlocked));
    assert_eq!(plain(&s), [PlainRec::one(PlainKind::LeaderUnlocked, ti4_engine_core::state::PlainArg::Leader(gila))]);
}

#[test]
fn the_job_window_keeps_only_which_job_it_is() {
    use ti4_engine_core::driver::live::{ask, is_open};
    use ti4_engine_core::driver::turn::{ask_job, open_finish, open_job_of, TurnFlowWin};
    let (mut s, k) = Scenario::lobby(3).phase(Phase::Action).active(Some(0)).build();
    let e = env(&k);
    assert_eq!(open_job_of(&s), None);
    assert!(ask_job(&s, &e).unwrap().is_none(), "no job, no question");
    open_finish(&mut s);
    assert_eq!(open_job_of(&s), Some(TurnJob::Finish));
    assert!(is_open::<TurnFlowWin>(&s));
    assert!(ask::<TurnFlowWin>(&s, &e).unwrap().is_none(), "a job that has not run asks nothing yet");
    // A question put by a job is asked by the job, and an answer to a job with no question is refused.
    let planet_system = ti4_engine_core_tests::scenario::first_planet().0;
    let (mut m, k) = Scenario::lobby(3).phase(Phase::Action).active(Some(0)).command_token(planet_system, 0).build();
    m.agenda_mut().laws.insert(Agenda::MinisterOfWar, Outcome::Seat(Seat(0)));
    let e = env(&k);
    advance_job(TurnJob::End, &mut m, &e, None).unwrap();
    assert!(ask_job(&m, &e).unwrap().is_some());
}

#[test]
fn predictive_intelligence_is_a_window_of_the_action_phase_only() {
    let (mut s, k) = Scenario::lobby(3).phase(Phase::Agenda).active(Some(0)).tech(0, Technology::PredictiveIntelligence).tokens(0, 2, 2, 2).build();
    assert_eq!(advance_job(TurnJob::End, &mut s, &env(&k), None).unwrap().0, Live::Done, "no restack is asked outside the action phase");
    let (mut s, k) = Scenario::lobby(3).phase(Phase::Action).active(Some(0)).tech(0, Technology::PredictiveIntelligence).tokens(0, 2, 2, 2).build();
    assert!(matches!(advance_job(TurnJob::End, &mut s, &env(&k), None).unwrap().0, Live::Asks(_)));
}

// -- Chaos Mapping at the start of a turn -------------------------------------------------------------------------------------------

/// An `Env` whose play facts hold the (empty) unit roster: the generic forms of the sources are what the seat may produce.
fn roster_env(k: &Kernel) -> Env<'_> {
    use ti4_engine_core::kernel::ability::{PlayFacts, PlaysProvider, SharedPlay};
    let roster: &'static ti4_engine_core::query::tech::UnitRoster = Box::leak(Box::default());
    let mut e = env(k);
    e.table.facts.plays_provider = Some(PlaysProvider(std::sync::Arc::new(move |core, _| {
        Some(PlayFacts {
            scope: 0,
            blanked: false,
            action_card: ti4_engine_core::state::Code(0),
            supply_base: ti4_engine_core::state::Code(0),
            note_order: |a, b| a.cmp(&b),
            seats: Vec::new(),
            name_group: Vec::new(),
            has_effect: Vec::new(),
            skimmer: false,
            roster: Some(roster),
            rules_version: core.meta().rules_version,
            tech_order: |a, b| a.cmp(&b),
            shared: SharedPlay::default(),
        })
    })));
    e
}

/// Seat 0 owns Chaos Mapping and a space dock on a planet it controls in the first plain system (a capacity of 3 and a planet to pay with).
fn chaos_table(goods: i16) -> (CoreState, Kernel, System) {
    let (system, planet) = ti4_engine_core_tests::scenario::first_planet();
    let (s, k) = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec])
        .phase(Phase::Action)
        .active(Some(0))
        .map_from(&[system])
        .control(planet, 0)
        .unit(system, Some(planet), Unit::SpaceDockI, 0, 1)
        .tech(0, Technology::ChaosMapping)
        .goods(0, goods)
        .tokens(0, 3, 3, 2)
        .build();
    (s, k, system)
}

#[test]
fn chaos_mapping_offers_every_system_with_a_production_capacity_and_a_decline_then_the_turn_goes_on() {
    use ti4_engine_core::choice::technology::Opt as TOpt;
    use ti4_engine_core::driver::turn::advance_prelude;
    let (mut s, k, system) = chaos_table(5);
    let e = roster_env(&k);
    let q = asked(advance_prelude(&mut s, &e, None));
    assert_eq!((q.actor, q.ask), (Seat(0), Ask::Technology(ti4_engine_core::choice::technology::Ask::ChaosMapping)));
    assert_eq!(q.opts.as_slice(), [Opt::Technology(TOpt::System(system)), Opt::Decline]);
    assert_eq!(s.pending().prepared_turn_seq, None, "nothing of a half-run job is kept");
    // Declining ends the window and the prelude goes on to the turn's beginning.
    let (live, _) = advance_ask(&mut s, &e, Some(Pick::Opt(Opt::Decline))).unwrap();
    assert_eq!(live, Live::Done);
    assert_eq!(s.pending().prepared_turn_seq, Some(0));
    // A table without a system that could produce is not asked.
    let (mut s, k, _) = chaos_table(5);
    s.board_mut().planet_mut(ti4_engine_core_tests::scenario::first_planet().1).units.clear();
    assert_eq!(advance_prelude(&mut s, &roster_env(&k), None).unwrap().0, Live::Done);
}

#[test]
fn chaos_mapping_produces_the_unit_the_seat_picks_and_pays_for_it() {
    use ti4_engine_core::choice::production::{Ask as PAsk, Opt as POpt};
    use ti4_engine_core::choice::technology::Opt as TOpt;
    use ti4_engine_core::driver::turn::advance_prelude;
    let (mut s, k, system) = chaos_table(5);
    let e = roster_env(&k);
    asked(advance_prelude(&mut s, &e, None));
    let q = asked(advance_ask(&mut s, &e, Some(Pick::Opt(Opt::Technology(TOpt::System(system))))));
    assert!(matches!(q.ask, Ask::Production(PAsk::One(_))), "{:?}", q.ask);
    let infantry = q.opts.as_slice().iter().copied().find(|o| matches!(o, Opt::Production(POpt::One { unit: Unit::InfantryI, .. }))).expect("an infantry is offered");
    let mut q = asked(advance_ask(&mut s, &e, Some(Pick::Opt(infantry))));
    // The bill is paid, then the turn begins.
    for _ in 0..8 {
        assert!(matches!(q.ask, Ask::Production(PAsk::Pay(_))), "{:?}", q.ask);
        let pay = q.opts.as_slice().iter().copied().find(|o| matches!(o, Opt::Production(POpt::TradeGood { .. }))).expect("a trade good pays");
        match advance_ask(&mut s, &e, Some(Pick::Opt(pay))).unwrap() {
            (Live::Asks(a), _) => q = a.choice,
            (Live::Done, _) => break,
        }
    }
    assert_eq!(s.pending().prepared_turn_seq, Some(0), "the turn began");
    let planet = ti4_engine_core_tests::scenario::first_planet().1;
    assert_eq!(s.board().planet(planet).units.as_slice().iter().filter(|p| p.kind() == Unit::InfantryI).count(), 1, "the infantry stands on the planet");
    assert_eq!(s.players().by_seat(Seat(0)).unwrap().econ.trade_goods, 4);
}

#[test]
fn a_chaos_mapping_unit_over_the_capacity_of_its_system_is_followed_by_the_removal_question() {
    use ti4_engine_core::choice::production::{Ask as PAsk, Opt as POpt};
    use ti4_engine_core::choice::technology::Opt as TOpt;
    use ti4_engine_core::driver::turn::advance_prelude;
    let (mut s, k, system) = chaos_table(5);
    // Three fighters fill the capacity of the dock: the fourth stands over it.
    let held = s.board().sys(system).units.as_slice().len();
    for _ in 0..3 {
        ti4_engine_core::kernel::fx::place_unit(&mut s, Seat(0), system, None, Unit::FighterI);
    }
    assert_eq!(s.board().sys(system).units.as_slice().len(), held + 3);
    let e = roster_env(&k);
    asked(advance_prelude(&mut s, &e, None));
    let q = asked(advance_ask(&mut s, &e, Some(Pick::Opt(Opt::Technology(TOpt::System(system))))));
    // A fourth fighter has no ship to ride in: it stands over the capacity of the system and the string asks which unit goes.
    let fighter = q.opts.as_slice().iter().copied().find(|o| matches!(o, Opt::Production(POpt::One { unit: Unit::FighterI, .. }))).expect("a fighter is offered");
    let mut q = asked(advance_ask(&mut s, &e, Some(Pick::Opt(fighter))));
    while matches!(q.ask, Ask::Production(PAsk::Pay(_))) {
        let pay = q.opts.as_slice().iter().copied().find(|o| matches!(o, Opt::Production(POpt::TradeGood { .. }))).expect("a trade good pays");
        q = asked(advance_ask(&mut s, &e, Some(Pick::Opt(pay))));
    }
    assert!(matches!(q.ask, Ask::Tactical(_)), "the enforcement asks which unit goes: {:?}", q.ask);
    assert_eq!(q.actor, Seat(0));
    let remove = q.pick(0).expect("a unit to remove");
    assert_eq!(advance_ask(&mut s, &e, Some(remove)).unwrap().0, Live::Done);
    assert_eq!(s.board().sys(system).units.as_slice().iter().filter(|p| p.owner() == Seat(0)).count(), 3, "one fighter was removed");
    assert_eq!(s.pending().prepared_turn_seq, Some(0));
}
