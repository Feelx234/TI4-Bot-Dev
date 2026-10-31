//! Lane C, KS1 to KS5: the status phase's arms (`driver::windows::{status, scoring, status_bookkeeping, status_finish}`) run the way the
//! live loop runs them: a held window, `advance` to a question or the end, a clone of the position standing for a checkpoint. The lockstep
//! against the string steps is `ti4-engine`'s `status_live_tests`; these tests pin what the arms write, announce and refuse.

use std::rc::Rc;

use ti4_engine_core::choice::objectives::{Offer, Opt as OOpt};
use ti4_engine_core::choice::strategy::{Ask as SAsk, Opt as SOpt, Pool};
use ti4_engine_core::containers::Cow;
use ti4_engine_core::driver::live::{advance, ask, is_open, Env, Live, LiveError, Out};
use ti4_engine_core::driver::turn::TurnFlowWin;
use ti4_engine_core::driver::windows::status::{advance_scoring, advance_status, advance_tokens, ask_scoring, ask_tokens};
use ti4_engine_core::driver::NoSeeds;
use ti4_engine_core::kernel::ability::Unknown;
use ti4_engine_core::kernel::stage::AbilityTable;
use ti4_engine_core::kernel::turn_codes::TurnGap;
use ti4_engine_core::kernel::Registry;
use ti4_engine_core::state::{DriverFlags, Emit, EventKind, LeaderStatus, ObjectiveRef, PlainArg, PlainKind, PlainRec, PlanetFlags, WindowKind};
use ti4_engine_core_tests::prelude::*;

fn env(k: &Kernel) -> Env<'_> {
    let mut table = AbilityTable::default();
    ti4_engine_core::driver::children::register(&mut table);
    // Every card its own printed name.
    table.facts.card_groups = (0..ActionCard::COUNT).map(|i| u8::try_from(i).unwrap_or(u8::MAX)).collect();
    Env { kernel: k, registry: Cow::new(Registry::default()), table, seeds: Rc::new(NoSeeds) }
}

fn plain(s: &CoreState) -> Vec<PlainRec> {
    s.ledger.events.iter().filter_map(|e| e.plain().copied()).collect()
}

fn typed(s: &CoreState) -> Vec<EventKind> {
    s.ledger.events.iter().filter_map(|e| if let Emit::Typed(t) = e { Some(t.kind) } else { None }).collect()
}

fn kinds(s: &CoreState) -> Vec<PlainKind> {
    plain(s).iter().map(|p| p.kind).collect()
}

fn gap(why: TurnGap) -> Result<(Live, Out), LiveError> {
    Err(LiveError::Unknown(Unknown::TurnGap(why)))
}

fn status(factions: usize) -> (CoreState, Kernel) {
    let (mut s, k) = Scenario::lobby(factions).phase(Phase::Status).active(None).build();
    // Three objectives to reveal, cards to draw.
    s.decks_mut().objective.set(vec![PublicObjective::ALL[0], PublicObjective::ALL[1], PublicObjective::ALL[2]]);
    s.decks_mut().action.set(ActionCard::ALL.iter().copied().take(30).collect());
    (s, k)
}

fn pool(p: Pool) -> Pick {
    Pick::Opt(Opt::Strategy(SOpt::Pool(p)))
}

// -- KS2: the opening ---------------------------------------------------------------------------------------------

#[test]
fn the_status_phase_opens_its_scoring_window_in_initiative_order() {
    let (mut s, k) = status(3);
    let (live, _) = advance_status(&mut s, &env(&k), None).unwrap();
    assert_eq!(live, Live::Done);
    assert!(s.pending().driver.has(DriverFlags::STATUS_RESOLVED), "the flag the step loop reads");
    assert!(is_open::<ti4_engine_core::driver::windows::scoring::StatusScoring>(&s));
    assert!(!is_open::<TurnFlowWin>(&s), "the job left the stack");
    assert_eq!(kinds(&s), [PlainKind::StatusScoringBegan]);
    let Some(ti4_engine_core::state::Window::Scoring(w)) = s.pending().window(WindowKind::Scoring) else { panic!("the window") };
    assert_eq!(w.pending.len(), 3);
    assert_eq!(w.timing, ti4_engine_core::state::ScoringTiming::Status);
}

#[test]
fn a_ship_in_an_entropic_scar_is_a_counted_gap_and_leaves_the_position_alone() {
    let scar = System::ALL.iter().copied().find(|s| s.data().anomalies.scar).expect("a scar system");
    let (mut s, k) = Scenario::lobby(3).phase(Phase::Status).unit(scar, None, Unit::CruiserI, 0, 1).build();
    let held = s.clone();
    assert_eq!(advance_status(&mut s, &env(&k), None), gap(TurnGap::EntropicScar));
    assert_eq!(s, held);
}

// -- KS1, KS3: scoring, then the bookkeeping --------------------------------------------------------------------------------

#[test]
fn a_window_nobody_can_score_in_closes_into_the_bookkeeping_in_one_call() {
    let (mut s, k) = status(3);
    let e = env(&k);
    advance_status(&mut s, &e, None).unwrap();
    assert_eq!(ask_scoring(&s, &e).unwrap(), None, "nobody has anything to score");
    let (live, _) = advance_scoring(&mut s, &e, None).unwrap();
    assert_eq!(live, Live::Done);
    assert!(!is_open::<ti4_engine_core::driver::windows::scoring::StatusScoring>(&s));
    assert!(is_open::<ti4_engine_core::rules::token_gain::TokenGain>(&s), "the token window is open");
    assert_eq!(s.objectives().revealed.len(), 1, "81.2: one objective revealed");
    for p in s.players().iter() {
        assert_eq!(p.cards.hand.len(), 1, "81.3: one card each (Scheming has its own test)");
    }
    assert!(kinds(&s).contains(&PlainKind::StatusBookkeepingResolved));
    let Some(ti4_engine_core::state::Window::Tokens(t)) = s.pending().window(WindowKind::Tokens) else { panic!("tokens") };
    assert_eq!(t.pending.len(), 6, "two tokens each");
    assert_eq!(t.report.action_cards_drawn.len(), 3);
}

