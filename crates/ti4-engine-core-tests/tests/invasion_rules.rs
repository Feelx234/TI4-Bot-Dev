//! D4-2d: the rules of the invasion (LRR 49, 42, 15.1f BOMBARDMENT, 27 custodians) as pure functions of a position: who can bombard what, how
//! ground forces are landed, which ground force takes a hit, how ground combat dice are pooled. Written from the rules text.

use ti4_engine_core::choice::invasion::Opt as IOpt;
use ti4_engine_core::generated::{Leader, PlanetTrait};
use ti4_engine_core::rules::invasion::{
    bombard_plan, bombardable, bombarders, can_sustain_here, commit_opts, commit_opts_from, commit_opts_with, custodians_cost, custodians_gate, ground_combat_value,
    ground_hit, ground_pools, ignores_planetary_shield, is_ground_force, landable, landable_planets, roll_bombard, take_hits, BombardRoll, GroundHit, GroundPool, Rd, Troop,
    CUSTODIANS_COST,
};
use ti4_engine_core::state::{LeaderStatus, MetaFlags, Piece, RoundKind};
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::rig::KitSeeds;
use ti4_engine_core_tests::scenario::plain_systems;

/// A plain tile with two planets.
fn sys() -> System {
    plain_systems().into_iter().find(|s| s.data().planets.len() >= 2 && !s.data().planets.iter().any(|p| p.data().traits.contains(&PlanetTrait::Cultural))).expect("a two-planet tile")
}

fn planets() -> (Planet, Planet) {
    let p = sys().data().planets;
    (p[0], p[1])
}

fn arena() -> Scenario {
    Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec]).map_from(&[sys()])
}

fn rd<'a>(s: &'a CoreState, k: &'a Kernel) -> Rd<'a> {
    Rd::new(s, k)
}

fn inv(o: IOpt) -> Opt {
    Opt::Invasion(o)
}

// ---- unit kinds -------------------------------------------------------------------------------

#[test]
fn infantry_and_mechs_and_the_hel_titan_are_ground_forces_ships_and_structures_are_not() {
    covers!("rules::invasion");
    let (s, k) = arena().build();
    for u in [Unit::InfantryI, Unit::InfantryII, Unit::FactionlessMech, Unit::HelTitanI, Unit::HelTitanII] {
        assert!(is_ground_force(rd(&s, &k), u), "{u:?}");
    }
    for u in [Unit::CruiserI, Unit::FighterI, Unit::PDSI, Unit::SpaceDockI, Unit::WarSun] {
        assert!(!is_ground_force(rd(&s, &k), u), "{u:?}");
    }
}

// ---- bombardment -------------------------------------------------------------------------------

#[test]
fn a_dreadnought_bombards_a_planet_with_ground_forces_and_a_pds_shields_it_unless_a_war_sun_comes() {
    covers!("rules::invasion::bombard_plan");
    let (p1, p2) = planets();
    let base = || arena().unit(sys(), None, Unit::DreadnoughtI, 0, 1).unit(sys(), Some(p1), Unit::InfantryI, 1, 2).control(p1, 1);
    let (s, k) = base().build();
    assert!(bombardable(rd(&s, &k), sys(), p1, Seat(0)));
    let plan = bombard_plan(rd(&s, &k), sys(), Seat(0));
    assert_eq!(plan.len(), 1, "only the planet with defenders");
    assert_eq!((plan[0].planet, plan[0].held, plan[0].victims.len()), (p1, 2, 1));
    assert_eq!(plan[0].rolls, vec![BombardRoll { unit: Unit::DreadnoughtI, hits_on: 5, dice: 1 }], "dreadnought: BOMBARDMENT 5");
    // A planetary shield (PDS) stops it.
    let (s, k) = base().unit(sys(), Some(p1), Unit::PDSI, 1, 1).build();
    assert!(!bombardable(rd(&s, &k), sys(), p1, Seat(0)));
    assert!(bombard_plan(rd(&s, &k), sys(), Seat(0)).is_empty());
    // ... unless a war sun is in the fleet.
    let (s, k) = base().unit(sys(), Some(p1), Unit::PDSI, 1, 1).unit(sys(), None, Unit::WarSun, 0, 1).build();
    assert!(bombardable(rd(&s, &k), sys(), p1, Seat(0)), "war suns ignore planetary shield");
    let plan = bombard_plan(rd(&s, &k), sys(), Seat(0));
    assert_eq!(plan[0].rolls.iter().map(|r| (r.unit, r.hits_on, r.dice)).collect::<Vec<_>>(), vec![(Unit::DreadnoughtI, 5, 1), (Unit::WarSun, 3, 3)]);
    // The other planet of the system has no defenders: it is not in the plan.
    assert!(!plan.iter().any(|p| p.planet == p2));
}

