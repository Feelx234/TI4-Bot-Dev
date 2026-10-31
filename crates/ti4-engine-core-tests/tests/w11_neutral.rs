//! W11: a neutral side in the small windows of the combat and of the invasion. The neutral force is an owner that is no seat (Thunder's Edge
//! rules 5 to 7, 9): nobody is asked for it, a hit goes to the unit lowest on the neutral reference card. Expected values come from the rules text.

use ti4_engine_core::rules::combat::assault_cannon::{AssaultCannon, Facts as AcFacts};
use ti4_engine_core::rules::combat::assign::{Assign, AssignHits};
use ti4_engine_core::rules::combat::effect_hits::{Absorb, Destroy};
use ti4_engine_core::rules::combat::sustain_offer::{Offer, SustainOffer};
use ti4_engine_core::rules::combat::{neutral_casualty, NEUTRAL_LOWEST_FIRST};
use ti4_engine_core::rules::invasion::absorb::AbsorbGround;
use ti4_engine_core::generated::ActionCard;
use ti4_engine_core::kernel::children::HitTexts;
use ti4_engine_core::state::{Code, EventKind, Key, Piece};
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::plain_systems;

fn sys() -> System {
    plain_systems()[0]
}

/// Seat 0 and 1 are seated; `Seat(2)` is the neutral owner (no seat).
const NEUTRAL: Seat = Seat(2);

fn arena() -> Scenario {
    Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec]).map_from(&[sys()])
}

/// The scenario's state with the neutral force's `ships` in the space area of `sys()`, in the order given.
fn with_neutral(sc: Scenario, ships: &[Unit]) -> (CoreState, Kernel) {
    let (mut state, kernel) = sc.build();
    for ship in ships {
        state.board_mut().sys_mut(sys()).units.push(Piece::new(*ship, NEUTRAL));
    }
    (state, kernel)
}

fn left(rig: &Rig<impl Window>) -> Vec<Unit> {
    rig.state.board().sys(sys()).units.iter().filter(|p| p.owner() == NEUTRAL).map(|p| p.kind()).collect()
}

#[test]
fn the_reference_card_lists_the_lowest_unit_first_and_the_first_candidate_of_that_type_is_taken() {
    assert_eq!(NEUTRAL_LOWEST_FIRST.len(), 11);
    assert_eq!(NEUTRAL_LOWEST_FIRST[0], Unit::NeutralSpaceDock);
    assert_eq!(NEUTRAL_LOWEST_FIRST[10], Unit::NeutralFlagship);
    // The fighter is lower on the card than the cruiser, whatever the order of the list.
    assert_eq!(neutral_casualty(&[Unit::NeutralCruiser, Unit::NeutralFighter, Unit::NeutralFighter]), Some(1));
    assert_eq!(neutral_casualty(&[Unit::NeutralDreadnought, Unit::NeutralCruiser]), Some(1));
    assert_eq!(neutral_casualty(&[Unit::NeutralInfantry, Unit::NeutralMech]), Some(0));
    // A unit that is on no card is taken last-resort first.
    assert_eq!(neutral_casualty(&[Unit::CruiserI]), Some(0));
    assert_eq!(neutral_casualty(&[]), None);
}

#[test]
fn a_hit_on_the_neutral_force_takes_the_lowest_unit_on_the_card_and_asks_nobody() {
    covers!("rules::combat::assign::AssignHits");
    let (state, kernel) = with_neutral(arena(), &[Unit::NeutralDreadnought, Unit::NeutralCruiser, Unit::NeutralFighter, Unit::NeutralDestroyer]);
    let assign = Assign { player: NEUTRAL, system: sys(), ability: false, non_fighters_first: false, subtype: Code(5), cause: Code(6), during_space_combat: true, staging: true, announce: true };
    let mut rig = Rig::from_parts(state, kernel, AssignHits::new(assign, 3));
    rig.run();
    assert!(rig.is_done(), "nobody is asked for the neutral side");
    assert_eq!(left(&rig), vec![Unit::NeutralDreadnought], "the fighter, the destroyer and the cruiser went, in that order");
    let lost: Vec<Unit> = rig.driver.emitted.iter().filter(|e| e.kind == EventKind::ShipDestroyed).filter_map(|e| e.payload.unit(Key::Unit)).collect();
    assert_eq!(lost, vec![Unit::NeutralFighter, Unit::NeutralDestroyer, Unit::NeutralCruiser]);
}

#[test]
fn a_destruction_by_a_card_takes_the_lowest_unit_of_the_neutral_force_too() {
    covers!("rules::combat::effect_hits::Destroy");
    let (state, kernel) = with_neutral(arena(), &[Unit::NeutralCruiser, Unit::NeutralDestroyer]);
    let mut rig = Rig::from_parts(state, kernel, Destroy::new(NEUTRAL, sys(), ActionCard::CourageousToTheEnd, Code(5), Code(6)));
    rig.run();
    assert!(rig.is_done(), "nobody chooses for the neutral side");
    assert_eq!(left(&rig), vec![Unit::NeutralCruiser], "the destroyer is lower on the card");
}

