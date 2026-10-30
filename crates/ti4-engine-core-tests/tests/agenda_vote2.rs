//! D4 lane A3: `rules/agenda/window.rs` (the vote) and the question builders of `rules/agenda/mod.rs` beyond lane A1: the Executive Order spender, Gila and
//! Elder Qanoj through their commander marks, Predictive Intelligence, Genetic Recombination with its tribute, the Blood Pact, extra votes and Zeal.
//! LRR 8 (agenda phase) and the printed text of the cards.

use ti4_engine_core::choice::agenda::{Ask as AAsk, Face, Opt as AOpt};
use ti4_engine_core::choice::Ask;
use ti4_engine_core::containers::SeatSet;
use ti4_engine_core::generated::{Agenda, Leader, Planet, Source, Technology};
use ti4_engine_core::keys::{Atom, MarkKey, MarkVal};
use ti4_engine_core::rules::agenda as ag;
use ti4_engine_core::rules::agenda::window::{Facts, Vote};
use ti4_engine_core::state::{ExtraVotes, Outcome};
use ti4_engine_core_tests::inv::ALL_SOURCES;
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::{plain_systems, system_of};

const FACTIONS: [Faction; 3] = [Faction::TheArborec, Faction::TheEmbersOfMuaat, Faction::TheYssarilTribes];

fn facts() -> Facts {
    Facts::new(ALL_SOURCES, false)
}

fn vote() -> Vote {
    Vote::new(Agenda::ALL[0], &[Outcome::For, Outcome::Against], &[Seat(0), Seat(1), Seat(2)], facts())
}

fn find(influence: u8) -> Planet {
    plain_systems()
        .iter()
        .flat_map(|s| s.data().planets.iter().copied())
        .find(|p| p.data().influence == influence && p.data().source == Source::Base && !p.data().legendary && p.data().resources > 0)
        .unwrap_or_else(|| panic!("a plain planet with {influence} influence and some resources"))
}

/// Seat 0 holds the planet with 1 influence, seat 1 the one with 2, seat 2 the one with 3.
fn table_with(factions: &[Faction]) -> (Scenario, [Planet; 3]) {
    let ps = [find(1), find(2), find(3)];
    let mut systems = Vec::new();
    for p in ps {
        let s = system_of(p);
        if !systems.contains(&s) {
            systems.push(s);
        }
    }
    let sc = Scenario::new(factions).map_from(&systems).control(ps[0], 0).control(ps[1], 1).control(ps[2], 2);
    (sc, ps)
}

fn table() -> (Scenario, [Planet; 3]) {
    table_with(&FACTIONS)
}

fn back(o: Outcome) -> Opt {
    Opt::Agenda(AOpt::Back(o, Default::default()))
}

fn exhaust(planet: Planet) -> impl Fn(&Opt) -> bool {
    move |o| matches!(o, Opt::Agenda(AOpt::Exhaust { planet: p, .. }) if *p == planet)
}

fn commander(s: &mut CoreState, seat: u8, leader: Leader) {
    s.marks_mut().tail.edit().insert(MarkKey::CommanderAbility(Seat(seat), leader), MarkVal::Atom(Atom::Num(1)));
}

fn count(rig: &Rig<Vote>, o: Outcome) -> i64 {
    rig.driver.window.counts().as_slice().iter().find(|(x, _)| *x == o).map_or(0, |(_, n)| *n)
}

fn ask_of(rig: &Rig<Vote>) -> AAsk {
    match &rig.asked().ask {
        Ask::Agenda(a) => a.clone(),
        other => panic!("not an agenda question: {other:?}"),
    }
}

fn shown_votes(rig: &Rig<Vote>, pick: impl Fn(&Opt) -> bool) -> i32 {
    match rig.asked().opts.iter().copied().find(|o| pick(o)) {
        Some(Opt::Agenda(AOpt::Back(_, shown))) => shown.votes,
        Some(Opt::Agenda(AOpt::Exhaust { shown, .. })) => shown.votes,
        other => panic!("not an option with votes: {other:?}"),
    }
}

// ---- Executive Order, Gila, Elder Qanoj -----------------------------------------------------------------