#[test]
fn bombardment_needs_defenders_that_are_not_the_invaders_own_and_nothing_else_stops_the_dice() {
    covers!("rules::invasion::bombard_plan");
    let (p1, _) = planets();
    let (s, k) = arena().unit(sys(), None, Unit::DreadnoughtI, 0, 1).unit(sys(), Some(p1), Unit::InfantryI, 0, 2).control(p1, 0).build();
    assert!(bombard_plan(rd(&s, &k), sys(), Seat(0)).is_empty(), "the invader's own ground forces are no target");
    let (s, k) = arena().unit(sys(), None, Unit::DreadnoughtI, 0, 1).unit(sys(), Some(p1), Unit::SpaceDockI, 1, 1).control(p1, 1).build();
    assert!(bombard_plan(rd(&s, &k), sys(), Seat(0)).is_empty(), "structures are not ground forces");
    assert_eq!(bombarders(&s, sys(), p1, Seat(0)).iter().map(|p| p.kind()).collect::<Vec<_>>(), vec![Unit::DreadnoughtI]);
}

#[test]
fn bombardment_dice_modifiers_plasma_scoring_blitz_galvanize_bunker() {
    covers!("rules::invasion::bombard_plan");
    let (p1, _) = planets();
    let mk = |f: &dyn Fn(Scenario) -> Scenario| f(arena().unit(sys(), None, Unit::DreadnoughtI, 0, 2).unit(sys(), Some(p1), Unit::InfantryI, 1, 2).control(p1, 1)).build();
    // Plasma Scoring: the first roller gets one die more.
    let (s, k) = mk(&|sc| sc.tech(0, Technology::PlasmaScoring));
    let dice: Vec<usize> = bombard_plan(rd(&s, &k), sys(), Seat(0))[0].rolls.iter().map(|r| r.dice).collect();
    assert_eq!(dice, vec![2, 1]);
    // Blitz makes a non-fighter ship without BOMBARDMENT roll one die at 6.
    let (mut s, k) = arena().unit(sys(), None, Unit::CruiserI, 0, 1).unit(sys(), None, Unit::FighterI, 0, 2).unit(sys(), Some(p1), Unit::InfantryI, 1, 1).control(p1, 1).build();
    assert!(bombard_plan(rd(&s, &k), sys(), Seat(0)).iter().all(|p| p.rolls.is_empty()), "cruisers cannot bombard");
    s.meta_mut().activation_seq = 3;
    s.players_mut().by_seat_mut(Seat(0)).unwrap().round_marks.push(RoundKind::BlitzInvasion, 3);
    let plan = bombard_plan(rd(&s, &k), sys(), Seat(0));
    assert_eq!(plan[0].rolls, vec![BombardRoll { unit: Unit::CruiserI, hits_on: 6, dice: 1 }], "the cruiser rolls at 6, the fighters not at all");
    // Bunker: the controller's defense lowers each result by 4 (the number rises).
    let (mut s, k) = mk(&|sc| sc);
    s.meta_mut().activation_seq = 2;
    s.players_mut().by_seat_mut(Seat(1)).unwrap().round_marks.push(RoundKind::BunkerInvasion, 2);
    assert_eq!(bombard_plan(rd(&s, &k), sys(), Seat(0))[0].rolls[0].hits_on, 9, "5 + 4");
    // A galvanized unit rolls a die more.
    let (mut s, k) = mk(&|sc| sc);
    let u = s.board_mut().sys_mut(sys()).units.as_mut_slice().iter_mut().next().unwrap();
    *u = u.with_galvanized(true);
    let dice: Vec<usize> = bombard_plan(rd(&s, &k), sys(), Seat(0))[0].rolls.iter().map(|r| r.dice).collect();
    assert_eq!(dice, vec![2, 1]);
}

