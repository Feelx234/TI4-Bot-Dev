//! D4 lane A3: the ported action cards that place or move pieces, bring in the neutral player, or research: Pirate Contract and Fleet, Mercenary
//! Contract, Ghost Ship, Refit Troops, Decoy Operation, Ghost Squad, Crash Landing, Focused Research, Divert Funding, Plagiarize, Impersonation.

use ti4_engine_core::choice::cards::Opt as CO;
use ti4_engine_core::generated::{ActionCard, Planet, Technology, Unit};
use ti4_engine_core::hex::Hex;
use ti4_engine_core::kernel::stage::AbilityTable;
use ti4_engine_core::query::tech::UnitRoster;
use ti4_engine_core::rules::action_cards::effects::Play;
use ti4_engine_core::rules::action_cards::{Env, Facts, Placed};
use ti4_engine_core::state::{Code, PlanetFlags, SysFlags};
use ti4_engine_core_tests::inv::ALL_SOURCES;
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::plain_systems;

const NEUTRAL: Seat = Seat(8);

fn four() -> Scenario {
    Scenario::new(&[Faction::TheArborec, Faction::TheEmiratesOfHacan, Faction::TheFederationOfSol, Faction::TheWinnu]).map_ring(7)
}

fn linked() -> AbilityTable {
    let mut t = AbilityTable::default();
    ti4_engine_core::driver::children::register(&mut t);
    t
}

fn roster() -> &'static UnitRoster {
    Box::leak(Box::new(UnitRoster::default()))
}

fn env() -> Env {
    let placed = |u: Unit| Placed { unit: Some(u), on_card: 0 };
    Env {
        mahact_held: 0,
        scope: ALL_SOURCES,
        blanked: false,
        action_card: Code(0),
        supply_base: Code(0),
        note_order: |a, b| a.cmp(&b),
        infantry: placed(Unit::InfantryI),
        cruiser: placed(Unit::CruiserI),
        fighter: placed(Unit::FighterI),
        facts: Facts {
            neutral: Some(NEUTRAL),
            action_draw_plain: true,
            rules_version: 35,
            destroyer: placed(Unit::DestroyerI),
            dreadnought: placed(Unit::DreadnoughtI),
            mech: placed(Unit::FactionlessMech),
            roster: Some(roster()),
            ..Facts::default()
        },
    }
}

fn rig_env(card: ActionCard, st: CoreState, e: Env) -> Rig<Play> {
    let play = Play::of(&st, card, Seat(0), e).unwrap_or_else(|| panic!("{card:?} has a play"));
    Rig::from_parts(st, ti4_engine_core_tests::kernel_all(), play).with_table(linked()).seeded(1)
}

fn rig(card: ActionCard, st: CoreState) -> Rig<Play> {
    rig_env(card, st, env())
}

fn o(x: CO) -> Opt {
    Opt::Cards(x)
}

fn sys(i: usize) -> System {
    plain_systems()[i]
}

fn planet(i: usize) -> Planet {
    sys(i).data().planets[0]
}

fn touch(s: &mut CoreState, system: System) {
    s.board_mut().sys_mut(system).flags.set(SysFlags::TOUCHED, true);
}

fn neutral_in_space(s: &CoreState, system: System) -> Vec<Unit> {
    s.board().sys(system).units.iter().filter(|u| u.owner() == NEUTRAL).map(|u| u.kind()).collect()
}

fn mine(s: &CoreState, planet: Planet, kind: Unit) -> usize {
    s.board().planet(planet).units.iter().filter(|u| u.owner() == Seat(0) && u.kind() == kind).count()
}

// ---- the neutral player ----------------------------------------------------------------------------------------

#[test]
fn pirate_contract_puts_a_neutral_destroyer_in_a_system_nobody_has_ships_in_outside_home_systems() {
    covers_card!(ActionCard::PirateContract1);
    let mut st = four().unit(sys(2), None, Unit::CruiserI, 1, 1).unit(sys(4), None, Unit::DestroyerI, 0, 1).state();
    for i in [1, 2, 3, 4] {
        touch(&mut st, sys(i));
    }
    // A touched home system is never a candidate.
    let home = System::ALL.iter().copied().find(|s| s.data().home_system && !s.data().planets.is_empty() && !s.data().hyperlane).unwrap();
    touch(&mut st, home);
    let mut r = rig(ActionCard::PirateContract1, st);
    r.run();
    // sys(2) and sys(4) hold a seated player's ships; sys(5) was never touched; the home system is excluded.
    assert_eq!(r.opts(), vec![o(CO::System(sys(1))), o(CO::System(sys(3)))].into_iter().collect::<Vec<_>>());
    r.assert_options_sound();
    r.pick(1);
    assert!(r.is_done());
    assert_eq!(neutral_in_space(&r.state, sys(3)), vec![Unit::NeutralDestroyer]);
    assert!(neutral_in_space(&r.state, sys(1)).is_empty());
    for card in [ActionCard::PirateContract2, ActionCard::PirateContract3, ActionCard::PirateContract4] {
        let mut st = four().state();
        touch(&mut st, sys(1));
        let mut again = rig(card, st);
        again.run();
        assert!(again.is_done(), "{card:?}: one candidate, taken unasked");
        assert_eq!(neutral_in_space(&again.state, sys(1)), vec![Unit::NeutralDestroyer], "{card:?}");
    }
}

