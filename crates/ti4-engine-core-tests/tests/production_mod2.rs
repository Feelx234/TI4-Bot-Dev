//! D4 lane A3: `rules/production/mod.rs` beyond lane A1: where a unit may be placed (`placements`, `spots_of`), what a unit costs and how many a
//! purchase makes (`build_for`, `post_build` through `BuildInfo`), what a bill may be paid with (`payment_faces`, `available`, `pay_opts`) and the
//! modules that add production (Magmus Reactor, Aerie Hololattice) or forbid it (Mitosis). LRR 67 (production) and 68.

use ti4_engine_core::choice::production::{ChooseAsk, Currency, Icon, Opt as POpt, PlaceAsk, Spot};
use ti4_engine_core::generated::{Agenda, Attachment, Breakthrough, Leader, Planet, Relic, Technology, UnitBaseType};
use ti4_engine_core::query::economy::Spend;
use ti4_engine_core::rules::production::{self as prod, BuildInfo, Cx, Extras, Held};
use ti4_engine_core::state::{LeaderStatus, Outcome};
use ti4_engine_core_tests::inv::ALL_SOURCES;
use ti4_engine_core_tests::prelude::*;

/// A system with two planets: New Albion (1 resource, 1 influence) and Starpoint (3 resources, 1 influence).
const SYS: System = System::StarpointNewAlbion;

fn two() -> (Planet, Planet) {
    let ps = SYS.data().planets;
    assert_eq!(ps.len(), 2);
    let (a, b) = (ps[0], ps[1]);
    assert!(a.data().resources < b.data().resources);
    (a, b)
}

fn duel(f0: Faction) -> Scenario {
    Scenario::new(&[f0, Faction::TheEmiratesOfHacan]).map_from(&[SYS, System::Wellon])
}

fn base() -> Scenario {
    duel(Faction::TheFederationOfSol)
}

fn cx<'a>(s: &'a CoreState, k: &'a Kernel) -> Cx<'a> {
    Cx::new(s, k, ALL_SOURCES, false)
}

fn ask() -> ChooseAsk {
    ChooseAsk {
        who: Seat(0),
        system: SYS,
        limit: 8,
        remaining: 8,
        free_capacity: 0,
        small_exemptions: 0,
        credit: 0,
        discount_remaining: 0,
        fleet_limit: 12,
        free_this_use: false,
        flagship_free: false,
        only_unit: None,
        max_unit_cost: None,
        produced: [0; 11],
        armada: 0,
        buildable: [Some(Unit::CruiserI), Some(Unit::FighterI), Some(Unit::InfantryI), Some(Unit::DreadnoughtI), Some(Unit::CarrierI), None, None, None, None],
        icon: Icon::Fresh,
    }
}

/// Seat 0 holds both planets, a dock on each, and has resources enough.
fn docked() -> Scenario {
    let (a, b) = two();
    base().control(a, 0).control(b, 0).unit(SYS, Some(a), Unit::SpaceDockI, 0, 1).unit(SYS, Some(b), Unit::SpaceDockI, 0, 1)
}

fn build(s: &CoreState, k: &Kernel, a: &ChooseAsk, unit: Unit) -> Option<BuildInfo> {
    let c = cx(s, k);
    let ex = Extras::default();
    let before = prod::standing_before(&c, Seat(0), a);
    let spendable = prod::available(&c, Seat(0), Spend::Resources, &ex);
    prod::build_for(&c, Seat(0), a, unit, &ex, &before, spendable)
}

// ---- where a unit may go -----------------------------------------------------------------------------

#[test]
fn a_ship_is_placed_in_space_unless_another_players_ship_blockades_the_system() {
    let (s, k) = docked().build();
    let c = cx(&s, &k);
    assert_eq!(prod::placements(&c, Seat(0), SYS, Unit::CruiserI).as_slice(), &[Spot::Space]);
    assert_eq!(prod::placements(&c, Seat(0), SYS, Unit::FighterI).as_slice(), &[Spot::Space]);
    let (s, k) = docked().unit(SYS, None, Unit::CruiserI, 1, 1).build();
    assert!(prod::placements(&cx(&s, &k), Seat(0), SYS, Unit::CruiserI).is_empty(), "a rival's ship in the system");
    // A rival's ground force is no blockade; our own ship is none either.
    let (s, k) = docked().unit(SYS, Some(two().0), Unit::InfantryI, 1, 2).unit(SYS, None, Unit::CruiserI, 0, 1).build();
    assert_eq!(prod::placements(&cx(&s, &k), Seat(0), SYS, Unit::CruiserI).as_slice(), &[Spot::Space]);
}