#[test]
fn an_empty_objective_deck_ends_the_game_at_the_reveal() {
    let (mut s, k) = status(3);
    s.decks_mut().objective.set(Vec::new());
    let e = env(&k);
    advance_status(&mut s, &e, None).unwrap();
    let (live, _) = advance_scoring(&mut s, &e, None).unwrap();
    assert_eq!(live, Live::Done);
    assert!(s.meta().finished);
    assert_eq!(plain(&s).last().map(|p| p.kind), Some(PlainKind::GameFinished));
    assert!(!is_open::<ti4_engine_core::rules::token_gain::TokenGain>(&s), "no token window after the end");
}

/// Seat 0 holds a flagship on the board and Engineer a Marvel is revealed: it is asked, scores, and the score is announced.
fn marvel() -> (CoreState, Kernel) {
    let planet = Planet::ALL.iter().copied().find(|p| p.data().system.is_some()).unwrap();
    let system = planet.data().system.unwrap();
    let (mut s, k) = Scenario::lobby(2).phase(Phase::Status).active(None).unit(system, None, Unit::Genesis, 0, 1).build();
    s.decks_mut().objective.set(vec![PublicObjective::ALL[0]]);
    s.objectives_mut().revealed.push(ObjectiveRef::Public(PublicObjective::EngineerAMarvel));
    (s, k)
}

#[test]
fn a_seat_that_can_score_is_asked_scores_and_the_window_goes_on_to_the_close() {
    let (mut s, k) = marvel();
    let e = env(&k);
    advance_status(&mut s, &e, None).unwrap();
    let q = ask_scoring(&s, &e).unwrap().expect("seat 0 may score").choice;
    assert_eq!(q.actor, Seat(0));
    assert!(q.opts.iter().any(|o| *o == Opt::Objectives(OOpt::Score(ObjectiveRef::Public(PublicObjective::EngineerAMarvel)))));
    assert!(q.can_decline());
    // Declining ends the player's turn; the close follows in the same call.
    let mut declined = s.clone();
    let (live, _) = advance_scoring(&mut declined, &e, Some(Pick::Opt(Opt::Decline))).unwrap();
    assert_eq!(live, Live::Done);
    assert_eq!(declined.players().by_seat(Seat(0)).unwrap().econ.victory_points, 0);
    // Scoring.
    let score = Pick::Opt(Opt::Objectives(OOpt::Score(ObjectiveRef::Public(PublicObjective::EngineerAMarvel))));
    let mut fork = s.clone();
    for st in [&mut s, &mut fork] {
        let (live, _) = advance_scoring(st, &e, Some(score)).unwrap();
        assert_eq!(live, Live::Done);
    }
    assert_eq!(s, fork, "a clone continues identically");
    assert_eq!(s.players().by_seat(Seat(0)).unwrap().econ.victory_points, 1);
    assert!(s.objectives().scored[0].public.contains(PublicObjective::EngineerAMarvel));
    let order = kinds(&s);
    let scored = order.iter().position(|k| *k == PlainKind::ObjectiveScored).expect("announced");
    assert!(scored < order.iter().position(|k| *k == PlainKind::StatusBookkeepingResolved).unwrap(), "81.1 before the bookkeeping");
}

#[test]
fn an_answer_that_is_no_option_leaves_the_window_as_it_was() {
    let (mut s, k) = marvel();
    let e = env(&k);
    advance_status(&mut s, &e, None).unwrap();
    let held = s.clone();
    let bogus = Pick::Opt(Opt::Objectives(OOpt::Score(ObjectiveRef::Public(PublicObjective::ExpandBorders))));
    assert_eq!(advance_scoring(&mut s, &e, Some(bogus)), Err(LiveError::Illegal));
    assert_eq!(s, held);
}

#[test]
fn the_win_ends_the_game_without_the_close() {
    let (mut s, k) = marvel();
    s.players_mut().edit(0).econ.victory_points = 9;
    let e = env(&k);
    advance_status(&mut s, &e, None).unwrap();
    let score = Pick::Opt(Opt::Objectives(OOpt::Score(ObjectiveRef::Public(PublicObjective::EngineerAMarvel))));
    let (live, _) = advance_scoring(&mut s, &e, Some(score)).unwrap();
    assert_eq!(live, Live::Done);
    assert!(s.meta().finished);
    assert_eq!(kinds(&s)[kinds(&s).len() - 2..], [PlainKind::ObjectiveScored, PlainKind::GameFinished]);
    assert!(!is_open::<ti4_engine_core::rules::token_gain::TokenGain>(&s), "nothing is closed after the end");
}

#[test]
fn the_third_scored_objective_unlocks_the_hero() {
    let (mut s, k) = marvel();
    let hero = Leader::ALL.iter().copied().find(|l| l.data().faction == FactionTag::Sol && l.data().kind == LeaderType::Hero).unwrap();
    s.players_mut().edit(0).leaders.status.insert(hero, LeaderStatus::Locked);
    s.objectives_mut().revealed.push(ObjectiveRef::Public(PublicObjective::ExpandBorders));
    for o in [PublicObjective::ExpandBorders, PublicObjective::ALL[3]] {
        ti4_engine_core::kernel::fx::record_score(&mut s, Seat(0), ObjectiveRef::Public(o));
    }
    let e = env(&k);
    advance_status(&mut s, &e, None).unwrap();
    let score = Pick::Opt(Opt::Objectives(OOpt::Score(ObjectiveRef::Public(PublicObjective::EngineerAMarvel))));
    advance_scoring(&mut s, &e, Some(score)).unwrap();
    assert_eq!(s.players().by_seat(Seat(0)).unwrap().leaders.status.get(&hero), Some(&LeaderStatus::Unlocked));
}