#[test]
fn the_executive_order_spender_may_exhaust_a_planet_for_its_resources_and_every_voter_sees_that() {
    let (sc, [_, b, _]) = table();
    // Seat 0 holds Starpoint instead (3 resources, 1 influence): the two faces cast different numbers.
    let a = Planet::Starpoint;
    let sc = sc.control(a, 0);
    let mut rig = Rig::new(sc.goods(0, 2), vote().with_spender(Seat(0)));
    rig.run().pick_opt(back(Outcome::For));
    let AAsk::Planets { spender, outcome, votes, .. } = ask_of(&rig) else { panic!("planets") };
    assert!(spender && outcome == Outcome::For && votes == 0);
    // The spender is offered a planet twice: for its influence and for its resources.
    let faces: Vec<(Face, i32)> = rig
        .asked()
        .opts
        .iter()
        .filter_map(|o| match o {
            Opt::Agenda(AOpt::Exhaust { planet, face, shown }) if *planet == a => Some((*face, shown.votes)),
            _ => None,
        })
        .collect();
    assert_eq!(faces, vec![(Face::Influence, 1), (Face::Resources, 3)]);
    // Exhausting it for the resources casts the resources.
    rig.pick_where(|o| matches!(o, Opt::Agenda(AOpt::Exhaust { planet, face: Face::Resources, .. }) if *planet == a));
    assert!(rig.state.board().planet(a).flags.has(ti4_engine_core::state::PlanetFlags::EXHAUSTED));
    // The other planet of the seat is left alone; goods may then buy more votes, one each (the spender's rate).
    rig.decline();
    let AAsk::TradeGoods { rate, goods, votes, .. } = ask_of(&rig) else { panic!("trade goods") };
    assert_eq!((rate, goods, votes), (1, 2, 3));
    assert_eq!(rig.asked().opts.iter().filter(|o| matches!(o, Opt::Agenda(AOpt::Spend(_)))).count(), 2, "spend 1 or 2");
    assert!(matches!(rig.asked().ctx.origin, ti4_engine_core::choice::Origin::Content(ti4_engine_core::choice::generic::Content::Technology(Technology::ExecutiveOrder))));
    rig.pick_opt(Opt::Agenda(AOpt::Spend(2)));
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().econ.trade_goods, 0);
    assert!(rig.driver.window.counts().is_empty() || count(&rig, Outcome::For) == 5);
    assert_eq!(rig.asked().actor, Seat(1));
    // The other voters are labelled too, but only influence is offered to them.
    rig.pick_opt(back(Outcome::Against));
    let AAsk::Planets { spender, .. } = ask_of(&rig) else { panic!("planets") };
    assert!(spender, "the table has a spender");
    assert!(rig.asked().opts.iter().all(|o| !matches!(o, Opt::Agenda(AOpt::Exhaust { face: Face::Resources, .. }))));
    rig.pick_where(exhaust(b));
    rig.decline();
    assert!(rig.is_done());
    assert_eq!(count(&rig, Outcome::For), 5);
    assert_eq!(count(&rig, Outcome::Against), 2);
}

#[test]
fn gila_the_silvertongue_pays_two_votes_per_trade_good_once_a_vote_was_cast() {
    let (sc, [a, _, _]) = table();
    let (mut s, k) = sc.goods(0, 3).build();
    commander(&mut s, 0, Leader::GilaTheSilvertongue);
    let mut rig = Rig::from_parts(s, k, vote());
    rig.run().pick_opt(back(Outcome::For));
    rig.pick_where(exhaust(a));
    let AAsk::TradeGoods { rate, goods, votes, .. } = ask_of(&rig) else { panic!("goods") };
    assert_eq!((rate, goods, votes), (2, 3, 1));
    assert!(matches!(rig.asked().ctx.origin, ti4_engine_core::choice::Origin::Content(ti4_engine_core::choice::generic::Content::Leader(Leader::GilaTheSilvertongue))));
    rig.pick_opt(Opt::Agenda(AOpt::Spend(2)));
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().econ.trade_goods, 1);
    // Not asked when no vote was cast: backing and then declining every planet banks nothing.
    let (sc2, _) = table();
    let (mut s2, k2) = sc2.goods(0, 3).build();
    commander(&mut s2, 0, Leader::GilaTheSilvertongue);
    let mut nothing = Rig::from_parts(s2, k2, vote());
    nothing.run().pick_opt(back(Outcome::For));
    nothing.decline();
    assert_eq!(nothing.asked().actor, Seat(1), "no trade good question without a vote");
    assert!(nothing.driver.window.counts().is_empty());
}