#[test]
fn ground_forces_and_structures_go_to_the_planets_of_the_producing_docks_in_planet_order() {
    let (a, b) = two();
    let (s, k) = docked().build();
    let c = cx(&s, &k);
    let mut want = vec![Spot::Planet(a), Spot::Planet(b)];
    want.sort();
    let got = prod::placements(&c, Seat(0), SYS, Unit::InfantryI);
    let mut sorted = got.as_slice().to_vec();
    sorted.sort();
    assert_eq!(sorted, want, "a planet with a dock each; planet order");
    assert_eq!(got.as_slice(), prod::placements(&c, Seat(0), SYS, Unit::FactionlessMech).as_slice());
    // Only one dock: only that planet.
    let (s, k) = base().control(a, 0).unit(SYS, Some(a), Unit::SpaceDockI, 0, 1).build();
    assert_eq!(prod::placements(&cx(&s, &k), Seat(0), SYS, Unit::InfantryI).as_slice(), &[Spot::Planet(a)]);
    // No dock, no planet.
    let (s, k) = base().control(a, 0).build();
    assert!(prod::placements(&cx(&s, &k), Seat(0), SYS, Unit::InfantryI).is_empty());
    // A rival's dock is not ours.
    let (s, k) = base().control(a, 1).unit(SYS, Some(a), Unit::SpaceDockI, 1, 1).build();
    assert!(prod::placements(&cx(&s, &k), Seat(0), SYS, Unit::InfantryI).is_empty());
}

#[test]
fn a_structure_goes_only_where_the_limit_the_zones_and_the_holy_planet_allow() {
    let (a, b) = two();
    // A second space dock cannot go onto the planet that has one (79.2): only the other planet is offered.
    let (s, k) = docked().build();
    assert!(prod::placements(&cx(&s, &k), Seat(0), SYS, Unit::SpaceDockI).is_empty(), "a dock on both planets");
    let (s, k) = base().control(a, 0).control(b, 0).unit(SYS, Some(a), Unit::SpaceDockI, 0, 1).unit(SYS, Some(b), Unit::PDSI, 0, 1).build();
    assert_eq!(prod::placements(&cx(&s, &k), Seat(0), SYS, Unit::PDSI).as_slice(), &[Spot::Planet(a)], "b has no producer; a has the dock");
    // A demilitarized zone takes no unit: the dock on it produces nothing there.
    let (mut s, k) = docked().build();
    s.agenda_mut().laws.insert(Agenda::DemilitarizedZone, Outcome::Planet(a));
    assert_eq!(prod::placements(&cx(&s, &k), Seat(0), SYS, Unit::InfantryI).as_slice(), &[Spot::Planet(b)]);
    // The blanked laws no longer forbid it.
    let c = Cx::new(&s, &k, ALL_SOURCES, true);
    assert_eq!(prod::placements(&c, Seat(0), SYS, Unit::InfantryI).len(), 2);
    // Holy Planet of Ixth: no production on the elected planet.
    let (mut s, k) = docked().build();
    s.agenda_mut().laws.insert(Agenda::HolyPlanetOfIxth, Outcome::Planet(b));
    assert_eq!(prod::placements(&cx(&s, &k), Seat(0), SYS, Unit::InfantryI).as_slice(), &[Spot::Planet(a)]);
    // A space station takes no unit of the dock's system.
    let station = System::TaranaOluzStation;
    let sp = station.data().planets;
    let (s, k) = Scenario::new(&[Faction::TheFederationOfSol, Faction::TheEmiratesOfHacan])
        .map_from(&[station])
        .control(sp[1], 0)
        .control(sp[0], 0)
        .unit(station, Some(sp[1]), Unit::SpaceDockI, 0, 1)
        .unit(station, Some(sp[0]), Unit::SpaceDockI, 0, 1)
        .build();
    assert_eq!(prod::placements(&cx(&s, &k), Seat(0), station, Unit::InfantryI).as_slice(), &[Spot::Planet(sp[0])]);
}

#[test]
fn mitosis_keeps_the_arborec_docks_from_producing_infantry() {
    let (a, b) = two();
    let arb = duel(Faction::TheArborec).control(a, 0).control(b, 0).unit(SYS, Some(a), Unit::SpaceDockI, 0, 1).unit(SYS, Some(b), Unit::SpaceDockI, 0, 1);
    let (s, k) = arb.clone().build();
    let c = cx(&s, &k);
    assert!(prod::placements(&c, Seat(0), SYS, Unit::InfantryI).is_empty(), "docks do not produce infantry");
    assert_eq!(prod::placements(&c, Seat(0), SYS, Unit::FactionlessMech).len(), 2);
    assert_eq!(prod::placements(&c, Seat(0), SYS, Unit::CruiserI).as_slice(), &[Spot::Space]);
    // The capacity that cannot make infantry is the docks' whole production.
    let all = prod::capacity(&c, Seat(0), SYS);
    assert_eq!(prod::barred_capacity(&c, Seat(0), SYS, UnitBaseType::Infantry), all);
    assert_eq!(prod::barred_capacity(&c, Seat(0), SYS, UnitBaseType::Fighter), 0);
    // The Federation's docks are not barred.
    let (s2, k2) = docked().build();
    assert_eq!(prod::barred_capacity(&cx(&s2, &k2), Seat(0), SYS, UnitBaseType::Infantry), 0);
    // A purchase of infantry is not offered when nothing can place them; mechs are.
    assert!(build(&s, &k, &ask(), Unit::InfantryI).is_none());
}