#[test]
fn a_neutral_card_is_not_played_without_a_neutral_seat_at_the_table() {
    let mut e = env();
    e.facts.neutral = None;
    let st = four().state();
    for card in [ActionCard::PirateContract1, ActionCard::PirateFleet, ActionCard::MercenaryContract] {
        assert!(Play::of(&st, card, Seat(0), e).is_none(), "{card:?}");
    }
}

#[test]
fn pirate_fleet_costs_three_resources_and_puts_a_neutral_fleet_in_the_system() {
    covers_card!(ActionCard::PirateFleet);
    let mut st = four().control(Planet::Starpoint, 0).state();
    touch(&mut st, sys(1));
    let mut r = rig(ActionCard::PirateFleet, st);
    r.run();
    assert!(r.is_done(), "Starpoint pays the bill alone and one system is open");
    assert!(r.state.board().planet(Planet::Starpoint).flags.has(PlanetFlags::EXHAUSTED), "3 resources paid with the planet");
    let mut fleet = neutral_in_space(&r.state, sys(1));
    fleet.sort();
    let mut want = vec![Unit::NeutralCarrier, Unit::NeutralCruiser, Unit::NeutralDestroyer, Unit::NeutralFighter, Unit::NeutralFighter];
    want.sort();
    assert_eq!(fleet, want);
    // Two resources do not pay it: the card has no play to offer, nothing is placed.
    let mut poor = four().control(Planet::Quann, 0).state();
    touch(&mut poor, sys(1));
    let mut r2 = rig(ActionCard::PirateFleet, poor);
    r2.run();
    assert!(neutral_in_space(&r2.state, sys(1)).is_empty());
}

#[test]
fn mercenary_contract_lands_two_neutral_infantry_on_an_empty_planet_for_two_trade_goods() {
    covers_card!(ActionCard::MercenaryContract);
    let mut st = four().goods(0, 3).state();
    // An empty planet that has a board entry, a planet with units, a home planet and an untouched one.
    st.board_mut().planet_mut(planet(1)).flags.set(PlanetFlags::UNITS_ENTRY, true);
    st.board_mut().planet_mut(planet(2)).flags.set(PlanetFlags::UNITS_ENTRY, true);
    st.board_mut().planet_mut(planet(2)).units.push(ti4_engine_core::state::Piece::new(Unit::InfantryI, Seat(1)));
    st.board_mut().planet_mut(Planet::Muaat).flags.set(PlanetFlags::UNITS_ENTRY, true);
    let mut r = rig(ActionCard::MercenaryContract, st.clone());
    r.run();
    assert!(r.is_done(), "the one empty planet outside the home systems, taken unasked");
    let there: Vec<Unit> = r.state.board().planet(planet(1)).units.iter().map(|u| u.kind()).collect();
    assert_eq!(there, vec![Unit::NeutralInfantry, Unit::NeutralInfantry]);
    assert!(r.state.board().planet(planet(1)).units.iter().all(|u| u.owner() == NEUTRAL));
    assert_eq!(r.state.players().by_seat(Seat(0)).unwrap().econ.trade_goods, 1, "two trade goods paid");
    // One trade good is not enough: nothing offered, nothing paid.
    let mut poor = st.clone();
    poor.players_mut().by_seat_mut(Seat(0)).unwrap().econ.trade_goods = 1;
    let mut r2 = rig(ActionCard::MercenaryContract, poor);
    r2.run();
    assert!(r2.is_done() && r2.state.board().planet(planet(1)).units.is_empty());
    assert_eq!(r2.state.players().by_seat(Seat(0)).unwrap().econ.trade_goods, 1);
}

// ---- Ghost Ship, Ghost Squad -----------------------------------------------------------------------------------------

