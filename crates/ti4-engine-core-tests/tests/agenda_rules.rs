//! D4-2a: the pure rules of `rules::agenda` (outcomes, voting order, votable planets, bars) over a hand-made `Env`.

use ti4_engine_core::containers::SmallList;
use ti4_engine_core::rules::agenda as ag;
use ti4_engine_core::rules::agenda::{Cx, Env};
use ti4_engine_core::state::Outcome;
use ti4_engine_core::keys::{Prediction, VoteBar};
use ti4_engine_core_tests::inv::ALL_SOURCES;
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::{planets_with_influence, systems_of};

#[derive(Default)]
struct E {
    nekro: Vec<Seat>,
    qanoj: Vec<Seat>,
    first: Vec<Seat>,
    oceans: Vec<Planet>,
}

impl Env for E {
    fn is_nekro(&self, who: Seat) -> bool {
        self.nekro.contains(&who)
    }
    fn elder_qanoj(&self, who: Seat) -> bool {
        self.qanoj.contains(&who)
    }
    fn votes_first(&self, who: Seat) -> bool {
        self.first.contains(&who)
    }
    fn oceans(&self, _: Seat) -> SmallList<Planet, 4> {
        let mut out = SmallList::new();
        for p in &self.oceans {
            out.push(*p);
        }
        out
    }
    fn resource_bonus(&self, _: Planet) -> i64 {
        0
    }
}

fn cx<'a>(core: &'a CoreState, env: &'a E) -> Cx<'a> {
    Cx { core, scope: ALL_SOURCES, blanked: false, env }
}

fn table(n: usize) -> Scenario {
    Scenario::lobby(n)
}

fn seats(list: &SmallList<Seat, 8>) -> Vec<u8> {
    list.as_slice().iter().map(|s| s.0).collect()
}

#[test]
fn a_law_is_an_agenda_of_the_law_kind_and_the_covert_agenda_is_voted_in_its_place() {
    covers!("rules::agenda::is_law");
    assert!(ag::is_law(Agenda::ArticlesOfWar) && !ag::is_law(Agenda::ArmsReduction));
    let mut s = table(3).state();
    assert_eq!(ag::voted_agenda(&s, Agenda::CovertLegislation), Agenda::CovertLegislation, "nothing drawn face down");
    s.agenda_mut().covert = Some(Agenda::ArmsReduction);
    assert_eq!(ag::voted_agenda(&s, Agenda::CovertLegislation), Agenda::ArmsReduction);
    assert_eq!(ag::voted_agenda(&s, Agenda::ArticlesOfWar), Agenda::ArticlesOfWar, "only Covert Legislation is replaced");
}

#[test]
fn the_outcomes_of_each_kind_of_election() {
    covers!("rules::agenda::outcomes");
    let planets = planets_with_influence(&[1, 2]);
    let mut s = table(3).map_from(&systems_of(&planets)).control(planets[0], 1).state();
    let env = E::default();
    let o = |a: Agenda| ag::outcomes(&cx(&s, &env), a).as_slice().to_vec();
    let of = |kind: AgendaElects| Agenda::ALL.into_iter().find(|a| a.data().elects == kind).unwrap_or_else(|| panic!("an agenda electing {kind:?}"));
    assert_eq!(o(of(AgendaElects::ForAgainst)), vec![Outcome::For, Outcome::Against]);
    assert_eq!(o(of(AgendaElects::Player)), vec![Outcome::Seat(Seat(0)), Outcome::Seat(Seat(1)), Outcome::Seat(Seat(2))]);
    assert_eq!(o(of(AgendaElects::Planet)), vec![Outcome::Planet(planets[0])], "only a planet somebody controls (8.11)");
    assert!(o(of(AgendaElects::Law)).is_empty(), "no law in play: no candidate");
    s.agenda_mut().laws.insert(Agenda::ArticlesOfWar, Outcome::For);
    let law = of(AgendaElects::Law);
    assert_eq!(ag::outcomes(&cx(&s, &env), law).as_slice(), &[Outcome::Law(Agenda::ArticlesOfWar)]);
}

#[test]
fn votes_added_by_a_card_count_for_the_agenda_they_were_played_on_only() {
    covers!("rules::agenda::extra_votes");
    let mut s = table(2).state();
    s.meta_mut().agenda_seq = 2;
    assert_eq!(ag::extra_votes(&s, Seat(0)), 0);
    let timers = &mut s.players_mut().by_seat_mut(Seat(0)).unwrap().card_timers;
    timers.extra_votes_agenda = Some(ti4_engine_core::state::ExtraVotes { agenda: 2, votes: 4 });
    assert_eq!(ag::extra_votes(&s, Seat(0)), 4);
    s.meta_mut().agenda_seq = 3;
    assert_eq!(ag::extra_votes(&s, Seat(0)), 0, "a later agenda");
}

#[test]
fn representative_government_makes_every_vote_one() {
    covers!("rules::agenda::flat_vote_amount");
    let mut s = table(3).state();
    let env = E::default();
    assert_eq!(ag::flat_vote_amount(&cx(&s, &env)), None);
    s.agenda_mut().laws.insert(Agenda::RepresentativeGovernment, Outcome::For);
    assert_eq!(ag::flat_vote_amount(&cx(&s, &env)), Some(1));
    let blanked = Cx { blanked: true, ..cx(&s, &env) };
    assert_eq!(ag::flat_vote_amount(&blanked), None, "Law's Order blanks the law");
}