#[test]
fn magmus_reactor_gives_a_supernova_with_the_holders_units_production_five() {
    let nova = System::Supernova43;
    let sc = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec]).map_from(&[nova]).unit(nova, None, Unit::CruiserI, 0, 1).tech(0, Technology::MagmusReactor);
    let (s, k) = sc.clone().build();
    let c = cx(&s, &k);
    assert_eq!(prod::module_production(&c, Seat(0), nova), 5);
    assert_eq!(prod::capacity(&c, Seat(0), nova), 5);
    assert!(prod::module_planet_producers(&c, Seat(0), nova).is_empty(), "the supernova has no planet");
    assert_eq!(prod::placements(&c, Seat(0), nova, Unit::InfantryI).as_slice(), &[Spot::Space], "the production of the tile itself");
    // Without the reactor, or without a unit there, nothing.
    let (s, k) = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec]).map_from(&[nova]).unit(nova, None, Unit::CruiserI, 0, 1).build();
    assert_eq!(prod::module_production(&cx(&s, &k), Seat(0), nova), 0);
    let (s, k) = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec]).map_from(&[nova]).tech(0, Technology::MagmusReactor).build();
    assert_eq!(prod::module_production(&cx(&s, &k), Seat(0), nova), 0);
    assert!(prod::placements(&cx(&s, &k), Seat(0), nova, Unit::InfantryI).is_empty());
}

#[test]
fn the_aerie_hololattice_gives_a_planet_with_a_structure_production_one() {
    let (a, b) = two();
    let sc = duel(Faction::TheArgentFlight).control(a, 0).unit(SYS, Some(a), Unit::PDSI, 0, 1).unit(SYS, Some(b), Unit::InfantryI, 0, 1).tech(0, Technology::AerieHololattice);
    let (s, k) = sc.build();
    let c = cx(&s, &k);
    // The PDS makes the planet a producer of 1; the planet with infantry alone is not, and the infantry does not count as structure.
    assert_eq!(prod::module_planet_producers(&c, Seat(0), SYS).as_slice(), &[(a, 1)]);
    assert_eq!(prod::module_production(&c, Seat(0), SYS), 1);
    assert_eq!(prod::capacity(&c, Seat(0), SYS), 1);
    assert_eq!(prod::placements(&c, Seat(0), SYS, Unit::InfantryI).as_slice(), &[Spot::Planet(a)]);
    // Without the technology, none.
    let (s2, k2) = duel(Faction::TheArgentFlight).control(a, 0).unit(SYS, Some(a), Unit::PDSI, 0, 1).build();
    assert!(prod::module_planet_producers(&cx(&s2, &k2), Seat(0), SYS).is_empty());
}

#[test]
fn a_planet_with_two_places_offers_each_only_when_there_are_two_or_more() {
    let (a, b) = two();
    let (s, k) = docked().build();
    let c = cx(&s, &k);
    let place = PlaceAsk { who: Seat(0), system: SYS, unit: Unit::InfantryI, made: 2, fleet_limit: 12, icon: Icon::Fresh };
    let opts = prod::place_opts(&c, &place);
    assert_eq!(opts.len(), 2);
    assert!(opts.as_slice().iter().all(|o| matches!(o, POpt::Place { spot: Spot::Planet(p) } if *p == a || *p == b)));
    // One place only: nothing to choose.
    let (s1, k1) = base().control(a, 0).unit(SYS, Some(a), Unit::SpaceDockI, 0, 1).build();
    assert!(prod::place_opts(&cx(&s1, &k1), &place).is_empty());
    assert_eq!(prod::spots_of(&cx(&s1, &k1), Seat(0), SYS, Unit::InfantryI, Icon::Fresh).as_slice(), &[Spot::Planet(a)]);
    // A unit with no spot has none, and no module adds any.
    assert!(prod::spots_of(&cx(&s1, &k1), Seat(0), System::Wellon, Unit::InfantryI, Icon::Fresh).is_empty());
    // The facts of one spot: how many arrive and the standing before and after.
    let info = prod::place_info(&c, &place, Spot::Planet(a));
    assert_eq!(info.placed, 2);
    assert!(info.after.fighters + info.after.carried >= info.before.fighters + info.before.carried);
    assert_eq!(info.before, prod::place_home_standing(&c, &place));
}

// ---- the price and the number made --------------------------------------------------------------------