#[test]
fn assault_cannon_takes_the_lowest_non_fighter_of_the_neutral_force() {
    covers!("rules::combat::assault_cannon::AssaultCannon");
    let ac = AcFacts { attacker: Seat(0), defender: NEUTRAL, system: sys(), subtype: Code(20), cause: Code(21), announce: true };
    let sc = arena().tech(0, Technology::AssaultCannon).unit(sys(), None, Unit::CruiserI, 0, 3);
    let (state, kernel) = with_neutral(sc, &[Unit::NeutralFighter, Unit::NeutralDreadnought, Unit::NeutralCruiser]);
    let mut rig = Rig::from_parts(state, kernel, AssaultCannon::new(ac));
    rig.run();
    assert!(rig.is_done(), "nobody is asked for the neutral side");
    assert_eq!(left(&rig), vec![Unit::NeutralFighter, Unit::NeutralDreadnought], "the cruiser is the lowest non-fighter ship; fighters are not taken");
}

#[test]
fn a_ground_hit_on_the_neutral_force_takes_the_lowest_ground_force_and_asks_nobody() {
    covers!("rules::invasion::absorb::AbsorbGround");
    let planet = sys().data().planets[0];
    let (mut state, kernel) = arena().build();
    // A mech that has sustained already cannot again: it stands first in the list, the infantry are lower on the card.
    state.board_mut().planet_mut(planet).units.push(Piece::new(Unit::NeutralMech, NEUTRAL).with_sustained(true));
    for kind in [Unit::NeutralInfantry, Unit::NeutralInfantry] {
        state.board_mut().planet_mut(planet).units.push(Piece::new(kind, NEUTRAL));
    }
    let mut rig = Rig::from_parts(state, kernel, AbsorbGround::new(NEUTRAL, sys(), planet, 2, false, false));
    rig.run();
    assert!(rig.is_done(), "nobody is asked for the neutral side");
    let left: Vec<Unit> = rig.state.board().planet(planet).units.iter().map(|p| p.kind()).collect();
    assert_eq!(left, vec![Unit::NeutralMech], "both infantry went before the damaged mech");
}

/// Reflective Shielding: the hits it staged while the use of SUSTAIN DAMAGE was announced are absorbed by the window of the use.
#[test]
fn the_hits_a_reflective_shielding_staged_are_absorbed_right_after_the_use_is_announced() {
    covers!("rules::combat::sustain_offer::SustainOffer");
    let sc = arena().unit(sys(), None, Unit::DreadnoughtI, 0, 1).unit(sys(), None, Unit::CruiserI, 1, 1).unit(sys(), None, Unit::DestroyerI, 1, 1);
    let (mut state, kernel) = sc.build();
    // The card was played in the window of the use: two hits against the producer of the hit (seat 1).
    state.transient_mut().combat.pending_reflective_hits = Some((sys(), Seat(1), 2));
    let offer = Offer { player: Seat(0), producer: Seat(1), system: sys(), ability: false, in_combat: true, non_fighters_first: false, source: Code(9), staging: false, announce: true };
    let mut rig = Rig::from_parts(state, kernel, SustainOffer::new(offer, 1));
    rig.run();
    // Seat 0 is asked to sustain with the dreadnought.
    rig.pick(0);
    // Seat 1 (two ships, no sustain) now chooses the casualties of the two reflected hits.
    assert_eq!(rig.asked().actor, Seat(1), "the producer of the cancelled hit absorbs the reflected hits");
    rig.pick(0);
    assert!(rig.is_done());
    assert!(rig.state.transient().combat.pending_reflective_hits.is_none(), "the staged hits are spent");
    let ships_of_1 = rig.state.board().sys(sys()).units.iter().filter(|p| p.owner() == Seat(1)).count();
    assert_eq!(ships_of_1, 0, "two hits, two ships");
}

#[test]
fn an_absorption_of_hits_is_announced_and_can_be_a_neutral_producers() {
    covers!("rules::combat::effect_hits::Absorb");
    let sc = arena().unit(sys(), None, Unit::CruiserI, 0, 2);
    let (state, kernel) = with_neutral(sc, &[Unit::NeutralDestroyer]);
    let texts = HitTexts { staging: false, commander: Code(1), casualty: Code(2), effect_hit: Code(3) };
    let mut rig = Rig::from_parts(state, kernel, Absorb::new(Seat(0), NEUTRAL, sys(), 1, texts).announced());
    rig.run();
    // Two cruisers: seat 0 chooses (one option).
    assert_eq!(rig.asked().actor, Seat(0));
}