#[test]
fn only_readied_planets_with_influence_vote_and_oceans_follow_the_own_planets() {
    covers!("rules::agenda::votable_planets");
    let planets = planets_with_influence(&[2, 1, 3]);
    let mut s = table(2).map_from(&systems_of(&planets)).control(planets[0], 0).control(planets[1], 0).control(planets[2], 1).state();
    let env = E::default();
    let own = |s: &CoreState, env: &E| ag::votable_planets(&cx(s, env), Seat(0)).as_slice().to_vec();
    let mut want = vec![planets[0], planets[1]];
    let got = own(&s, &env);
    assert!(got.len() == 2 && want.iter().all(|p| got.contains(p)), "the seat's own planets only: {got:?}");
    ti4_engine_core::kernel::fx::exhaust_planet(&mut s, planets[0]);
    assert_eq!(own(&s, &env), vec![planets[1]], "an exhausted planet does not vote");
    let with_ocean = E { oceans: vec![planets[2]], ..E::default() };
    assert_eq!(own(&s, &with_ocean), vec![planets[1], planets[2]], "oceans come after the own planets");
    want.clear();
    assert_eq!(ag::influence_of(&cx(&s, &env), planets[2]), 3);
}

#[test]
fn the_reveal_order_starts_left_of_the_speaker_and_ends_with_the_speaker() {
    covers!("rules::agenda::reveal_order");
    let s = table(4).speaker(2).state();
    assert_eq!(seats(&ag::reveal_order(&s)), vec![3, 0, 1, 2]);
    let one = table(1).state();
    assert_eq!(seats(&ag::reveal_order(&one)), vec![0]);
}

#[test]
fn the_voting_order_is_clockwise_with_the_speaker_after_the_others_and_first_voters_and_hackers_at_the_ends() {
    covers!("rules::agenda::voting_order");
    let s = table(4).speaker(1).state();
    let plain = E::default();
    assert_eq!(seats(&ag::voting_order(&cx(&s, &plain))), vec![2, 3, 0, 1], "the speaker last");
    let firsts = E { first: vec![Seat(3)], ..E::default() };
    assert_eq!(seats(&ag::voting_order(&cx(&s, &firsts))), vec![3, 2, 0, 1], "a first voter opens");
    let speaker_first = E { first: vec![Seat(1)], ..E::default() };
    assert_eq!(seats(&ag::voting_order(&cx(&s, &speaker_first))), vec![1, 2, 3, 0], "the speaker's own first slot");
    let nekro = E { nekro: vec![Seat(2)], ..E::default() };
    assert_eq!(seats(&ag::voting_order(&cx(&s, &nekro))), vec![3, 0, 1], "Nekro does not vote");
    let qanoj = E { nekro: vec![Seat(2)], qanoj: vec![Seat(2)], ..E::default() };
    assert_eq!(seats(&ag::voting_order(&cx(&s, &qanoj))), vec![2, 3, 0, 1], "Elder Qanoj always does");
    let mut predicted = table(4).speaker(1).state();
    predicted.agenda_mut().predictions.insert(Seat(0), Prediction::Plain(Outcome::For));
    assert_eq!(seats(&ag::voting_order(&cx(&predicted, &plain))), vec![2, 3, 1], "a seat that predicted does not vote");
    let mut hack = table(4).speaker(1).state();
    hack.meta_mut().agenda_seq = 5;
    hack.players_mut().by_seat_mut(Seat(2)).unwrap().card_timers.hack_votes_last_agenda = ti4_engine_core::containers::Opt32::some(5);
    assert_eq!(seats(&ag::voting_order(&cx(&hack, &plain))), vec![3, 0, 1, 2], "Hack Election's seat votes after the rest");
}

#[test]
fn checks_and_balances_against_readies_three_planets_and_the_political_secret_bars_from_rules_version_17() {
    covers!("rules::agenda::ready_limit");
    let mut s = table(2).state();
    assert_eq!(ag::ready_limit(&s, false), None);
    s.agenda_mut().laws.insert(Agenda::ChecksAndBalances, Outcome::For);
    assert_eq!(ag::ready_limit(&s, false), None, "elected For");
    s.agenda_mut().laws.insert(Agenda::ChecksAndBalances, Outcome::Against);
    assert_eq!(ag::ready_limit(&s, false), Some(3));
    assert_eq!(ag::ready_limit(&s, true), None, "blanked");
    s.agenda_mut().predictions.insert(Seat(1), Prediction::Barred(VoteBar::PoliticalSecret));
    assert!(ag::political_secret_bars(&s, Seat(1)) && !ag::political_secret_bars(&s, Seat(0)));
    let old = { let mut o = table(2).rules(16).state(); o.agenda_mut().predictions.insert(Seat(1), Prediction::Barred(VoteBar::PoliticalSecret)); o };
    assert!(!ag::political_secret_bars(&old, Seat(1)), "before rules version 17");
}

#[test]
fn the_text_order_of_planets_is_a_total_order() {
    covers!("rules::agenda::planet_text_cmp");
    let sample: Vec<Planet> = Planet::ALL.iter().copied().step_by(7).collect();
    for a in &sample {
        assert_eq!(ag::planet_text_cmp(*a, *a), std::cmp::Ordering::Equal);
        for b in &sample {
            assert_eq!(ag::planet_text_cmp(*a, *b), ag::planet_text_cmp(*b, *a).reverse(), "{a:?} {b:?}");
            for c in &sample {
                if ag::planet_text_cmp(*a, *b).is_le() && ag::planet_text_cmp(*b, *c).is_le() {
                    assert!(ag::planet_text_cmp(*a, *c).is_le(), "transitive: {a:?} {b:?} {c:?}");
                }
            }
        }
    }
    let mut sorted = sample.clone();
    sorted.sort_by(|a, b| ag::planet_text_cmp(*a, *b));
    sorted.dedup();
    assert_eq!(sorted.len(), sample.len(), "distinct planets never compare equal");
}