#[test]
fn a_unit_is_offered_when_it_is_affordable_and_has_a_place_and_the_supply_has_one() {
    let (s, k) = docked().goods(0, 0).build();
    let info = build(&s, &k, &ask(), Unit::CruiserI).expect("a cruiser");
    assert_eq!((info.printed, info.pair, info.cost, info.discount_used, info.made, info.placed), (2, 1, 2, 0, 1, 1));
    assert!(info.affordable && !info.exchange);
    assert_eq!(info.spots.as_slice(), &[Spot::Space]);
    let fighter = build(&s, &k, &ask(), Unit::FighterI).unwrap();
    assert_eq!((fighter.printed, fighter.pair, fighter.made), (1, 2, 2), "two fighters for one resource");
    assert_eq!(build(&s, &k, &ask(), Unit::DreadnoughtI).unwrap().cost, 4);
    // A bill the planets cannot pay: no offer. The two planets are worth 1 + 3 = 4 resources.
    let (poor, kp) = base().control(two().0, 0).unit(SYS, Some(two().0), Unit::SpaceDockI, 0, 1).build();
    assert!(build(&poor, &kp, &ask(), Unit::DreadnoughtI).is_none(), "4 resources are needed, 1 is available");
    assert!(build(&poor, &kp, &ask(), Unit::CruiserI).is_none());
    assert!(build(&poor, &kp, &ask(), Unit::FighterI).is_some(), "one resource pays two fighters");
    // Trade goods and a credit pay too.
    let (rich, kr) = base().control(two().0, 0).unit(SYS, Some(two().0), Unit::SpaceDockI, 0, 1).goods(0, 3).build();
    assert!(build(&rich, &kr, &ask(), Unit::DreadnoughtI).is_some());
    let credit = ChooseAsk { credit: 3, ..ask() };
    assert!(build(&poor, &kp, &credit, Unit::DreadnoughtI).is_some());
    // The system is blockaded: ships have no place.
    let (blocked, kb) = docked().unit(SYS, None, Unit::CruiserI, 1, 1).build();
    assert!(build(&blocked, &kb, &ask(), Unit::CruiserI).is_none());
    assert!(build(&blocked, &kb, &ask(), Unit::InfantryI).is_some());
}

#[test]
fn only_one_unit_and_a_cost_ceiling_narrow_the_offer() {
    let (s, k) = docked().build();
    let only = ChooseAsk { only_unit: Some(Unit::CarrierI), ..ask() };
    assert!(build(&s, &k, &only, Unit::CruiserI).is_none());
    assert!(build(&s, &k, &only, Unit::CarrierI).is_some());
    // A ceiling of 2 resources: a cruiser (2) passes, a carrier (3) does not; the pair of fighters (half a resource each) passes.
    let cap = ChooseAsk { max_unit_cost: Some(2), ..ask() };
    assert!(build(&s, &k, &cap, Unit::CruiserI).is_some());
    assert!(build(&s, &k, &cap, Unit::CarrierI).is_none());
    assert!(build(&s, &k, &cap, Unit::FighterI).is_some());
    let none = ChooseAsk { max_unit_cost: Some(0), ..ask() };
    assert!(build(&s, &k, &none, Unit::FighterI).is_none(), "even half a resource costs more than nothing");
    assert!(build(&s, &k, &ChooseAsk { max_unit_cost: Some(1), ..ask() }, Unit::FighterI).is_some());
}

#[test]
fn a_discount_a_free_use_and_a_free_flagship_change_what_is_paid() {
    let (s, k) = docked().build();
    let paid = build(&s, &k, &ask(), Unit::DreadnoughtI).unwrap();
    assert_eq!((paid.cost, paid.discount_used, paid.printed), (4, 0, 4));
    let discount = ChooseAsk { discount_remaining: 3, ..ask() };
    let d = build(&s, &k, &discount, Unit::DreadnoughtI).unwrap();
    assert_eq!((d.cost, d.discount_used), (1, 3));
    let big = ChooseAsk { discount_remaining: 9, ..ask() };
    let d = build(&s, &k, &big, Unit::DreadnoughtI).unwrap();
    assert_eq!((d.cost, d.discount_used), (0, 4), "no more than the price is taken");
    let free = ChooseAsk { free_this_use: true, ..ask() };
    let f = build(&s, &k, &free, Unit::DreadnoughtI).unwrap();
    assert_eq!((f.cost, f.discount_used), (0, 4), "Harrugh Gefhara: the whole price is the discount");
    // The Nomad commander waives only the flagship's resources.
    let waived = ChooseAsk { flagship_free: true, buildable: [Some(Unit::FactionlessFlagship), Some(Unit::DreadnoughtI), None, None, None, None, None, None, None], ..ask() };
    let flagship = build(&s, &k, &waived, Unit::FactionlessFlagship).unwrap();
    assert_eq!((flagship.cost, flagship.discount_used), (0, 0));
    assert_eq!(build(&s, &k, &waived, Unit::DreadnoughtI).unwrap().cost, 4);
    // Affordability follows the cost left after the waiver: a seat with nothing can still build the free flagship.
    let (broke, kb) = base().control(two().0, 1).unit(SYS, None, Unit::CarrierI, 0, 1).unit(SYS, Some(two().0), Unit::SpaceDockI, 0, 1).build();
    assert!(build(&broke, &kb, &waived, Unit::FactionlessFlagship).is_none() || build(&broke, &kb, &waived, Unit::FactionlessFlagship).unwrap().cost == 0);
}