#[test]
fn ghost_ship_puts_a_destroyer_into_a_wormhole_system_without_enemy_ships_outside_home_systems() {
    covers_card!(ActionCard::GhostShip);
    let alpha = System::AlphaWormhole;
    let beta = System::BetaWormhole;
    let p = plain_systems();
    let sc = Scenario::new(&[Faction::TheArborec, Faction::TheEmiratesOfHacan])
        .map_hexes(&[(p[0], Hex::new(0, 0)), (alpha, Hex::new(1, 0)), (beta, Hex::new(2, 0)), (p[1], Hex::new(3, 0))])
        .unit(beta, None, Unit::CruiserI, 1, 1);
    let mut r = rig(ActionCard::GhostShip, sc.clone().state());
    r.run();
    // Beta holds an enemy ship: only alpha, taken unasked.
    assert!(r.is_done());
    assert_eq!(r.state.board().sys(alpha).units.iter().filter(|u| u.owner() == Seat(0) && u.kind() == Unit::DestroyerI).count(), 1);
    assert_eq!(r.state.board().sys(beta).units.iter().filter(|u| u.owner() == Seat(0)).count(), 0);
    // Both open: the holder chooses, in the order of the map.
    let both = Scenario::new(&[Faction::TheArborec, Faction::TheEmiratesOfHacan]).map_hexes(&[(p[0], Hex::new(0, 0)), (beta, Hex::new(1, 0)), (alpha, Hex::new(2, 0))]);
    let mut r2 = rig(ActionCard::GhostShip, both.state());
    r2.run();
    assert_eq!(r2.opts(), vec![o(CO::System(beta)), o(CO::System(alpha))]);
    r2.assert_options_sound();
    r2.pick(1);
    assert_eq!(r2.state.board().sys(alpha).units.iter().filter(|u| u.owner() == Seat(0)).count(), 1);
}

#[test]
fn ghost_squad_moves_all_ground_forces_of_one_type_between_the_planets_we_control_in_the_active_system() {
    covers_card!(ActionCard::GhostSquad);
    let two = System::StarpointNewAlbion;
    let (a, b) = (two.data().planets[0], two.data().planets[1]);
    let st = Scenario::new(&[Faction::TheArborec, Faction::TheEmiratesOfHacan])
        .map_from(&[two])
        .active_system(two)
        .control(a, 0)
        .control(b, 0)
        .unit(two, Some(a), Unit::InfantryI, 0, 3)
        .unit(two, Some(a), Unit::FactionlessMech, 0, 1)
        .unit(two, Some(b), Unit::InfantryI, 0, 1)
        .unit(two, Some(a), Unit::PDSI, 0, 1)
        .state();
    let mut r = rig(ActionCard::GhostSquad, st);
    r.run();
    // From a: infantry and mech to b; from b: infantry to a. Structures are no ground force.
    let moves: Vec<(Planet, Planet, Unit, u8)> = r
        .opts()
        .iter()
        .filter_map(|x| if let Opt::Cards(CO::Move { from, to, kind, count }) = x { Some((*from, *to, *kind, *count)) } else { None })
        .collect();
    assert_eq!(moves.len(), 3);
    assert!(moves.contains(&(a, b, Unit::InfantryI, 3)) && moves.contains(&(a, b, Unit::FactionlessMech, 1)) && moves.contains(&(b, a, Unit::InfantryI, 1)));
    assert!(r.asked().opts.iter().any(|x| matches!(x, Opt::Decline)), "the holder may stop");
    r.assert_options_sound();
    r.pick_opt(o(CO::Move { from: a, to: b, kind: Unit::InfantryI, count: 3 }));
    assert_eq!(mine(&r.state, a, Unit::InfantryI), 0);
    assert_eq!(mine(&r.state, b, Unit::InfantryI), 4);
    // The planet that received is not a source any more; the mech may still go.
    let left: Vec<_> = r.opts().iter().filter_map(|x| if let Opt::Cards(CO::Move { from, kind, .. }) = x { Some((*from, *kind)) } else { None }).collect();
    assert!(left.contains(&(a, Unit::FactionlessMech)) && !left.iter().any(|(f, _)| *f == b), "{left:?}");
    r.decline();
    assert!(r.is_done());
    assert_eq!(mine(&r.state, a, Unit::PDSI), 1, "structures stay");
}

// ---- the ground cards ----------------------------------------------------------------------------------------------------