#[test]
fn a_locked_commander_whose_condition_does_not_hold_is_judged_and_stays_locked_without_a_gap() {
    // (Lane F2 filled the unlock hook of every in-scope faction: the gap `TurnGap::CommanderUnlock` of lane C is never raised.)
    let (mut s, k) = Scenario::new(&[Faction::TheArborec, Faction::TheEmiratesOfHacan]).phase(Phase::Status).active(None).build();
    s.decks_mut().objective.set(vec![PublicObjective::ALL[0]]);
    let commander = Leader::ALL.iter().copied().find(|l| l.data().faction == FactionTag::Arborec && l.data().kind == LeaderType::Commander).unwrap();
    s.players_mut().edit(0).leaders.status.insert(commander, LeaderStatus::Locked);
    let e = env(&k);
    advance_status(&mut s, &e, None).unwrap();
    assert!(advance_scoring(&mut s, &e, None).is_ok(), "no counted gap for a commander");
    assert_eq!(s.players().by_seat(Seat(0)).unwrap().leaders.status.get(&commander), Some(&LeaderStatus::Locked));
}

// -- KS4, KS5: the tokens and the close ----------------------------------------------------------------------------------------

fn at_tokens() -> (CoreState, Kernel) {
    let (mut s, k) = status(2);
    let e = env(&k);
    advance_status(&mut s, &e, None).unwrap();
    advance_scoring(&mut s, &e, None).unwrap();
    (s, k)
}

#[test]
fn the_last_token_closes_into_the_redistribution_and_the_end_of_the_phase() {
    let (mut s, k) = at_tokens();
    let e = env(&k);
    let (pa, pb) = (s.players().by_seat(Seat(0)).unwrap().econ.clone(), s.players().by_seat(Seat(1)).unwrap().econ.clone());
    // Four tokens: asked one at a time, the seat that holds them first.
    for i in 0..3 {
        let (live, _) = advance_tokens(&mut s, &e, Some(pool(Pool::Strategic))).map(|(l, o)| (l, o)).unwrap_or_else(|_| panic!("token {i}"));
        let Live::Asks(_) = live else { panic!("another token is owed") };
    }
    // The last token ends the window and asks the first redistribution in the same call.
    let (live, _) = advance_tokens(&mut s, &e, Some(pool(Pool::Strategic))).unwrap();
    let Live::Asks(q) = live else { panic!("the redistribution of the first seat") };
    assert!(matches!(q.choice.ask, Ask::Strategy(SAsk::StatusRedistribute { .. })), "{:?}", q.choice.ask);
    assert_eq!(q.choice.actor, Seat(0));
    assert!(is_open::<TurnFlowWin>(&s), "the close is suspended on its question");
    assert!(s.pending().nested.is_some());
    let _ = (pa, pb);
    // A clone mid-close continues identically.
    let mut fork = s.clone();
    assert_eq!(ask_tokens(&fork, &e).unwrap().unwrap().choice, q.choice);
    let first = q.choice.pick(0).unwrap();
    for st in [&mut s, &mut fork] {
        let (live, _) = advance_tokens(st, &e, Some(first)).unwrap();
        let Live::Asks(second) = live else { panic!("the second seat's redistribution") };
        assert_eq!(second.choice.actor, Seat(1));
        let (live, _) = advance_tokens(st, &e, Some(second.choice.pick(0).unwrap())).unwrap();
        assert_eq!(live, Live::Done);
    }
    assert_eq!(s, fork, "a clone mid-window continues identically");
    assert!(!is_open::<TurnFlowWin>(&s) && !is_open::<ti4_engine_core::rules::token_gain::TokenGain>(&s));
    let order = kinds(&s);
    assert_eq!(order[order.len() - 2..], [PlainKind::CommandTokensGained, PlainKind::StatusPhaseResolved]);
    assert_eq!(typed(&s).last(), Some(&EventKind::StatusPhaseEnded));
}

#[test]
fn the_close_readies_repairs_and_returns_the_strategy_cards_except_a_retained_one() {
    let planet = Planet::ALL.iter().copied().find(|p| p.data().system.is_some()).unwrap();
    let system = planet.data().system.unwrap();
    let (mut s, k) = Scenario::lobby(2).phase(Phase::Status).active(None).control(planet, 0).exhaust(planet).unit(system, None, Unit::DreadnoughtI, 0, 1).build();
    s.decks_mut().objective.set(vec![PublicObjective::ALL[0]]);
    s.board_mut().sys_mut(system).units.as_mut_slice().iter_mut().for_each(|u| *u = u.with_sustained(true));
    for (seat, card) in [(0usize, StrategyCard::Leadership), (1, StrategyCard::Politics)] {
        s.players_mut().edit(seat).strat = [card].into_iter().collect();
        s.players_mut().edit(seat).econ.passed = true;
    }
    s.players_mut().edit(1).econ.stability = true;
    s.players_mut().edit(1).strat_exhausted.insert(StrategyCard::Politics);
    let e = env(&k);
    advance_status(&mut s, &e, None).unwrap();
    advance_scoring(&mut s, &e, None).unwrap();
    let mut live = advance_tokens(&mut s, &e, None);
    let mut guard = 0;
    while let Ok((Live::Asks(q), _)) = &live {
        guard += 1;
        assert!(guard < 40);
        let pick = q.choice.pick(0).unwrap();
        live = advance_tokens(&mut s, &e, Some(pick));
    }
    assert_eq!(live.unwrap().0, Live::Done);
    assert!(!s.board().planet(planet).flags.has(PlanetFlags::EXHAUSTED), "81.6: the planet readies");
    assert!(s.board().sys(system).units.as_slice().iter().all(|u| !u.sustained()), "81.7: the unit repairs");
    assert!(s.players().by_seat(Seat(0)).unwrap().strat.is_empty(), "81.8: the card goes back");
    assert_eq!(s.players().by_seat(Seat(1)).unwrap().strat.as_slice(), [StrategyCard::Politics], "Political Stability keeps it");
    assert!(s.players().by_seat(Seat(1)).unwrap().strat_exhausted.is_empty(), "and readies it");
    assert!(s.players().iter().all(|p| !p.econ.passed));
    assert!(typed(&s).contains(&EventKind::StrategyCardsWouldReturn));
}