#[test]
fn elder_qanoj_casts_one_more_vote_with_every_planet() {
    let (sc, [a, _, _]) = table();
    let (mut s, k) = sc.build();
    commander(&mut s, 0, Leader::ElderQanoj);
    let mut rig = Rig::from_parts(s, k, vote());
    rig.run().pick_opt(back(Outcome::For));
    assert_eq!(shown_votes(&rig, exhaust(a)), 2, "1 influence and 1 for Qanoj");
    rig.pick_where(exhaust(a));
    rig.pick_opt(back(Outcome::Against));
    let b = rig.asked().opts.iter().find_map(|o| match o {
        Opt::Agenda(AOpt::Exhaust { planet, shown, .. }) => Some((*planet, shown.votes)),
        _ => None,
    });
    assert_eq!(b.map(|(_, v)| v), Some(2), "the others cast the influence only");
    rig.decline();
    rig.decline();
    assert_eq!(count(&rig, Outcome::For), 2);
}

// ---- Predictive Intelligence ------------------------------------------------------------------------------------

#[test]
fn predictive_intelligence_casts_three_more_votes_and_is_exhausted_unless_the_outcome_wins() {
    let (sc, [a, b, c]) = table();
    let run = |third_votes: bool| {
        let mut rig = Rig::new(sc.clone().tech(0, Technology::PredictiveIntelligence), vote());
        rig.run().pick_opt(back(Outcome::For));
        rig.pick_where(exhaust(a));
        let AAsk::Predictive { outcome, votes, .. } = ask_of(&rig) else { panic!("predictive") };
        assert_eq!((outcome, votes), (Outcome::For, 1));
        assert_eq!(rig.asked().opts.len(), 2);
        rig.pick_opt(Opt::Agenda(AOpt::CastPredictive));
        rig.pick_opt(back(Outcome::Against));
        rig.pick_where(exhaust(b));
        if third_votes {
            rig.pick_opt(back(Outcome::Against));
            rig.pick_where(exhaust(c));
        } else {
            rig.decline();
        }
        rig
    };
    // 1 + 3 = 4 for against 2: the prediction holds, the card stays ready.
    let won = run(false);
    assert!(won.is_done());
    assert_eq!(count(&won, Outcome::For), 4);
    assert_eq!(won.driver.window.winner(), Some(Outcome::For));
    assert!(!won.state.players().by_seat(Seat(0)).unwrap().tech.exhausted.contains(Technology::PredictiveIntelligence));
    // Against has 2 + 3 = 5: the prediction failed and the card is exhausted.
    let lost = run(true);
    assert_eq!(lost.driver.window.winner(), Some(Outcome::Against));
    assert!(lost.state.players().by_seat(Seat(0)).unwrap().tech.exhausted.contains(Technology::PredictiveIntelligence));
}

#[test]
fn predictive_intelligence_declined_costs_nothing_and_an_exhausted_or_old_card_is_not_offered() {
    let (sc, [a, _, _]) = table();
    let mut rig = Rig::new(sc.clone().tech(0, Technology::PredictiveIntelligence), vote());
    rig.run().pick_opt(back(Outcome::For));
    rig.pick_where(exhaust(a));
    rig.decline();
    assert_eq!(rig.asked().actor, Seat(1));
    rig.decline();
    rig.decline();
    assert_eq!(rig.driver.window.winner(), Some(Outcome::For));
    assert_eq!(count(&rig, Outcome::For), 1);
    assert!(!rig.state.players().by_seat(Seat(0)).unwrap().tech.exhausted.contains(Technology::PredictiveIntelligence));
    // The card is exhausted: no question.
    let (mut s, k) = sc.clone().tech(0, Technology::PredictiveIntelligence).build();
    s.players_mut().by_seat_mut(Seat(0)).unwrap().tech.exhausted.insert(Technology::PredictiveIntelligence);
    let mut spent = Rig::from_parts(s, k, vote());
    spent.run().pick_opt(back(Outcome::For));
    spent.pick_where(exhaust(a));
    assert_eq!(spent.asked().actor, Seat(1));
    // Before the technology's rules version (15) the card has no effect on votes.
    let (mut s, k) = sc.tech(0, Technology::PredictiveIntelligence).build();
    s.meta_mut().rules_version = 14;
    let mut old = Rig::from_parts(s, k, vote());
    old.run().pick_opt(back(Outcome::For));
    old.pick_where(exhaust(a));
    assert_eq!(old.asked().actor, Seat(1));
}

