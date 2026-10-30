//! D4 lane A3: `rules/agenda/mod.rs` reads that lane A1 did not reach: the elections that list planets, strategy cards and scored secrets by rules
//! version, the planets a voter may exhaust (a planet with no influence is not one), the faces of a spender, the shown numbers.

use ti4_engine_core::choice::agenda::{Ask as AAsk, Face, Opt as AOpt};
use ti4_engine_core::containers::SmallList;
use ti4_engine_core::generated::{Agenda, AgendaElects, Planet, SecretObjective, StrategyCard};
use ti4_engine_core::rules::agenda as ag;
use ti4_engine_core::rules::agenda::{Cx, Env};
use ti4_engine_core::state::{ObjectiveRef, Outcome};
use ti4_engine_core::tables::{source_bit, Scope};
use ti4_engine_core_tests::inv::ALL_SOURCES;
use ti4_engine_core_tests::prelude::*;

#[derive(Default)]
struct E {
    qanoj: Vec<Seat>,
}

impl Env for E {
    fn is_nekro(&self, _: Seat) -> bool {
        false
    }
    fn elder_qanoj(&self, who: Seat) -> bool {
        self.qanoj.contains(&who)
    }
    fn votes_first(&self, _: Seat) -> bool {
        false
    }
    fn oceans(&self, _: Seat) -> SmallList<Planet, 4> {
        SmallList::new()
    }
    fn resource_bonus(&self, _: Planet) -> i64 {
        0
    }
}

fn table() -> Scenario {
    Scenario::lobby(3)
}

fn elects(kind: AgendaElects) -> Agenda {
    Agenda::ALL.into_iter().find(|a| a.data().elects == kind).unwrap_or_else(|| panic!("an agenda that elects {kind:?}"))
}

fn mk<'a>(s: &'a CoreState, e: &'a E) -> Cx<'a> {
    Cx { core: s, scope: ALL_SOURCES, blanked: false, env: e }
}

#[test]
fn a_planet_election_lists_controlled_planets_and_the_non_home_one_leaves_out_homes_of_the_sources_in_play_and_mecatol() {
    let agenda = elects(AgendaElects::NonHomeNonMecatolPlanet);
    assert_eq!(agenda, Agenda::ColonialRedistribution);
    let s = table().control(Planet::Quann, 0).control(Planet::Nestphar, 0).control(Planet::MecatolRex, 1).control(Planet::Lodor, 2).control(Planet::Olergodt, 2).state();
    let e = E::default();
    let all = ag::outcomes(&Cx { core: &s, scope: ALL_SOURCES, blanked: false, env: &e }, agenda);
    let mut want = vec![Outcome::Planet(Planet::Quann), Outcome::Planet(Planet::Lodor), Outcome::Planet(Planet::Olergodt)];
    want.sort_by(|a, b| ag::text_cmp(*a, *b));
    let mut got = all.as_slice().to_vec();
    got.sort_by(|a, b| ag::text_cmp(*a, *b));
    assert_eq!(got, want, "no homeworld, no Mecatol Rex");
    // A planet nobody controls is never a candidate; a planet with no control at all is not on the list.
    let none = table().state();
    assert!(ag::outcomes(&Cx { core: &none, scope: ALL_SOURCES, blanked: false, env: &e }, agenda).is_empty());
    // A plain planet election (Core Mining) offers the homeworld and Mecatol Rex as well.
    let plain = ag::outcomes(&Cx { core: &s, scope: ALL_SOURCES, blanked: false, env: &e }, Agenda::CoreMining);
    assert_eq!(plain.len(), 5);
    // A homeworld outside the sources in play is an ordinary planet: Muaat's home, with only the Prophecy of Kings in play.
    let outside = table().control(Planet::Muaat, 0).state();
    let pok = Scope(source_bit(ti4_engine_core::generated::Source::Pok));
    assert!(ag::outcomes(&Cx { core: &outside, scope: ALL_SOURCES, blanked: false, env: &e }, agenda).is_empty());
    assert_eq!(ag::outcomes(&Cx { core: &outside, scope: pok, blanked: false, env: &e }, agenda).as_slice(), &[Outcome::Planet(Planet::Muaat)]);
}

#[test]
fn galactic_crisis_pact_offers_the_strategy_cards_lowest_initiative_first_from_rules_version_31() {
    let agenda = elects(AgendaElects::StrategyCard);
    let mut s = table().state();
    s.strategy_mut().initiative.insert(StrategyCard::Trade, 5);
    s.strategy_mut().initiative.insert(StrategyCard::Leadership, 1);
    s.strategy_mut().initiative.insert(StrategyCard::Politics, 3);
    let e = E::default();
    assert_eq!(
        ag::outcomes(&mk(&s, &e), agenda).as_slice(),
        &[Outcome::StrategyCard(StrategyCard::Leadership), Outcome::StrategyCard(StrategyCard::Politics), Outcome::StrategyCard(StrategyCard::Trade)]
    );
    // Two cards with the same initiative: the card order breaks the tie.
    s.strategy_mut().initiative.insert(StrategyCard::Warfare, 3);
    let tied = ag::outcomes(&mk(&s, &e), agenda);
    assert_eq!(tied.len(), 4);
    assert_eq!(&tied.as_slice()[1..3], &[Outcome::StrategyCard(StrategyCard::Politics), Outcome::StrategyCard(StrategyCard::Warfare)]);
    s.meta_mut().rules_version = 30;
    assert!(ag::outcomes(&mk(&s, &e), agenda).is_empty(), "before the pact existed nothing was a candidate");
}