#[test]
fn asking_an_arm_whose_window_is_not_open_asks_nothing() {
    let (s, k) = status(2);
    assert_eq!(ask_scoring(&s, &env(&k)), Err(LiveError::NotOpen));
    assert_eq!(ask::<ti4_engine_core::driver::windows::scoring::StatusScoring>(&s, &env(&k)), Err(LiveError::NotOpen));
    let _ = advance::<ti4_engine_core::driver::windows::scoring::StatusScoring>;
}

// -- Scheming, the hand limit, the staged discard ----------------------------------------------------------------------------

fn yssaril_table() -> (CoreState, Kernel) {
    let (mut s, k) = Scenario::new(&[Faction::TheFederationOfSol, Faction::TheYssarilTribes]).phase(Phase::Status).active(None).build();
    s.decks_mut().objective.set(vec![PublicObjective::ALL[0]]);
    s.decks_mut().action.set(ActionCard::ALL.iter().copied().skip(10).take(30).collect());
    s.players_mut().edit(1).cards.hand = [ActionCard::ALL[1], ActionCard::ALL[2]].into_iter().collect();
    for seat in 0..2 {
        s.players_mut().edit(seat).strat = [StrategyCard::Leadership].into_iter().collect();
    }
    (s, k)
}

#[test]
fn scheming_draws_one_more_and_asks_which_card_to_discard_and_a_clone_continues_identically() {
    let (mut s, k) = yssaril_table();
    let e = env(&k);
    advance_status(&mut s, &e, None).unwrap();
    let before = s.players().by_seat(Seat(1)).unwrap().cards.hand.len();
    let (live, _) = advance_scoring(&mut s, &e, None).unwrap();
    let Live::Asks(q) = live else { panic!("Yssaril draws two cards and discards one: a question") };
    assert_eq!(q.choice.ask, Ask::Cards(ti4_engine_core::choice::cards::Ask::OwnHand { reason: ti4_engine_core::choice::cards::Reason::Scheming }));
    assert_eq!(q.choice.actor, Seat(1));
    assert!(!q.choice.can_decline(), "the discard is mandatory");
    assert!(is_open::<TurnFlowWin>(&s) && s.pending().nested.is_some(), "the close is suspended on its question");
    // Nothing of the half-run job is kept.
    assert_eq!(s.players().by_seat(Seat(1)).unwrap().cards.hand.len(), before);
    let mut fork = s.clone();
    assert_eq!(ask_scoring(&fork, &e).unwrap().unwrap().choice, q.choice, "a replay of the suspension asks the same question");
    let discard = q.choice.pick(0).unwrap();
    for st in [&mut s, &mut fork] {
        let (live, _) = advance_scoring(st, &e, Some(discard)).unwrap();
        assert_eq!(live, Live::Done);
    }
    assert_eq!(s, fork, "a clone mid-window continues identically");
    assert_eq!(s.players().by_seat(Seat(1)).unwrap().cards.hand.len(), before + 1, "two drawn, one discarded");
    assert!(is_open::<ti4_engine_core::rules::token_gain::TokenGain>(&s));
}

#[test]
fn the_staged_discard_is_announced_after_the_first_typed_event_of_the_close() {
    let (mut s, k) = yssaril_table();
    let e = env(&k);
    advance_status(&mut s, &e, None).unwrap();
    let (live, _) = advance_scoring(&mut s, &e, None).unwrap();
    let Live::Asks(q) = live else { panic!("the discard") };
    advance_scoring(&mut s, &e, Some(q.choice.pick(0).unwrap())).unwrap();
    let mut live = advance_tokens(&mut s, &e, None);
    let mut guard = 0;
    while let Ok((Live::Asks(q), _)) = &live {
        guard += 1;
        assert!(guard < 40);
        let pick = q.choice.pick(0).unwrap();
        live = advance_tokens(&mut s, &e, Some(pick));
    }
    assert_eq!(live.unwrap().0, Live::Done);
    let order = typed(&s);
    let would = order.iter().position(|k| *k == EventKind::StrategyCardsWouldReturn).expect("announced");
    let discarded = order.iter().position(|k| *k == EventKind::ActionCardDiscarded).expect("the staged discard is announced, not kept");
    assert_eq!(discarded, would + 1, "right after the typed event that follows the draw");
    assert!(s.marks().tail.iter().all(|(k, _)| !matches!(k, ti4_engine_core::keys::MarkKey::CardsStaged(_))), "the marks are gone");
    assert_eq!(s.transient().combat.last_action_discarded.map(|(who, _)| who), Some(Seat(1)));
    assert_eq!(s.cards().discarded_action.len(), 1);
}

#[test]
fn a_hand_over_its_limit_after_the_draw_is_asked_to_discard_one() {
    let (mut s, k) = Scenario::lobby(2).phase(Phase::Status).active(None).build();
    s.decks_mut().objective.set(vec![PublicObjective::ALL[0]]);
    s.decks_mut().action.set(ActionCard::ALL.iter().copied().skip(20).take(30).collect());
    s.players_mut().edit(0).cards.hand = ActionCard::ALL.iter().copied().take(7).collect();
    let e = env(&k);
    advance_status(&mut s, &e, None).unwrap();
    let (live, _) = advance_scoring(&mut s, &e, None).unwrap();
    let Live::Asks(q) = live else { panic!("eight cards: discard one") };
    assert!(matches!(q.choice.ask, Ask::Cards(ti4_engine_core::choice::cards::Ask::HandLimit { held: 8 })), "{:?}", q.choice.ask);
    // Before rules version 17 the status draw is not subject to the hand limit.
    let (mut old, k) = Scenario::lobby(2).phase(Phase::Status).active(None).rules(16).build();
    old.decks_mut().objective.set(vec![PublicObjective::ALL[0]]);
    old.decks_mut().action.set(ActionCard::ALL.iter().copied().skip(20).take(30).collect());
    old.players_mut().edit(0).cards.hand = ActionCard::ALL.iter().copied().take(7).collect();
    let e = env(&k);
    advance_status(&mut old, &e, None).unwrap();
    assert_eq!(advance_scoring(&mut old, &e, None).unwrap().0, Live::Done);
    assert_eq!(old.players().by_seat(Seat(0)).unwrap().cards.hand.len(), 8);
}

