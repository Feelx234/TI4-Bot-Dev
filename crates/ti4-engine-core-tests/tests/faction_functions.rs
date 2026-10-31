//! D4A4: the component actions of the core that are functions, not windows (`rules::faction_component`, `rules::muaat_component::inferno`),
//! found untested by the mutation check of the files. Printed texts: Lazax Gate Folding: "ACTION: Exhaust this card to place 1 infantry
//! from your reinforcements on Mecatol Rex"; ???_ERROR_ERROR_???: "ACTION: Exhaust this card to repair all of your damaged units";
//! Terraform: "Attach to a non-home planet you control other than Mecatol Rex: +1 resource, +1 influence"; The Inferno: "ACTION: Spend 1
//! token from your strategy pool to place 1 cruiser in this system".

use ti4_engine_core::generated::{Attachment, Planet, PromissoryNote, Technology as T};
use ti4_engine_core::rules::faction_component::{c4r_repair, lazax_gate_folding, terraform};
use ti4_engine_core::rules::muaat_component::{inferno, inferno_system, InfernoFacts};
use ti4_engine_core::state::{Code, NoteKey};
use ti4_engine_core_tests::inv::ALL_SOURCES;
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::{plain_systems, system_of};

fn pieces(s: &CoreState, planet: Planet, seat: u8, unit: Unit) -> usize {
    s.board().planet(planet).units.as_slice().iter().filter(|u| u.owner() == Seat(seat) && u.kind() == unit).count()
}

#[test]
fn lazax_gate_folding_places_an_infantry_on_mecatol_when_the_card_is_ready_and_the_seat_controls_it() {
    let mecatol = system_of(Planet::MecatolRex);
    let sc = || Scenario::new(&[Faction::TheWinnu, Faction::TheArborec]).map_from(&[mecatol]).tech(0, T::LazaxGateFolding).control(Planet::MecatolRex, 0);
    let mut s = sc().state();
    assert!(lazax_gate_folding(&mut s, ALL_SOURCES, Seat(0), mecatol, Some(Unit::InfantryI)));
    assert_eq!(pieces(&s, Planet::MecatolRex, 0, Unit::InfantryI), 1);
    assert!(s.players().by_seat(Seat(0)).unwrap().tech.exhausted.contains(T::LazaxGateFolding), "exhaust this card");
    assert!(!lazax_gate_folding(&mut s, ALL_SOURCES, Seat(0), mecatol, Some(Unit::InfantryI)), "an exhausted card cannot be used again");
    assert_eq!(pieces(&s, Planet::MecatolRex, 0, Unit::InfantryI), 1);
    let mut nocard = Scenario::new(&[Faction::TheWinnu, Faction::TheArborec]).map_from(&[mecatol]).control(Planet::MecatolRex, 0).state();
    assert!(!lazax_gate_folding(&mut nocard, ALL_SOURCES, Seat(0), mecatol, Some(Unit::InfantryI)));
    let mut other = Scenario::new(&[Faction::TheWinnu, Faction::TheArborec]).map_from(&[mecatol]).tech(0, T::LazaxGateFolding).control(Planet::MecatolRex, 1).state();
    assert!(!lazax_gate_folding(&mut other, ALL_SOURCES, Seat(0), mecatol, Some(Unit::InfantryI)), "Mecatol must be ours");
    let mut noform = sc().state();
    assert!(!lazax_gate_folding(&mut noform, ALL_SOURCES, Seat(0), mecatol, None));
    assert!(!noform.players().by_seat(Seat(0)).unwrap().tech.exhausted.contains(T::LazaxGateFolding), "no form to place: nothing is exhausted");
}

#[test]
fn the_nekro_repair_exhausts_the_card_and_repairs_only_our_damaged_units() {
    let p = plain_systems()[1].data().planets[0];
    let sys = system_of(p);
    let mut s = Scenario::new(&[Faction::TheNekroVirus, Faction::TheArborec]).map_from(&[sys]).tech(0, T::ERRORERROR).control(p, 0).unit(sys, None, Unit::DreadnoughtI, 0, 1).unit(sys, None, Unit::DreadnoughtI, 1, 1).state();
    for u in s.board_mut().sys_mut(sys).units.as_mut_slice() {
        *u = u.with_sustained(true);
    }
    assert!(c4r_repair(&mut s, Seat(0)));
    let sustained = |s: &CoreState, seat: u8| s.board().sys(sys).units.as_slice().iter().filter(|u| u.owner() == Seat(seat) && u.sustained()).count();
    assert_eq!((sustained(&s, 0), sustained(&s, 1)), (0, 1), "all of our damaged units, nobody else's");
    assert!(s.players().by_seat(Seat(0)).unwrap().tech.exhausted.contains(T::ERRORERROR));
    let mut none = Scenario::new(&[Faction::TheNekroVirus, Faction::TheArborec]).state();
    assert!(!c4r_repair(&mut none, Seat(0)), "without the card nothing happens");
    }