// ---- Genetic Recombination -------------------------------------------------------------------------------------

fn recombination() -> (Scenario, [Planet; 3]) {
    let (sc, ps) = table();
    (sc.tech(1, Technology::GeneticRecombination).tokens(0, 2, 2, 2).tokens(1, 2, 2, 2).tokens(2, 2, 0, 2), ps)
}

#[test]
fn the_holder_of_genetic_recombination_names_the_outcome_a_voter_must_back() {
    let (sc, [a, _, _]) = recombination();
    let mut rig = Rig::new(sc, vote());
    rig.run();
    // Before seat 0 votes the holder (seat 1) is asked, once.
    assert_eq!(rig.asked().actor, Seat(1));
    let AAsk::Recombine { voter, .. } = ask_of(&rig) else { panic!("recombine") };
    assert_eq!(voter, Seat(0));
    assert_eq!(rig.asked().opts.len(), 3, "an outcome each, or not");
    rig.assert_options_sound();
    rig.pick_opt(Opt::Agenda(AOpt::Demand(Outcome::Against)));
    assert!(rig.state.players().by_seat(Seat(1)).unwrap().tech.exhausted.contains(Technology::GeneticRecombination), "the card is exhausted by the demand");
    // The voter has a planet and a fleet token: back the outcome or return a token.
    assert_eq!(rig.asked().actor, Seat(0));
    let AAsk::Tribute { outcome, .. } = ask_of(&rig) else { panic!("tribute") };
    assert_eq!(outcome, Outcome::Against);
    assert_eq!(rig.asked().opts.as_slice(), &[Opt::Agenda(AOpt::TributeVote), Opt::Agenda(AOpt::TributeToken)]);
    rig.pick_opt(Opt::Agenda(AOpt::TributeVote));
    // The voter must back the demanded outcome and may not abstain, nor leave the planets unexhausted.
    assert_eq!(rig.asked().actor, Seat(0));
    assert_eq!(rig.opts(), vec![back(Outcome::Against)]);
    rig.pick(0);
    assert_eq!(rig.opts().iter().filter(|o| matches!(o, Opt::Decline)).count(), 0, "at least one vote");
    rig.pick_where(exhaust(a));
    // The card was used: nobody else is offered it, and the table goes on.
    assert_eq!(rig.asked().actor, Seat(1));
    assert!(matches!(ask_of(&rig), AAsk::Vote { .. }));
    assert_eq!(count(&rig, Outcome::Against), 1, "seat 0's single vote is banked when it is done");
}

#[test]
fn the_tribute_of_a_token_frees_the_voter_and_a_voter_with_nothing_to_pay_with_is_bound() {
    let (sc, _) = recombination();
    let mut rig = Rig::new(sc.clone(), vote());
    rig.run().pick_opt(Opt::Agenda(AOpt::Demand(Outcome::For)));
    rig.pick_opt(Opt::Agenda(AOpt::TributeToken));
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().econ.fleet_tokens, 1, "a fleet token is returned");
    assert!(rig.opts().contains(&back(Outcome::Against)) && rig.opts().contains(&Opt::Decline), "the voter is free");
    // Declining the offer: nothing happens and the card stays ready.
    let mut declined = Rig::new(sc.clone(), vote());
    declined.run().decline();
    assert!(!declined.state.players().by_seat(Seat(1)).unwrap().tech.exhausted.contains(Technology::GeneticRecombination));
    assert_eq!(declined.asked().actor, Seat(0));
    // Seat 2 has a planet and no fleet token: the demand binds it straight away, with no question about the tribute.
    let mut bound = Rig::new(sc, vote());
    bound.run().decline(); // the holder passes on seat 0
    bound.decline(); // seat 0 abstains
    bound.decline(); // seat 1 abstains; the holder is never asked about its own vote
    assert_eq!(bound.asked().actor, Seat(1));
    let AAsk::Recombine { voter, .. } = ask_of(&bound) else { panic!("recombine") };
    assert_eq!(voter, Seat(2));
    bound.pick_opt(Opt::Agenda(AOpt::Demand(Outcome::Against)));
    assert_eq!(bound.asked().actor, Seat(2));
    assert!(matches!(ask_of(&bound), AAsk::Vote { .. }));
    assert_eq!(bound.opts(), vec![back(Outcome::Against)]);
    assert_eq!(bound.state.players().by_seat(Seat(2)).unwrap().econ.fleet_tokens, 0);
}

