//! D4-2a: `rules::production::pay::PayBill` (the window that pays a price in resources or influence by exhausting planets and spending goods).

use ti4_engine_core::generated::{Planet, Source};
use ti4_engine_core::query::economy::Spend;
use ti4_engine_core::rules::production::pay::PayBill;
use ti4_engine_core::state::PlanetFlags;
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::{plain_systems, system_of};

fn base_planets(wanted_influence: &[u8]) -> Vec<Planet> {
    let mut out: Vec<Planet> = Vec::new();
    for w in wanted_influence {
        let found = plain_systems()
            .iter()
            .flat_map(|s| s.data().planets.iter().copied())
            .find(|p| p.data().influence == *w && p.data().source == Source::Base && !out.contains(p))
            .unwrap_or_else(|| panic!("a planet with {w} influence"));
        out.push(found);
    }
    out
}

fn table(planets: &[Planet], goods: i16) -> Scenario {
    let mut systems: Vec<System> = Vec::new();
    for p in planets {
        let s = system_of(*p);
        if !systems.contains(&s) {
            systems.push(s);
        }
    }
    let mut sc = Scenario::new(&[Faction::TheArborec, Faction::TheEmbersOfMuaat]).map_from(&systems).goods(0, goods);
    for p in planets {
        sc = sc.control(*p, 0);
    }
    sc
}

fn rig(planets: &[Planet], goods: i16, cost: i64) -> Rig<PayBill> {
    Rig::new(table(planets, goods), PayBill::new(Seat(0), Spend::Influence, cost))
}

fn spent_value(r: &Rig<PayBill>, planets: &[Planet]) -> i64 {
    let planet_part: i64 = planets.iter().filter(|p| r.state.board().planet(**p).flags.has(PlanetFlags::EXHAUSTED)).map(|p| i64::from(p.data().influence)).sum();
    let goods_part = i64::from(r.start_state().players().by_seat(Seat(0)).unwrap().econ.trade_goods - r.state.players().by_seat(Seat(0)).unwrap().econ.trade_goods);
    planet_part + goods_part
}

#[test]
fn a_bill_is_paid_with_planets_and_goods_until_the_price_is_met() {
    covers!("rules::production::pay::PayBill");
    let planets = base_planets(&[2, 3]);
    let mut r = rig(&planets, 2, 4);
    r.run();
    assert!(!r.is_done(), "several ways to pay: it asks");
    r.assert_options_sound();
    r.auto(ti4_engine_core_tests::rig::Policy::First);
    assert_eq!(r.driver.window.outcome, Some(true));
    assert!(spent_value(&r, &planets) >= 4, "the price was covered");
    assert!(spent_value(&r, &planets) <= 4 + 3, "and not paid many times over");
}

#[test]
fn a_bill_nobody_can_pay_is_refused_before_anything_is_spent() {
    covers!("rules::production::pay::PayBill");
    let planets = base_planets(&[1]);
    let mut r = rig(&planets, 1, 5);
    r.run();
    assert!(r.is_done());
    assert_eq!(r.driver.window.outcome, Some(false));
    assert_eq!(delta(r.start_state(), &r.state), "", "nothing was spent on a bill that cannot be met");
}

#[test]
fn a_bill_of_nothing_is_paid_at_once_and_an_exhausted_planet_pays_nothing() {
    covers!("rules::production::pay::PayBill");
    let planets = base_planets(&[2]);
    let mut free = rig(&planets, 0, 0);
    free.run();
    assert_eq!(free.driver.window.outcome, Some(true));
    let mut spent = Rig::new(table(&planets, 0).exhaust(planets[0]), PayBill::new(Seat(0), Spend::Influence, 2));
    spent.run();
    assert_eq!(spent.driver.window.outcome, Some(false), "the only planet is exhausted");
}

#[test]
fn a_bill_smoke() {
    let planets = base_planets(&[1, 2, 3]);
    smoke("pay_bill/three_planets_and_goods", || rig(&planets, 3, 5));
}
