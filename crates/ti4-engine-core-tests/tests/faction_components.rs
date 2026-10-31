//! D4A4: the faction component actions of the core that are windows: Orbital Drop (Sol, `rules::faction_component`) and Star Forge / The Nucleus
//! with the Ember Colossus placements (Muaat, `rules::muaat_component`), against the printed texts.
//!
//! Orbital Drop: "ACTION: Spend 1 token from your strategy pool to place 2 infantry from your reinforcements on any planet you control.
//! Then, you may spend 3 resources to place 1 mech on that planet." Star Forge: "ACTION: Spend 1 token from your strategy pool to place either
//! 2 fighters or 1 destroyer from your reinforcements in a system that contains 1 or more of your war suns." Ember Colossus (DEPLOY /
//! Muaat mech): "At the start of a space combat? no: after you use your STAR FORGE faction ability, you may place 1 infantry from your
//! reinforcements in the system with your mech or in an adjacent system."

use ti4_engine_core::choice::promissory::{Ask as PAsk, Opt as POpt};
use ti4_engine_core::generated::{Planet, Source};
use ti4_engine_core::rules::faction_component::{OrbitalDrop, OrbitalFacts};
use ti4_engine_core::rules::muaat_component::{ForgeFacts, Mode, StarForge};
use ti4_engine_core::state::Code;
use ti4_engine_core_tests::inv::ALL_SOURCES;
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::rig::Policy;
use ti4_engine_core_tests::scenario::{plain_systems, system_of};

fn pl(o: POpt) -> Opt {
    Opt::Promissory(o)
}

fn count(s: &CoreState, p: Planet, seat: u8, unit: Unit) -> usize {
    s.board().planet(p).units.as_slice().iter().filter(|u| u.owner() == Seat(seat) && u.kind() == unit).count()
}

fn tokens(s: &CoreState, seat: u8) -> i16 {
    s.players().by_seat(Seat(seat)).unwrap().econ.strategic_tokens
}

// ---- Orbital Drop -----------------------------------------------------------------------------

fn resource_planets(n: usize) -> Vec<Planet> {
    let mut out: Vec<Planet> = Vec::new();
    for s in plain_systems() {
        for p in s.data().planets {
            if p.data().source == Source::Base && p.data().resources >= 2 && !out.iter().any(|q| system_of(*q) == s) && out.len() < n {
                out.push(*p);
            }
        }
    }
    assert_eq!(out.len(), n);
    out
}

/// A planet worth 3 resources on a plain tile: the price of the mech paid with one exhaust.
fn rich_planet() -> Planet {
    plain_systems().into_iter().flat_map(|s| s.data().planets.iter().copied().collect::<Vec<_>>()).find(|p| p.data().resources >= 3).expect("a 3-resource planet")
}

fn sol_table(ps: &[Planet], strat: i16) -> Scenario {
    let mut systems: Vec<System> = ps.iter().map(|p| system_of(*p)).collect();
    systems.dedup();
    let mut sc = Scenario::new(&[Faction::TheFederationOfSol, Faction::TheArborec]).map_from(&systems).tokens(0, 3, 3, strat);
    for p in ps {
        sc = sc.control(*p, 0);
    }
    sc
}

fn facts(can_pay: bool) -> OrbitalFacts {
    OrbitalFacts { infantry: Some(Unit::InfantryI), mech: Some(Unit::FactionlessMech), can_pay, scope: ALL_SOURCES, source: Code(31) }
}

#[test]
fn orbital_drop_spends_a_strategy_token_places_two_infantry_on_the_planet_chosen_and_offers_the_mech() {
    covers!("rules::faction_component::OrbitalDrop");
    let ps = resource_planets(2);
    let mut rig = Rig::new(sol_table(&ps, 2), OrbitalDrop::new(Seat(0), facts(true)));
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Promissory(PAsk::OrbitalPlanet)));
    assert_eq!(rig.opts().len(), 2, "any planet the seat controls");
    rig.assert_options_sound();
    assert_eq!(tokens(&rig.state, 0), 2, "the token is spent only after the planet is named");
    rig.pick_where(|o| matches!(o, Opt::Promissory(POpt::Spot(_, p)) if *p == ps[1]));
    assert_eq!(count(&rig.state, ps[1], 0, Unit::InfantryI), 2, "2 infantry");
    assert_eq!(tokens(&rig.state, 0), 1, "1 token from the strategy pool");
    assert!(rig.driver.window.done);
    assert!(matches!(rig.asked().ask, Ask::Promissory(PAsk::OrbitalDeploy { .. })));
    assert_eq!(rig.opts(), vec![pl(POpt::Deploy), Opt::Decline]);
    rig.decline();
    assert!(rig.is_done());
    assert_eq!(count(&rig.state, ps[1], 0, Unit::FactionlessMech), 0, "no mech without paying");
    smoke("faction_components/orbital", || Rig::new(sol_table(&ps, 2), OrbitalDrop::new(Seat(0), facts(true))));
}

