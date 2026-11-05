//! D4-2b: the hero windows of `rules::leader_windows`: Rin, the Masters' Legacy (swap technologies) and Xxekir Grom of Thunder's Edge (place PDS and mechs).

use ti4_engine_core::choice::leaders::{HeroUnit, Opt as LOpt};
use ti4_engine_core::rules::leader_windows::{RinHero, XxchaHeroTe};
use ti4_engine_core::state::PlanetFlags;
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::{planets_with_influence, system_of};

fn all_techs() -> Vec<Technology> {
    Technology::ALL.to_vec()
}

fn is_unit_upgrade(t: Technology) -> bool {
    t.data().types.contains(&TechType::Unitupgrade)
}

fn rin(owned: &[Technology]) -> Rig<RinHero> {
    let mut sc = Scenario::new(&[Faction::TheUniversitiesOfJolNar, Faction::TheArborec]);
    for t in owned {
        sc = sc.tech(0, *t);
    }
    Rig::new(sc, RinHero::new(Seat(0), all_techs(), all_techs()))
}

#[test]
fn rin_offers_each_replacement_of_the_same_colour_and_keeping_comes_last() {
    covers!("rules::leader_windows::RinHero");
    let held = Technology::GravityDrive;
    let mut rig = rin(&[held]);
    rig.run();
    let opts = rig.opts();
    assert_eq!(opts.last(), Some(&Opt::Leaders(LOpt::Keep)), "a decider that takes the first option trades; declining is last");
    let trades: Vec<Technology> = opts.iter().filter_map(|o| if let Opt::Leaders(LOpt::Trade(t)) = o { Some(*t) } else { None }).collect();
    assert!(!trades.is_empty());
    let colour = held.data().types[0];
    for t in &trades {
        assert_eq!(t.data().types.first(), Some(&colour), "{t:?}: same colour");
        assert!(!is_unit_upgrade(*t) && *t != held, "{t:?}");
    }
    rig.assert_options_sound();
}

#[test]
fn rin_trading_replaces_the_technology_and_keeping_changes_nothing_but_still_counts_as_used() {
    covers!("rules::leader_windows::RinHero");
    let held = Technology::GravityDrive;
    let mut rig = rin(&[held]);
    rig.run();
    let Opt::Leaders(LOpt::Trade(new)) = rig.opts()[0] else { panic!("a trade first") };
    rig.pick(0);
    assert!(rig.is_done() && rig.driver.window.used);
    let owned = &rig.state.players().by_seat(Seat(0)).unwrap().tech.owned;
    assert!(!owned.contains(held) && owned.contains(new));
    let mut keep = rin(&[held]);
    keep.run().pick_opt(Opt::Leaders(LOpt::Keep));
    assert!(keep.is_done() && keep.driver.window.used, "an opportunity declined is still a use");
    assert!(keep.state.players().by_seat(Seat(0)).unwrap().tech.owned.contains(held));
    let _ = keep.replay();
}

/// Heavy fix 1 (bench seeds v2/69, v4/7, v5/99, v5/10 of the int-26 heavy run): since lane core-lent-gaps a swap re-forms the seat's units
/// (90.8, the string's `apply_unit_upgrades` after each swap) whenever the window holds the faction roster, which the native play always lends.
/// A plain mech on the board of a Jol-Nar seat becomes its own mech; without the roster nothing changes. This is the rules change that moved
/// the four bench games; the baselines were re-recorded for it (docs/HEAVYFIX1_RESUME.md).
#[test]
fn rin_re_forms_the_seats_units_after_a_swap_when_the_roster_is_held() {
    covers!("rules::leader_windows::RinHero");
    let (system, planet) = ti4_engine_core_tests::scenario::first_planet();
    let build = |roster: bool| {
        let sc = Scenario::new(&[Faction::TheUniversitiesOfJolNar, Faction::TheArborec])
            .tech(0, Technology::GravityDrive)
            .unit(system, Some(planet), Unit::FactionlessMech, 0, 1);
        let mut rig = Rig::new(sc, RinHero::new(Seat(0), all_techs(), all_techs()));
        if roster {
            let mut table = ti4_engine_core::kernel::ability::AbilityTable::default();
            let mut units = ti4_engine_core::query::tech::UnitRoster::default();
            units.set(Faction::TheUniversitiesOfJolNar, UnitBaseType::Mech, Some(Unit::ShieldPaling));
            table.facts.roster = Some(Box::leak(Box::new(units)));
            rig = rig.with_table(table);
        }
        rig.run();
        let Opt::Leaders(LOpt::Trade(_)) = rig.opts()[0] else { panic!("a trade first") };
        rig.pick(0);
        assert!(rig.is_done() && rig.driver.window.used);
        rig.state.board().planet(planet).units.as_slice().iter().map(|u| u.kind()).collect::<Vec<_>>()
    };
    assert_eq!(build(true), vec![Unit::ShieldPaling], "the plain mech became the faction's own after the swap");
    assert_eq!(build(false), vec![Unit::FactionlessMech], "without the roster the window cannot re-form (the lent windows that hold none)");
}