#[test]
fn refit_troops_replaces_up_to_two_infantry_by_mechs_in_place() {
    covers_card!(ActionCard::RefitTroops);
    let st = four().control(planet(1), 0).unit(sys(1), Some(planet(1)), Unit::InfantryI, 0, 2).control(planet(2), 0).unit(sys(2), Some(planet(2)), Unit::InfantryI, 0, 1).unit(sys(2), Some(planet(2)), Unit::InfantryI, 1, 3).state();
    let mut r = rig(ActionCard::RefitTroops, st.clone());
    r.run();
    assert_eq!(r.opts().len(), 3, "each infantry of ours, never a rival's");
    r.assert_options_sound();
    r.pick(0);
    // The second question also offers to stop, and the infantry just chosen is not offered again.
    assert_eq!(r.opts().len(), 3);
    assert!(r.opts().contains(&o(CO::Stop)));
    r.pick(0);
    assert!(r.is_done());
    let total_inf = mine(&r.state, planet(1), Unit::InfantryI) + mine(&r.state, planet(2), Unit::InfantryI);
    let total_mech = mine(&r.state, planet(1), Unit::FactionlessMech) + mine(&r.state, planet(2), Unit::FactionlessMech);
    assert_eq!((total_inf, total_mech), (1, 2), "two infantry became two mechs");
    assert_eq!(r.state.board().planet(planet(2)).units.iter().filter(|u| u.owner() == Seat(1)).count(), 3);
    // Stopping after one.
    let mut one = rig(ActionCard::RefitTroops, st.clone());
    one.run().pick(0);
    one.pick_opt(o(CO::Stop));
    assert!(one.is_done());
    let m = mine(&one.state, planet(1), Unit::FactionlessMech) + mine(&one.state, planet(2), Unit::FactionlessMech);
    assert_eq!(m, 1);
    // With the mechs used up (four in the box) nothing is offered.
    let full = st.clone();
    let mut e = env();
    e.facts.mech = Placed { unit: Some(Unit::FactionlessMech), on_card: 4 };
    let mut none = rig_env(ActionCard::RefitTroops, full, e);
    none.run();
    assert!(none.is_done());
    assert_eq!(mine(&none.state, planet(1), Unit::InfantryI), 2);
}

#[test]
fn decoy_operation_moves_up_to_two_ground_forces_onto_a_planet_we_control_in_the_active_system() {
    covers_card!(ActionCard::DecoyOperation);
    let two = System::StarpointNewAlbion;
    let (a, b) = (two.data().planets[0], two.data().planets[1]);
    let other = sys(3);
    let sc = Scenario::new(&[Faction::TheArborec, Faction::TheEmiratesOfHacan])
        .map_from(&[two, other])
        .active_system(two)
        .control(a, 0)
        .control(b, 0)
        .control(other.data().planets[0], 0)
        .unit(two, Some(a), Unit::InfantryI, 0, 1)
        .unit(other, Some(other.data().planets[0]), Unit::InfantryI, 0, 2)
        .unit(other, Some(other.data().planets[0]), Unit::FactionlessMech, 0, 1)
        .unit(other, Some(other.data().planets[0]), Unit::InfantryI, 1, 4);
    let mut r = rig(ActionCard::DecoyOperation, sc.clone().state());
    r.run();
    assert_eq!(r.opts().len(), 4, "every ground force of ours on the board, the rival's not");
    r.assert_options_sound();
    r.pick(1);
    assert!(r.opts().contains(&o(CO::Stop)), "the second pull may stop");
    assert_eq!(r.opts().len(), 4, "three others and the stop");
    r.pick(0);
    // Where to: the planets of ours in the active system.
    assert_eq!(r.opts().len(), 2);
    r.assert_options_sound();
    r.pick_opt(o(CO::Spot(two, b)));
    assert!(r.is_done());
    let on_b = r.state.board().planet(b).units.iter().filter(|u| u.owner() == Seat(0)).count();
    assert_eq!(on_b, 2, "two forces landed on b");
    let left_far: usize = r.state.board().planet(other.data().planets[0]).units.iter().filter(|u| u.owner() == Seat(0)).count();
    assert_eq!(left_far + r.state.board().planet(a).units.iter().filter(|u| u.owner() == Seat(0)).count(), 2, "two of our four forces were taken");
    assert_eq!(r.state.board().planet(other.data().planets[0]).units.iter().filter(|u| u.owner() == Seat(1)).count(), 4);
    // Two or fewer ground forces: both are taken without a question and only the destination is asked.
    let few = sc.clone().state();
    let mut few = few;
    few.board_mut().planet_mut(other.data().planets[0]).units.retain(|u| u.owner() == Seat(1));
    let mut r2 = rig(ActionCard::DecoyOperation, few);
    r2.run();
    assert_eq!(r2.opts().len(), 2, "only the destination");
    assert!(matches!(r2.asked().ask, Ask::Cards(ti4_engine_core::choice::cards::Ask::Pick { what: ti4_engine_core::choice::cards::What::Land, .. })));
    // Nowhere to land (no planet of ours in the active system): nothing is taken.
    let mut away = sc.state();
    away.meta_mut().active_system = Some(other);
    away.board_mut().planet_mut(other.data().planets[0]).control = None;
    let before = away.clone();
    let mut r3 = rig(ActionCard::DecoyOperation, away);
    r3.run();
    assert!(r3.is_done() && r3.state == before);
}

