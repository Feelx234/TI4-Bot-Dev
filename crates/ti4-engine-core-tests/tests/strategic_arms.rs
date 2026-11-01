//! Lane E, KX1 to KX3: the strategic action's arms (`driver::windows::strategic`) run the way the live loop runs them: a held window,
//! `advance` to a question or the end, a clone of the position standing for a checkpoint. The lockstep against the string steps is
//! `ti4-engine`'s `strategic_live_tests`; these tests pin what the arms write, announce and refuse.

use std::rc::Rc;

use ti4_engine_core::choice::leaders::{Ask as LAsk, Opt as LOpt};
use ti4_engine_core::choice::strategy::{Ask as SAsk, Opt as SOpt};
use ti4_engine_core::containers::Cow;
use ti4_engine_core::driver::live::{ask, is_open, Env, Live, LiveError};
use ti4_engine_core::driver::windows::strategic::{
    after::open_after, begin::begin_strategic, advance_finished, advance_leader_followers, advance_secondary, ask_leader_followers, ask_secondary, secondary_of, select_leader_strategy_followers, LeaderFollowersWin, SecondaryWin,
};
use ti4_engine_core::driver::NoSeeds;
use ti4_engine_core::kernel::stage::AbilityTable;
use ti4_engine_core::kernel::Registry;
use ti4_engine_core::state::{PlainKind, SecondaryHow, Window as OpenWindow, WindowKind};
use ti4_engine_core_tests::prelude::*;

