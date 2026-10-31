//! D4-2c: `kernel::fx_strategy` (a note taken, an agent or technology exhausted, the unit upgrades a technology researched swaps in on the board).

use ti4_engine_core::kernel::fx_strategy as fx;
use ti4_engine_core::query::tech::UnitRoster;
use ti4_engine_core::state::{CoreState, EventKind, LeaderStatus, NoteKey};
use ti4_engine_core_tests::inv::ALL_SOURCES;
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::{first_planet, plain_systems};

fn table() -> Scenario {
    Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec])
}

#[test]
fn only_a_readied_leader_is_exhausted_and_only_once() {
    covers!("kernel::fx_strategy::exhaust_leader");
    let mut s = table().state();
    let p = s.players_mut().by_seat_mut(Seat(0)).unwrap();
    p.leaders.status.insert(Leader::ALL[0], LeaderStatus::Readied);
    p.leaders.status.insert(Leader::ALL[1], LeaderStatus::Locked);
    assert!(fx::exhaust_leader(&mut s, Seat(0), Leader::ALL[0]));
    assert_eq!(s.players().by_seat(Seat(0)).unwrap().leaders.status.get(&Leader::ALL[0]), Some(&LeaderStatus::Exhausted));
    assert!(!fx::exhaust_leader(&mut s, Seat(0), Leader::ALL[0]), "already exhausted");
    assert!(!fx::exhaust_leader(&mut s, Seat(0), Leader::ALL[1]), "locked");
    assert!(!fx::exhaust_leader(&mut s, Seat(0), Leader::ALL[2]), "not held");
    assert!(!fx::exhaust_leader(&mut s, Seat(9), Leader::ALL[0]));
}

fn nomad_listens() -> ti4_engine_core::kernel::Kernel {
    let mut k = ti4_engine_core::kernel::Kernel::plain();
    k.hooks.direct.nomad_watches_agent_exhaustion.push(|_| true);
    k
}

#[test]
fn a_readied_agent_is_exhausted_and_a_listening_nomad_is_told_once() {
    covers!("kernel::fx_strategy::exhaust_ready_agent");
    let mut s = table().state();
    let p = s.players_mut().by_seat_mut(Seat(0)).unwrap();
    p.leaders.status.insert(Leader::ALL[0], LeaderStatus::Readied);
    p.leaders.status.insert(Leader::ALL[1], LeaderStatus::Locked);
    let k = nomad_listens();
    assert!(fx::exhaust_ready_agent(&mut s, &k, Seat(0), Leader::ALL[0]));
    assert_eq!(s.players().by_seat(Seat(0)).unwrap().leaders.status.get(&Leader::ALL[0]), Some(&LeaderStatus::Exhausted));
    let told = |s: &CoreState| s.pending().staged.iter().filter(|e| e.kind == EventKind::AgentExhausted).count();
    assert_eq!(told(&s), 1, "the exhaustion is staged for the Nomad");
    assert!(!fx::exhaust_ready_agent(&mut s, &k, Seat(0), Leader::ALL[0]), "already exhausted");
    assert!(!fx::exhaust_ready_agent(&mut s, &k, Seat(0), Leader::ALL[1]), "locked");
    assert!(!fx::exhaust_ready_agent(&mut s, &k, Seat(0), Leader::ALL[2]), "not held");
    assert_eq!(told(&s), 1, "a refused exhaustion tells nobody");
    // Nobody listens: the same exhaustion stages nothing.
    let mut quiet = table().state();
    quiet.players_mut().by_seat_mut(Seat(0)).unwrap().leaders.status.insert(Leader::ALL[0], LeaderStatus::Readied);
    assert!(fx::exhaust_ready_agent(&mut quiet, &ti4_engine_core::kernel::Kernel::plain(), Seat(0), Leader::ALL[0]));
    assert_eq!(told(&quiet), 0);
}

#[test]
fn a_technology_is_exhausted_only_when_owned_and_ready() {
    covers!("kernel::fx_strategy::exhaust_technology");
    let mut s = table().tech(0, Technology::GravityDrive).state();
    assert!(!fx::exhaust_technology(&mut s, Seat(0), Technology::PlasmaScoring), "not owned");
    assert!(fx::exhaust_technology(&mut s, Seat(0), Technology::GravityDrive));
    assert!(!fx::exhaust_technology(&mut s, Seat(0), Technology::GravityDrive), "already exhausted");
    assert!(!fx::exhaust_technology(&mut s, Seat(9), Technology::GravityDrive));
}

#[test]
fn a_note_taken_goes_to_the_holder_and_faceup_when_it_lives_in_the_play_area() {
    covers!("kernel::fx_strategy::take_note");
    let mut s = table().state();
    let ordinary = NoteKey { note: PromissoryNote::Ceasefire, faction: Faction::TheEmbersOfMuaat };
    fx::take_note(&mut s, Seat(1), ordinary);
    assert_eq!(s.cards().promissory.get(&ordinary), Some(&Seat(1)));
    use ti4_engine_core::query::promissory::is_play_area;
    let throne = NoteKey { note: PromissoryNote::SupportForTheThrone, faction: Faction::TheEmbersOfMuaat };
    fx::take_note(&mut s, Seat(1), throne);
    assert_eq!(s.cards().promissory_faceup.contains_key(&throne), is_play_area(throne.note, throne.faction), "faceup exactly when the note lives in the play area");
    assert_eq!(s.cards().promissory_faceup.contains_key(&ordinary), is_play_area(ordinary.note, ordinary.faction));
}

#[test]
fn a_researched_upgrade_replaces_the_seats_older_units_everywhere_on_the_board_and_leaves_the_others() {
    covers!("kernel::fx_strategy::apply_unit_upgrades");
    let (system, planet) = first_planet();
    let other = plain_systems()[1];
    let sc = table()
        .map_from(&[system, other])
        .tech(0, Technology::CruiserII)
        .unit(system, None, Unit::CruiserI, 0, 2)
        .unit(system, None, Unit::CruiserI, 1, 1)
        .unit(other, None, Unit::CruiserI, 0, 1)
        .unit(system, Some(planet), Unit::InfantryI, 0, 1);
    let (mut s, k) = sc.build();
    let roster = UnitRoster::default();
    let swaps = fx::unit_swaps(&s, &k, ALL_SOURCES, &roster, Seat(0));
    assert!(swaps.contains(&(Unit::CruiserI, Unit::CruiserII)), "{swaps:?}");
    assert!(fx::unit_swaps(&s, &k, ALL_SOURCES, &roster, Seat(9)).is_empty());
    fx::apply_unit_upgrades(&mut s, &k, ALL_SOURCES, &roster, Seat(0));
    let kinds = |sys: System, seat: u8| s.board().sys(sys).units.as_slice().iter().filter(|p| p.owner() == Seat(seat)).map(|p| p.kind()).collect::<Vec<_>>();
    assert_eq!(kinds(system, 0), vec![Unit::CruiserII; 2]);
    assert_eq!(kinds(other, 0), vec![Unit::CruiserII]);
    assert_eq!(kinds(system, 1), vec![Unit::CruiserI], "another seat keeps its older ship");
    assert_eq!(s.board().planet(planet).units.as_slice().iter().map(|p| p.kind()).collect::<Vec<_>>(), vec![Unit::InfantryI], "no infantry upgrade researched");
    let before = s.clone();
    fx::apply_unit_upgrades(&mut s, &k, ALL_SOURCES, &roster, Seat(0));
    assert!(s == before, "applying twice changes nothing more");
}