#[test]
fn the_neural_motivator_draws_a_second_card() {
    let (mut s, k) = Scenario::lobby(2).phase(Phase::Status).active(None).tech(0, Technology::NeuralMotivator).build();
    s.decks_mut().objective.set(vec![PublicObjective::ALL[0]]);
    s.decks_mut().action.set(ActionCard::ALL.iter().copied().skip(20).take(30).collect());
    let e = env(&k);
    advance_status(&mut s, &e, None).unwrap();
    advance_scoring(&mut s, &e, None).unwrap();
    assert_eq!(s.players().by_seat(Seat(0)).unwrap().cards.hand.len(), 2);
    assert_eq!(s.players().by_seat(Seat(1)).unwrap().cards.hand.len(), 1);
}

#[test]
fn the_command_tokens_on_the_board_come_back_at_81_4() {
    let planet = Planet::ALL.iter().copied().find(|p| p.data().system.is_some()).unwrap();
    let system = planet.data().system.unwrap();
    let (mut s, k) = Scenario::lobby(2).phase(Phase::Status).active(None).command_token(system, 0).command_token(system, 1).build();
    s.decks_mut().objective.set(vec![PublicObjective::ALL[0]]);
    let e = env(&k);
    advance_status(&mut s, &e, None).unwrap();
    advance_scoring(&mut s, &e, None).unwrap();
    assert!(s.board().sys(system).cmd.is_empty());
    let Some(ti4_engine_core::state::Window::Tokens(t)) = s.pending().window(WindowKind::Tokens) else { panic!("tokens") };
    assert_eq!(t.report.returned_command_tokens.as_slice(), [(system, Seat(0)), (system, Seat(1))]);
}

// -- the redistribution, the event window's caps, the offers (survivors of the mutation runs) --------------------------------------

use ti4_engine_core::driver::windows::scoring::{open_event, CoreOffers, EventScoring, StatusScoring};
use ti4_engine_core::rules::scoring_book::Offers;
use ti4_engine_core::kernel::stage::Held;
use ti4_engine_core::state::{Feat, ScoringTiming, Tokens};

/// A closing token window with nothing left to place: the next call runs the finish job to its first question.
fn finish_of(factions: &[Faction], rules: u32, tokens: (i16, i16, i16)) -> (CoreState, Kernel) {
    let mut sc = Scenario::new(factions).phase(Phase::Status).active(None).rules(rules).tokens(0, tokens.0, tokens.1, tokens.2);
    if tokens == (0, 0, 0) {
        sc = sc.tokens(1, 0, 0, 0);
    }
    let (mut s, k) = sc.build();
    ti4_engine_core::rules::token_gain::TokenGain::new(Tokens { pending: Default::default(), placed: Default::default(), report: Default::default() }).to_pending(&mut s);
    (s, k)
}

fn ask_redistribution(s: &mut CoreState, k: &Kernel) -> Option<(u8, u8)> {
    match advance_tokens(s, &env(k), None).unwrap().0 {
        Live::Asks(q) => match q.choice.ask {
            Ask::Strategy(SAsk::StatusRedistribute { total, floor }) => {
                assert_eq!(q.choice.actor, Seat(0));
                assert!(!q.choice.opts.is_empty());
                assert!(!q.choice.ctx.optional && !q.choice.ctx.space_battle && q.choice.ctx.invasion_seq.is_none() && q.choice.ctx.target.is_none(), "a plain mandatory question");
                Some((total, floor))
            }
            other => panic!("{other:?}"),
        },
        Live::Done => None,
    }
}

#[test]
fn the_redistribution_asks_the_whole_sheet_with_the_floor_of_the_faction_and_the_version() {
    let (mut s, k) = finish_of(&[Faction::TheFederationOfSol, Faction::TheEmiratesOfHacan], 40, (3, 3, 2));
    assert_eq!(ask_redistribution(&mut s, &k), Some((8, 2)), "two in the fleet pool at least");
    let (mut s, k) = finish_of(&[Faction::TheBaronyOfLetnev, Faction::TheEmiratesOfHacan], 40, (3, 3, 2));
    assert_eq!(ask_redistribution(&mut s, &k), Some((8, 0)), "Letnev may empty its fleet pool from rules version 4");
    let (mut s, k) = finish_of(&[Faction::TheBaronyOfLetnev, Faction::TheEmiratesOfHacan], 3, (3, 3, 2));
    assert_eq!(ask_redistribution(&mut s, &k), Some((8, 2)), "before the version the usual floor");
    let (mut s, k) = finish_of(&[Faction::TheFederationOfSol, Faction::TheEmiratesOfHacan], 40, (0, 0, 0));
    assert_eq!(advance_tokens(&mut s, &env(&k), None).map(|r| r.0.clone()).unwrap_or(Live::Done), Live::Done, "an empty sheet asks nothing (the other seat has the default tokens)");
}

#[test]
fn an_event_window_opens_with_its_occurrence_and_caps() {
    let (mut s, _) = Scenario::lobby(3).phase(Phase::Action).build();
    open_event(&mut s, ScoringTiming::Action, 7, true);
    let Some(ti4_engine_core::state::Window::EventScoring(w)) = s.pending().window(WindowKind::EventScoring) else { panic!("open") };
    assert_eq!((w.timing, w.occurrence, w.one_per_player, w.pending.len()), (ScoringTiming::Action, Some(7), true, 3));
    let (mut s, _) = Scenario::lobby(3).phase(Phase::Status).build();
    ti4_engine_core::driver::live::open(&mut s, &StatusScoring::status(&[Seat(0), Seat(1)]));
    let Some(ti4_engine_core::state::Window::Scoring(w)) = s.pending().window(WindowKind::Scoring) else { panic!("open") };
    assert_eq!((w.timing, w.occurrence, w.one_per_player, w.pending.len()), (ScoringTiming::Status, None, false, 2));
    let _ = EventScoring::event(&[Seat(0)], ScoringTiming::Agenda, 1, false);
}