#[test]
fn crash_landing_lands_a_ground_force_from_the_space_area_of_the_system_the_last_ship_was_lost_in() {
    covers_card!(ActionCard::CrashLanding);
    let two = System::StarpointNewAlbion;
    let (a, b) = (two.data().planets[0], two.data().planets[1]);
    let mut st = Scenario::new(&[Faction::TheArborec, Faction::TheEmiratesOfHacan]).map_from(&[two]).unit(two, Some(a), Unit::InfantryI, 1, 1).state();
    // Ground forces in the space area (carried by the ships that were lost): the kit places none there.
    for (kind, seat, n) in [(Unit::InfantryI, 0, 2), (Unit::FactionlessMech, 0, 1), (Unit::InfantryI, 1, 1)] {
        for _ in 0..n {
            st.board_mut().sys_mut(two).units.push(ti4_engine_core::state::Piece::new(kind, Seat(seat)));
        }
    }
    st.transient_mut().combat.last_ship_destroyed = Some((two, Seat(0), Unit::CruiserI));
    let mut r = rig(ActionCard::CrashLanding, st.clone());
    r.run();
    assert_eq!(r.opts().len(), 2, "the kinds of ground force of ours in space: infantry and mech");
    r.assert_options_sound();
    let infantry = r.opts().iter().position(|x| matches!(x, Opt::Cards(CO::Kind(Unit::InfantryI)))).unwrap();
    r.pick(infantry);
    assert_eq!(r.opts(), vec![o(CO::Spot(two, a)), o(CO::Spot(two, b))]);
    r.pick(0);
    assert!(r.is_done());
    let in_space = |s: &CoreState, kind: Unit| s.board().sys(two).units.iter().filter(|u| u.owner() == Seat(0) && u.kind() == kind).count();
    assert_eq!(in_space(&r.state, Unit::InfantryI), 1, "one of the two landed");
    assert_eq!(mine(&r.state, a, Unit::InfantryI), 1);
    // Another player's forces were on the planet: the holder coexists there.
    assert!(r.state.board().planet(a).coexist.contains(Seat(0)));
    assert!(r.state.board().planet(a).flags.has(PlanetFlags::COEXIST_ENTRY));
    // On the empty planet nobody coexists.
    let mut empty = rig(ActionCard::CrashLanding, st.clone());
    empty.run().pick(infantry);
    empty.pick(1);
    assert!(!empty.state.board().planet(b).coexist.contains(Seat(0)));
    // The last ship was another player's, or no ship was lost, or nothing of ours is in space: nothing happens.
    let mut theirs = st.clone();
    theirs.transient_mut().combat.last_ship_destroyed = Some((two, Seat(1), Unit::CruiserI));
    let mut r2 = rig(ActionCard::CrashLanding, theirs.clone());
    r2.run();
    assert!(r2.is_done() && r2.state == theirs);
    let mut none = st.clone();
    none.transient_mut().combat.last_ship_destroyed = None;
    let mut r3 = rig(ActionCard::CrashLanding, none.clone());
    r3.run();
    assert!(r3.is_done() && r3.state == none);
    // Mecatol Rex is no landing place.
    let mut m = Scenario::new(&[Faction::TheArborec, Faction::TheEmiratesOfHacan]).map_from(&[System::MecatolRex18]).state();
    m.board_mut().sys_mut(System::MecatolRex18).units.push(ti4_engine_core::state::Piece::new(Unit::InfantryI, Seat(0)));
    m.transient_mut().combat.last_ship_destroyed = Some((System::MecatolRex18, Seat(0), Unit::CruiserI));
    let mut r4 = rig(ActionCard::CrashLanding, m.clone());
    r4.run();
    assert!(r4.is_done() && r4.state == m);
}

// ---- technology -----------------------------------------------------------------------------------------------------------------

fn owned(s: &CoreState, seat: u8) -> Vec<Technology> {
    s.players().by_seat(Seat(seat)).unwrap().tech.owned.iter().collect()
}

#[test]
fn focused_research_pays_four_trade_goods_and_researches_the_chosen_technology() {
    covers_card!(ActionCard::FocusedResearch);
    let st = four().goods(0, 5).state();
    let mut r = rig(ActionCard::FocusedResearch, st);
    r.run();
    let options: Vec<Technology> = r.opts().iter().filter_map(|x| if let Opt::Cards(CO::Tech(t)) = x { Some(*t) } else { None }).collect();
    assert!(!options.is_empty());
    assert!(options.contains(&Technology::NeuralMotivator), "a technology without prerequisites is on offer");
    assert!(!options.contains(&Technology::CruiserII), "a unit upgrade needs prerequisites the seat does not have");
    r.assert_options_sound();
    r.pick_opt(o(CO::Tech(Technology::NeuralMotivator)));
    assert!(r.is_done());
    assert_eq!(owned(&r.state, 0), vec![Technology::NeuralMotivator]);
    assert_eq!(r.state.players().by_seat(Seat(0)).unwrap().econ.trade_goods, 1, "four paid");
    // Fewer than four trade goods: the card cannot resolve.
    let poor = four().goods(0, 3).state();
    let mut r2 = rig(ActionCard::FocusedResearch, poor.clone());
    r2.run();
    assert!(r2.is_done() && r2.state == poor);
}