#[test]
fn a_voter_without_planets_pays_the_tribute_in_a_token_without_being_asked_and_one_with_neither_is_free() {
    let (sc, ps) = recombination();
    // Seat 0 has no planet: only the token can pay.
    let no_planet = Scenario::new(&FACTIONS)
        .map_from(&[system_of(ps[1]), system_of(ps[2])])
        .control(ps[1], 1)
        .control(ps[2], 2)
        .tech(1, Technology::GeneticRecombination)
        .tokens(0, 2, 2, 2)
        .tokens(1, 2, 2, 2)
        .tokens(2, 2, 0, 2);
    let mut rig = Rig::new(no_planet.clone(), vote());
    rig.run().pick_opt(Opt::Agenda(AOpt::Demand(Outcome::For)));
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().econ.fleet_tokens, 1, "paid without a question");
    assert!(matches!(ask_of(&rig), AAsk::Vote { .. }) && rig.asked().actor == Seat(0));
    // Neither a planet nor a token: nothing to pay, nothing binds.
    let neither = no_planet.tokens(0, 2, 0, 2);
    let mut free = Rig::new(neither, vote());
    free.run().pick_opt(Opt::Agenda(AOpt::Demand(Outcome::For)));
    assert_eq!(free.state.players().by_seat(Seat(0)).unwrap().econ.fleet_tokens, 0);
    assert!(matches!(ask_of(&free), AAsk::Vote { .. }));
    assert!(free.opts().contains(&Opt::Decline));
    // A voter with a planet but no token is bound to the outcome without a question about the tribute.
    let (planets_only, _) = recombination();
    let mut bound = Rig::new(planets_only.tokens(0, 2, 0, 2), vote());
    bound.run().pick_opt(Opt::Agenda(AOpt::Demand(Outcome::Against)));
    assert!(matches!(ask_of(&bound), AAsk::Vote { .. }));
    assert_eq!(bound.opts(), vec![back(Outcome::Against)]);
    let _ = sc;
}

// ---- the arithmetic of the ballot --------------------------------------------------------------------------------

#[test]
fn the_blood_pact_adds_four_votes_when_the_partner_backed_the_same_outcome() {
    let (sc, [a, b, _]) = table();
    let mut pact = facts();
    pact.pact[1] = SeatSet::EMPTY;
    pact.pact[1].insert(Seat(0));
    let window = Vote::new(Agenda::ALL[0], &[Outcome::For, Outcome::Against], &[Seat(0), Seat(1), Seat(2)], pact);
    let run = |seat1: Outcome| {
        let mut rig = Rig::new(sc.clone(), window.clone());
        rig.run().pick_opt(back(Outcome::For));
        rig.pick_where(exhaust(a));
        rig.pick_opt(back(seat1));
        rig.pick_where(exhaust(b));
        rig.decline();
        rig
    };
    let same = run(Outcome::For);
    assert_eq!(count(&same, Outcome::For), 1 + (2 + 4), "seat 1's two votes and the pact's four");
    let other = run(Outcome::Against);
    assert_eq!(count(&other, Outcome::For), 1);
    assert_eq!(count(&other, Outcome::Against), 2, "no pact bonus for a different outcome");
    // The partner has to have voted already: seat 0 has no pact partner.
    assert_eq!(count(&same, Outcome::Against), 0);
}

