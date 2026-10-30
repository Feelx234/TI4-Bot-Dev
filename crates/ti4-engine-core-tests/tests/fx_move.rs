//! D4-2c: `kernel::fx_move` (activating a system, the one-per-activation boosts, moving and losing units, the ion storm, the arrival count).

use ti4_engine_core::kernel::fx_move as mv;
use ti4_engine_core::state::{PendingTag, Piece, SysFlags};
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::{first_planet, plain_systems};

fn table() -> Scenario {
    let s = plain_systems();
    Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec]).map_from(&[s[0], s[1]]).tokens(0, 2, 2, 2)
}

#[test]
fn activating_spends_a_tactic_token_places_ours_and_opens_the_move() {
    covers!("kernel::fx_move::activate");
    let (system, _) = first_planet();
    let mut s = table().state();
    assert_eq!(mv::activate(&mut s, Seat(0), system, false), Ok(()));
    assert_eq!(s.players().by_seat(Seat(0)).unwrap().econ.tactic_tokens, 1);
    assert!(s.board().sys(system).cmd.contains(Seat(0)) && s.board().sys(system).flags.has(SysFlags::TOUCHED));
    assert_eq!((s.meta().active_system, s.pending().tag, s.meta().activation_seq), (Some(system), Some(PendingTag::Move), 1));
}

#[test]
fn an_activation_is_refused_with_our_token_in_the_system_unless_leave_is_given_or_with_an_empty_pool() {
    covers!("kernel::fx_move::activate");
    let (system, _) = first_planet();
    let mut s = table().command_token(system, 0).state();
    let before = s.clone();
    assert_eq!(mv::activate(&mut s, Seat(0), system, false), Err(mv::ActivateErr::AlreadyActivated));
    assert!(s == before, "nothing is spent");
    assert_eq!(mv::activate(&mut s, Seat(0), system, true), Ok(()), "a Mahact commander's leave");
    let mut empty = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec]).tokens(0, 0, 2, 2).state();
    assert_eq!(mv::activate(&mut empty, Seat(0), system, false), Err(mv::ActivateErr::NoTacticToken));
    assert!(!mv::spend_tactic_token(&mut empty, Seat(0)) && !mv::spend_tactic_token(&mut empty, Seat(9)));
    assert!(!empty.board().sys(system).cmd.contains(Seat(0)), "no token placed on a refusal");
}

#[test]
fn gravity_drive_and_the_ionian_gate_are_each_used_once_per_activation_or_exhaustion() {
    covers!("kernel::fx_move::use_gravity_drive");
    let mut s = table().tech(0, Technology::GravityDrive).state();
    mv::begin_move(&mut s, plain_systems()[0]);
    assert!(mv::gravity_drive_available(&s, Seat(0)) && !mv::gravity_drive_available(&s, Seat(1)));
    assert!(mv::use_gravity_drive(&mut s, Seat(0)));
    assert!(!mv::gravity_drive_available(&s, Seat(0)) && !mv::use_gravity_drive(&mut s, Seat(0)), "once per activation");
    mv::begin_move(&mut s, plain_systems()[1]);
    assert!(mv::gravity_drive_available(&s, Seat(0)), "a new activation");
    assert!(!mv::use_gravity_drive(&mut s, Seat(1)));
    // The Ionian gate needs Tempesta.
    let mut t = table().control(Planet::Tempesta, 0).state();
    assert!(mv::ionian_available(&t, Seat(0)) && !mv::ionian_available(&t, Seat(1)));
    assert!(mv::use_ionian(&mut t, Seat(0)));
    assert!(!mv::ionian_available(&t, Seat(0)) && !mv::use_ionian(&mut t, Seat(0)), "the legendary card is exhausted");
}