fn pe_table(version: u32) -> (CoreState, Kernel) {
    let (mut s, k) = Scenario::lobby(2).phase(Phase::Action).rules(version).build();
    s.players_mut().edit(0).cards.secrets = [SecretObjective::ProveEndurance].into_iter().collect();
    s.players_mut().edit(0).misc.event_feats.push((Feat::LastToPass, 3));
    (s, k)
}

#[test]
fn a_secret_is_offered_in_the_window_of_the_occurrence_its_feat_was_recorded_for_and_only_once_per_player() {
    let pe = Pick::Opt(Opt::Objectives(OOpt::Score(ObjectiveRef::Secret(SecretObjective::ProveEndurance))));
    let (mut s, k) = pe_table(40);
    open_event(&mut s, ScoringTiming::Action, 3, true);
    let e = env(&k);
    let q = ask::<EventScoring>(&s, &e).unwrap().expect("the feat of occurrence 3").choice;
    assert_eq!(q.actor, Seat(0));
    // Another occurrence: the feat does not count.
    let (mut other, k2) = pe_table(40);
    open_event(&mut other, ScoringTiming::Action, 4, true);
    assert!(ask::<EventScoring>(&other, &env(&k2)).unwrap().is_none());
    // The seat used its one score of this occurrence.
    let (mut used, k3) = pe_table(40);
    ti4_engine_core::kernel::fx::record_occurrence_score(&mut used, Seat(0), 3);
    open_event(&mut used, ScoringTiming::Action, 3, true);
    assert!(ask::<EventScoring>(&used, &env(&k3)).unwrap().is_none(), "capped by the occurrence");
    let (mut any, k4) = pe_table(40);
    ti4_engine_core::kernel::fx::record_occurrence_score(&mut any, Seat(0), 3);
    open_event(&mut any, ScoringTiming::Action, 3, false);
    assert!(ask::<EventScoring>(&any, &env(&k4)).unwrap().is_some(), "not capped when any number may score");
    // Scoring it: the secret leaves the hand, the occurrence is recorded, and the window closes (nobody else has a feat).
    let (live, _) = advance_scoring_event(&mut s, &e, pe);
    assert_eq!(live, Live::Done);
    assert!(s.players().by_seat(Seat(0)).unwrap().cards.secrets.is_empty());
    assert!(s.objectives().scored_feats.iter().any(|f| *f == (Seat(0), 3)));
    // A wrong-timed secret is not offered in a window of another phase.
    let (mut agenda, k5) = pe_table(40);
    open_event(&mut agenda, ScoringTiming::Agenda, 3, true);
    assert!(ask::<EventScoring>(&agenda, &env(&k5)).unwrap().is_none());
}

fn advance_scoring_event(s: &mut CoreState, e: &Env<'_>, pick: Pick) -> (Live, Out) {
    ti4_engine_core::driver::windows::status::advance_event_scoring(s, e, Some(pick)).unwrap()
}

#[test]
fn a_leaked_event_secret_keeps_its_timing_from_the_rules_version_that_says_so() {
    for (version, offered) in [(40, true), (24, true), (23, false)] {
        let (mut s, k) = Scenario::lobby(2).phase(Phase::Action).rules(version).build();
        s.objectives_mut().revealed.push(ObjectiveRef::Secret(SecretObjective::ProveEndurance));
        s.players_mut().edit(1).misc.event_feats.push((Feat::LastToPass, 3));
        open_event(&mut s, ScoringTiming::Action, 3, true);
        let asked = ask::<EventScoring>(&s, &env(&k)).unwrap();
        assert_eq!(asked.is_some(), offered, "rules version {version}");
        if offered {
            assert_eq!(asked.unwrap().choice.actor, Seat(1));
        }
    }
}

#[test]
fn a_status_secret_is_offered_by_its_position_not_by_a_feat_and_not_twice() {
    let planet = Planet::ALL.iter().copied().find(|p| p.data().legendary && p.data().system.is_some()).expect("a legendary planet");
    let build = |held: bool| {
        let sc = Scenario::lobby(2).phase(Phase::Status).active(None);
        let (mut s, k) = if held { sc.control(planet, 0).build() } else { sc.build() };
        s.players_mut().edit(0).cards.secrets = [SecretObjective::SeizeAnIcon, SecretObjective::ProveEndurance].into_iter().collect();
        (s, k)
    };
    let (mut s, k) = build(true);
    ti4_engine_core::driver::live::open(&mut s, &StatusScoring::status(&[Seat(0), Seat(1)]));
    let q = ask::<StatusScoring>(&s, &env(&k)).unwrap().expect("Seize an Icon").choice;
    let offered: Vec<_> = q.opts.iter().filter(|o| matches!(o, Opt::Objectives(OOpt::Score(_)))).collect();
    assert_eq!(offered, [&Opt::Objectives(OOpt::Score(ObjectiveRef::Secret(SecretObjective::SeizeAnIcon)))], "the action secret is not a status offer");
    let (mut s2, k2) = build(false);
    ti4_engine_core::driver::live::open(&mut s2, &StatusScoring::status(&[Seat(0), Seat(1)]));
    assert!(ask::<StatusScoring>(&s2, &env(&k2)).unwrap().is_none(), "the position does not satisfy it");
    // Already scored: not offered again.
    let (mut s3, k3) = build(true);
    ti4_engine_core::kernel::fx::record_score(&mut s3, Seat(0), ObjectiveRef::Secret(SecretObjective::SeizeAnIcon));
    ti4_engine_core::driver::live::open(&mut s3, &StatusScoring::status(&[Seat(0), Seat(1)]));
    assert!(ask::<StatusScoring>(&s3, &env(&k3)).unwrap().is_none());
    let mut offers = CoreOffers::new(&s, &k, ScoringTiming::Status, None);
    assert!(!offers.scored_at_occurrence(Seat(0)) && offers.plots(Seat(0)).is_empty());
    assert_eq!(offers.secrets(Seat(0)).as_slice(), [SecretObjective::SeizeAnIcon]);
    assert!(!offers.unsupported);
    // Strengthen Bonds cannot be judged by the core: the window stops with a counted gap.
    let (mut s4, k4) = build(true);
    s4.players_mut().edit(0).cards.secrets = [SecretObjective::StrengthenBonds].into_iter().collect();
    ti4_engine_core::driver::live::open(&mut s4, &StatusScoring::status(&[Seat(0)]));
    let held = s4.clone();
    assert_eq!(advance_scoring(&mut s4, &env(&k4), None), gap(TurnGap::ScoringSecret));
    assert_eq!(s4, held);
}

