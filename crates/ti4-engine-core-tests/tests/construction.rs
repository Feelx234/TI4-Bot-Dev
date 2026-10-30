//! D4-2a: `rules::production::strategy_cards::Construction` (the primary: a PDS or a space dock, then a PDS; the secondary: one structure).

use ti4_engine_core::choice::strategy::{Ask as SAsk, Opt as SOpt};
use ti4_engine_core::kernel::children::StructureFacts;
use ti4_engine_core::rules::production::strategy_cards::Construction;
use ti4_engine_core::state::Code;
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::{planets_with_influence, systems_of};

fn facts() -> StructureFacts {
    StructureFacts { pds: Some((Unit::PDSI, false)), dock: Some((Unit::SpaceDockI, false)), source: Code(3) }
}

/// Seat 0 controls two planets (in two systems); seat 1 is a bystander.
fn table() -> (Scenario, Vec<Planet>) {
    let planets = planets_with_influence(&[1, 2]);
    let sc = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec]).map_from(&systems_of(&planets)).control(planets[0], 0).control(planets[1], 0);
    (sc, planets)
}

fn structure(pds: bool, planet: Planet) -> Opt {
    Opt::Strategy(SOpt::Structure { pds, system: ti4_engine_core_tests::scenario::system_of(planet), planet })
}

fn units_on(rig: &Rig<Construction>, planet: Planet, unit: Unit) -> usize {
    rig.state.board().planet(planet).units.as_slice().iter().filter(|p| p.kind() == unit && p.owner() == Seat(0)).count()
}

#[test]
fn the_primary_places_a_pds_or_a_dock_and_then_only_a_pds() {
    covers!("rules::production::strategy_cards::Construction");
    let (sc, planets) = table();
    let mut rig = Rig::new(sc, Construction::new(Seat(0), true, facts()));
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Strategy(SAsk::PlaceStructure { step: 1, only_pds: false })), "{:?}", rig.asked().ask);
    let first = rig.opts();
    for p in &planets {
        assert!(first.contains(&structure(true, *p)) && first.contains(&structure(false, *p)), "{p:?}: a PDS or a dock");
    }
    assert!(first.contains(&Opt::Decline));
    rig.assert_options_sound();
    rig.pick_opt(structure(false, planets[0]));
    assert!(matches!(rig.asked().ask, Ask::Strategy(SAsk::PlaceStructure { step: 2, only_pds: true })));
    let second = rig.opts();
    assert!(second.iter().all(|o| !matches!(o, Opt::Strategy(SOpt::Structure { pds: false, .. }))), "the second structure is a PDS");
    rig.pick_opt(structure(true, planets[1]));
    assert!(rig.is_done());
    assert_eq!(units_on(&rig, planets[0], Unit::SpaceDockI), 1);
    assert_eq!(units_on(&rig, planets[1], Unit::PDSI), 1);
    ti4_engine_core_tests::assert_snapshot("construction/primary", &rig.snapshot());
}

#[test]
fn the_secondary_places_one_structure_and_ends() {
    covers!("rules::production::strategy_cards::Construction");
    let (sc, planets) = table();
    let mut rig = Rig::new(sc, Construction::new(Seat(0), false, facts()));
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Strategy(SAsk::PlaceStructure { step: 0, only_pds: false })));
    rig.pick_opt(structure(true, planets[1]));
    assert!(rig.is_done());
    assert_eq!(units_on(&rig, planets[1], Unit::PDSI), 1);
}

#[test]
fn declining_ends_the_step_but_the_primary_still_offers_the_second_structure() {
    covers!("rules::production::strategy_cards::Construction");
    let (sc, planets) = table();
    let mut rig = Rig::new(sc, Construction::new(Seat(0), true, facts()));
    rig.run().decline();
    assert!(!rig.is_done(), "declining the first structure leaves the second");
    rig.decline();
    assert!(rig.is_done());
    assert!(planets.iter().all(|p| units_on(&rig, *p, Unit::PDSI) == 0));
}

#[test]
fn a_planet_at_its_structure_limit_is_not_offered_that_structure() {
    covers!("rules::production::strategy_cards::Construction");
    let (sc, planets) = table();
    let system = ti4_engine_core_tests::scenario::system_of(planets[0]);
    let sc = sc.unit(system, Some(planets[0]), Unit::SpaceDockI, 0, 1).unit(system, Some(planets[0]), Unit::PDSI, 0, 2);
    let mut rig = Rig::new(sc, Construction::new(Seat(0), false, facts()));
    rig.run();
    let opts = rig.opts();
    assert!(!opts.contains(&structure(false, planets[0])), "one dock per planet");
    assert!(!opts.contains(&structure(true, planets[0])), "two PDS per planet");
    assert!(opts.contains(&structure(true, planets[1])));
}

#[test]
fn a_seat_with_no_planet_is_asked_nothing_and_an_illegal_structure_is_refused() {
    covers!("rules::production::strategy_cards::Construction");
    let planets = planets_with_influence(&[1]);
    let none = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec]).map_from(&systems_of(&planets));
    let mut rig = Rig::new(none, Construction::new(Seat(0), true, facts()));
    rig.run();
    assert!(rig.is_done());
    let (sc, planets) = table();
    let mut rig = Rig::new(sc, Construction::new(Seat(0), false, facts()));
    rig.run();
    rig.expect_illegal(Pick::Opt(structure(true, Planet::MecatolRex)));
    rig.expect_illegal(Pick::Opt(Opt::Strategy(SOpt::Build)));
    let _ = planets;
}

#[test]
fn construction_survives_every_answer_policy() {
    covers!("rules::production::strategy_cards::Construction");
    smoke_quiet(|| Rig::new(table().0, Construction::new(Seat(0), true, facts())));
    smoke_quiet(|| Rig::new(table().0, Construction::new(Seat(0), false, facts())));
}