#[test]
fn divert_funding_returns_one_technology_and_researches_another() {
    covers_card!(ActionCard::DivertFunding);
    let st = four().tech(0, Technology::NeuralMotivator).tech(0, Technology::CruiserII).tech(0, Technology::Bioplasmosis).state();
    let mut r = rig(ActionCard::DivertFunding, st);
    r.run();
    // Only a technology that is neither a unit upgrade nor a faction technology can be returned: one candidate, returned unasked.
    let mut now = owned(&r.state, 0);
    now.sort();
    let mut kept = vec![Technology::CruiserII, Technology::Bioplasmosis];
    kept.sort();
    assert_eq!(now, kept, "the neural motivator was returned at once");
    // Then a technology to research: the returned one is researchable again.
    assert!(r.opts().contains(&o(CO::Tech(Technology::NeuralMotivator))));
    assert!(!r.opts().contains(&o(CO::Tech(Technology::CruiserII))), "already owned");
    r.assert_options_sound();
    r.pick_opt(o(CO::Tech(Technology::BioStims)));
    assert!(r.is_done());
    let mut now = owned(&r.state, 0);
    now.sort();
    let mut want = vec![Technology::CruiserII, Technology::Bioplasmosis, Technology::BioStims];
    want.sort();
    assert_eq!(now, want, "the neural motivator went, the bio-stims came");
    // Two returnable technologies: the holder chooses.
    let two = four().tech(0, Technology::NeuralMotivator).tech(0, Technology::GravityDrive).state();
    let mut r2 = rig(ActionCard::DivertFunding, two);
    r2.run();
    assert_eq!(r2.opts().len(), 2);
    r2.assert_options_sound();
    // Nothing to return: the card does nothing.
    let none = four().tech(0, Technology::CruiserII).state();
    let mut r3 = rig(ActionCard::DivertFunding, none.clone());
    r3.run();
    assert!(r3.is_done() && r3.state == none);
}

#[test]
fn plagiarize_takes_a_non_faction_technology_from_a_neighbour_for_five_influence() {
    covers_card!(ActionCard::Plagiarize);
    // Neighbours of seat 0 at a table of four are seats 3 and 1.
    let st = four()
        .control(Planet::Starpoint, 0)
        .control(Planet::Torkan, 0)
        .control(Planet::Lodor, 0)
        .tech(1, Technology::NeuralMotivator)
        .tech(1, Technology::Bioplasmosis) // a faction technology
        .tech(3, Technology::GravityDrive)
        .tech(2, Technology::DuraniumArmor) // across the table
        .tech(0, Technology::GravityDrive)
        .state();
    let mut r = rig(ActionCard::Plagiarize, st.clone());
    r.run();
    // The bill of five influence is paid first: Torkan 3, Starpoint 1 and Lodor 1 are all needed.
    assert!(matches!(r.asked().ask, Ask::Production(_)));
    while !r.is_done() {
        r.pick(0);
    }
    // Seat 3's gravity drive is already ours, seat 1's faction technology and seat 2's technology are out of reach: one candidate.
    let mut now = owned(&r.state, 0);
    now.sort();
    let mut want = vec![Technology::GravityDrive, Technology::NeuralMotivator];
    want.sort();
    assert_eq!(now, want);
    assert_eq!(owned(&r.state, 1), vec![Technology::Bioplasmosis], "taken from the neighbour");
    assert_eq!(owned(&r.state, 3), vec![Technology::GravityDrive]);
    for p in [Planet::Starpoint, Planet::Torkan, Planet::Lodor] {
        assert!(r.state.board().planet(p).flags.has(PlanetFlags::EXHAUSTED), "{p:?}");
    }
    // Two candidates: the holder chooses, the neighbour before first.
    let more = st.clone();
    let mut more = more;
    more.players_mut().by_seat_mut(Seat(3)).unwrap().tech.owned.insert(Technology::AntimassDeflectors);
    let mut r2 = rig(ActionCard::Plagiarize, more);
    r2.run();
    while matches!(r2.asked().ask, Ask::Production(_)) {
        r2.pick(0);
    }
    let steals: Vec<(u8, Technology)> = r2.opts().iter().filter_map(|x| if let Opt::Cards(CO::Steal { from, tech }) = x { Some((from.0, *tech)) } else { None }).collect();
    assert_eq!(steals, vec![(3, Technology::AntimassDeflectors), (1, Technology::NeuralMotivator)]);
    r2.assert_options_sound();
    r2.pick(0);
    assert!(r2.is_done());
    assert!(owned(&r2.state, 0).contains(&Technology::AntimassDeflectors));
    assert!(!owned(&r2.state, 3).contains(&Technology::AntimassDeflectors));
    assert!(owned(&r2.state, 1).contains(&Technology::NeuralMotivator));
    // Not enough influence: the card does nothing.
    let poor = four().control(Planet::Torkan, 0).tech(1, Technology::NeuralMotivator).state();
    let mut r3 = rig(ActionCard::Plagiarize, poor);
    r3.run();
    assert!(!owned(&r3.state, 0).contains(&Technology::NeuralMotivator));
}