#[test]
fn orbital_drop_with_one_planet_goes_unasked_and_paying_three_resources_brings_the_mech() {
    covers!("rules::faction_component::OrbitalDrop");
    let ps = vec![rich_planet()];
    let mut rig = Rig::new(sol_table(&ps, 1), OrbitalDrop::new(Seat(0), facts(true)));
    rig.run();
    assert_eq!(count(&rig.state, ps[0], 0, Unit::InfantryI), 2, "one planet: no question about where");
    rig.pick_opt(pl(POpt::Deploy));
    rig.auto(Policy::First);
    assert!(rig.is_done() && !rig.driver.window.unsupported);
    assert_eq!(count(&rig.state, ps[0], 0, Unit::FactionlessMech), 1, "place 1 mech on that planet");
    assert!(ti4_engine_core::query::board::exhausted(&rig.state, ps[0]), "the 3 resources came from exhausting the planet");
}

#[test]
fn orbital_drop_does_not_resolve_without_a_token_a_planet_and_offers_no_mech_when_it_cannot_be_paid() {
    covers!("rules::faction_component::OrbitalDrop");
    let ps = resource_planets(1);
    let mut broke = Rig::new(sol_table(&ps, 0), OrbitalDrop::new(Seat(0), facts(true)));
    broke.run();
    assert!(broke.is_done() && !broke.driver.window.done, "22.3: an action that cannot resolve is not performed");
    assert_eq!(count(&broke.state, ps[0], 0, Unit::InfantryI), 0);
    let mut nowhere = Rig::new(Scenario::new(&[Faction::TheFederationOfSol, Faction::TheArborec]).tokens(0, 3, 3, 2), OrbitalDrop::new(Seat(0), facts(true)));
    nowhere.run();
    assert!(nowhere.is_done() && !nowhere.driver.window.done);
    let mut poor = Rig::new(sol_table(&ps, 1), OrbitalDrop::new(Seat(0), facts(false)));
    poor.run();
    assert!(poor.is_done() && poor.driver.window.done, "the infantry stand; with no 3 resources the mech is not offered");
    assert_eq!(count(&poor.state, ps[0], 0, Unit::InfantryI), 2);
}

// ---- Star Forge -------------------------------------------------------------------------------

fn forge_facts(near: Vec<(System, Vec<System>)>) -> ForgeFacts {
    ForgeFacts {
        scope: ALL_SOURCES,
        fighter: Some(Unit::FighterI),
        destroyer: Some(Unit::DestroyerI),
        infantry: Some(Unit::InfantryI),
        fleet_limit: 8,
        near,
        source: Code(32),
        mech: Code(33),
    }
}

fn forge_scene(strat: i16) -> Scenario {
    let s = plain_systems();
    let on = s[1].data().planets[0];
    Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec])
        .map_ring(7)
        .tokens(0, 3, 3, strat)
        .unit(s[0], None, Unit::WarSun, 0, 1)
        .unit(s[2], None, Unit::WarSun, 0, 1)
        .control(on, 0)
        .unit(s[1], Some(on), Unit::EmberColossus, 0, 1)
}

fn ships(s: &CoreState, system: System, unit: Unit) -> usize {
    s.board().sys(system).units.as_slice().iter().filter(|u| u.owner() == Seat(0) && u.kind() == unit).count()
}

#[test]
fn star_forge_places_two_fighters_or_a_destroyer_where_a_war_sun_stands_and_spends_the_token_after_it() {
    covers!("rules::muaat_component::StarForge");
    let s = plain_systems();
    let f = || forge_facts(vec![(s[0], vec![s[1]]), (s[2], vec![])]);
    let mut rig = Rig::new(forge_scene(2), StarForge::new(Seat(0), Mode::StarForge, f()));
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Promissory(PAsk::StarForge)));
    assert_eq!(
        rig.opts(),
        vec![pl(POpt::Forge { system: s[0], fighters: true }), pl(POpt::Forge { system: s[0], fighters: false }), pl(POpt::Forge { system: s[2], fighters: true }), pl(POpt::Forge { system: s[2], fighters: false }), Opt::Decline],
        "fighters or a destroyer, in each system of a war sun, in system order"
    );
    rig.assert_options_sound();
    assert_eq!(tokens(&rig.state, 0), 2);
    rig.pick_opt(pl(POpt::Forge { system: s[2], fighters: true }));
    assert_eq!(ships(&rig.state, s[2], Unit::FighterI), 2, "2 fighters");
    assert_eq!(tokens(&rig.state, 0), 1, "1 token from the strategy pool");
    assert!(rig.is_done() && rig.driver.window.done, "no mech next to system 2: nothing more is asked");
    let mut dest = Rig::new(forge_scene(2), StarForge::new(Seat(0), Mode::StarForge, f()));
    dest.run().pick_opt(pl(POpt::Forge { system: s[2], fighters: false }));
    assert_eq!(ships(&dest.state, s[2], Unit::DestroyerI), 1, "1 destroyer");
    smoke("faction_components/forge", || Rig::new(forge_scene(2), StarForge::new(Seat(0), Mode::StarForge, f())));
}

