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
fn a_locked_commander_without_a_hook_stops_the_bookkeeping_with_a_counted_gap() {
    let (mut s, k) = Scenario::new(&[Faction::TheArborec, Faction::TheEmiratesOfHacan]).phase(Phase::Status).active(None).build();
    s.decks_mut().objective.set(vec![PublicObjective::ALL[0]]);
    let commander = Leader::ALL.iter().copied().find(|l| l.data().faction == FactionTag::Arborec && l.data().kind == LeaderType::Commander).unwrap();
    s.players_mut().edit(0).leaders.status.insert(commander, LeaderStatus::Locked);
    let e = env(&k);
    advance_status(&mut s, &e, None).unwrap();
    let held = s.clone();
    assert_eq!(advance_scoring(&mut s, &e, None), gap(TurnGap::CommanderUnlock));
    assert_eq!(s, held, "the window is as it was");
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
    assert_eq!(q.choice.ask, Ask::Cards(ti4_engine_core::choice::cards::Ask::OwnHand { reason: ti4_engine_core::choice::cards::Reason::SchemingDiscard }));
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