#[test]
fn impersonation_pays_three_influence_and_gives_the_holder_a_secret_objective() {
    covers_card!(ActionCard::Impersonation);
    let mut st = four().control(Planet::Torkan, 0).state();
    st.decks_mut().secret = ti4_engine_core::containers::Deck::new(vec![ti4_engine_core::generated::SecretObjective::ALL[0], ti4_engine_core::generated::SecretObjective::ALL[1]]);
    let mut r = rig(ActionCard::Impersonation, st);
    r.run();
    assert!(r.is_done(), "Torkan pays the three alone");
    assert!(r.state.board().planet(Planet::Torkan).flags.has(PlanetFlags::EXHAUSTED));
    assert_eq!(r.state.players().by_seat(Seat(0)).unwrap().cards.secrets.as_slice(), &[ti4_engine_core::generated::SecretObjective::ALL[0]]);
    assert_eq!(r.state.decks().secret.cards().len(), 1);
}

// ---- Brilliance, Reveal Prototype, Strategize and Overrule ------------------------------------------------------------------

#[test]
fn brilliance_gives_a_player_without_a_breakthrough_their_own() {
    use ti4_engine_core::generated::Breakthrough;
    covers_card!(ActionCard::Brilliance);
    let mut e = env();
    e.facts.fracture_in_play = true; // the Fracture roll is not asked of a position that has it already
    e.facts.own_breakthrough[2] = Some(Breakthrough::BellumGloriosum);
    e.facts.own_breakthrough[3] = Some(Breakthrough::ArchonsGift);
    let mut st = four().state();
    st.players_mut().by_seat_mut(Seat(1)).unwrap().cards.breakthrough = Some(Breakthrough::Mindsieve);
    let mut r = rig_env(ActionCard::Brilliance, st.clone(), e);
    r.run();
    assert_eq!(r.opts(), vec![o(CO::Seat(Seat(0))), o(CO::Seat(Seat(2))), o(CO::Seat(Seat(3)))], "every player without one, the holder too");
    r.assert_options_sound();
    r.pick_opt(o(CO::Seat(Seat(2))));
    assert!(r.is_done());
    assert_eq!(r.state.players().by_seat(Seat(2)).unwrap().cards.breakthrough, Some(Breakthrough::BellumGloriosum));
    assert_eq!(r.state.players().by_seat(Seat(3)).unwrap().cards.breakthrough, None);
    assert_eq!(r.state.players().by_seat(Seat(1)).unwrap().cards.breakthrough, Some(Breakthrough::Mindsieve));
    // A faction without a breakthrough of its own gains nothing.
    let mut r2 = rig_env(ActionCard::Brilliance, st, {
        let mut e = env();
        e.facts.fracture_in_play = true;
        e
    });
    r2.run().pick_opt(o(CO::Seat(Seat(3))));
    assert_eq!(r2.state.players().by_seat(Seat(3)).unwrap().cards.breakthrough, None);
}

#[test]
fn strategize_performs_the_secondary_of_a_strategy_card_another_player_holds_or_nobody_does() {
    covers_card!(ActionCard::Strategize1);
    let mut st = four().state();
    st.players_mut().by_seat_mut(Seat(1)).unwrap().strat.push(StrategyCard::Trade);
    st.players_mut().by_seat_mut(Seat(2)).unwrap().strat.push(StrategyCard::Leadership);
    st.players_mut().by_seat_mut(Seat(2)).unwrap().strat.push(StrategyCard::Warfare);
    st.players_mut().by_seat_mut(Seat(2)).unwrap().strat_exhausted.insert(StrategyCard::Warfare);
    st.players_mut().by_seat_mut(Seat(0)).unwrap().strat.push(StrategyCard::Politics);
    st.strategy_mut().unclaimed = [StrategyCard::Imperial].into_iter().collect();
    for card in [ActionCard::Strategize1, ActionCard::Strategize2, ActionCard::Strategize3, ActionCard::Strategize4] {
        let mut r = rig(card, st.clone());
        r.run();
        // Other seats' ready cards in seating order, then those on the mat; ours and the exhausted ones are left out.
        assert_eq!(
            r.opts(),
            vec![
                o(CO::Perform { card: StrategyCard::Trade, holder: Some(Seat(1)) }),
                o(CO::Perform { card: StrategyCard::Leadership, holder: Some(Seat(2)) }),
                o(CO::Perform { card: StrategyCard::Imperial, holder: None }),
            ],
            "{card:?}"
        );
        r.assert_options_sound();
    }
    // The Trade secondary replenishes the holder's commodities.
    let mut r = rig(ActionCard::Strategize1, st.clone());
    r.run().pick_opt(o(CO::Perform { card: StrategyCard::Trade, holder: Some(Seat(1)) }));
    assert!(r.is_done());
    let limit = i16::from(Faction::TheArborec.data().commodities);
    assert_eq!(r.state.players().by_seat(Seat(0)).unwrap().econ.commodities, limit);
    // The Imperial secondary draws a secret objective.
    let mut with_deck = st.clone();
    with_deck.decks_mut().secret = ti4_engine_core::containers::Deck::new(vec![ti4_engine_core::generated::SecretObjective::ALL[2]]);
    let mut imperial = rig(ActionCard::Strategize1, with_deck);
    imperial.run().pick_opt(o(CO::Perform { card: StrategyCard::Imperial, holder: None }));
    assert!(imperial.is_done());
    assert_eq!(imperial.state.players().by_seat(Seat(0)).unwrap().cards.secrets.as_slice(), &[ti4_engine_core::generated::SecretObjective::ALL[2]]);
    // A table where the replenishments cause events: the string performs the ability.
    let mut e = env();
    e.facts.replenish_events = true;
    let mut events = rig_env(ActionCard::Strategize1, st, e);
    events.run().pick_opt(o(CO::Perform { card: StrategyCard::Trade, holder: Some(Seat(1)) }));
    assert_eq!(events.state.players().by_seat(Seat(0)).unwrap().econ.commodities, 0, "nothing replenished by the core");
}