#[test]
fn rin_with_nothing_to_swap_is_not_used_and_unit_upgrades_are_not_swapped() {
    covers!("rules::leader_windows::RinHero");
    let mut none = rin(&[]);
    none.run();
    assert!(none.is_done() && !none.driver.window.used);
    let mut upgrade = rin(&[Technology::CruiserII]);
    upgrade.run();
    assert!(upgrade.is_done() && !upgrade.driver.window.used, "a unit upgrade is not a technology Rin may replace");
}

#[test]
fn rin_asks_once_per_swappable_technology_and_survives_every_policy() {
    covers!("rules::leader_windows::RinHero");
    let mut rig = rin(&[Technology::GravityDrive, Technology::PlasmaScoring]);
    rig.run();
    let mut asked = 0;
    while !rig.is_done() {
        asked += 1;
        rig.assert_options_sound();
        rig.pick_opt(Opt::Leaders(LOpt::Keep));
    }
    assert_eq!(asked, 2);
    smoke_quiet(|| rin(&[Technology::GravityDrive, Technology::PlasmaScoring]));
}

fn mech() -> Unit {
    Unit::ALL.into_iter().find(|u| u.data().base_type == UnitBaseType::Mech && !u.data().structure).expect("a mech")
}

fn xxekir(planets: &[Planet], pds_left: usize, mech_left: usize) -> Rig<XxchaHeroTe> {
    let mut sc = Scenario::new(&[Faction::TheXxchaKingdom, Faction::TheArborec]);
    let spots: Vec<(System, Planet)> = planets.iter().map(|p| (system_of(*p), *p)).collect();
    let systems: Vec<System> = ti4_engine_core_tests::scenario::systems_of(planets);
    sc = sc.map_from(&systems);
    for p in planets {
        sc = sc.control(*p, 0).exhaust(*p);
    }
    Rig::new(sc, XxchaHeroTe::new(Seat(0), &spots, Some((Unit::PDSI, pds_left)), Some((mech(), mech_left))))
}

fn count(rig: &Rig<XxchaHeroTe>, planet: Planet, unit: Unit) -> usize {
    rig.state.board().planet(planet).units.as_slice().iter().filter(|p| p.kind() == unit && p.owner() == Seat(0)).count()
}

#[test]
fn xxekir_places_the_chosen_units_on_the_chosen_planets_and_readies_each_planet_it_used() {
    covers!("rules::leader_windows::XxchaHeroTe");
    let planets = planets_with_influence(&[1, 2]);
    let mut rig = xxekir(&planets, 2, 1);
    rig.run();
    assert_eq!(rig.opts(), vec![Opt::Leaders(LOpt::HeroPlace(HeroUnit::Pds)), Opt::Leaders(LOpt::HeroPlace(HeroUnit::Mech)), Opt::Leaders(LOpt::HeroStop)]);
    rig.assert_options_sound();
    rig.pick_opt(Opt::Leaders(LOpt::HeroPlace(HeroUnit::Pds)));
    assert_eq!(rig.opts().len(), 2, "which planet gets it");
    let choice = rig.opts()[1];
    rig.pick(1);
    rig.pick_opt(Opt::Leaders(LOpt::HeroStop));
    assert!(rig.is_done() && rig.driver.window.used);
    let Opt::Leaders(LOpt::HeroPlanet(chosen, _)) = choice else { panic!("a planet") };
    let other = planets.iter().copied().find(|p| *p != chosen).unwrap();
    assert_eq!(count(&rig, chosen, Unit::PDSI), 1);
    assert!(!rig.state.board().planet(chosen).flags.has(PlanetFlags::EXHAUSTED), "readied");
    assert!(rig.state.board().planet(other).flags.has(PlanetFlags::EXHAUSTED), "the other planet got nothing and stays exhausted");
}

#[test]
fn xxekir_stops_after_four_placements_and_offers_only_what_is_left_of_each_kind() {
    covers!("rules::leader_windows::XxchaHeroTe");
    let planets = planets_with_influence(&[1]);
    let mut rig = xxekir(&planets, 9, 0);
    rig.run();
    assert_eq!(rig.opts(), vec![Opt::Leaders(LOpt::HeroPlace(HeroUnit::Pds)), Opt::Leaders(LOpt::HeroStop)], "no mech left to place");
    for _ in 0..4 {
        rig.pick_opt(Opt::Leaders(LOpt::HeroPlace(HeroUnit::Pds)));
    }
    assert!(rig.is_done(), "four is the most");
    assert_eq!(count(&rig, planets[0], Unit::PDSI), 4);
}

#[test]
fn xxekir_with_no_planet_is_never_used_and_a_foreign_answer_is_refused() {
    covers!("rules::leader_windows::XxchaHeroTe");
    let mut none = xxekir(&[], 1, 1);
    none.run();
    assert!(none.is_done() && !none.driver.window.used);
    let planets = planets_with_influence(&[1]);
    let mut rig = xxekir(&planets, 1, 0);
    rig.run();
    rig.expect_illegal(Pick::Opt(Opt::Leaders(LOpt::HeroPlace(HeroUnit::Mech))));
    rig.expect_illegal(Pick::Opt(Opt::Decline));
    smoke_quiet(|| xxekir(&planets_with_influence(&[1, 2]), 2, 1));
}
