//! D4 lane A2: the prerequisite payment of the Technology strategy card (`Research::exhaust_specialties`): which specialty planets are exhausted
//! for a technology the owned colours do not cover, and which waivers (Research Team laws, the Prophet's Tears, Psychoarchaeology, the war sun
//! law, AI Development Algorithm) spare them; plus the Research Agreement that follows a research.

use ti4_engine_core::choice::strategy::Opt as SOpt;
use ti4_engine_core::generated::{Agenda, Faction, PromissoryNote, Relic, TechType, Technology as T};
use ti4_engine_core::keys::Outcome;
use ti4_engine_core::query::board;
use ti4_engine_core::query::tech::prerequisites;
use ti4_engine_core::rules::production::strategy_cards::Research;
use ti4_engine_core::state::NoteKey;
use ti4_engine_core_tests::inv::ALL_SOURCES;
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::{plain_systems, system_of};

fn research() -> Research {
    Research::new(Seat(0), true, 0, ALL_SOURCES)
}

fn blues(n: usize) -> Vec<Planet> {
    let v: Vec<Planet> = Planet::ALL.into_iter().filter(|p| p.data().specialties == [TechType::Propulsion] && p.data().system.is_some()).take(n).collect();
    assert_eq!(v.len(), n, "enough blue specialty planets");
    v
}

/// A technology that needs `n` blue prerequisites and nothing else, with no faction.
fn blue_tech(n: u16) -> T {
    T::ALL
        .into_iter()
        .find(|t| t.data().faction.is_none() && ti4_engine_core::query::tech::in_deck(*t) && prerequisites(*t) == [0, 0, n, 0])
        .unwrap_or_else(|| panic!("a technology with {n} blue prerequisites"))
}

fn scenario(planets: &[Planet]) -> Scenario {
    let mut systems: Vec<System> = planets.iter().map(|p| system_of(*p)).collect();
    systems.dedup();
    systems.push(plain_systems()[0]);
    let mut seen = Vec::new();
    systems.retain(|s| {
        let fresh = !seen.contains(s);
        seen.push(*s);
        fresh
    });
    let mut sc = Scenario::new(&[Faction::TheArborec, Faction::TheEmbersOfMuaat]).map_from(&systems);
    for p in planets {
        sc = sc.control(*p, 0);
    }
    sc
}

fn run(sc: Scenario, t: T) -> Rig<Research> {
    let mut rig = Rig::new(sc, research());
    rig.run();
    assert!(rig.opts().contains(&Opt::Strategy(SOpt::Research(t))), "{t:?} is on offer: {:?}", rig.opts());
    rig.pick_opt(Opt::Strategy(SOpt::Research(t)));
    rig
}

fn exhausted(rig: &Rig<Research>, ps: &[Planet]) -> Vec<bool> {
    ps.iter().map(|p| board::exhausted(&rig.state, *p)).collect()
}

#[test]
fn a_technology_with_two_blue_prerequisites_exhausts_two_blue_specialty_planets() {
    covers!("rules::production::strategy_cards::Research");
    let t = blue_tech(2);
    let ps = blues(3);
    let rig = run(scenario(&ps), t);
    assert!(rig.state.players().by_seat(Seat(0)).unwrap().tech.owned.contains(t));
    assert_eq!(exhausted(&rig, &ps).iter().filter(|e| **e).count(), 2, "exactly the two the card needs: {:?}", exhausted(&rig, &ps));
}

#[test]
fn an_already_exhausted_planet_does_not_count_and_one_blue_planet_cannot_pay_two() {
    covers!("rules::production::strategy_cards::Research");
    let t = blue_tech(2);
    let ps = blues(2);
    let (mut s, k) = scenario(&ps).build();
    ti4_engine_core::kernel::fx::exhaust_planet(&mut s, ps[0]);
    let mut rig = Rig::from_parts(s, k, research());
    rig.run();
    assert!(!rig.opts().contains(&Opt::Strategy(SOpt::Research(t))), "only one ready blue planet: two prerequisites are not met");
}