#[test]
fn extra_votes_count_for_the_agenda_they_were_played_on_and_zeal_adds_one_per_player() {
    let (sc, [a, b, c]) = table_with(&[Faction::TheArgentFlight, Faction::TheEmbersOfMuaat, Faction::TheYssarilTribes]);
    let (mut s, k) = sc.build();
    s.meta_mut().agenda_seq = 3;
    s.players_mut().by_seat_mut(Seat(1)).unwrap().card_timers.extra_votes_agenda = Some(ExtraVotes { agenda: 3, votes: 5 });
    s.players_mut().by_seat_mut(Seat(2)).unwrap().card_timers.extra_votes_agenda = Some(ExtraVotes { agenda: 2, votes: 7 });
    let mut rig = Rig::from_parts(s, k, vote());
    rig.run().pick_opt(back(Outcome::For));
    rig.pick_where(exhaust(a));
    rig.pick_opt(back(Outcome::Against));
    rig.pick_where(exhaust(b));
    rig.pick_opt(back(Outcome::Against));
    rig.pick_where(exhaust(c));
    assert!(rig.is_done());
    // Zeal: one more vote for each of the three players, on top of the planet's 1 influence.
    assert_eq!(count(&rig, Outcome::For), 1 + 3);
    // Seat 1: its 2, the 5 played on this agenda; seat 2: its 3, and not the 7 of an earlier agenda.
    assert_eq!(count(&rig, Outcome::Against), 2 + 5 + 3);
    // Nothing cast, nothing recorded: no zeal for an abstaining Argent seat.
    let (sc, _) = table_with(&[Faction::TheArgentFlight, Faction::TheEmbersOfMuaat, Faction::TheYssarilTribes]);
    let mut quiet = Rig::new(sc, vote());
    quiet.run().pick_opt(back(Outcome::For));
    quiet.decline();
    quiet.decline();
    quiet.decline();
    assert!(quiet.driver.window.counts().is_empty());
}

#[test]
fn a_vote_with_the_obligation_to_cast_leaves_the_planet_question_open_until_one_planet_is_spent() {
    // `planets_question` directly: must_cast removes the decline option.
    let (sc, ps) = table();
    let s = sc.state();
    struct E;
    impl ag::Env for E {
        fn is_nekro(&self, _: Seat) -> bool {
            false
        }
        fn elder_qanoj(&self, _: Seat) -> bool {
            false
        }
        fn votes_first(&self, _: Seat) -> bool {
            false
        }
        fn oceans(&self, _: Seat) -> ti4_engine_core::containers::SmallList<Planet, 4> {
            ti4_engine_core::containers::SmallList::new()
        }
        fn resource_bonus(&self, _: Planet) -> i64 {
            0
        }
    }
    let cx = ag::Cx { core: &s, scope: ALL_SOURCES, blanked: false, env: &E };
    let choices = [Outcome::For, Outcome::Against];
    let with = ag::planets_question(&cx, Agenda::ALL[0], Seat(0), Outcome::For, 0, false, false, &choices, &[], false).unwrap();
    let must = ag::planets_question(&cx, Agenda::ALL[0], Seat(0), Outcome::For, 0, false, false, &choices, &[], true).unwrap();
    assert_eq!(with.opts.len(), must.opts.len() + 1);
    assert!(with.opts.iter().any(|o| matches!(o, Opt::Decline)) && !must.opts.iter().any(|o| matches!(o, Opt::Decline)));
    assert!(must.opts.iter().any(exhaust(ps[0])));
    // A seat with no planet to exhaust has no question; the tally holds eight outcomes at most.
    assert!(ag::planets_question(&cx, Agenda::ALL[0], Seat(1), Outcome::For, 0, false, false, &choices, &[], false).is_some());
    let none = Scenario::new(&FACTIONS).state();
    let cx2 = ag::Cx { core: &none, scope: ALL_SOURCES, blanked: false, env: &E };
    assert!(ag::planets_question(&cx2, Agenda::ALL[0], Seat(0), Outcome::For, 0, false, false, &choices, &[], false).is_none());
    let nine: Vec<Outcome> = (0..9).map(|i| Outcome::Seat(Seat(i))).collect();
    assert!(ag::planets_question(&cx, Agenda::ALL[0], Seat(0), Outcome::For, 0, false, false, &nine, &[], false).is_none(), "more outcomes than the tally holds");
    let eight: Vec<Outcome> = (0..8).map(|i| Outcome::Seat(Seat(i))).collect();
    assert!(ag::planets_question(&cx, Agenda::ALL[0], Seat(0), Outcome::For, 0, false, false, &eight, &[], false).is_some());
}