#[test]
fn an_event_window_that_leaves_the_agenda_queue_behind_closes_into_the_next_reveal() {
    // Lane D took the queue: the window's close runs the job that reveals the agendas held behind it (here the deck is empty and the
    // agenda elects a law nobody holds: nothing goes to a vote and the phase ends).
    let (mut s, k) = Scenario::lobby(2).phase(Phase::Agenda).build();
    open_event(&mut s, ScoringTiming::Agenda, 1, true);
    s.pending_mut().open(ti4_engine_core::state::Window::AgendaQueueAfterEventScoring([Agenda::ALL[0]].into_iter().collect()));
    let (live, _) = ti4_engine_core::driver::windows::status::advance_event_scoring(&mut s, &env(&k), None).unwrap();
    assert_eq!(live, Live::Done);
    assert!(s.pending().window(ti4_engine_core::state::WindowKind::AgendaQueueAfterEventScoring).is_none(), "the queue was taken");
    assert!(s.pending().window(ti4_engine_core::state::WindowKind::TurnFlow).is_none() && s.pending().window(ti4_engine_core::state::WindowKind::Voting).is_none());
    let (mut plain, k) = Scenario::lobby(2).phase(Phase::Agenda).build();
    open_event(&mut plain, ScoringTiming::Agenda, 1, true);
    assert_eq!(ti4_engine_core::driver::windows::status::advance_event_scoring(&mut plain, &env(&k), None).unwrap().0, Live::Done);
}

#[test]
fn a_hand_exactly_at_the_limit_after_the_draw_asks_nothing_and_one_distinct_card_is_discarded_without_a_question() {
    let (mut s, k) = Scenario::lobby(2).phase(Phase::Status).active(None).build();
    s.decks_mut().objective.set(vec![PublicObjective::ALL[0]]);
    s.decks_mut().action.set(ActionCard::ALL.iter().copied().skip(20).take(30).collect());
    s.players_mut().edit(0).cards.hand = ActionCard::ALL.iter().copied().take(6).collect();
    let e = env(&k);
    advance_status(&mut s, &e, None).unwrap();
    assert_eq!(advance_scoring(&mut s, &e, None).unwrap().0, Live::Done, "seven cards are within the limit");
    // Scheming over a hand whose cards all share one printed name: nothing to choose.
    let (mut y, k) = yssaril_table();
    let mut e = env(&k);
    e.table.facts.card_groups = vec![0; ActionCard::COUNT];
    advance_status(&mut y, &e, None).unwrap();
    assert_eq!(advance_scoring(&mut y, &e, None).unwrap().0, Live::Done);
    assert_eq!(y.players().by_seat(Seat(1)).unwrap().cards.hand.len(), 3, "2 held + 2 drawn - 1 discarded");
    assert!(y.marks().tail.iter().any(|(k, _)| matches!(k, ti4_engine_core::keys::MarkKey::CardsStaged(_))), "the discard waits for the announcement");
}

#[test]
fn a_fresh_window_with_something_to_score_runs_to_its_question() {
    let (mut s, k) = marvel();
    let e = env(&k);
    advance_status(&mut s, &e, None).unwrap();
    let held = s.clone();
    let (live, _) = advance_scoring(&mut s, &e, None).unwrap();
    let Live::Asks(q) = live else { panic!("seat 0 is asked") };
    assert_eq!(q.choice.ask, Ask::Objectives(ti4_engine_core::choice::objectives::Ask::Score { vp: 0 }));
    assert_eq!(q.choice, ask_scoring(&held, &e).unwrap().unwrap().choice, "the stage question and the question the step puts are one");
    // The seat holds points: the question shows them.
    let mut rich = held.clone();
    rich.players_mut().edit(0).econ.victory_points = 4;
    let Live::Asks(q) = advance_scoring(&mut rich, &e, None).unwrap().0 else { panic!("asked") };
    assert_eq!(q.choice.ask, Ask::Objectives(ti4_engine_core::choice::objectives::Ask::Score { vp: 4 }));
    assert_eq!(q.choice, ask_scoring(&rich, &e).unwrap().unwrap().choice);
}

#[test]
fn the_close_readies_the_technologies_of_every_seat_and_repairs_the_units_on_planets() {
    let planet = Planet::ALL.iter().copied().find(|p| p.data().system.is_some()).unwrap();
    let (mut s, k) = Scenario::lobby(2).phase(Phase::Status).active(None).unit(planet.data().system.unwrap(), Some(planet), Unit::InfantryI, 0, 1).build();
    s.board_mut().planet_mut(planet).units.as_mut_slice().iter_mut().for_each(|u| *u = u.with_sustained(true));
    for seat in 0..2 {
        s.players_mut().edit(seat).tech.exhausted = [Technology::NeuralMotivator].into_iter().collect();
    }
    ti4_engine_core::rules::token_gain::TokenGain::new(Tokens { pending: Default::default(), placed: Default::default(), report: Default::default() }).to_pending(&mut s);
    let e = env(&k);
    let mut live = advance_tokens(&mut s, &e, None);
    while let Ok((Live::Asks(q), _)) = &live {
        let pick = q.choice.pick(0).unwrap();
        live = advance_tokens(&mut s, &e, Some(pick));
    }
    assert_eq!(live.unwrap().0, Live::Done);
    assert!(s.players().iter().all(|p| p.tech.exhausted.is_empty()), "every seat's technologies ready");
    assert!(s.board().planet(planet).units.as_slice().iter().all(|u| !u.sustained()), "81.7 on planets too");
}

