//! W16: the judges the cards of other islands' windows need before their hosts exist: whether a window matters to Harness Energy (the tile
//! catalogue and the rift hook), to Infiltrate (the swaps of the box) and to Bribery (the seat's potential goods). The positions here are the
//! rules of each judge; the answers against the string engine's, position by position, are the lockstep tests of `ti4-engine`
//! (`action_cards::tests::*_judge_*`).

use ti4_engine_core::generated::{ActionCard as C, System};
use ti4_engine_core::rules::action_cards::{infiltrate_swaps, reaction_can_matter_with, system_is_anomaly, Hooks, Window, RULES_INFILTRATE_FIX};
use ti4_engine_core::rules::production::supply;
use ti4_engine_core::tables::Scope;
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::plain_systems;

const ALL: Scope = Scope(u32::MAX);

fn no_hooks() -> Hooks<'static> {
    Hooks { rifts: &[], forbidden: &|_| false }
}

fn arena() -> Scenario {
    let t = plain_systems();
    Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec, Faction::TheVuilraithCabal]).map_from(&[t[1], t[2]])
}

// ---- Harness Energy ----------------------------------------------------------------------------------

#[test]
fn a_system_is_an_anomaly_by_its_tile_flags_or_by_a_rift_a_piece_makes() {
    covers!("rules::action_cards::system_is_anomaly");
    let mut seen = (0, 0);
    for system in System::ALL {
        let a = system.data().anomalies;
        let expected = a.asteroid_field || a.gravity_rift || a.nebula || a.supernova || a.scar;
        assert_eq!(system_is_anomaly(ALL, system, &[]), expected, "{system:?}");
        assert!(system_is_anomaly(ALL, system, &[system]), "{system:?}: a rift the pieces make is an anomaly");
        assert!(!system_is_anomaly(ALL, system, &[System::ALL[0]]) || system == System::ALL[0] || expected);
        seen.0 += usize::from(expected);
        seen.1 += usize::from(!expected);
    }
    assert!(seen.0 > 5 && seen.1 > 30, "the catalogue has both kinds: {seen:?}");
    // A system outside the scope is not looked up (the string reads the catalogue of the sources in play).
    let outside = System::ALL.iter().copied().find(|s| s.data().anomalies.nebula).unwrap();
    assert!(!system_is_anomaly(Scope(0), outside, &[]));
}

#[test]
fn harness_energy_matters_in_the_window_of_an_anomaly_only() {
    covers!("rules::action_cards::reaction_can_matter_with");
    let t = plain_systems();
    let s = arena().state();
    let nebula = System::ALL.iter().copied().find(|s| s.data().anomalies.nebula).unwrap();
    let w = |system| Window { system, actor: Some(Seat(0)), planet: None };
    assert!(reaction_can_matter_with(&s, ALL, Seat(0), C::HarnessEnergy, w(nebula), &no_hooks(), (None, None)));
    assert!(!reaction_can_matter_with(&s, ALL, Seat(0), C::HarnessEnergy, w(t[1]), &no_hooks(), (None, None)));
    let rifts = [t[1]];
    let hooks = Hooks { rifts: &rifts, forbidden: &|_| false };
    assert!(reaction_can_matter_with(&s, ALL, Seat(0), C::HarnessEnergy, w(t[1]), &hooks, (None, None)), "a rift a module's piece makes");
    // (The actor is the guard of the printed window, not of this judge: the string's answer does not read it.)
    assert!(reaction_can_matter_with(&s, ALL, Seat(1), C::HarnessEnergy, Window { system: nebula, actor: Some(Seat(0)), planet: None }, &no_hooks(), (None, None)));
}

// ---- Infiltrate ------------------------------------------------------------------------------------------

fn taken(extra: impl FnOnce(Scenario, Planet, System) -> Scenario) -> (CoreState, Planet, System) {
    let t = plain_systems();
    let (sys, planet) = (t[1], t[1].data().planets[0]);
    let sc = extra(arena().control(planet, 0), planet, sys);
    (sc.state(), planet, sys)
}

#[test]
fn infiltrate_pairs_every_rival_pds_and_space_dock_with_the_form_the_holder_places() {
    covers!("rules::action_cards::infiltrate_swaps");
    let (s, planet, _) = taken(|sc, planet, sys| {
        sc.unit(sys, Some(planet), Unit::PDSI, 1, 1).unit(sys, Some(planet), Unit::SpaceDockI, 2, 1).unit(sys, Some(planet), Unit::InfantryI, 1, 2).unit(sys, Some(planet), Unit::PDSI, 0, 1)
    });
    let swaps = infiltrate_swaps(&s, ALL, Seat(0), planet, (Some(Unit::PDSII), None), &|_| false);
    let found: Vec<(Seat, Unit, Unit)> = swaps.as_slice().iter().map(|(p, u)| (p.owner(), p.kind(), *u)).collect();
    assert_eq!(found, vec![(Seat(1), Unit::PDSI, Unit::PDSII), (Seat(2), Unit::SpaceDockI, Unit::SpaceDockI)], "the PDS becomes the holder's PDS form, the dock the plain dock; infantry and our own PDS stay");
}