#[test]
fn terraform_attaches_to_one_non_home_non_mecatol_planet_we_control_once() {
    let ps = plain_systems();
    let (a, b) = (ps[1].data().planets[0], ps[2].data().planets[0]);
    let key = NoteKey { note: PromissoryNote::Terraform, faction: Faction::TheArborec };
    let st = || {
        let mut s = Scenario::new(&[Faction::TheTitansOfUl, Faction::TheArborec]).map_from(&[ps[1], ps[2], system_of(Planet::MecatolRex)]).control(a, 0).control(b, 0).control(Planet::MecatolRex, 0).state();
        s.cards_mut().promissory.edit().insert(key, Seat(0));
        s
    };
    let mut s = st();
    assert!(!terraform(&mut s, Seat(0), a, true), "not a home planet");
    assert!(!terraform(&mut s, Seat(0), Planet::MecatolRex, false), "not Mecatol Rex");
    assert!(!terraform(&mut s, Seat(1), a, false), "the holder only");
    let mut foreign = st();
    foreign.board_mut().planet_mut(a).control = Some(Seat(1));
    assert!(!terraform(&mut foreign, Seat(0), a, false), "a planet we control");
    assert!(terraform(&mut s, Seat(0), a, false));
    assert!(s.board().planet(a).attach.as_slice().contains(&Attachment::Terraform));
    assert!(s.cards().promissory_faceup.contains_key(&key), "the card is in play");
    assert!(!terraform(&mut s, Seat(0), b, false), "once in the game");
    let mut nonote = Scenario::new(&[Faction::TheTitansOfUl, Faction::TheArborec]).map_from(&[ps[1]]).control(a, 0).state();
    assert!(!terraform(&mut nonote, Seat(0), a, false));
    let mut own = st();
    own.cards_mut().promissory.edit().remove(&key);
    own.cards_mut().promissory.edit().insert(NoteKey { note: PromissoryNote::Terraform, faction: Faction::TheTitansOfUl }, Seat(0));
    assert!(!terraform(&mut own, Seat(0), a, false), "our own faction's note is not played this way");
}

#[test]
fn the_inferno_places_a_cruiser_in_its_system_and_spends_a_strategy_token_after_it() {
    let sys = plain_systems()[1];
    let facts = InfernoFacts { scope: ALL_SOURCES, cruiser: Some(Unit::CruiserI), fleet_limit: 8, source: Code(5) };
    let sc = |strat: i16| Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec]).map_from(&[sys, plain_systems()[2]]).tokens(0, 3, 3, strat).unit(sys, None, Unit::TheInferno, 0, 1);
    let (mut s, k) = sc(2).build();
    assert_eq!(inferno_system(&s, &k, Seat(0), &facts), Some(sys));
    assert!(inferno(&mut s, &k, Seat(0), &facts));
    let n = |s: &CoreState, u: Unit| s.board().sys(sys).units.as_slice().iter().filter(|p| p.owner() == Seat(0) && p.kind() == u).count();
    assert_eq!(n(&s, Unit::CruiserI), 1, "1 cruiser");
    assert_eq!(s.players().by_seat(Seat(0)).unwrap().econ.strategic_tokens, 1, "1 token from the strategy pool");
    let (poor, k2) = sc(0).build();
    assert_eq!(inferno_system(&poor, &k2, Seat(0), &facts), None, "no token");
    let (nofl, k3) = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec]).map_from(&[sys]).tokens(0, 3, 3, 2).build();
    assert_eq!(inferno_system(&nofl, &k3, Seat(0), &facts), None, "no flagship on the board");
    let nocruiser = InfernoFacts { cruiser: None, ..facts };
    assert_eq!(inferno_system(&s, &k, Seat(0), &nocruiser), None);
}