#[test]
fn a_position_secret_of_an_event_window_is_offered_without_a_feat_and_a_leaked_status_secret_by_its_position() {
    // Dictate Policy (agenda timing): three laws in play satisfy it by position.
    let build = |laws: usize| {
        let (mut s, k) = Scenario::lobby(2).phase(Phase::Agenda).build();
        s.players_mut().edit(0).cards.secrets = [SecretObjective::DictatePolicy].into_iter().collect();
        for law in Agenda::ALL.iter().take(laws) {
            s.agenda_mut().laws.insert(*law, ti4_engine_core::keys::Outcome::For);
        }
        open_event(&mut s, ScoringTiming::Agenda, 1, true);
        (s, k)
    };
    let (s, k) = build(3);
    assert!(ask::<EventScoring>(&s, &env(&k)).unwrap().is_some());
    let (s, k) = build(2);
    assert!(ask::<EventScoring>(&s, &env(&k)).unwrap().is_none());
    // A status secret made public is scored by the position of its holder, and only while not scored.
    let planet = Planet::ALL.iter().copied().find(|p| p.data().legendary && p.data().system.is_some()).unwrap();
    let mk = |scored: bool| {
        let (mut s, k) = Scenario::lobby(2).phase(Phase::Status).active(None).control(planet, 0).build();
        s.objectives_mut().revealed.push(ObjectiveRef::Secret(SecretObjective::SeizeAnIcon));
        if scored {
            ti4_engine_core::kernel::fx::record_score(&mut s, Seat(0), ObjectiveRef::Secret(SecretObjective::SeizeAnIcon));
        }
        ti4_engine_core::driver::live::open(&mut s, &StatusScoring::status(&[Seat(0), Seat(1)]));
        (s, k)
    };
    let (s, k) = mk(false);
    let q = ask::<StatusScoring>(&s, &env(&k)).unwrap().expect("offered").choice;
    assert_eq!(q.actor, Seat(0));
    let (s, k) = mk(true);
    assert!(ask::<StatusScoring>(&s, &env(&k)).unwrap().is_none());
}

#[test]
fn a_seat_with_one_token_is_asked_to_redistribute_and_an_empty_deck_draws_nothing() {
    let (mut s, k) = finish_of(&[Faction::TheFederationOfSol, Faction::TheEmiratesOfHacan], 40, (1, 0, 0));
    assert_eq!(ask_redistribution(&mut s, &k), Some((1, 2)), "one token is still a sheet; the split list handles a total under the floor");
    let (mut s, k) = Scenario::lobby(2).phase(Phase::Status).active(None).build();
    s.decks_mut().objective.set(vec![PublicObjective::ALL[0]]);
    s.decks_mut().action.set(Vec::new());
    let e = env(&k);
    advance_status(&mut s, &e, None).unwrap();
    advance_scoring(&mut s, &e, None).unwrap();
    let Some(ti4_engine_core::state::Window::Tokens(t)) = s.pending().window(WindowKind::Tokens) else { panic!("tokens") };
    assert_eq!(t.report.action_cards_drawn.as_slice(), [(Seat(0), 0), (Seat(1), 0)]);
}

#[test]
fn a_revealed_event_secret_is_no_status_offer_and_a_held_one_is_offered_once() {
    // Prove Endurance has no position requirement: made public in the status window it is not scoreable.
    let (mut s, k) = Scenario::lobby(2).phase(Phase::Status).active(None).build();
    s.objectives_mut().revealed.push(ObjectiveRef::Secret(SecretObjective::ProveEndurance));
    ti4_engine_core::driver::live::open(&mut s, &StatusScoring::status(&[Seat(0), Seat(1)]));
    assert!(ask::<StatusScoring>(&s, &env(&k)).unwrap().is_none());
    // Held and revealed at once, with its feat recorded: one offer.
    let (mut s, k) = pe_table(40);
    s.objectives_mut().revealed.push(ObjectiveRef::Secret(SecretObjective::ProveEndurance));
    open_event(&mut s, ScoringTiming::Action, 3, true);
    let q = ask::<EventScoring>(&s, &env(&k)).unwrap().expect("offered").choice;
    let scores = q.opts.iter().filter(|o| matches!(o, Opt::Objectives(OOpt::Score(_)))).count();
    assert_eq!(scores, 1);
}

#[test]
fn a_leaked_secret_the_core_cannot_judge_stops_the_window_with_a_counted_gap() {
    let (mut s, k) = Scenario::lobby(2).phase(Phase::Status).active(None).build();
    s.objectives_mut().revealed.push(ObjectiveRef::Secret(SecretObjective::StrengthenBonds));
    ti4_engine_core::driver::live::open(&mut s, &StatusScoring::status(&[Seat(0)]));
    let held = s.clone();
    assert_eq!(advance_scoring(&mut s, &env(&k), None), gap(TurnGap::ScoringSecret));
    assert_eq!(s, held);
}

#[test]
fn scheming_without_the_printed_name_groups_is_a_counted_gap_and_a_public_leak_scores_as_public() {
    let (mut y, k) = yssaril_table();
    let mut e = env(&k);
    e.table.facts.card_groups = Vec::new();
    advance_status(&mut y, &e, None).unwrap();
    let held = y.clone();
    assert_eq!(advance_scoring(&mut y, &e, None), gap(TurnGap::NoRoster));
    assert_eq!(y, held);
    // A secret made public that its holder's position satisfies is scored as a public objective: the public cap is used, the hand untouched.
    let planet = Planet::ALL.iter().copied().find(|p| p.data().legendary && p.data().system.is_some()).unwrap();
    let (mut s, k) = Scenario::lobby(2).phase(Phase::Status).active(None).control(planet, 0).build();
    s.decks_mut().objective.set(vec![PublicObjective::ALL[0]]);
    s.objectives_mut().revealed.push(ObjectiveRef::Secret(SecretObjective::SeizeAnIcon));
    let e = env(&k);
    advance_status(&mut s, &e, None).unwrap();
    let pick = Pick::Opt(Opt::Objectives(OOpt::Score(ObjectiveRef::Secret(SecretObjective::SeizeAnIcon))));
    advance_scoring(&mut s, &e, Some(pick)).unwrap();
    assert_eq!(s.players().by_seat(Seat(0)).unwrap().econ.victory_points, 1);
}