#[test]
fn classified_document_leaks_offers_the_scored_secrets_not_yet_public_from_rules_version_32() {
    let agenda = elects(AgendaElects::ScoredSecretObjective);
    let (a, b, c) = (SecretObjective::ALL[2], SecretObjective::ALL[5], SecretObjective::ALL[9]);
    let mut s = table().state();
    s.objectives_mut().scored[0].secret.insert(c);
    s.objectives_mut().scored[1].secret.insert(a);
    s.objectives_mut().scored[2].secret.insert(b);
    s.objectives_mut().revealed.push(ObjectiveRef::Secret(b));
    let e = E::default();
    assert_eq!(ag::outcomes(&mk(&s, &e), agenda).as_slice(), &[Outcome::Secret(a), Outcome::Secret(c)], "in order, the public one left out");
    // The same secret scored twice (two seats' sets) is one candidate.
    s.objectives_mut().scored[2].secret.insert(a);
    assert_eq!(ag::outcomes(&mk(&s, &e), agenda).as_slice(), &[Outcome::Secret(a), Outcome::Secret(c)]);
    s.meta_mut().rules_version = 31;
    assert!(ag::outcomes(&mk(&s, &e), agenda).is_empty());
}

#[test]
fn a_planet_with_no_influence_casts_no_vote_and_no_resource_face_without_resources() {
    // Capha: 3 resources, 0 influence. Torkan: 0 resources, 3 influence.
    let s = table().control(Planet::Capha, 0).control(Planet::Torkan, 0).state();
    let e = E::default();
    let c = Cx { core: &s, scope: ALL_SOURCES, blanked: false, env: &e };
    assert_eq!(ag::votable_planets(&c, Seat(0)).as_slice(), &[Planet::Torkan]);
    let offers = |spender: bool| ag::planet_offers(&c, Seat(0), spender).as_slice().to_vec();
    assert_eq!(offers(false), vec![(Planet::Torkan, Face::Influence, 3)]);
    // The spender may also exhaust for resources: Capha for its three, Torkan not at all.
    let mut spender = offers(true);
    spender.sort_by_key(|(p, f, _)| (*p, *f as u8));
    let mut want = vec![(Planet::Torkan, Face::Influence, 3), (Planet::Capha, Face::Resources, 3)];
    want.sort_by_key(|(p, f, _)| (*p, *f as u8));
    assert_eq!(spender, want);
    // Elder Qanoj adds a vote to every planet that casts, either face.
    let q = E { qanoj: vec![Seat(0)] };
    let cq = Cx { core: &s, scope: ALL_SOURCES, blanked: false, env: &q };
    let mut with = ag::planet_offers(&cq, Seat(0), true).as_slice().to_vec();
    with.sort_by_key(|(p, f, _)| (*p, *f as u8));
    let mut want = vec![(Planet::Torkan, Face::Influence, 4), (Planet::Capha, Face::Resources, 4)];
    want.sort_by_key(|(p, f, _)| (*p, *f as u8));
    assert_eq!(with, want);
}

#[test]
fn the_questions_of_a_vote_are_plain_rule_questions_and_a_tiebreak_shows_no_votes() {
    let s = table().control(Planet::Quann, 0).state();
    let e = E::default();
    let c = Cx { core: &s, scope: ALL_SOURCES, blanked: false, env: &e };
    let choices = [Outcome::For, Outcome::Against];
    let vote = ag::vote_question(&c, Agenda::ALL[0], Seat(0), &choices, &[(Outcome::For, 4)], None);
    assert!(!vote.ctx.optional && !vote.ctx.space_battle && vote.ctx.outstanding.is_empty() && vote.ctx.target.is_none() && vote.ctx.invasion_seq.is_none());
    // The shown votes are the tally so far.
    let shown: Vec<i32> = vote.opts.iter().filter_map(|o| if let Opt::Agenda(AOpt::Back(_, sh)) = o { Some(sh.votes) } else { None }).collect();
    assert_eq!(shown, vec![4, 0]);
    // Bound to one outcome: that one only, and no abstaining.
    let bound = ag::vote_question(&c, Agenda::ALL[0], Seat(0), &choices, &[], Some(Outcome::Against));
    assert_eq!(bound.opts.len(), 1);
    let tie = ag::tiebreak_question(&c, Agenda::ALL[0], &choices);
    assert!(!tie.ctx.optional && !tie.ctx.space_battle);
    let shown: Vec<i32> = tie.opts.iter().filter_map(|o| if let Opt::Agenda(AOpt::Back(_, sh)) = o { Some(sh.votes) } else { None }).collect();
    assert_eq!(shown, vec![0, 0]);
    let planets = ag::planets_question(&c, Agenda::ALL[0], Seat(0), Outcome::For, 2, false, true, &choices, &[(Outcome::For, 5)], false).unwrap();
    assert!(!planets.ctx.optional && !planets.ctx.space_battle);
    let Ask::Agenda(AAsk::Planets { votes, tally, spender, .. }) = planets.ask else { panic!("planets") };
    assert_eq!((votes, spender, tally.as_slice().len()), (2, true, 2));
    assert_eq!(tally.as_slice()[0], (Outcome::For, 5));
    assert_eq!(tally.as_slice()[1], (Outcome::Against, 0));
}