#[test]
fn the_trade_good_question_lists_one_to_all_the_goods_up_to_255_and_names_whose_ability_it_is() {
    let s = Scenario::new(&FACTIONS).state();
    let q = ag::trade_goods_question(&s, Agenda::ALL[0], Seat(0), Outcome::For, 2, 3, 1).unwrap();
    let spends: Vec<u8> = q.opts.iter().filter_map(|o| match o {
        Opt::Agenda(AOpt::Spend(n)) => Some(*n),
        _ => None,
    }).collect();
    assert_eq!(spends, vec![1, 2, 3]);
    assert_eq!(q.opts.iter().last(), Some(&Opt::Decline));
    let Ask::Agenda(AAsk::TradeGoods { rate, goods, votes, outcome, .. }) = q.ask else { panic!("goods") };
    assert_eq!((rate, goods, votes, outcome), (1, 3, 2, Outcome::For));
    let gila = ag::trade_goods_question(&s, Agenda::ALL[0], Seat(0), Outcome::For, 2, 255, 2).unwrap();
    assert_eq!(gila.opts.len(), 256);
    assert!(matches!(gila.ctx.origin, ti4_engine_core::choice::Origin::Content(ti4_engine_core::choice::generic::Content::Leader(Leader::GilaTheSilvertongue))));
    assert!(ag::trade_goods_question(&s, Agenda::ALL[0], Seat(0), Outcome::For, 2, 256, 1).is_none());
    // A negative count of goods has nothing to spend; the question is only the decline.
    let none = ag::trade_goods_question(&s, Agenda::ALL[0], Seat(0), Outcome::For, 0, -4, 1).unwrap();
    assert_eq!(none.opts.len(), 1);
    // The rate must fit a byte.
    assert!(ag::trade_goods_question(&s, Agenda::ALL[0], Seat(0), Outcome::For, 0, 1, 300).is_none());
}

#[test]
fn the_tribute_question_offers_what_the_voter_can_do_and_nothing_when_neither() {
    let s = Scenario::new(&FACTIONS).state();
    let q = |vote: bool, pay: bool| ag::tribute_question(&s, Agenda::ALL[0], Seat(0), Outcome::For, vote, pay);
    assert_eq!(q(true, true).unwrap().opts.as_slice(), &[Opt::Agenda(AOpt::TributeVote), Opt::Agenda(AOpt::TributeToken)]);
    assert_eq!(q(true, false).unwrap().opts.as_slice(), &[Opt::Agenda(AOpt::TributeVote)]);
    assert_eq!(q(false, true).unwrap().opts.as_slice(), &[Opt::Agenda(AOpt::TributeToken)]);
    assert!(q(false, false).is_none());
}

#[test]
fn covert_legislation_is_revealed_to_its_speaker_alone_in_the_frame_of_the_question() {
    let mut s = Scenario::new(&FACTIONS).speaker(1).state();
    s.agenda_mut().covert = Some(Agenda::ArmsReduction);
    let f = ag::frame(&s, Agenda::CovertLegislation, Seat(1));
    assert!(f.reveal && f.covert == Some(Agenda::ArmsReduction) && f.agenda == Agenda::CovertLegislation);
    assert!(!ag::frame(&s, Agenda::CovertLegislation, Seat(0)).reveal, "not the speaker");
    assert!(!ag::frame(&s, Agenda::ArmsReduction, Seat(1)).reveal, "not the covert agenda");
    // The vote question for the speaker carries the same frame.
    let recomb = ag::recombine_question(&s, Agenda::ALL[0], Seat(2), Seat(0), &[Outcome::For, Outcome::Against]);
    assert_eq!(recomb.actor, Seat(2));
    assert_eq!(recomb.opts.len(), 3);
    let tie = {
        struct E;
        impl ag::Env for E {
            fn is_nekro(&self, _: Seat) -> bool {
                false
            }
            fn elder_qanoj(&self, _: Seat) -> bool {
                false
            }
            fn votes_first(&self, _: Seat) -> bool {
                false
            }
            fn oceans(&self, _: Seat) -> ti4_engine_core::containers::SmallList<Planet, 4> {
                ti4_engine_core::containers::SmallList::new()
            }
            fn resource_bonus(&self, _: Planet) -> i64 {
                0
            }
        }
        ag::tiebreak_question(&ag::Cx { core: &s, scope: ALL_SOURCES, blanked: false, env: &E }, Agenda::CovertLegislation, &[Outcome::For, Outcome::Against])
    };
    assert_eq!(tie.actor, Seat(1), "the speaker decides");
    assert!(matches!(tie.ask, Ask::Agenda(AAsk::Tiebreak { frame }) if frame.reveal));
    let pred = ag::predictive_question(&s, Agenda::ALL[0], Seat(0), Outcome::For, 4);
    assert_eq!(pred.opts.as_slice(), &[Opt::Agenda(AOpt::CastPredictive), Opt::Decline]);
}