#[test]
fn a_disable_strips_the_shield_only_while_it_is_not_the_shield_owners_own_disable() {
    covers!("rules::invasion::bombardable");
    let (p1, _) = planets();
    let (mut s, k) = arena().unit(sys(), None, Unit::DreadnoughtI, 0, 1).unit(sys(), Some(p1), Unit::InfantryI, 1, 1).unit(sys(), Some(p1), Unit::PDSI, 1, 1).control(p1, 1).build();
    assert!(!bombardable(rd(&s, &k), sys(), p1, Seat(0)));
    s.meta_mut().activation_seq = 7;
    s.players_mut().by_seat_mut(Seat(0)).unwrap().round_marks.push(RoundKind::DisableInvasion, 7);
    assert!(bombardable(rd(&s, &k), sys(), p1, Seat(0)), "the invader's Disable removes the shield");
    s.players_mut().by_seat_mut(Seat(0)).unwrap().round_marks.clear(RoundKind::DisableInvasion);
    s.players_mut().by_seat_mut(Seat(1)).unwrap().round_marks.push(RoundKind::DisableInvasion, 7);
    assert!(!bombardable(rd(&s, &k), sys(), p1, Seat(0)), "the shield owner's own Disable does nothing for it");
}

#[test]
fn the_l1z1x_commander_and_arc_secundus_ignore_the_planetary_shield() {
    covers!("rules::invasion::bombardable");
    let (p1, _) = planets();
    let base = |f: Faction| Scenario::new(&[f, Faction::TheArborec]).map_from(&[sys()]).unit(sys(), Some(p1), Unit::InfantryI, 1, 1).unit(sys(), Some(p1), Unit::PDSI, 1, 1).control(p1, 1);
    let (mut s, k) = base(Faction::TheL1Z1XMindnet).unit(sys(), None, Unit::DreadnoughtI, 0, 1).build();
    assert!(!ignores_planetary_shield(&s, Seat(0)));
    s.players_mut().by_seat_mut(Seat(0)).unwrap().leaders.status.insert(Leader::L1z1xcommander, LeaderStatus::Unlocked);
    assert!(ignores_planetary_shield(&s, Seat(0)));
    assert!(bombardable(rd(&s, &k), sys(), p1, Seat(0)));
    let (s, k) = base(Faction::TheBaronyOfLetnev).unit(sys(), None, Unit::ArcSecundus, 0, 1).build();
    assert!(bombardable(rd(&s, &k), sys(), p1, Seat(0)), "Arc Secundus");
}

#[test]
fn conventions_of_war_forbids_bombarding_cultural_planets() {
    covers!("rules::invasion::bombardable");
    let cultural = plain_systems().into_iter().find_map(|s| s.data().planets.iter().copied().find(|p| p.data().traits.contains(&PlanetTrait::Cultural) && p.data().system == Some(s))).expect("a cultural planet");
    let system = cultural.data().system.unwrap();
    let sc = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec]).map_from(&[system]).unit(system, None, Unit::DreadnoughtI, 0, 1).unit(system, Some(cultural), Unit::InfantryI, 1, 1).control(cultural, 1);
    let (s, k) = sc.clone().build();
    assert!(bombardable(rd(&s, &k), system, cultural, Seat(0)), "without the law");
    let (mut s, k) = sc.build();
    s.agenda_mut().laws.insert(ti4_engine_core::generated::Agenda::ConventionsOfWar, ti4_engine_core::keys::Outcome::For);
    assert!(!bombardable(rd(&s, &k), system, cultural, Seat(0)), "Conventions of War outranks every shield breaker");
}

#[test]
fn bombardment_dice_are_drawn_planet_by_planet_unit_by_unit() {
    covers!("rules::invasion::roll_bombard");
    let (p1, _) = planets();
    let (s, k) = arena().unit(sys(), None, Unit::DreadnoughtI, 0, 1).unit(sys(), None, Unit::WarSun, 0, 1).unit(sys(), Some(p1), Unit::InfantryI, 1, 1).control(p1, 1).build();
    let plan = bombard_plan(rd(&s, &k), sys(), Seat(0));
    let mut rng = s.rng.clone();
    let drawn = roll_bombard(&plan, &mut rng, &KitSeeds(5));
    assert_eq!(drawn.iter().map(|d| (d.unit, d.hits_on, d.faces.len())).collect::<Vec<_>>(), vec![(Unit::DreadnoughtI, 5, 1), (Unit::WarSun, 3, 3)]);
    assert!(drawn.iter().flat_map(|d| d.faces.iter()).all(|f| (1..=10).contains(f)));
    let mut again = s.rng.clone();
    assert_eq!(roll_bombard(&plan, &mut again, &KitSeeds(5)), drawn, "the same stream gives the same dice");
}