#[test]
fn star_forge_then_asks_each_ember_colossus_in_or_next_to_the_system_whether_to_place_an_infantry() {
    covers!("rules::muaat_component::StarForge");
    let s = plain_systems();
    let on = s[1].data().planets[0];
    let f = || forge_facts(vec![(s[0], vec![s[1]]), (s[2], vec![])]);
    let mut rig = Rig::new(forge_scene(2), StarForge::new(Seat(0), Mode::StarForge, f()));
    rig.run().pick_opt(pl(POpt::Forge { system: s[0], fighters: true }));
    assert!(matches!(rig.asked().ask, Ask::Promissory(PAsk::EmberColossus { system, planet: Some(p) }) if system == s[1] && p == on), "the mech on the adjacent planet: {:?}", rig.asked().ask);
    assert_eq!(rig.opts(), vec![pl(POpt::Ember), Opt::Decline]);
    rig.assert_options_sound();
    rig.pick_opt(pl(POpt::Ember));
    assert!(rig.is_done());
    assert_eq!(count(&rig.state, on, 0, Unit::InfantryI), 1, "1 infantry with the mech");
    let mut no = Rig::new(forge_scene(2), StarForge::new(Seat(0), Mode::StarForge, f()));
    no.run().pick_opt(pl(POpt::Forge { system: s[0], fighters: true }));
    no.decline();
    assert!(no.is_done());
    assert_eq!(count(&no.state, on, 0, Unit::InfantryI), 0);
    let mut far = Rig::new(forge_scene(2), StarForge::new(Seat(0), Mode::StarForge, forge_facts(vec![(s[0], vec![])])));
    far.run().pick_opt(pl(POpt::Forge { system: s[0], fighters: true }));
    assert!(far.is_done(), "the mech is not next to this forge");
}

#[test]
fn star_forge_needs_a_muaat_player_a_strategy_token_and_a_war_sun_and_may_be_declined() {
    covers!("rules::muaat_component::StarForge");
    let s = plain_systems();
    let f = || forge_facts(vec![]);
    let mut dec = Rig::new(forge_scene(2), StarForge::new(Seat(0), Mode::StarForge, f()));
    dec.run().decline();
    assert!(dec.is_done() && !dec.driver.window.done);
    assert_eq!(delta(dec.start_state(), &dec.state), "");
    let mut broke = Rig::new(forge_scene(0), StarForge::new(Seat(0), Mode::StarForge, f()));
    broke.run();
    assert!(broke.is_done() && !broke.driver.window.done, "no token");
    let other = Scenario::new(&[Faction::TheArborec, Faction::TheEmbersOfMuaat]).map_ring(7).tokens(0, 3, 3, 2).unit(s[0], None, Unit::WarSun, 0, 1);
    let mut wrong = Rig::new(other, StarForge::new(Seat(0), Mode::StarForge, f()));
    wrong.run();
    assert!(wrong.is_done() && !wrong.driver.window.done, "only the Muaat player");
    let nowar = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec]).map_ring(7).tokens(0, 3, 3, 2).unit(s[0], None, Unit::DreadnoughtI, 0, 1);
    let mut none = Rig::new(nowar, StarForge::new(Seat(0), Mode::StarForge, f()));
    none.run();
    assert!(none.is_done() && !none.driver.window.done, "no war sun");
}

#[test]
fn the_nucleus_forges_without_a_token_and_exhausts_the_legendary_card_afterwards() {
    covers!("rules::muaat_component::StarForge");
    let s = plain_systems();
    let sc = || {
        let mut st = forge_scene(0).state();
        st.board_mut().planet_mut(Planet::Avernus).control = Some(Seat(0));
        st
    };
    let mut rig = Rig::from_parts(sc(), kernel_all(), StarForge::new(Seat(0), Mode::Nucleus, forge_facts(vec![])));
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Promissory(PAsk::StarForge)));
    rig.pick_opt(pl(POpt::Forge { system: s[0], fighters: false }));
    assert!(rig.is_done() && rig.driver.window.done);
    assert_eq!(tokens(&rig.state, 0), 0, "no token is spent");
    assert!(rig.state.players().by_seat(Seat(0)).unwrap().cards.legendary_exhausted.contains_key(&Planet::Avernus), "the card is exhausted");
    let mut declined = Rig::from_parts(sc(), kernel_all(), StarForge::new(Seat(0), Mode::Nucleus, forge_facts(vec![])));
    declined.run().decline();
    assert!(!declined.state.players().by_seat(Seat(0)).unwrap().cards.legendary_exhausted.contains_key(&Planet::Avernus), "declined: the card stays ready");
    let mut without = Rig::from_parts(forge_scene(0).state(), kernel_all(), StarForge::new(Seat(0), Mode::Nucleus, forge_facts(vec![])));
    without.run();
    assert!(without.is_done() && !without.driver.window.done, "the Nucleus is not controlled");
}