/// The live loop's table: the play facts hold the unit roster (lane F's closing menu, which the end of every action runs, and the Finish job
/// of the action need it: without one they stop at the counted gap `NoRoster`).
fn env(k: &Kernel) -> Env<'_> {
    use ti4_engine_core::kernel::ability::{PlayFacts, PlaysProvider, SharedPlay};
    let roster: &'static ti4_engine_core::query::tech::UnitRoster = Box::leak(Box::default());
    let mut table = AbilityTable::default();
    ti4_engine_core::driver::children::register(&mut table);
    table.facts.plays_provider = Some(PlaysProvider(std::sync::Arc::new(move |core, _| {
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
    Env { kernel: k, registry: Cow::new(Registry::default()), table, seeds: Rc::new(NoSeeds) }
}

fn kinds(s: &CoreState) -> Vec<PlainKind> {
    s.ledger.events.iter().filter_map(|e| e.plain().map(|p| p.kind)).collect()
}

fn typed(s: &CoreState) -> Vec<ti4_engine_core::state::EventKind> {
    s.ledger.events.iter().filter_map(|e| if let ti4_engine_core::state::Emit::Typed(t) = e { Some(t.kind) } else { None }).collect()
}

fn pick(o: SOpt) -> Pick {
    Pick::Opt(Opt::Strategy(o))
}

/// Four seats, seat 0 active with the Trade card in hand and the follower window open; the followers hold `tokens` strategy tokens each.
fn trade_window(tokens: [i16; 3]) -> (CoreState, Kernel) {
    let (mut s, k) = Scenario::lobby(4).phase(Phase::Action).active(Some(0)).build();
    for (i, t) in tokens.into_iter().enumerate() {
        let seat = Seat(i as u8 + 1);
        let p = s.players_mut().by_seat_mut(seat).unwrap();
        p.econ.strategic_tokens = t;
        p.econ.commodities = 0;
    }
    s.players_mut().by_seat_mut(Seat(0)).unwrap().strat.push(StrategyCard::Trade);
    s.pending_mut().open(OpenWindow::StrategySecondary(secondary_of(Seat(0), StrategyCard::Trade, &[Seat(1), Seat(2), Seat(3)], true)));
    (s, k)
}

fn first_asked(s: &CoreState, e: &Env<'_>) -> Choice {
    ask_secondary(s, e).unwrap().expect("a follower is asked").choice
}

#[test]
fn a_follower_who_may_not_follow_is_recorded_and_the_next_is_asked() {
    // Seat 1 has no token (and Masters of Trade do not help every faction): it is skipped in the order of the window.
    let (mut s, k) = trade_window([0, 2, 2]);
    let e = env(&k);
    let q = first_asked(&s, &e);
    let free = |seat| ti4_engine_core::driver::windows::strategic::facts::follower_facts(&s, &k, Seat(0), StrategyCard::Trade, seat);
    assert!(!free(Seat(1)).eligible || !free(Seat(1)).costs_token, "seat 1 is out or free");
    assert_eq!(q.actor, if free(Seat(1)).eligible { Seat(1) } else { Seat(2) });
    let (live, _) = advance_secondary(&mut s, &e, None).unwrap();
    assert!(matches!(live, Live::Asks(_)));
    let Some(OpenWindow::StrategySecondary(w)) = s.pending().window(WindowKind::StrategySecondary) else { panic!("open") };
    assert!(w.resolved.as_slice().iter().all(|(_, how)| *how == SecondaryHow::Ineligible), "only ineligible followers are recorded before the first question");
}

#[test]
fn a_follow_spends_the_token_says_so_and_the_last_answer_ends_the_action() {
    let (mut s, k) = trade_window([2, 2, 2]);
    let e = env(&k);
    let mut left = 3;
    // Everyone follows; the card is exhausted by the last one.
    loop {
        let q = first_asked(&s, &e);
        let tokens = s.players().by_seat(q.actor).unwrap().econ.strategic_tokens;
        let costs = matches!(q.ask, Ask::Strategy(SAsk::Follow { costs_token: true, .. }));
        let (live, _) = advance_secondary(&mut s, &e, Some(pick(SOpt::Follow))).unwrap();
        let after = s.players().by_seat(q.actor).unwrap().econ.strategic_tokens;
        assert_eq!(after, tokens - i16::from(costs), "the token is paid exactly when the question says so");
        left -= 1;
        if matches!(live, Live::Done) {
            break;
        }
        assert!(left > 0, "the window ended too late");
    }
    assert!(s.players().by_seat(Seat(0)).unwrap().strat_exhausted.contains(StrategyCard::Trade), "the last follower exhausts the card");
    let k = kinds(&s);
    assert_eq!(k.iter().filter(|p| **p == PlainKind::StrategySecondaryFollowed).count(), 3);
    assert!(k.contains(&PlainKind::StrategicActionComplete), "the action is complete");
    assert!(typed(&s).contains(&ti4_engine_core::state::EventKind::ActionCompleted), "the end of the action (`finish_action`) is part of the same call");
    assert!(!is_open::<SecondaryWin>(&s));
}

#[test]
fn an_illegal_answer_leaves_the_position_as_it_was() {
    let (mut s, k) = trade_window([2, 2, 2]);
    let e = env(&k);
    let held = s.clone();
    assert_eq!(advance_secondary(&mut s, &e, Some(pick(SOpt::PrimaryInstead))), Err(LiveError::Illegal));
    assert_eq!(s, held);
}

#[test]
fn a_clone_taken_while_a_follower_is_asked_continues_identically() {
    let (mut s, k) = trade_window([2, 2, 2]);
    let e = env(&k);
    advance_secondary(&mut s, &e, None).unwrap();
    let mut other = s.clone();
    let a = advance_secondary(&mut s, &e, Some(pick(SOpt::Skip))).unwrap();
    let b = advance_secondary(&mut other, &e, Some(pick(SOpt::Skip))).unwrap();
    assert_eq!(s, other);
    assert_eq!(a.0, b.0);
}

#[test]
fn when_nobody_may_follow_the_call_ends_the_window_and_the_action_without_a_question() {
    // Seat 0 plays; nobody has a token and nobody is free (the lobby's seats hold no Masters of Trade at seats 1 to 3 here).
    let (mut s, k) = Scenario::lobby(4).phase(Phase::Action).active(Some(0)).build();
    for seat in 1..4u8 {
        s.players_mut().by_seat_mut(Seat(seat)).unwrap().econ.strategic_tokens = 0;
        s.players_mut().by_seat_mut(Seat(seat)).unwrap().econ.commodities = 0;
    }
    s.players_mut().by_seat_mut(Seat(0)).unwrap().strat.push(StrategyCard::Warfare);
    s.pending_mut().open(OpenWindow::StrategySecondary(secondary_of(Seat(0), StrategyCard::Warfare, &[Seat(1), Seat(2), Seat(3)], true)));
    let e = env(&k);
    assert_eq!(ask_secondary(&s, &e).unwrap(), None);
    let (live, _) = advance_secondary(&mut s, &e, None).unwrap();
    assert_eq!(live, Live::Done);
    assert!(s.players().by_seat(Seat(0)).unwrap().strat_exhausted.contains(StrategyCard::Warfare));
    assert!(kinds(&s).contains(&PlainKind::StrategicActionComplete));
}

#[test]
fn a_preview_window_neither_finishes_the_action_nor_exhausts_the_card() {
    let (mut s, k) = trade_window([2, 2, 2]);
    let mut w = secondary_of(Seat(0), StrategyCard::Trade, &[Seat(2)], false);
    w.preview = true;
    s.pending_mut().open(OpenWindow::StrategySecondary(w));
    let e = env(&k);
    let q = first_asked(&s, &e);
    assert_eq!(q.actor, Seat(2));
    let (live, _) = advance_secondary(&mut s, &e, Some(pick(SOpt::Skip))).unwrap();
    assert_eq!(live, Live::Done);
    let k = kinds(&s);
    assert!(k.contains(&PlainKind::StrategySecondaryPreviewComplete));
    assert!(!k.contains(&PlainKind::StrategicActionComplete) && !typed(&s).contains(&ti4_engine_core::state::EventKind::ActionCompleted));
    assert!(!s.players().by_seat(Seat(0)).unwrap().strat_exhausted.contains(StrategyCard::Trade));
}

// -- the printed abilities the strategic action and the closing menu read ---------------------------------------------------------------

/// Hacan's printed abilities are read by the Envs of the core as the string reads them: Masters of Trade (the Trade secondary costs no token),
/// Guild Ships (trade with anybody) and Arbiters (trade action cards). Found by the shadow of the follower window behind a real game: the closing
/// menu of a turn offered Hacan's transactions on the string side and ended the turn on the core side.
#[test]
fn hacans_printed_abilities_are_read_by_the_strategy_and_trade_envs() {
    use ti4_engine_core::driver::kernel_env::KernelEnv;
    use ti4_engine_core::query::trade::Env as TradeEnv;
    use ti4_engine_core::rules::strategy::Env as StrategyEnv;
    let (s, k) = Scenario::new(&[Faction::TheEmiratesOfHacan, Faction::TheFederationOfSol]).build();
    let env = KernelEnv::new(&s, &k);
    assert!(StrategyEnv::secondary_is_free(&env, Seat(0), Some(ti4_engine_core::generated::StrategyCardKind::Trade)), "Masters of Trade");
    assert!(!StrategyEnv::secondary_is_free(&env, Seat(0), Some(ti4_engine_core::generated::StrategyCardKind::Warfare)));
    assert!(!StrategyEnv::secondary_is_free(&env, Seat(1), Some(ti4_engine_core::generated::StrategyCardKind::Trade)));
    let trade = ti4_engine_core::rules::trade::KernelEnv { core: &s, kernel: &k };
    assert!(trade.wide(Seat(0)) && !trade.wide(Seat(1)), "Guild Ships");
    assert!(trade.trades_action_cards(Seat(0)) && !trade.trades_action_cards(Seat(1)), "Arbiters");
    // Jol-Nar's Brilliant substitutes the primary of Technology.
    let (s, k) = Scenario::new(&[Faction::TheUniversitiesOfJolNar, Faction::TheFederationOfSol]).build();
    let env = KernelEnv::new(&s, &k);
    assert!(StrategyEnv::substitutes_primary(&env, Seat(0), Some(ti4_engine_core::generated::StrategyCardKind::Technology)));
    assert!(!StrategyEnv::substitutes_primary(&env, Seat(0), Some(ti4_engine_core::generated::StrategyCardKind::Trade)));
}

// -- a question the follower's ability asks ----------------------------------------------------------------------------------------

#[test]
fn a_clone_taken_while_the_followers_ability_asks_continues_identically() {
    // Leadership's secondary: the purchase is the follower's own question; the payment it opens is asked from inside the step.
    let (mut s, k) = Scenario::lobby(3).phase(Phase::Action).active(Some(0)).goods(1, 6).goods(2, 0).build();
    s.players_mut().by_seat_mut(Seat(0)).unwrap().strat.push(StrategyCard::Leadership);
    s.pending_mut().open(OpenWindow::StrategySecondary(secondary_of(Seat(0), StrategyCard::Leadership, &[Seat(1), Seat(2)], true)));
    let e = env(&k);
    let q = first_asked(&s, &e);
    assert_eq!(q.actor, Seat(1), "seat 2 cannot afford a token and is not asked");
    let (live, _) = advance_secondary(&mut s, &e, Some(pick(SOpt::Follow))).unwrap();
    let Live::Asks(asking) = live else { panic!("the payment asks") };
    assert!(s.pending().nested.is_some(), "the step is suspended on a question from inside it");
    let mut other = s.clone();
    let choice = asking.choice.clone();
    let answer = choice.pick(0).expect("an option");
    let a = advance_secondary(&mut s, &e, Some(answer)).unwrap();
    let b = advance_secondary(&mut other, &e, Some(answer)).unwrap();
    assert_eq!(s, other, "the clone continues to the same position");
    assert_eq!(a.0, b.0);
}

// -- the Winnu hero's follower selection ------------------------------------------------------------------------------------------

fn hero() -> (CoreState, Kernel) {
    let (mut s, k) = Scenario::new(&[Faction::TheWinnu, Faction::TheXxchaKingdom, Faction::TheArborec, Faction::TheBaronyOfLetnev]).phase(Phase::Action).active(Some(0)).build();
    for seat in 1..4u8 {
        s.players_mut().by_seat_mut(Seat(seat)).unwrap().econ.strategic_tokens = 2;
    }
    s.pending_mut().open(OpenWindow::LeaderStrategy { player: Seat(0), leader: Leader::MathisMathinus, card: StrategyCard::Trade });
    select_leader_strategy_followers(&mut s);
    (s, k)
}

#[test]
fn the_winnu_hero_asks_about_every_other_seat_in_order_and_opens_the_follower_window_with_the_yes_answers() {
    let (mut s, k) = hero();
    let e = env(&k);
    assert!(is_open::<LeaderFollowersWin>(&s));
    let mut asked = Vec::new();
    for yes in [true, false, true] {
        let q = ask_leader_followers(&s, &e).unwrap().expect("a question").choice;
        let Ask::Leaders(LAsk::WinnuFollower { follower }) = q.ask else { panic!("{:?}", q.ask) };
        assert_eq!(q.actor, Seat(0), "the hero's owner decides");
        asked.push(follower);
        advance_leader_followers(&mut s, &e, Some(Pick::Opt(Opt::Leaders(LOpt::HeroFollower(yes))))).unwrap();
    }
    assert_eq!(asked, [Seat(1), Seat(2), Seat(3)]);
    assert!(!is_open::<LeaderFollowersWin>(&s));
    let Some(OpenWindow::StrategySecondary(w)) = s.pending().window(WindowKind::StrategySecondary) else { panic!("the follower window opens") };
    assert_eq!((w.primary, w.card, w.exhausts_card), (Seat(0), StrategyCard::Trade, false), "a card nobody holds");
    assert_eq!(w.followers.as_slice(), [Seat(1), Seat(3)]);
}

#[test]
fn the_end_of_the_hero_strategy_purges_the_hero_and_says_so() {
    let (mut s, k) = hero();
    s.players_mut().by_seat_mut(Seat(0)).unwrap().leaders.status.insert(Leader::MathisMathinus, ti4_engine_core::state::LeaderStatus::Readied);
    let e = env(&k);
    for _ in 0..3 {
        advance_leader_followers(&mut s, &e, Some(Pick::Opt(Opt::Leaders(LOpt::HeroFollower(false))))).unwrap();
    }
    // Nobody follows: the window opens with no follower and ends in one call.
    let (live, _) = advance_secondary(&mut s, &e, None).unwrap();
    assert_eq!(live, Live::Done);
    let k = kinds(&s);
    let resolved = k.iter().position(|p| *p == PlainKind::ComponentActionResolved).expect("the hero's action resolved");
    let complete = k.iter().position(|p| *p == PlainKind::StrategicActionComplete).expect("the action is complete");
    assert!(resolved < complete, "the hero's strategy completes before the action does");
    assert!(s.pending().window(WindowKind::LeaderStrategy).is_none());
    assert!(s.players().by_seat(Seat(0)).unwrap().leaders.status.get(&Leader::MathisMathinus).is_none_or(|st| *st != ti4_engine_core::state::LeaderStatus::Readied), "purged");
}

#[test]
fn a_hero_whose_card_has_no_followers_opens_the_follower_window_at_once() {
    let (mut s, _) = Scenario::lobby(3).phase(Phase::Action).active(Some(0)).build();
    s.pending_mut().open(OpenWindow::LeaderStrategy { player: Seat(0), leader: Leader::HeshAndPrit, card: StrategyCard::Trade });
    select_leader_strategy_followers(&mut s);
    assert!(s.pending().window(WindowKind::LeaderFollowers).is_none());
    assert!(s.pending().window(WindowKind::StrategySecondary).is_some());
}

// -- the end of a game ---------------------------------------------------------------------------------------------------------

#[test]
fn a_game_that_ended_during_an_action_closes_the_windows_it_held() {
    let (mut s, k) = hero();
    s.meta_mut().finished = true;
    let e = env(&k);
    let (live, _) = advance_finished(&mut s, &e, None).unwrap();
    assert_eq!(live, Live::Done);
    assert!(s.pending().window(WindowKind::StrategySecondary).is_none() && s.pending().window(WindowKind::LeaderStrategy).is_none());
    assert!(kinds(&s).contains(&PlainKind::ComponentActionResolved));
    let _ = ask::<SecondaryWin>(&s, &e);
}

// -- the strategic action chosen from the menu (KX3) --------------------------------------------------------------------------------------

fn menu(card: StrategyCard) -> (CoreState, Kernel) {
    let (mut s, k) = Scenario::lobby(4).phase(Phase::Action).active(Some(0)).build();
    s.players_mut().by_seat_mut(Seat(0)).unwrap().strat.push(card);
    (s, k)
}

#[test]
fn a_strategic_action_says_it_began_and_opens_the_follower_window_for_the_others_clockwise() {
    // The Trade primary asks whom to replenish: the seat declines it ("done"), then the window opens.
    let (mut s, k) = menu(StrategyCard::Trade);
    let e = env(&k);
    let mut answers = Vec::new();
    let (mut live, _) = begin_strategic(&mut s, &e, StrategyCard::Trade).unwrap();
    while let Live::Asks(asking) = live {
        answers.push(asking.choice.ask.clone());
        let done = asking.choice.opts.iter().position(|o| matches!(o, Opt::Done)).unwrap_or(0);
        live = advance_secondary_or_job(&mut s, &e, asking.choice.pick(done).unwrap());
    }
    assert!(!answers.is_empty(), "the primary asked whom to replenish: {answers:?}");
    let Some(OpenWindow::StrategySecondary(w)) = s.pending().window(WindowKind::StrategySecondary) else { panic!("the follower window opens") };
    assert_eq!((w.primary, w.card, w.exhausts_card, w.preview), (Seat(0), StrategyCard::Trade, true, false));
    assert_eq!(w.followers.as_slice(), [Seat(1), Seat(2), Seat(3)], "clockwise from the primary player, the player excluded");
    assert!(kinds(&s).contains(&PlainKind::StrategicActionBegan));
    assert!(typed(&s).contains(&ti4_engine_core::state::EventKind::StrategicActionBegan));
    assert!(!s.players().by_seat(Seat(0)).unwrap().strat_exhausted.contains(StrategyCard::Trade), "the card is exhausted when the last follower is done, not before");
}

fn advance_secondary_or_job(s: &mut CoreState, e: &Env<'_>, pick: Pick) -> Live {
    ti4_engine_core::driver::live::advance::<ti4_engine_core::driver::turn::TurnFlowWin>(s, e, Some(pick)).unwrap().0
}

#[test]
fn a_card_the_seat_does_not_hold_or_has_used_is_refused_and_the_position_stays() {
    let (mut s, k) = menu(StrategyCard::Trade);
    let e = env(&k);
    let held = s.clone();
    assert!(begin_strategic(&mut s, &e, StrategyCard::Warfare).is_err(), "not held");
    assert_eq!(s, held);
    s.players_mut().by_seat_mut(Seat(0)).unwrap().strat_exhausted.insert(StrategyCard::Trade);
    let held = s.clone();
    assert!(begin_strategic(&mut s, &e, StrategyCard::Trade).is_err(), "used");
    assert_eq!(s, held);
}

#[test]
fn the_strategic_action_is_the_one_action_extreme_duress_does_not_punish() {
    let (mut s, k) = menu(StrategyCard::Warfare);
    s.players_mut().by_seat_mut(Seat(0)).unwrap().econ.duress_by = Some(Seat(1));
    s.players_mut().by_seat_mut(Seat(0)).unwrap().econ.trade_goods = 3;
    let e = env(&k);
    let (mut live, _) = begin_strategic(&mut s, &e, StrategyCard::Warfare).unwrap_or_else(|_| (Live::Done, Default::default()));
    while let Live::Asks(asking) = live {
        live = advance_secondary_or_job(&mut s, &e, asking.choice.pick(0).unwrap());
    }
    assert_eq!(s.players().by_seat(Seat(0)).unwrap().econ.duress_by, None, "the duress lifts quietly");
    assert_eq!(s.players().by_seat(Seat(0)).unwrap().econ.trade_goods, 3, "nothing is handed over");
}

// -- the end of the free tactical action (Thunder's Edge Warfare) -------------------------------------------------------------------------

fn after(hero_card: Option<StrategyCard>) -> (CoreState, Kernel) {
    let (mut s, k) = Scenario::lobby(3).phase(Phase::Action).active(Some(0)).tokens(0, 2, 3, 3).build();
    match hero_card {
        Some(card) => s.pending_mut().open(OpenWindow::LeaderStrategy { player: Seat(0), leader: Leader::HeshAndPrit, card }),
        None => s.pending_mut().open(OpenWindow::StrategySecondaryAfterTactical(secondary_of(Seat(0), StrategyCard::WarfareThundersEdge, &[Seat(1), Seat(2)], true))),
    }
    open_after(&mut s);
    (s, k)
}

#[test]
fn after_the_free_action_the_seat_redistributes_and_the_held_follower_window_opens() {
    let (mut s, k) = after(None);
    let e = env(&k);
    let (live, _) = ti4_engine_core::driver::live::advance::<ti4_engine_core::driver::turn::TurnFlowWin>(&mut s, &e, None).unwrap();
    let Live::Asks(asking) = live else { panic!("the seat is asked to redistribute") };
    assert_eq!(asking.choice.ask, Ask::Strategy(SAsk::TeWarfareRedistributeAfter { total: 8, floor: 2 }));
    assert_eq!(asking.choice.actor, Seat(0));
    // 0 tactic, 5 fleet, 3 strategy.
    let at = asking.choice.opts.iter().position(|o| *o == Opt::Strategy(SOpt::Split { tactic: 0, fleet: 5, strategic: 3 })).expect("offered");
    let (live, _) = ti4_engine_core::driver::live::advance::<ti4_engine_core::driver::turn::TurnFlowWin>(&mut s, &e, asking.choice.pick(at)).unwrap();
    assert_eq!(live, Live::Done);
    let p = s.players().by_seat(Seat(0)).unwrap();
    assert_eq!((p.econ.tactic_tokens, p.econ.fleet_tokens, p.econ.strategic_tokens), (0, 5, 3));
    assert!(s.pending().window(WindowKind::StrategySecondaryAfterTactical).is_none());
    assert!(s.pending().window(WindowKind::StrategySecondary).is_some(), "the follower window held for the end of the action opens");
}

#[test]
fn after_the_free_action_a_heros_strategy_goes_on_to_its_followers() {
    let (mut s, k) = after(Some(StrategyCard::Trade));
    let e = env(&k);
    let (live, _) = ti4_engine_core::driver::live::advance::<ti4_engine_core::driver::turn::TurnFlowWin>(&mut s, &e, None).unwrap();
    let Live::Asks(asking) = live else { panic!("asks") };
    ti4_engine_core::driver::live::advance::<ti4_engine_core::driver::turn::TurnFlowWin>(&mut s, &e, asking.choice.pick(0)).unwrap();
    assert!(s.pending().window(WindowKind::StrategySecondary).is_some(), "a hero whose card has no followers opens the window at once");
}

#[test]
fn a_game_that_ended_during_the_free_action_asks_for_no_redistribution() {
    let (mut s, k) = after(None);
    s.meta_mut().finished = true;
    let e = env(&k);
    let (live, _) = ti4_engine_core::driver::live::advance::<ti4_engine_core::driver::turn::TurnFlowWin>(&mut s, &e, None).unwrap();
    assert_eq!(live, Live::Done);
}

// -- what the string asks in a research that the core window does not carry ---------------------------------------------------------

/// Doctor Sucaban (the Jol-Nar agent readied at the table) is offered to a seat that researches with infantry on the board: the core window does
/// not ask it, so the follow is not carried (`Began::Unsupported`, a counted gap) instead of researching without the offer. Found by the shadow behind a
/// real game, where the string asked "Doctor Sucaban: exhaust to let a trade infantry for research".
#[test]
fn a_research_the_string_would_ask_doctor_sucaban_about_is_not_carried() {
    use ti4_engine_core::driver::windows::strategic::facts::research_asks_more;
    use ti4_engine_core::state::LeaderStatus;
    let planet = ti4_engine_core_tests::scenario::first_planet();
    let build = |infantry: usize| {
        Scenario::new(&[Faction::TheFederationOfSol, Faction::TheXxchaKingdom, Faction::TheUniversitiesOfJolNar])
            .unit(planet.0, Some(planet.1), Unit::InfantryI, 1, infantry)
            .build()
            .0
    };
    let mut s = build(1);
    let agent = |s: &mut CoreState, st| {
        s.players_mut().by_seat_mut(Seat(2)).unwrap().leaders.status.insert(Leader::DoctorSucaban, st);
    };
    agent(&mut s, LeaderStatus::Readied);
    assert!(research_asks_more(&s, Seat(1)), "the agent is readied and the seat has infantry to trade");
    assert!(!research_asks_more(&s, Seat(0)), "a seat without infantry on the board has nothing to trade");
    agent(&mut s, LeaderStatus::Exhausted);
    assert!(!research_asks_more(&s, Seat(1)), "an exhausted agent asks nothing");
    let mut none = build(0);
    agent(&mut none, LeaderStatus::Readied);
    assert!(!research_asks_more(&none, Seat(1)));
}

// -- what the windows record, route and refuse (found by the mutation run) -------------------------------------------------------------

#[test]
fn the_followers_who_may_not_follow_are_written_to_the_payload_when_the_window_asks() {
    // Construction costs a token for everybody (no seat of the lobby follows it for free): seat 1 has none.
    let (mut s, k) = Scenario::lobby(4).phase(Phase::Action).active(Some(0)).tokens(1, 0, 0, 0).tokens(2, 3, 3, 2).build();
    s.players_mut().by_seat_mut(Seat(0)).unwrap().strat.push(StrategyCard::Construction);
    s.pending_mut().open(OpenWindow::StrategySecondary(secondary_of(Seat(0), StrategyCard::Construction, &[Seat(1), Seat(2), Seat(3)], true)));
    let e = env(&k);
    let (live, _) = advance_secondary(&mut s, &e, None).unwrap();
    assert!(matches!(live, Live::Asks(_)));
    let Some(OpenWindow::StrategySecondary(w)) = s.pending().window(WindowKind::StrategySecondary) else { panic!("open") };
    assert_eq!((w.next, w.resolved.as_slice()), (1, &[(Seat(1), SecondaryHow::Ineligible)][..]), "the skipped follower is recorded for the next call");
}

#[test]
fn a_window_open_beside_a_job_is_the_arms_first() {
    let (mut s, k) = trade_window([2, 2, 2]);
    ti4_engine_core::driver::turn::open_job(&mut s, ti4_engine_core::state::TurnJob::Prelude);
    let e = env(&k);
    assert_eq!(ask_secondary(&s, &e).unwrap().map(|a| a.choice.actor), Some(first_asked(&s, &e).actor), "the follower is asked, not the job");
    let (live, _) = advance_secondary(&mut s, &e, None).unwrap();
    assert!(matches!(live, Live::Asks(_)), "the follower window runs, not the job");
}

#[test]
fn only_the_winnu_hero_asks_who_follows() {
    // The Winnu seat with another hero's card, a seat that is not the Winnu with Mathis Mathinus: no question, the window opens at once.
    for (faction, leader) in [(Faction::TheWinnu, Leader::HeshAndPrit), (Faction::TheFederationOfSol, Leader::MathisMathinus)] {
        let (mut s, _) = Scenario::new(&[faction, Faction::TheArborec, Faction::TheXxchaKingdom]).phase(Phase::Action).active(Some(0)).build();
        s.pending_mut().open(OpenWindow::LeaderStrategy { player: Seat(0), leader, card: StrategyCard::Trade });
        select_leader_strategy_followers(&mut s);
        assert!(s.pending().window(WindowKind::LeaderFollowers).is_none(), "{faction:?} with {leader:?}");
        assert!(s.pending().window(WindowKind::StrategySecondary).is_some());
    }
}

#[test]
fn the_heros_follower_question_is_the_heros_not_optional_and_outside_a_battle() {
    let (s, k) = hero();
    let q = ask_leader_followers(&s, &env(&k)).unwrap().unwrap().choice;
    assert!(!q.ctx.optional && !q.ctx.space_battle && q.ctx.target.is_none() && q.ctx.invasion_seq.is_none());
    assert_eq!(q.opts.as_slice(), [Opt::Leaders(LOpt::HeroFollower(true)), Opt::Leaders(LOpt::HeroFollower(false))]);
}

// -- Jae Mir Kan: a waiver the Nomad's suite would hear is not carried -----------------------------------------------------------------

fn jae_mir_kan(nomad: bool) -> (CoreState, Kernel, Seat) {
    let system = ti4_engine_core_tests::scenario::first_planet().0;
    let factions: Vec<Faction> = if nomad {
        vec![Faction::TheFederationOfSol, Faction::TheMahactGeneSorcerers, Faction::TheNomad]
    } else {
        vec![Faction::TheFederationOfSol, Faction::TheMahactGeneSorcerers, Faction::TheArborec]
    };
    let (mut s, k) = Scenario::new(&factions).phase(Phase::Action).active(Some(0)).command_token(system, 0).build();
    s.players_mut().by_seat_mut(Seat(0)).unwrap().strat.push(StrategyCard::Trade);
    s.players_mut().by_seat_mut(Seat(1)).unwrap().leaders.status.insert(Leader::JaeMirKan, ti4_engine_core::state::LeaderStatus::Readied);
    s.pending_mut().open(OpenWindow::StrategySecondary(secondary_of(Seat(0), StrategyCard::Trade, &[Seat(1)], true)));
    let _ = system;
    (s, k, Seat(1))
}

#[test]
fn jae_mir_kan_is_offered_as_a_waiver_and_pays_with_the_primary_players_token() {
    let (mut s, k, mahact) = jae_mir_kan(false);
    let e = env(&k);
    let q = first_asked(&s, &e);
    assert_eq!(q.actor, mahact);
    let waiver = q.opts.iter().copied().find(|o| matches!(o, Opt::Strategy(SOpt::WaiveBy(Faction::TheMahactGeneSorcerers, _)))).expect("Jae Mir Kan is offered");
    let tokens = s.players().by_seat(mahact).unwrap().econ.strategic_tokens;
    advance_secondary(&mut s, &e, Some(Pick::Opt(waiver))).expect("the waiver is paid");
    assert_eq!(s.players().by_seat(mahact).unwrap().econ.strategic_tokens, tokens, "no strategy token is spent");
    assert_eq!(s.players().by_seat(mahact).unwrap().leaders.status.get(&Leader::JaeMirKan), Some(&ti4_engine_core::state::LeaderStatus::Exhausted), "the agent is exhausted");
    assert!(kinds(&s).contains(&PlainKind::StrategySecondaryFollowed));
}

#[test]
fn a_waiver_a_nomad_would_hear_is_offered_and_stops_the_step_when_it_is_taken() {
    let (mut s, k, mahact) = jae_mir_kan(true);
    let e = env(&k);
    let q = first_asked(&s, &e);
    let waiver = q.opts.iter().copied().find(|o| matches!(o, Opt::Strategy(SOpt::WaiveBy(..)))).expect("offered like the string offers it");
    let held = s.clone();
    assert_eq!(
        advance_secondary(&mut s, &e, Some(Pick::Opt(waiver))),
        Err(LiveError::Unknown(ti4_engine_core::kernel::ability::Unknown::TurnGap(ti4_engine_core::kernel::turn_codes::TurnGap::StrategyAbility)))
    );
    assert_eq!(s, held, "nothing was paid");
    assert_eq!(s.players().by_seat(mahact).unwrap().leaders.status.get(&Leader::JaeMirKan), Some(&ti4_engine_core::state::LeaderStatus::Readied));
}

// -- the redistribution after the free action ------------------------------------------------------------------------------------------

#[test]
fn the_letnev_fleet_pool_may_be_emptied_after_the_free_action_and_a_seat_without_tokens_is_not_asked() {
    let (mut s, k) = Scenario::new(&[Faction::TheBaronyOfLetnev, Faction::TheArborec]).phase(Phase::Action).active(Some(0)).tokens(0, 1, 1, 1).build();
    s.pending_mut().open(OpenWindow::StrategySecondaryAfterTactical(secondary_of(Seat(0), StrategyCard::WarfareThundersEdge, &[Seat(1)], true)));
    open_after(&mut s);
    let e = env(&k);
    let (live, _) = ti4_engine_core::driver::live::advance::<ti4_engine_core::driver::turn::TurnFlowWin>(&mut s, &e, None).unwrap();
    let Live::Asks(asking) = live else { panic!("asks") };
    assert!(asking.choice.opts.iter().any(|o| *o == Opt::Strategy(SOpt::Split { tactic: 3, fleet: 0, strategic: 0 })), "Letnev may empty the fleet pool");
    // A seat with no command token at all has nothing to redistribute.
    let (mut none, k) = Scenario::new(&[Faction::TheFederationOfSol, Faction::TheArborec]).phase(Phase::Action).active(Some(0)).tokens(0, 0, 0, 0).build();
    none.pending_mut().open(OpenWindow::StrategySecondaryAfterTactical(secondary_of(Seat(0), StrategyCard::WarfareThundersEdge, &[Seat(1)], true)));
    open_after(&mut none);
    let (live, _) = ti4_engine_core::driver::live::advance::<ti4_engine_core::driver::turn::TurnFlowWin>(&mut none, &env(&k), None).unwrap();
    assert_eq!(live, Live::Done);
}

#[test]
fn a_position_without_the_map_slice_names_no_system_for_the_free_action_and_does_not_read_the_map() {
    // Found by the mutation run (the scratch build keeps the debug assertions): `Galaxy` asserts the map slice is loaded.
    use ti4_engine_core::driver::windows::strategic::facts::facts_at;
    let (mut s, k) = Scenario::lobby(3).phase(Phase::Action).active(Some(0)).build();
    s.loaded = ti4_engine_core::state::SliceSet(ti4_engine_core::state::SliceSet::ALL.0 & !(1 << ti4_engine_core::state::Slice::Map as u32));
    let facts = facts_at(&s, &k, &Default::default(), None, Seat(0), StrategyCard::WarfareThundersEdge);
    assert!(facts.free.is_some_and(|f| f.systems().is_empty()), "no map, no system to name");
}

// -- more of what the windows write (the mutation run's survivors) ---------------------------------------------------------------------------------

#[test]
fn the_payload_records_how_each_follower_answered() {
    use ti4_engine_core::state::SecondaryHow::{Declined, Followed};
    let (mut s, k) = trade_window([2, 2, 2]);
    let e = env(&k);
    let first = first_asked(&s, &e).actor;
    advance_secondary(&mut s, &e, Some(pick(SOpt::Follow))).unwrap();
    let second = first_asked(&s, &e).actor;
    advance_secondary(&mut s, &e, Some(pick(SOpt::Skip))).unwrap();
    let Some(OpenWindow::StrategySecondary(w)) = s.pending().window(WindowKind::StrategySecondary) else { panic!("open") };
    let answered: Vec<_> = w.resolved.as_slice().iter().filter(|(_, how)| *how != SecondaryHow::Ineligible).copied().collect();
    assert_eq!(answered, [(first, Followed), (second, Declined)]);
    assert!(!w.exhausts_card.eq(&false), "the primary player's own card");
}

#[test]
fn a_hero_without_followers_holds_a_card_nobody_holds() {
    let (mut s, _) = Scenario::lobby(3).phase(Phase::Action).active(Some(0)).build();
    s.pending_mut().open(OpenWindow::LeaderStrategy { player: Seat(0), leader: Leader::HeshAndPrit, card: StrategyCard::Trade });
    select_leader_strategy_followers(&mut s);
    let Some(OpenWindow::StrategySecondary(w)) = s.pending().window(WindowKind::StrategySecondary) else { panic!("open") };
    assert!(!w.exhausts_card && !w.preview && w.followers.is_empty());
}

#[test]
fn a_seat_with_one_command_token_is_still_asked_to_redistribute_after_the_free_action() {
    let (mut s, k) = Scenario::new(&[Faction::TheFederationOfSol, Faction::TheArborec]).phase(Phase::Action).active(Some(0)).tokens(0, 1, 0, 0).build();
    s.pending_mut().open(OpenWindow::StrategySecondaryAfterTactical(secondary_of(Seat(0), StrategyCard::WarfareThundersEdge, &[Seat(1)], true)));
    open_after(&mut s);
    let (live, _) = ti4_engine_core::driver::live::advance::<ti4_engine_core::driver::turn::TurnFlowWin>(&mut s, &env(&k), None).unwrap();
    assert!(matches!(live, Live::Asks(_)));
}

#[test]
fn a_winnu_primary_player_lets_a_follower_return_acquiescence_instead_of_a_token() {
    use ti4_engine_core::state::NoteKey;
    let (mut s, k) = Scenario::new(&[Faction::TheWinnu, Faction::TheArborec]).phase(Phase::Action).active(Some(0)).build();
    let note = NoteKey { note: ti4_engine_core::generated::PromissoryNote::Acquiescence, faction: Faction::TheWinnu };
    s.cards_mut().promissory.edit().insert(note, Seat(1));
    s.players_mut().by_seat_mut(Seat(1)).unwrap().econ.strategic_tokens = 0;
    s.players_mut().by_seat_mut(Seat(0)).unwrap().strat.push(StrategyCard::Trade);
    s.pending_mut().open(OpenWindow::StrategySecondary(secondary_of(Seat(0), StrategyCard::Trade, &[Seat(1)], true)));
    let e = env(&k);
    let q = first_asked(&s, &e);
    assert_eq!(q.opts.as_slice(), [Opt::Strategy(SOpt::Skip), Opt::Strategy(SOpt::WaiveBy(Faction::TheWinnu, 0))], "no token: the waiver instead of the follow");
    advance_secondary(&mut s, &e, Some(Pick::Opt(Opt::Strategy(SOpt::WaiveBy(Faction::TheWinnu, 0))))).unwrap();
    assert_eq!(s.cards().promissory.get(&note), Some(&Seat(0)), "the note went home");
}

// -- Xxcha's Peace Accords and the Ssruu copy of Doctor Sucaban -----------------------------------------------------------------------------

/// Xxcha at seat 1 controls and has exhausted `pa`; `pb` stands in the next system, uncontrolled (and controlled by seat 0 when `taken`).
fn xxcha_table(taken: bool) -> (CoreState, Kernel) {
    use ti4_engine_core_tests::scenario::system_of;
    let planets: Vec<_> = Planet::ALL.iter().copied().filter(|p| p.data().system.is_some() && p.data().traits.is_empty().eq(&false)).collect();
    let pa = planets[0];
    let pb = planets.iter().copied().find(|p| system_of(*p) != system_of(pa)).expect("a second system");
    let mut sc = Scenario::new(&[Faction::TheFederationOfSol, Faction::TheXxchaKingdom])
        .phase(Phase::Action)
        .active(Some(0))
        .map_from(&[system_of(pa), system_of(pb)])
        .control(pa, 1)
        .exhaust(pa)
        .tokens(1, 3, 3, 2);
    if taken {
        // Every other planet of the two systems is somebody's.
        for p in Planet::ALL.iter().copied().filter(|p| p.data().system.is_some_and(|sys| sys == system_of(pa) || sys == system_of(pb)) && *p != pa) {
            sc = sc.control(p, 0);
        }
    }
    let (mut s, k) = sc.build();
    s.players_mut().by_seat_mut(Seat(0)).unwrap().strat.push(StrategyCard::DiplomacyBase);
    s.pending_mut().open(OpenWindow::StrategySecondary(secondary_of(Seat(0), StrategyCard::DiplomacyBase, &[Seat(1)], true)));
    (s, k)
}

#[test]
fn an_xxcha_follower_of_diplomacy_with_a_planet_to_annex_stops_at_a_counted_gap() {
    let (mut s, k) = xxcha_table(false);
    let e = env(&k);
    let q = first_asked(&s, &e);
    assert_eq!(q.actor, Seat(1));
    let (live, _) = advance_secondary(&mut s, &e, Some(pick(SOpt::Follow))).unwrap();
    let Live::Asks(ready) = live else { panic!("the ready question") };
    let held = s.clone();
    let r = advance_secondary(&mut s, &e, ready.choice.pick(0));
    assert_eq!(r, Err(LiveError::Unknown(ti4_engine_core::kernel::ability::Unknown::TurnGap(ti4_engine_core::kernel::turn_codes::TurnGap::PeaceAccords))));
    assert_eq!(s, held);
}

#[test]
fn an_xxcha_follower_with_nothing_to_annex_goes_on() {
    let (mut s, k) = xxcha_table(true);
    let e = env(&k);
    let (live, _) = advance_secondary(&mut s, &e, Some(pick(SOpt::Follow))).unwrap();
    let Live::Asks(ready) = live else { panic!("the ready question") };
    let r = advance_secondary(&mut s, &e, ready.choice.pick(0));
    assert!(!matches!(r, Err(LiveError::Unknown(ti4_engine_core::kernel::ability::Unknown::TurnGap(ti4_engine_core::kernel::turn_codes::TurnGap::PeaceAccords)))), "{r:?}");
}

#[test]
fn a_yssaril_agent_copying_the_jolnar_agent_is_doctor_sucaban_too() {
    use ti4_engine_core::driver::windows::strategic::facts::research_asks_more;
    use ti4_engine_core::state::LeaderStatus;
    let planet = ti4_engine_core_tests::scenario::first_planet();
    let (mut s, _) = Scenario::new(&[Faction::TheFederationOfSol, Faction::TheYssarilTribes, Faction::TheUniversitiesOfJolNar]).unit(planet.0, Some(planet.1), Unit::InfantryI, 0, 1).build();
    s.players_mut().by_seat_mut(Seat(1)).unwrap().leaders.status.insert(Leader::Ssruu, LeaderStatus::Readied);
    assert!(research_asks_more(&s, Seat(0)), "Ssruu may copy Doctor Sucaban while a Jol-Nar sits at the table");
    let (mut s, _) = Scenario::new(&[Faction::TheFederationOfSol, Faction::TheYssarilTribes, Faction::TheArborec]).unit(planet.0, Some(planet.1), Unit::InfantryI, 0, 1).build();
    s.players_mut().by_seat_mut(Seat(1)).unwrap().leaders.status.insert(Leader::Ssruu, LeaderStatus::Readied);
    assert!(!research_asks_more(&s, Seat(0)), "no Jol-Nar agent to copy");
}

// -- Titans' Hecatoncheires in the construction facts (found by combining lane E with lane W12: integration 11) --------------------------------

/// "When you would place a PDS on a planet, you may place 1 mech and 1 infantry on that planet instead" (Titans' Hecatoncheires): the facts of the
/// Construction ability carry the two forms for a Titans seat and nothing for any other. The native facts of lane E predated lane W12's
/// `StructureFacts::alt` and would have placed a PDS without the offer the string makes.
#[test]
fn the_construction_facts_carry_titans_hecatoncheires_and_no_other_seats() {
    use ti4_engine_core::driver::windows::strategic::facts::structure_facts;
    use ti4_engine_core::kernel::turn_codes::TurnCodes;
    let roster: &'static ti4_engine_core::query::tech::UnitRoster = Box::leak(Box::default());
    let (s, k) = Scenario::new(&[Faction::TheTitansOfUl, Faction::TheFederationOfSol]).build();
    let titans = structure_facts(&s, &k, roster, Seat(0), &TurnCodes::default()).expect("facts");
    assert_eq!(titans.alt.map(|(mech, infantry)| (mech.data().base_type, infantry.data().base_type)), Some((UnitBaseType::Mech, UnitBaseType::Infantry)));
    let sol = structure_facts(&s, &k, roster, Seat(1), &TurnCodes::default()).expect("facts");
    assert_eq!(sol.alt, None, "only the Titans have the alternative");
}

#[test]
fn the_systems_of_the_free_action_are_the_map_and_the_systems_the_string_holds_an_entry_for_not_every_cell_of_the_board() {
    // Found by the lockstep with a map: the board has a cell for every system of the tables (231), the string only the ones in play; the whole
    // list overflowed the facts and the primary was refused (a counted gap in every real game).
    use ti4_engine_core::driver::windows::strategic::facts::facts_at;
    use ti4_engine_core_tests::scenario::plain_systems;
    let p = plain_systems();
    let (mut s, k) = Scenario::lobby(3).phase(Phase::Action).active(Some(0)).map_from(&[p[0], p[1]]).build();
    s.board_mut().sys_mut(p[5]).flags.set(ti4_engine_core::state::SysFlags::TOUCHED, true);
    let facts = facts_at(&s, &k, &Default::default(), None, Seat(0), StrategyCard::WarfareThundersEdge);
    let systems = facts.free.expect("a list that fits").systems().to_vec();
    assert!(systems.contains(&p[0]) && systems.contains(&p[1]), "the printed map");
    assert!(systems.contains(&p[5]), "a system the string holds an entry for, beside the map");
    assert!(!systems.contains(&p[6]), "a system nobody touched is not in play");
    assert!(systems.len() < 20, "{} systems", systems.len());
}