// ---- landing -------------------------------------------------------------------------------

#[test]
fn only_ground_forces_in_the_space_area_can_land_in_board_order() {
    covers!("rules::invasion::landable");
    let (p1, _) = planets();
    let (mut s, k) = arena().unit(sys(), None, Unit::CruiserI, 0, 1).unit(sys(), Some(p1), Unit::InfantryI, 0, 1).build();
    for u in [Unit::InfantryI, Unit::FactionlessMech] {
        s.board_mut().sys_mut(sys()).units.push(Piece::new(u, Seat(0)));
    }
    s.board_mut().sys_mut(sys()).units.push(Piece::new(Unit::InfantryI, Seat(1)));
    let got: Vec<Unit> = landable(rd(&s, &k), Seat(0), sys()).iter().map(|p| p.kind()).collect();
    assert_eq!(got, vec![Unit::InfantryI, Unit::FactionlessMech], "ships stay, the opponent's infantry is not ours, the one on the planet is already landed");
}

#[test]
fn mecatol_rex_is_off_the_table_until_the_custodians_token_is_removed() {
    covers!("rules::invasion::landable_planets");
    let rex = System::MecatolRex18;
    let sc = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec]).map_from(&[rex]);
    let (mut s, k) = sc.build();
    assert!(!landable_planets(rd(&s, &k), rex).contains(&Planet::MecatolRex));
    s.meta_mut().flags.set(MetaFlags::CUSTODIANS_REMOVED, true);
    assert!(landable_planets(rd(&s, &k), rex).contains(&Planet::MecatolRex));
    assert!(!landable_planets(rd(&s, &k), rex).contains(&Planet::CustodiaVigilia));
    let (s2, k2) = arena().build();
    assert_eq!(landable_planets(rd(&s2, &k2), sys()), sys().data().planets.to_vec());
}

#[test]
fn commit_options_list_each_distinguishable_landing_once_then_the_end() {
    covers!("rules::invasion::commit_opts");
    let (p1, p2) = planets();
    let troops = [Piece::new(Unit::InfantryI, Seat(0)), Piece::new(Unit::InfantryI, Seat(0)), Piece::new(Unit::FactionlessMech, Seat(0)), Piece::new(Unit::InfantryI, Seat(0)).with_sustained(true)];
    let opts = commit_opts(&troops, &[p1, p2]);
    let o = opts.as_slice();
    assert_eq!(o.len(), 6 + 1, "three distinguishable troops x two planets, plus the end");
    assert_eq!(o[0], inv(IOpt::Commit { index: 0, planet: p1, unit: Unit::InfantryI, damaged: false, from: None }));
    assert_eq!(o[1], inv(IOpt::Commit { index: 0, planet: p2, unit: Unit::InfantryI, damaged: false, from: None }));
    assert_eq!(o[2], inv(IOpt::Commit { index: 2, planet: p1, unit: Unit::FactionlessMech, damaged: false, from: None }), "the second infantry is a repeat");
    assert_eq!(*o.last().unwrap(), inv(IOpt::DoneCommitting));
    // A troop already standing on the planet it would land on has no landing there.
    let with_origin = [Troop { piece: Piece::new(Unit::InfantryI, Seat(0)), origin: Some((sys(), p1)), temporary: false }];
    let o = commit_opts_from(sys(), &with_origin, &[p1, p2]);
    assert_eq!(o.as_slice().len(), 2);
    assert!(matches!(o.as_slice()[0], Opt::Invasion(IOpt::Commit { planet, from: Some(_), .. }) if planet == p2));
    // The Crimson deploy lands between the troops and the end.
    let o = commit_opts_with(sys(), &[], &[p1], &[p2]);
    assert_eq!(o.as_slice(), &[inv(IOpt::Deploy { planet: p2 }), inv(IOpt::DoneCommitting)]);
}