// ---- survivor-driven (mutation round 1) ------------------------------------------------------------------------------

#[test]
fn the_helios_dock_raises_the_resources_the_spender_casts_from_its_planet() {
    let a = Planet::Starpoint;
    let sys_a = system_of(a);
    let sc = Scenario::new(&[Faction::LastBastion, Faction::TheEmbersOfMuaat, Faction::TheYssarilTribes]).map_from(&[sys_a]).control(a, 0).unit(sys_a, Some(a), Unit::BastionSpacedock, 0, 1);
    let mut rig = Rig::new(sc, vote().with_spender(Seat(0)));
    rig.run().pick_opt(back(Outcome::For));
    let resources = rig
        .asked()
        .opts
        .iter()
        .find_map(|o| match o {
            Opt::Agenda(AOpt::Exhaust { planet, face: Face::Resources, shown }) if *planet == a => Some(shown.votes),
            _ => None,
        })
        .expect("the resources face");
    assert_eq!(resources, 3 + 1, "Starpoint's three resources and the dock's one");
    let influence = rig.asked().opts.iter().find_map(|o| match o {
        Opt::Agenda(AOpt::Exhaust { planet, face: Face::Influence, shown }) if *planet == a => Some(shown.votes),
        _ => None,
    });
    assert_eq!(influence, Some(1), "the dock changes resources only");
}

#[test]
fn a_demanded_outcome_binds_only_the_voter_it_was_demanded_of() {
    // Seat 1 holds Genetic Recombination and demands Against of seat 0 (who has a planet but no token: bound). Seat 1's own ballot comes next, and a
    // later voter who also backs Against is free to cast nothing.
    let (sc, [a, b, c]) = recombination();
    let sc = sc.tokens(0, 2, 0, 2);
    let mut rig = Rig::new(sc, vote());
    rig.run().pick_opt(Opt::Agenda(AOpt::Demand(Outcome::Against)));
    rig.pick(0);
    assert!(!rig.opts().contains(&Opt::Decline), "seat 0 must cast at least one vote");
    rig.pick_where(exhaust(a));
    // Seat 1 backs Against too and may leave it at that.
    rig.pick_opt(back(Outcome::Against));
    assert!(rig.opts().contains(&Opt::Decline), "the obligation is seat 0's alone: {}", rig.transcript());
    rig.pick_where(exhaust(b));
    let _ = c;
}

#[test]
fn a_holder_who_declines_for_a_later_voter_is_not_asked_again_for_that_voter() {
    let (sc, [_, _, _]) = recombination();
    let mut rig = Rig::new(sc, vote());
    rig.run().decline(); // for seat 0
    rig.decline(); // seat 0 abstains
    rig.decline(); // seat 1 abstains (it holds the card: never asked about itself)
    let AAsk::Recombine { voter, .. } = ask_of(&rig) else { panic!("recombine for seat 2") };
    assert_eq!(voter, Seat(2));
    rig.decline();
    assert_eq!(rig.asked().actor, Seat(2), "seat 2's own ballot, not the same offer again");
    assert!(matches!(ask_of(&rig), AAsk::Vote { .. }));
    rig.decline();
    assert!(!rig.is_done(), "nobody voted: the speaker decides");
    assert_eq!(rig.asked().actor, Seat(0));
}

#[test]
fn a_spender_without_trade_goods_and_a_player_who_cast_nothing_are_asked_no_more() {
    let (sc, [a, _, _]) = table();
    // Executive Order spender with no trade good: the planets are banked, the next voter is asked.
    let mut rig = Rig::new(sc.clone(), vote().with_spender(Seat(0)));
    rig.run().pick_opt(back(Outcome::For));
    rig.pick_where(|o| matches!(o, Opt::Agenda(AOpt::Exhaust { planet, face: Face::Influence, .. }) if *planet == a));
    assert_eq!(rig.asked().actor, Seat(1), "no goods question without goods");
    // Predictive Intelligence: backing an outcome but casting no vote does not offer the extra votes.
    let mut none = Rig::new(sc.tech(0, Technology::PredictiveIntelligence), vote());
    none.run().pick_opt(back(Outcome::For));
    none.decline();
    assert_eq!(none.asked().actor, Seat(1), "no predictive question for an empty ballot");
    assert!(none.driver.window.counts().is_empty());
}