#[test]
fn the_number_made_is_the_pair_within_what_is_left_of_the_limit_and_small_exemptions_add_one_for_fighters_and_infantry() {
    let (s, k) = docked().build();
    let at = |remaining: i32, small: i32, unit: Unit| {
        let a = ChooseAsk { remaining, small_exemptions: small, ..ask() };
        build(&s, &k, &a, unit).map(|i| (i.made, i.placed))
    };
    assert_eq!(at(8, 0, Unit::FighterI), Some((2, 2)));
    assert_eq!(at(1, 0, Unit::FighterI), Some((1, 1)), "only one unit of the limit is left");
    assert_eq!(at(0, 0, Unit::FighterI), None);
    assert_eq!(at(0, 1, Unit::FighterI), Some((1, 1)), "the Cabal commander's exemption");
    assert_eq!(at(0, 1, Unit::InfantryI), Some((1, 1)));
    assert_eq!(at(0, 2, Unit::FighterI), Some((2, 2)));
    assert_eq!(at(0, 5, Unit::FighterI), Some((2, 2)), "never more than the pair");
    assert_eq!(at(0, 1, Unit::CruiserI), None, "the exemptions are for fighters and infantry only");
    assert_eq!(at(0, 1, Unit::FactionlessMech), None);
    assert_eq!(at(1, 0, Unit::CruiserI), Some((1, 1)));
}

#[test]
fn the_supply_caps_how_many_units_arrive() {
    // Five dreadnoughts are all the plastic there is: with four on the board one more can arrive, with five none.
    let four = docked().unit(System::Wellon, None, Unit::DreadnoughtI, 0, 4);
    let (s, k) = four.build();
    let info = build(&s, &k, &ask(), Unit::DreadnoughtI).unwrap();
    assert_eq!((info.made, info.placed), (1, 1));
    let (s5, k5) = docked().unit(System::Wellon, None, Unit::DreadnoughtI, 0, 5).build();
    assert!(build(&s5, &k5, &ask(), Unit::DreadnoughtI).is_none(), "no dreadnought left in the supply");
    assert_eq!(prod::placed_of(&cx(&s5, &k5), Seat(0), Unit::DreadnoughtI, 1), 0);
    assert_eq!(prod::placed_of(&cx(&s, &k), Seat(0), Unit::DreadnoughtI, 3), 1);
    assert_eq!(prod::placed_of(&cx(&s, &k), Seat(0), Unit::CruiserI, 1), 1);
    // Eight cruisers in the box (seven out: one left); fighters and infantry are not limited by the plastic.
    let (sc, kc) = docked().unit(System::Wellon, None, Unit::CruiserI, 0, 7).build();
    assert_eq!(prod::placed_of(&cx(&sc, &kc), Seat(0), Unit::CruiserI, 3), 1);
    let (sf, kf) = docked().unit(System::Wellon, None, Unit::FighterI, 0, 20).build();
    assert_eq!(prod::placed_of(&cx(&sf, &kf), Seat(0), Unit::FighterI, 2), 2);
    // A rival's pieces do not count against the seat's supply.
    let (sr, kr) = docked().unit(System::Wellon, None, Unit::DreadnoughtI, 1, 5).build();
    assert_eq!(prod::placed_of(&cx(&sr, &kr), Seat(0), Unit::DreadnoughtI, 2), 2);
}

#[test]
fn what_is_left_of_the_limit_and_of_the_free_capacity_after_a_build() {
    let (s, k) = docked().build();
    let after = |a: ChooseAsk, unit: Unit| {
        let i = build(&s, &k, &a, unit).unwrap();
        (i.remaining_after, i.free_after)
    };
    // A cruiser takes one place of the limit; it opens no free capacity.
    assert_eq!(after(ask(), Unit::CruiserI), (7, 0));
    // Two fighters take two places...
    assert_eq!(after(ask(), Unit::FighterI), (6, 0));
    // ... unless free capacity (Bellum Gloriosum's open capacity ships) carries them, and what it carried is used up.
    let free = ChooseAsk { free_capacity: 3, ..ask() };
    assert_eq!(after(free, Unit::FighterI), (8, 1));
    let free1 = ChooseAsk { free_capacity: 1, ..ask() };
    assert_eq!(after(free1, Unit::FighterI), (7, 0), "one carried free, one takes a place");
    // Small exemptions are spent by fighters and infantry after the free capacity.
    let small = ChooseAsk { small_exemptions: 1, ..ask() };
    assert_eq!(after(small, Unit::FighterI), (7, 0));
    assert_eq!(after(small, Unit::InfantryI), (7, 0));
    assert_eq!(after(small, Unit::CruiserI), (7, 1), "a cruiser does not use the exemption");
    // A mech is a ground force: free capacity carries it, an exemption does not.
    let mixed = ChooseAsk { free_capacity: 1, small_exemptions: 1, ..ask() };
    assert_eq!(after(mixed, Unit::FactionlessMech), (8, 1));
    assert_eq!(after(mixed, Unit::FighterI), (8, 0), "one by free capacity, one by the exemption");
}