// ---- hits on ground forces ----------------------------------------------------------------------

#[test]
fn a_hit_is_absorbed_by_sustain_damage_first_then_the_cheapest_ground_force_dies() {
    covers!("rules::invasion::ground_hit");
    let (p1, _) = planets();
    let sc = arena().unit(sys(), Some(p1), Unit::InfantryI, 1, 2).unit(sys(), Some(p1), Unit::FactionlessMech, 1, 1).control(p1, 1);
    let (s, k) = sc.build();
    let hit = ground_hit(rd(&s, &k), sys(), p1, Seat(1), false);
    assert!(matches!(hit, GroundHit::Sustained(p) if p.kind() == Unit::FactionlessMech), "the mech's SUSTAIN DAMAGE absorbs the first hit: {hit:?}");
    let (after, destroyed) = take_hits(rd(&s, &k), sys(), p1, Seat(1), 3, false);
    assert_eq!(destroyed, 2, "mech sustains, then two infantry die");
    let left: Vec<(Unit, bool)> = after.board().planet(p1).units.iter().map(|p| (p.kind(), p.sustained())).collect();
    assert_eq!(left, vec![(Unit::FactionlessMech, true)], "the damaged mech remains");
    let (after, destroyed) = take_hits(rd(&s, &k), sys(), p1, Seat(1), 9, false);
    assert_eq!((destroyed, after.board().planet(p1).units.len()), (3, 0), "hits beyond the forces are wasted (15.2a)");
    assert_eq!(ground_hit(rd(&after, &k), sys(), p1, Seat(1), false), GroundHit::Wasted);
}

#[test]
fn a_damaged_unit_is_lost_before_a_healthy_one_of_the_same_cost_and_structures_never_take_hits() {
    covers!("rules::invasion::ground_hit");
    let (p1, _) = planets();
    let (mut s, k) = arena().unit(sys(), Some(p1), Unit::InfantryI, 1, 2).unit(sys(), Some(p1), Unit::PDSI, 1, 1).unit(sys(), Some(p1), Unit::SpaceDockI, 1, 1).control(p1, 1).build();
    let first = s.board_mut().planet_mut(p1).units.as_mut_slice().iter_mut().find(|p| p.kind() == Unit::InfantryI).unwrap();
    *first = first.with_sustained(true);
    match ground_hit(rd(&s, &k), sys(), p1, Seat(1), false) {
        GroundHit::Destroyed(p) => assert!(p.kind() == Unit::InfantryI && p.sustained(), "the damaged infantry goes first"),
        other => panic!("{other:?}"),
    }
    let (after, destroyed) = take_hits(rd(&s, &k), sys(), p1, Seat(1), 5, false);
    assert_eq!(destroyed, 2);
    let left: Vec<Unit> = after.board().planet(p1).units.iter().map(|p| p.kind()).collect();
    assert_eq!(left, vec![Unit::PDSI, Unit::SpaceDockI], "structures stay (KD-2)");
}

#[test]
fn non_euclidean_shielding_lets_a_sustain_cancel_a_second_hit_in_a_batch() {
    covers!("rules::invasion::take_hits");
    let (p1, _) = planets();
    let sc = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheBaronyOfLetnev]).map_from(&[sys()]).unit(sys(), Some(p1), Unit::FactionlessMech, 1, 1).unit(sys(), Some(p1), Unit::InfantryI, 1, 2).control(p1, 1).tech(1, Technology::NonEuclideanShielding);
    let (s, k) = sc.build();
    let (after, destroyed) = take_hits(rd(&s, &k), sys(), p1, Seat(1), 2, false);
    assert_eq!(destroyed, 0, "one sustain cancels both hits");
    assert_eq!(after.board().planet(p1).units.len(), 3);
    assert!(can_sustain_here(rd(&s, &k), Piece::new(Unit::FactionlessMech, Seat(1)), sys(), p1));
    assert!(!can_sustain_here(rd(&s, &k), Piece::new(Unit::FactionlessMech, Seat(1)).with_sustained(true), sys(), p1));
    assert!(!can_sustain_here(rd(&s, &k), Piece::new(Unit::InfantryI, Seat(1)), sys(), p1));
}

// ---- ground combat dice ----------------------------------------------------------------------