#[test]
fn overrule_performs_the_primary_ability_and_the_trade_primary_gives_three_trade_goods() {
    covers_card!(ActionCard::Overrule);
    let mut st = four().goods(0, 1).state();
    st.players_mut().by_seat_mut(Seat(1)).unwrap().strat.push(StrategyCard::Trade);
    let mut r = rig(ActionCard::Overrule, st.clone());
    r.run();
    // One card on offer: taken unasked. The Trade primary gives the three trade goods at once and offers the others a replenishment.
    assert!(matches!(r.asked().ask, Ask::Strategy(_)));
    assert_eq!(r.state.players().by_seat(Seat(0)).unwrap().econ.trade_goods, 1 + 3);
    assert_eq!(r.opts().len(), 4, "the three others and done");
    r.assert_options_sound();
    r.pick(3);
    assert!(r.is_done());
    assert_eq!(r.state.players().by_seat(Seat(0)).unwrap().econ.commodities, i16::from(Faction::TheArborec.data().commodities));
}

#[test]
fn reveal_prototype_pays_four_resources_and_researches_a_unit_upgrade_of_the_line_in_the_combat() {
    covers_card!(ActionCard::RevealPrototype);
    let mut e = env();
    // The unit upgrades of the line of a unit in the combat; some the seat cannot research (Cruiser II: no red or yellow prerequisite),
    // one is the Arborec's own (Letani Warrior II stands for Infantry II).
    for t in [Technology::InfantryII, Technology::FighterII, Technology::CruiserII, Technology::LetaniWarriorII] {
        e.facts.reveal.insert(t);
    }
    let st = four()
        .control(Planet::Starpoint, 0)
        .control(Planet::Capha, 0)
        .tech(0, Technology::NeuralMotivator)
        .tech(0, Technology::DacxiveAnimators)
        .tech(0, Technology::GravityDrive)
        .state();
    let mut r = rig_env(ActionCard::RevealPrototype, st.clone(), e);
    r.run();
    let techs: Vec<Technology> = r.opts().iter().filter_map(|x| if let Opt::Cards(CO::Tech(t)) = x { Some(*t) } else { None }).collect();
    assert_eq!(techs, vec![Technology::FighterII, Technology::LetaniWarriorII], "{}", r.transcript());
    r.assert_options_sound();
    r.pick(0);
    // Four resources are paid by a child window: Starpoint 3 and Capha 3 are the planets to choose from.
    assert!(matches!(r.asked().ask, Ask::Production(_)));
    while !r.is_done() {
        r.pick(0);
    }
    assert!(owned(&r.state, 0).contains(&Technology::FighterII));
    assert!(!owned(&r.state, 0).contains(&Technology::LetaniWarriorII));
    let exhausted = [Planet::Starpoint, Planet::Capha].iter().filter(|p| r.state.board().planet(**p).flags.has(PlanetFlags::EXHAUSTED)).count();
    assert_eq!(exhausted, 2, "3 + 3 pays 4 only with both");
    // Only one eligible upgrade is chosen unasked and goes straight to the bill.
    let mut e2 = env();
    e2.facts.reveal.insert(Technology::FighterII);
    let mut one = rig_env(ActionCard::RevealPrototype, st, e2);
    one.run();
    assert!(matches!(one.asked().ask, Ask::Production(_)));
}