#[test]
fn bellum_gloriosum_opens_the_capacity_of_the_ship_built() {
    let (mut s, k) = docked().build();
    s.players_mut().by_seat_mut(Seat(0)).unwrap().cards.breakthrough = Some(Breakthrough::BellumGloriosum);
    let info = build(&s, &k, &ask(), Unit::CarrierI).unwrap();
    // A carrier carries four: they are free capacity for the fighters and ground forces that follow.
    assert_eq!((info.remaining_after, info.free_after), (7, 4));
    let cruiser = build(&s, &k, &ask(), Unit::CruiserI).unwrap();
    assert_eq!(cruiser.free_after, i64::from(Unit::CruiserI.data().capacity.unwrap_or(0)));
    // A fighter opens nothing and a ground force is not a ship.
    assert_eq!(build(&s, &k, &ask(), Unit::InfantryI).unwrap().free_after, 0);
    // Without the breakthrough no capacity is opened.
    let (plain, kp) = docked().build();
    assert_eq!(build(&plain, &kp, &ask(), Unit::CarrierI).unwrap().free_after, 0);
}

#[test]
fn the_standing_after_a_build_is_given_only_when_there_is_one_place_to_put_it() {
    let (s, k) = docked().build();
    let ship = build(&s, &k, &ask(), Unit::CruiserI).unwrap();
    assert!(ship.after_paid.is_some(), "a cruiser has the one place, space");
    assert!(ship.after_exchange.is_none() && ship.exchange_after == (0, 0), "no exchange offered");
    let ground = build(&s, &k, &ask(), Unit::InfantryI).unwrap();
    assert_eq!(ground.spots.len(), 2);
    assert!(ground.after_paid.is_none(), "two planets to choose from");
    let a = ship.after_paid.unwrap();
    assert!(a.ships > ship.before.ships, "one ship more");
    assert_eq!(a.fleet_charged, ship.before.fleet_charged + 1);
}

#[test]
fn the_buildable_units_are_listed_in_option_order_each_with_the_number_made() {
    let (s, k) = docked().build();
    let c = cx(&s, &k);
    let opts = prod::build_opts(&c, Seat(0), &ask(), &Extras::default());
    assert_eq!(
        opts.as_slice(),
        &[
            POpt::Build { unit: Unit::CruiserI, made: 1 },
            POpt::Build { unit: Unit::FighterI, made: 2 },
            POpt::Build { unit: Unit::InfantryI, made: 2 },
            POpt::Build { unit: Unit::DreadnoughtI, made: 1 },
            POpt::Build { unit: Unit::CarrierI, made: 1 },
        ]
    );
    // What the seat cannot afford is left out.
    let (poor, kp) = base().control(two().0, 0).unit(SYS, Some(two().0), Unit::SpaceDockI, 0, 1).build();
    let opts = prod::build_opts(&cx(&poor, &kp), Seat(0), &ask(), &Extras::default());
    assert_eq!(opts.as_slice(), &[POpt::Build { unit: Unit::FighterI, made: 2 }, POpt::Build { unit: Unit::InfantryI, made: 2 }]);
    // One unit of the limit left: the pairs shrink to one.
    let tight = ChooseAsk { remaining: 1, ..ask() };
    let opts = prod::build_opts(&c, Seat(0), &tight, &Extras::default());
    assert!(opts.as_slice().contains(&POpt::Build { unit: Unit::FighterI, made: 1 }));
}

// ---- paying -------------------------------------------------------------------------------------------------

fn spend_of(s: &CoreState, k: &Kernel, planet: Planet, kind: Spend) -> Vec<(Spend, i64)> {
    prod::payment_faces(&cx(s, k), Seat(0), planet, kind).as_slice().to_vec()
}

#[test]
fn a_planet_pays_in_the_currency_of_the_bill_and_the_alternate_face_only_with_the_right_ability() {
    let (a, b) = two();
    let (s, k) = docked().build();
    let (ar, ai) = (i64::from(a.data().resources), i64::from(a.data().influence));
    assert_eq!(spend_of(&s, &k, a, Spend::Resources), vec![(Spend::Resources, ar)]);
    assert_eq!(spend_of(&s, &k, a, Spend::Influence), vec![(Spend::Influence, ai)]);
    assert_eq!(prod::max_face_value(&cx(&s, &k), Seat(0), b, Spend::Resources), i64::from(b.data().resources));
    // Xxekir Grom: both values at once.
    let (mut grom, kg) = docked().build();
    grom.players_mut().by_seat_mut(Seat(0)).unwrap().leaders.status.insert(Leader::XxekirGrom, LeaderStatus::Unlocked);
    assert_eq!(spend_of(&grom, &kg, a, Spend::Resources), vec![(Spend::Resources, ar + ai)]);
    let (mut locked, kl) = docked().build();
    locked.players_mut().by_seat_mut(Seat(0)).unwrap().leaders.status.insert(Leader::XxekirGrom, LeaderStatus::Locked);
    assert_eq!(spend_of(&locked, &kl, a, Spend::Resources), vec![(Spend::Resources, ar)], "only an unlocked commander");
    // Freelancers and Archon's Gift: influence also pays for units (resources bills only).
    let (mut free, kf) = docked().build();
    free.meta_mut().influence_pays_for_units.insert(Seat(0));
    let br = i64::from(b.data().resources);
    let bi = i64::from(b.data().influence);
    assert_eq!(spend_of(&free, &kf, b, Spend::Resources), vec![(Spend::Resources, br), (Spend::Influence, bi)]);
    assert_eq!(spend_of(&free, &kf, b, Spend::Influence), vec![(Spend::Influence, bi)], "an influence bill has no alternate");
    assert_eq!(prod::max_face_value(&cx(&free, &kf), Seat(0), b, Spend::Resources), br.max(bi));
    let (mut gift, kg2) = docked().build();
    gift.players_mut().by_seat_mut(Seat(0)).unwrap().cards.breakthrough = Some(Breakthrough::ArchonsGift);
    assert_eq!(spend_of(&gift, &kg2, b, Spend::Resources), vec![(Spend::Resources, br), (Spend::Influence, bi)]);
    // The same value on both faces is not offered twice (New Albion: 1 and 1).
    assert_eq!(spend_of(&free, &kf, a, Spend::Resources), vec![(Spend::Resources, ar)]);
}