#[test]
fn infiltrate_needs_plastic_left_in_the_box_counted_as_it_goes_and_a_form_no_rule_bars() {
    covers!("rules::action_cards::infiltrate_swaps");
    // Two rival docks, the holder's box holds one: the first swaps, the second stays (31.4: "counted as it goes").
    let t = plain_systems();
    let (sys, planet) = (t[1], t[1].data().planets[0]);
    let mut sc = arena().control(planet, 0).unit(sys, Some(planet), Unit::SpaceDockI, 1, 2);
    // The holder's other two docks stand elsewhere: a box of three holds one.
    let elsewhere = t[2].data().planets[0];
    sc = sc.unit(t[2], Some(elsewhere), Unit::SpaceDockI, 0, 2);
    let s = sc.state();
    let swaps = infiltrate_swaps(&s, ALL, Seat(0), planet, (None, None), &|_| false);
    assert_eq!(swaps.len(), 1, "one dock left in the box, two rival docks: one swap");
    // A form a faction rule bars from being placed by an effect is not placed.
    assert!(infiltrate_swaps(&s, ALL, Seat(0), planet, (None, None), &|u| u == Unit::SpaceDockI).is_empty());
    // An empty box: nothing.
    let (rich, planet, _) = taken(|sc, planet, sys| {
        let mut sc = sc.unit(sys, Some(planet), Unit::PDSI, 1, 1);
        for _ in 0..6 {
            sc = sc.unit(plain_systems()[2], Some(plain_systems()[2].data().planets[0]), Unit::PDSI, 0, 1);
        }
        sc
    });
    assert!(infiltrate_swaps(&rich, ALL, Seat(0), planet, (None, None), &|_| false).is_empty(), "six PDS on the board: none left");
}

#[test]
fn infiltrate_matters_for_the_seat_that_took_a_planet_with_a_swap_and_above_rules_version_one() {
    covers!("rules::action_cards::reaction_can_matter_with");
    let (mut s, planet, sys) = taken(|sc, planet, sys| sc.unit(sys, Some(planet), Unit::PDSI, 1, 1));
    let w = |actor: Option<u8>, planet: Option<Planet>| Window { system: sys, actor: actor.map(Seat), planet };
    let judge = |s: &CoreState, who: u8, w| reaction_can_matter_with(s, ALL, Seat(who), C::Infiltrate, w, &no_hooks(), (None, None));
    s.meta_mut().rules_version = RULES_INFILTRATE_FIX;
    assert!(judge(&s, 0, w(Some(0), Some(planet))), "the gainer, with a PDS of a rival on the planet");
    assert!(!judge(&s, 1, w(Some(0), Some(planet))), "another seat is not the gainer");
    assert!(!judge(&s, 0, w(None, Some(planet))), "nobody gained it");
    assert!(judge(&s, 0, w(Some(1), None)) , "no planet named: the window stands");
    // Nothing to swap on a planet without a structure of a rival.
    let (bare, planet2, sys2) = taken(|sc, planet, sys| sc.unit(sys, Some(planet), Unit::InfantryI, 1, 2));
    let mut bare = bare;
    bare.meta_mut().rules_version = RULES_INFILTRATE_FIX;
    assert!(!reaction_can_matter_with(&bare, ALL, Seat(0), C::Infiltrate, Window { system: sys2, actor: Some(Seat(0)), planet: Some(planet2) }, &no_hooks(), (None, None)));
    // Below rules version 1 the card is offered after every capture.
    bare.meta_mut().rules_version = 0;
    assert!(reaction_can_matter_with(&bare, ALL, Seat(0), C::Infiltrate, Window { system: sys2, actor: Some(Seat(1)), planet: Some(planet2) }, &no_hooks(), (None, None)));
}

// ---- Bribery ---------------------------------------------------------------------------------------------

#[test]
fn the_potential_goods_of_a_seat_are_the_trade_goods_it_can_spend_and_bribery_wants_one() {
    covers!("rules::production::supply::potential_goods");
    for goods in [0_i16, 1, 2, 7] {
        let (s, k) = arena().goods(1, goods).commodities(1, 2).build();
        assert_eq!(supply::potential_goods(&s, &k, Seat(1)), i64::from(goods), "commodities are not goods (no agent window is open)");
        assert_eq!(supply::potential_goods(&s, &k, Seat(1)), supply::spendable_goods(&s, &k, Seat(1)));
        assert_eq!(ti4_engine_core::rules::action_cards::cost_is_payable(&s, Seat(1), C::Bribery, supply::potential_goods(&s, &k, Seat(1))), Some(goods >= 1));
    }
}

#[test]
fn infiltrate_counts_the_plastic_of_each_kind_on_its_own_and_as_it_goes() {
    covers!("rules::action_cards::infiltrate_swaps");
    let t = plain_systems();
    let (sys, planet) = (t[1], t[1].data().planets[0]);
    let far = t[2].data().planets[0];
    // Two rival docks, a box of two docks: both swap (the second sees one used, not two).
    let s = arena().control(planet, 0).unit(sys, Some(planet), Unit::SpaceDockI, 1, 2).unit(t[2], Some(far), Unit::SpaceDockI, 0, 1).state();
    assert_eq!(infiltrate_swaps(&s, ALL, Seat(0), planet, (None, None), &|_| false).len(), 2);
    // One PDS left in the box and one dock left: a rival PDS and a rival dock both swap (the kinds count apart).
    let s = arena()
        .control(planet, 0)
        .unit(sys, Some(planet), Unit::PDSI, 1, 1)
        .unit(sys, Some(planet), Unit::SpaceDockI, 1, 1)
        .unit(t[2], Some(far), Unit::PDSI, 0, 5)
        .unit(t[2], Some(far), Unit::SpaceDockI, 0, 2)
        .state();
    let swaps = infiltrate_swaps(&s, ALL, Seat(0), planet, (None, None), &|_| false);
    assert_eq!(swaps.len(), 2, "one PDS and one dock: {swaps:?}");
}