#[test]
fn a_research_team_law_or_the_prophets_tears_waive_one_prerequisite_and_spare_a_planet() {
    covers!("rules::production::strategy_cards::Research");
    let t = blue_tech(2);
    let ps = blues(2);
    let other = Planet::ALL.into_iter().find(|p| p.data().system.is_some() && !ps.contains(p) && p.data().specialties.is_empty()).unwrap();
    // The law is elected onto a ready planet we control.
    let mut sc = scenario(&ps).map_from(&[system_of(ps[0]), system_of(ps[1]), system_of(other)]).control(other, 0);
    let (mut s, k) = {
        let (s, k) = sc.clone().build();
        (s, k)
    };
    s.agenda_mut().laws.insert(Agenda::ResearchTeamPropulsion, Outcome::Planet(other));
    let mut rig = Rig::from_parts(s, k, research());
    rig.run();
    rig.pick_opt(Opt::Strategy(SOpt::Research(t)));
    assert_eq!(exhausted(&rig, &ps).iter().filter(|e| **e).count(), 1, "one prerequisite waived, one planet exhausted");
    sc = scenario(&ps);
    let (mut s, k) = sc.build();
    s.players_mut().by_seat_mut(Seat(0)).unwrap().cards.relics.push(Relic::TheProphetsTears);
    let mut rig = Rig::from_parts(s, k, research());
    rig.run();
    rig.pick_opt(Opt::Strategy(SOpt::Research(t)));
    assert_eq!(exhausted(&rig, &ps).iter().filter(|e| **e).count(), 1, "the Prophet's Tears waive one");
}

#[test]
fn a_technology_without_prerequisites_or_a_waived_war_sun_exhausts_nothing() {
    covers!("rules::production::strategy_cards::Research");
    let ps = blues(1);
    let free = run(scenario(&ps), T::AntimassDeflectors);
    assert_eq!(exhausted(&free, &ps), vec![false]);
    // Publicize Weapon Schematics with a war sun owner: the war sun needs nothing.
    let (mut s, k) = scenario(&ps).tech(1, T::WarSun).build();
    s.agenda_mut().laws.insert(Agenda::PublicizeWeaponSchematics, Outcome::For);
    let mut rig = Rig::from_parts(s, k, research());
    rig.run();
    assert!(rig.opts().contains(&Opt::Strategy(SOpt::Research(T::WarSun))));
    rig.pick_opt(Opt::Strategy(SOpt::Research(T::WarSun)));
    assert!(rig.state.players().by_seat(Seat(0)).unwrap().tech.owned.contains(T::WarSun));
    assert_eq!(exhausted(&rig, &ps), vec![false], "no planet is exhausted for a waived war sun");
}

#[test]
fn a_research_agreement_gives_its_holder_the_same_technology_and_goes_back_to_its_owner() {
    covers!("rules::production::strategy_cards::Research");
    let (mut s, k) = Scenario::new(&[Faction::TheArborec, Faction::TheEmbersOfMuaat]).map_ring(4).build();
    let key = NoteKey { note: PromissoryNote::ResearchAgreement, faction: Faction::TheArborec };
    s.cards_mut().promissory.edit().insert(key, Seat(1));
    let mut rig = Rig::from_parts(s.clone(), k.clone(), research());
    rig.run();
    rig.pick_opt(Opt::Strategy(SOpt::Research(T::AntimassDeflectors)));
    assert!(rig.state.players().by_seat(Seat(0)).unwrap().tech.owned.contains(T::AntimassDeflectors));
    assert!(rig.state.players().by_seat(Seat(1)).unwrap().tech.owned.contains(T::AntimassDeflectors), "the holder of the note researches it too");
    assert_eq!(rig.state.cards().promissory.get(&key), Some(&Seat(0)), "the note is back with its owner");
    // A holder that already has the technology: the note stays where it is.
    let mut s2 = s;
    s2.players_mut().by_seat_mut(Seat(1)).unwrap().tech.owned.insert(T::AntimassDeflectors);
    let mut kept = Rig::from_parts(s2, k, research());
    kept.run();
    kept.pick_opt(Opt::Strategy(SOpt::Research(T::AntimassDeflectors)));
    assert_eq!(kept.state.cards().promissory.get(&key), Some(&Seat(1)));
}