#[test]
fn what_is_available_adds_the_planets_the_trade_goods_the_commodities_the_triad_and_war_machine() {
    let (a, b) = two();
    let (r_a, r_b) = (i64::from(a.data().resources), i64::from(b.data().resources));
    let (s, k) = docked().goods(0, 2).build();
    let ex = Extras::default();
    assert_eq!(prod::available(&cx(&s, &k), Seat(0), Spend::Resources, &ex), r_a + r_b + 2);
    assert_eq!(prod::available(&cx(&s, &k), Seat(0), Spend::Influence, &ex), i64::from(a.data().influence) + i64::from(b.data().influence) + 2);
    // Commodities a Keleres agent lets the seat spend count like goods.
    let with = Extras { commodities: 3, oceans: &[] };
    assert_eq!(prod::available(&cx(&s, &k), Seat(0), Spend::Resources, &with), r_a + r_b + 2 + 3);
    // An exhausted planet pays nothing.
    let (spent, ks) = docked().exhaust(b).build();
    assert_eq!(prod::available(&cx(&spent, &ks), Seat(0), Spend::Resources, &ex), r_a);
    // War Machine adds five resources for the activation, to resource bills only.
    let (mut wm, kw) = docked().build();
    wm.meta_mut().activation_seq = 4;
    wm.players_mut().by_seat_mut(Seat(0)).unwrap().round_marks.push(ti4_engine_core::state::RoundKind::WarMachine, 4);
    assert_eq!(prod::available(&cx(&wm, &kw), Seat(0), Spend::Resources, &ex), r_a + r_b + 5);
    assert_eq!(prod::available(&cx(&wm, &kw), Seat(0), Spend::Influence, &ex), i64::from(a.data().influence) + i64::from(b.data().influence));
    // The Triad counts as a planet card worth three plus a point for every kind of fragment the seat holds.
    let (mut triad, kt) = docked().build();
    triad.players_mut().by_seat_mut(Seat(0)).unwrap().cards.relics.push(Relic::TheTriad);
    assert_eq!(prod::available(&cx(&triad, &kt), Seat(0), Spend::Resources, &ex), r_a + r_b + 3);
    let f = &mut triad.players_mut().by_seat_mut(Seat(0)).unwrap().cards.fragments;
    f.set(ti4_engine_core::generated::ExploreType::Cultural, 2);
    f.set(ti4_engine_core::generated::ExploreType::Frontier, 0);
    f.set(ti4_engine_core::generated::ExploreType::Hazardous, 1);
    assert_eq!(prod::available(&cx(&triad, &kt), Seat(0), Spend::Resources, &ex), r_a + r_b + 3 + 2, "two kinds held");
    // Planet cards off the map (the Deepwrought's oceans) are appended after the planets.
    let list = prod::spendable_planets(&cx(&s, &k), Seat(0), &Extras { commodities: 0, oceans: &[Planet::Quann] });
    assert_eq!(list.as_slice().last(), Some(&Planet::Quann));
    assert_eq!(list.len(), 3);
}

#[test]
fn the_worth_of_a_trade_good_is_one_and_mirror_computing_makes_it_two() {
    let (s, k) = docked().goods(0, 2).build();
    assert_eq!(prod::trade_good_worth(&cx(&s, &k), Seat(0)), 1);
    let (m, km) = duel(Faction::TheMentakCoalition).tech(0, Technology::MirrorComputing).goods(0, 2).build();
    assert_eq!(prod::trade_good_worth(&cx(&m, &km), Seat(0)), 2);
    assert_eq!(prod::available(&cx(&m, &km), Seat(0), Spend::Resources, &Extras::default()), 4);
    assert_eq!(prod::trade_good_worth(&cx(&m, &km), Seat(1)), 1, "only the Mentak with the technology");
    let held = Held::of(&cx(&m, &km), Seat(0));
    assert_eq!(held.extras().commodities, 0);
    assert!(held.extras().oceans.is_empty());
}