#[test]
fn the_gravleash_anchor_keeps_the_best_move_per_origin() {
    covers!("kernel::fx_move::anchor_gravleash");
    let (a, b) = (plain_systems()[0], plain_systems()[1]);
    let mut s = table().state();
    mv::anchor_gravleash(&mut s, a, 3);
    mv::anchor_gravleash(&mut s, a, 2);
    mv::anchor_gravleash(&mut s, a, 5);
    mv::anchor_gravleash(&mut s, b, 1);
    let m = &s.transient().movement.gravleash_move_values;
    assert_eq!((m.get(&a).copied(), m.get(&b).copied()), (Some(5), Some(1)));
}

#[test]
fn ships_move_between_spaces_passengers_are_taken_aboard_and_losses_remove_the_first_match() {
    covers!("kernel::fx_move::move_ship");
    let (a, planet) = first_planet();
    let b = plain_systems()[1];
    let cruiser = Piece::new(Unit::CruiserI, Seat(0));
    let infantry = Piece::new(Unit::InfantryI, Seat(0));
    let mut s = table().control(planet, 0).unit(a, None, Unit::CruiserI, 0, 2).unit(a, Some(planet), Unit::InfantryI, 0, 2).state();
    mv::move_ship(&mut s, a, b, cruiser);
    assert_eq!((s.board().sys(a).units.as_slice().len(), s.board().sys(b).units.as_slice().len()), (1, 1));
    mv::take_aboard(&mut s, a, b, infantry, Some(planet));
    assert_eq!(s.board().planet(planet).units.as_slice().len(), 1, "one infantry left the planet");
    assert!(s.board().sys(b).units.as_slice().contains(&infantry), "and arrived in space");
    mv::destroy_ship(&mut s, b, cruiser);
    assert!(!s.board().sys(b).units.as_slice().contains(&cruiser));
    mv::lose_passenger(&mut s, b, infantry, None);
    assert!(!s.board().sys(b).units.as_slice().contains(&infantry));
    mv::destroy_ship(&mut s, b, cruiser);
    assert!(s.board().sys(b).flags.has(SysFlags::TOUCHED), "a removal that finds nothing still touches the system");
}

#[test]
fn a_special_operations_passenger_lost_is_counted() {
    covers!("kernel::fx_move::lose_passenger");
    let (a, planet) = first_planet();
    let spec = Piece::new(Unit::SpecOpsII, Seat(0));
    let mut s = table().unit(a, Some(planet), Unit::SpecOpsII, 0, 1).state();
    mv::lose_passenger(&mut s, a, spec, Some(planet));
    assert_eq!(s.players().by_seat(Seat(0)).unwrap().misc.spec_ops_destroyed, 1);
    assert!(s.board().planet(planet).units.as_slice().is_empty(), "it left the planet");
    mv::lose_passenger(&mut s, a, Piece::new(Unit::InfantryI, Seat(0)), Some(planet));
    assert_eq!(s.players().by_seat(Seat(0)).unwrap().misc.spec_ops_destroyed, 1, "only Special Operations count");
}

#[test]
fn the_ion_storm_flips_when_a_ship_moves_into_or_out_of_it_and_the_arrival_count_is_per_activation() {
    covers!("kernel::fx_move::flip_ion_storm");
    let (a, b, c) = (plain_systems()[0], plain_systems()[1], plain_systems()[2]);
    let mut s = table().state();
    assert!(!mv::flip_ion_storm(&mut s, a, b), "no ion storm");
    s.meta_mut().ion_storm = Some((a, Wormhole::Alpha));
    assert!(!mv::flip_ion_storm(&mut s, b, c), "the storm is elsewhere");
    assert!(mv::flip_ion_storm(&mut s, b, a));
    assert_eq!(s.meta().ion_storm, Some((a, Wormhole::Beta)));
    assert!(mv::flip_ion_storm(&mut s, a, c));
    assert_eq!(s.meta().ion_storm, Some((a, Wormhole::Alpha)));
    mv::begin_move(&mut s, a);
    assert_eq!((mv::count_arrival(&mut s), mv::count_arrival(&mut s)), (1, 2));
    mv::begin_move(&mut s, b);
    assert_eq!(mv::count_arrival(&mut s), 1, "a new activation counts from one");
}