#[test]
fn ground_combat_values_and_pools_group_the_forces_by_number_ascending() {
    covers!("rules::invasion::ground_pools");
    let (p1, _) = planets();
    let sc = arena().unit(sys(), Some(p1), Unit::InfantryI, 0, 3).unit(sys(), Some(p1), Unit::FactionlessMech, 0, 1).unit(sys(), Some(p1), Unit::InfantryII, 0, 1).unit(sys(), Some(p1), Unit::PDSI, 0, 1).control(p1, 0);
    let (s, k) = sc.build();
    assert_eq!(ground_combat_value(rd(&s, &k), Seat(0), sys(), p1, Unit::InfantryI), Some(8));
    assert_eq!(ground_combat_value(rd(&s, &k), Seat(0), sys(), p1, Unit::InfantryII), Some(7));
    assert_eq!(ground_combat_value(rd(&s, &k), Seat(0), sys(), p1, Unit::PDSI), None, "a PDS does not roll in ground combat");
    let pools = ground_pools(rd(&s, &k), Seat(0), sys(), p1);
    let shape: Vec<(i64, i64, Vec<(Unit, u32)>)> = pools.into_iter().map(|p: GroundPool| (p.value, p.dice, p.types)).collect();
    let mech_value = ground_combat_value(rd(&s, &k), Seat(0), sys(), p1, Unit::FactionlessMech).expect("a mech rolls");
    assert!(mech_value <= 6, "mechs hit on 6 or better");
    assert!(shape.windows(2).all(|w| w[0].0 < w[1].0), "ascending, one pool per value: {shape:?}");
    let total: i64 = shape.iter().map(|p| p.1).sum();
    assert_eq!(total, 5, "one die per ground force: 3 + 1 + 1");
    assert!(shape.iter().any(|(v, d, t)| *v == 8 && *d == 3 && t == &vec![(Unit::InfantryI, 3)]));
    assert!(shape.iter().any(|(v, d, _)| *v == 7 && *d == 1));
    // The opponent has no ground forces here.
    assert!(ground_pools(rd(&s, &k), Seat(1), sys(), p1).is_empty());
}

#[test]
fn a_galvanized_ground_force_rolls_an_extra_die() {
    covers!("rules::invasion::ground_pools");
    let (p1, _) = planets();
    let (mut s, k) = arena().unit(sys(), Some(p1), Unit::InfantryI, 0, 2).control(p1, 0).build();
    let u = s.board_mut().planet_mut(p1).units.as_mut_slice().iter_mut().next().unwrap();
    *u = u.with_galvanized(true);
    let pools = ground_pools(rd(&s, &k), Seat(0), sys(), p1);
    assert_eq!(pools.iter().map(|p| p.dice).sum::<i64>(), 3);
}

// ---- the custodians token -------------------------------------------------------------------

#[test]
fn the_custodians_cost_six_influence_and_need_a_ground_force_in_space_on_mecatol_rex() {
    covers!("rules::invasion::custodians_gate");
    let rex = System::MecatolRex18;
    let sc = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec]).map_from(&[rex]);
    let (mut s, k) = sc.clone().build();
    assert_eq!(custodians_cost(rd(&s, &k), Seat(0)), CUSTODIANS_COST);
    assert_eq!(CUSTODIANS_COST, 6);
    assert!(!custodians_gate(rd(&s, &k), Seat(0), rex), "no ground force to land");
    s.board_mut().sys_mut(rex).units.push(Piece::new(Unit::InfantryI, Seat(0)));
    assert!(custodians_gate(rd(&s, &k), Seat(0), rex));
    assert!(!custodians_gate(rd(&s, &k), Seat(1), rex), "the opponent has nothing there");
    assert!(!custodians_gate(rd(&s, &k), Seat(0), sys()), "only Mecatol Rex");
    s.meta_mut().flags.set(MetaFlags::CUSTODIANS_REMOVED, true);
    assert!(!custodians_gate(rd(&s, &k), Seat(0), rex), "the token is gone");
    // Winnu pay nothing.
    let winnu = Scenario::new(&[Faction::TheWinnu, Faction::TheArborec]).map_from(&[rex]).build();
    assert_eq!(custodians_cost(rd(&winnu.0, &winnu.1), Seat(0)), 0, "Blood Ties");
}