#[test]
fn the_payment_options_are_the_planet_faces_that_leave_the_bill_payable_then_goods_then_commodities() {
    let (a, b) = two();
    let (r_a, r_b) = (i64::from(a.data().resources), i64::from(b.data().resources));
    let (s, k) = docked().goods(0, 1).build();
    let ex = Extras::default();
    // A bill of 3: Starpoint (3) pays alone; New Albion (1) would strand the rest only if the others cannot cover it (3 + 1 goods does).
    let (ask, opts) = prod::pay_opts(&cx(&s, &k), Seat(0), Spend::Resources, 0, 3, &ex);
    assert_eq!((ask.kind, ask.owed, ask.pool, ask.goods), (Currency::Resources, 3, r_a + r_b + 1, 1));
    let kinds: Vec<_> = opts.as_slice().iter().map(|o| match o {
        POpt::Exhaust { planet, source, shown } => (Some(*planet), Some(*source), shown.worth),
        POpt::TradeGood { shown } => (None, None, shown.worth),
        other => panic!("{other:?}"),
    }).collect();
    assert_eq!(kinds.last().map(|k| k.0), Some(None), "the trade good comes last");
    assert!(opts.as_slice().iter().any(|o| matches!(o, POpt::Exhaust { planet, shown, .. } if *planet == b && i64::from(shown.worth) == r_b)));
    // The pool after is what is left once that planet is spent.
    for o in opts.as_slice() {
        if let POpt::Exhaust { planet, shown, .. } = o {
            let removed = prod::max_face_value(&cx(&s, &k), Seat(0), *planet, Spend::Resources);
            assert_eq!(i64::from(shown.pool_after), r_a + r_b + 1 - removed);
        }
    }
    // Paid so far counts: a bill of 5 with 3 already paid owes 2.
    let (ask2, _) = prod::pay_opts(&cx(&s, &k), Seat(0), Spend::Resources, 3, 5, &ex);
    assert_eq!(ask2.owed, 2);
    // A planet that would strand the bill is left out: with only the two planets (no goods) and a bill of the whole pool, both are needed, so both stay.
    let (nogoods, kn) = docked().build();
    let (_, all) = prod::pay_opts(&cx(&nogoods, &kn), Seat(0), Spend::Resources, 0, r_a + r_b, &ex);
    assert_eq!(all.len(), 2);
    // One resource more than the pool can pay: every option strands the bill, so nothing is offered.
    let (_, none) = prod::pay_opts(&cx(&nogoods, &kn), Seat(0), Spend::Resources, 0, r_a + r_b + 1, &ex);
    assert!(none.is_empty());
    // Commodities after the goods.
    let with = Extras { commodities: 2, oceans: &[] };
    let (_, last) = prod::pay_opts(&cx(&s, &k), Seat(0), Spend::Resources, 0, 3, &with);
    let tail: Vec<_> = last.as_slice().iter().rev().take(2).collect();
    assert!(matches!(tail[0], POpt::Commodity { .. }) && matches!(tail[1], POpt::TradeGood { .. }));
    let _ = Attachment::RichWorld;
}

#[test]
fn the_producing_systems_invasion_is_added_to_the_context_of_a_question_about_it() {
    use ti4_engine_core::choice::{Ctx, Origin, Subtype, Target};
    let (mut s, _) = docked().build();
    let blank = || Ctx { origin: Origin::Rule(ti4_engine_core::choice::generic::Rule::R8_10), subtype: Subtype::Agenda(ti4_engine_core::choice::agenda::Subtype::CastVote), optional: false, target: None, outstanding: Default::default(), space_battle: false, invasion_seq: None };
    let mut idle = blank();
    prod::enrich_ctx(&s, &mut idle);
    assert_eq!(idle.invasion_seq, None, "no invasion going on");
    s.transient_mut().invasion.active = Some(ti4_engine_core::Cow::new(ti4_engine_core::state::ActiveInvasion {
        system: SYS,
        invader: Seat(0),
        seq: 7,
        phase: ti4_engine_core::state::InvasionPhase::default(),
        planet: None,
        defender: None,
        ground_round: 0,
        last_step: None,
    }));
    let mut here = blank();
    here.target = Some(Target::System(SYS));
    prod::enrich_ctx(&s, &mut here);
    assert_eq!(here.invasion_seq, Some(7));
    let mut elsewhere = blank();
    elsewhere.target = Some(Target::System(System::Wellon));
    prod::enrich_ctx(&s, &mut elsewhere);
    assert_eq!(elsewhere.invasion_seq, None, "a question about another system");
    let mut open = blank();
    prod::enrich_ctx(&s, &mut open);
    assert_eq!(open.invasion_seq, Some(7), "no target: it is about the invasion");
    let mut battle = blank();
    battle.space_battle = true;
    prod::enrich_ctx(&s, &mut battle);
    assert_eq!(battle.invasion_seq, None, "never a space battle");
}
