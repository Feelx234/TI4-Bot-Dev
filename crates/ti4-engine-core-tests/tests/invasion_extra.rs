//! D4 lane A2: tests written from the first mutation run over `rules/invasion` (survivors that were real gaps): bombardment over several
//! planets and groups, the feat conditions, coexistence in the choice of the next fight, the flags of a landing and of a Crimson deploy.

use ti4_engine_core::choice::invasion::{Ask as IAsk, Opt as IOpt};
use ti4_engine_core::containers::{Cow, SmallList, SortedMap};
use ti4_engine_core::rules::invasion::advance::{Advance, Facts as AdvFacts, Upcoming};
use ti4_engine_core::rules::invasion::bombard::{Bombardment, Cursor, Entry};
use ti4_engine_core::rules::invasion::commit::{CommitGround, LandOne};
use ti4_engine_core::rules::invasion::{bombard_plan, PlanetBombard, Rd};
use ti4_engine_core::state::{ActiveInvasion, Code, EventKind, Feat, InvasionStep, Key, Piece, PlanetFlags, SysFlags};
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::plain_systems;

fn two_planet_systems() -> (System, System) {
    let mut it = plain_systems().into_iter().filter(|s| s.data().planets.len() >= 2);
    (it.next().expect("a two-planet tile"), it.next().expect("another two-planet tile"))
}

fn sys() -> System {
    two_planet_systems().0
}

fn planets() -> (Planet, Planet) {
    let p = sys().data().planets;
    (p[0], p[1])
}

fn template(planet: Planet) -> InvasionStep {
    InvasionStep { planet, kind: Code(5), round: 0, before: vec![], after: vec![], dice: vec![], hits: SortedMap::default(), harrow_hits: 0 }
}

fn entries(plan: &[PlanetBombard], groups: &[&[usize]]) -> Vec<Entry> {
    plan.iter().zip(groups).map(|(p, g)| Entry { planet: p.planet, template: template(p.planet), groups: g.iter().copied().collect::<SmallList<usize, 8>>(), held: p.held, victims: p.victims }).collect()
}

fn active(mut s: CoreState, planet: Planet) -> CoreState {
    s.transient_mut().invasion.active = Some(Cow::new(ActiveInvasion { system: sys(), invader: Seat(0), seq: 1, phase: Code(1), planet: Some(planet), defender: Some(Seat(1)), ground_round: 0, last_step: None }));
    s
}

fn units_on(rig: &Rig<impl Window>, planet: Planet, seat: u8) -> usize {
    rig.state.board().planet(planet).units.iter().filter(|p| p.owner() == Seat(seat)).count()
}

fn bombarding() -> Scenario {
    let (p1, p2) = planets();
    Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec]).map_from(&[sys()]).unit(sys(), None, Unit::DreadnoughtI, 0, 2).unit(sys(), Some(p1), Unit::InfantryI, 1, 2).unit(sys(), Some(p2), Unit::InfantryI, 1, 3).control(p1, 1).control(p2, 1)
}

// ---- several planets and groups --------------------------------------------------------------------

#[test]
fn bombardment_goes_over_every_planet_of_the_plan_in_order_and_each_planet_gets_its_own_groups() {
    covers!("rules::invasion::bombard::Bombardment");
    let (p1, p2) = planets();
    let (s, k) = bombarding().build();
    let plan = bombard_plan(Rd::new(&s, &k), sys(), Seat(0));
    assert_eq!(plan.len(), 2);
    let ordered: Vec<Planet> = plan.iter().map(|p| p.planet).collect();
    let mut rig = Rig::from_parts(active(s, p1), k, Bombardment::new(Seat(0), sys(), 7, true, Code(2), entries(&plan, &[&[1, 1], &[2]]), 0));
    rig.run();
    assert!(rig.is_done() && !rig.driver.window.paused());
    let (a, b) = (ordered[0], ordered[1]);
    let want = |_: Planet| 2; // two groups of one hit on the first planet, one group of two on the second
    let held = |p: Planet| if p == p1 { 2 } else { 3 };
    for p in [a, b] {
        assert_eq!(units_on(&rig, p, 1), held(p) - want(p), "{p:?}: the groups of its own entry landed on it");
    }
    assert_eq!(rig.driver.window.kills, 4);
    assert!(p2 != p1);
}

#[test]
fn a_planet_with_no_hits_is_completed_and_the_next_planet_still_gets_its_groups() {
    covers!("rules::invasion::bombard::Bombardment");
    let (p1, _) = planets();
    let (s, k) = bombarding().build();
    let plan = bombard_plan(Rd::new(&s, &k), sys(), Seat(0));
    let mut rig = Rig::from_parts(active(s, p1), k, Bombardment::new(Seat(0), sys(), 7, true, Code(2), entries(&plan, &[&[], &[1]]), 0));
    rig.run();
    assert!(rig.is_done());
    assert_eq!(rig.driver.window.kills, 1, "the first planet took nothing, the second one hit");
    let total: usize = plan.iter().map(|p| units_on(&rig, p.planet, 1)).sum();
    assert_eq!(total, 5 - 1);
}

#[test]
fn the_last_ground_forces_feat_needs_every_defender_destroyed_and_nothing_of_another_player_left_on_the_planet() {
    covers!("rules::invasion::bombard::Bombardment");
    let (p1, _) = planets();
    let build = |extra: &dyn Fn(Scenario) -> Scenario, hits: &[usize]| {
        let (s, k) = extra(Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec]).map_from(&[sys()]).unit(sys(), None, Unit::DreadnoughtI, 0, 2).unit(sys(), Some(p1), Unit::InfantryI, 1, 2).control(p1, 1)).build();
        let plan = bombard_plan(Rd::new(&s, &k), sys(), Seat(0));
        let mut rig = Rig::from_parts(active(s, p1), k, Bombardment::new(Seat(0), sys(), 9, true, Code(2), entries(&plan, &[hits]), 0));
        rig.run();
        rig
    };
    let feat = |rig: &Rig<Bombardment>| rig.state.players().by_seat(Seat(0)).unwrap().misc.event_feats.as_slice().contains(&(Feat::BombardedOutTheLastGroundForces, 9));
    assert!(feat(&build(&|sc| sc, &[2])), "both destroyed");
    assert!(!feat(&build(&|sc| sc, &[1])), "one is left: not the last");
    // A structure of the defender that stays (a space dock is no ground force) keeps the feat away.
    assert!(!feat(&build(&|sc| sc.unit(sys(), Some(p1), Unit::SpaceDockI, 1, 1), &[2])), "the planet still holds units of another player");
    // The invader's own forces on the planet do not count against it.
    assert!(feat(&build(&|sc| sc.unit(sys(), Some(p1), Unit::InfantryI, 0, 1), &[2])));
    let rig = build(&|sc| sc, &[2]);
    assert_eq!(rig.driver.window.scoring, vec![(9, false)]);
}

#[test]
fn a_two_victim_planet_with_two_groups_asks_for_each_group_and_completes_after_the_last() {
    covers!("rules::invasion::bombard::Bombardment");
    let (p1, _) = planets();
    let sc = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec, Faction::TheNaaluCollective])
        .map_from(&[sys()])
        .unit(sys(), None, Unit::DreadnoughtI, 0, 2)
        .unit(sys(), Some(p1), Unit::InfantryI, 1, 2)
        .unit(sys(), Some(p1), Unit::InfantryI, 2, 2)
        .control(p1, 1);
    let (s, k) = sc.build();
    let plan = bombard_plan(Rd::new(&s, &k), sys(), Seat(0));
    assert_eq!(plan[0].victims.len(), 2);
    let groups: SmallList<usize, 8> = [1usize, 1].into_iter().collect();
    let cursor = Cursor { planet_index: 0, planet: p1, groups: groups.clone(), next_group: 0, taken: 0, held: plan[0].held, victims: plan[0].victims };
    let at = |c: Cursor| Bombardment::new(Seat(0), sys(), 1, true, Code(2), entries(&plan, &[&[1, 1]]), 0).at_cursor(c, true);
    let mut rig = Rig::from_parts(active(s, p1), k.clone(), at(cursor));
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Invasion(IAsk::BombardTarget { hits: 1, .. })));
    rig.pick_opt(Opt::Invasion(IOpt::BombardTarget { seat: Seat(1) }));
    assert_eq!(units_on(&rig, p1, 1), 1, "the first group landed on the first player");
    assert!(rig.driver.window.paused(), "another group is still to be placed: the window waits at the cursor");
    assert_eq!(rig.driver.window.string_stage(), ti4_engine_core::rules::invasion::bombard::Where::Choosing);
    assert!(rig.state.players().by_seat(Seat(0)).unwrap().misc.event_feats.is_empty());
    // The last group completes the planet and the plan moves on.
    let state = rig.state.clone();
    let last = Cursor { planet_index: 0, planet: p1, groups, next_group: 1, taken: 1, held: plan[0].held, victims: plan[0].victims };
    let mut finish = Rig::from_parts(state, k, at(last));
    finish.run();
    finish.pick_opt(Opt::Invasion(IOpt::BombardTarget { seat: Seat(2) }));
    assert_eq!(units_on(&finish, p1, 2), 1);
    assert_eq!(finish.driver.window.string_stage(), ti4_engine_core::rules::invasion::bombard::Where::Bombarding, "the planet is done: the plan goes on");
    assert_eq!(finish.driver.window.kills, 1);
}

#[test]
fn a_target_that_has_lost_its_last_ground_force_ends_the_choice_without_a_question() {
    covers!("rules::invasion::bombard::Bombardment");
    let (p1, _) = planets();
    let sc = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec, Faction::TheNaaluCollective]).map_from(&[sys()]).unit(sys(), None, Unit::DreadnoughtI, 0, 2).unit(sys(), Some(p1), Unit::InfantryI, 0, 1).control(p1, 1);
    let (s, k) = sc.build();
    // Only the invader's own units are left on the planet: the pause is recognised as over (7.2).
    let plan = vec![PlanetBombard { planet: p1, rolls: vec![], held: 2, victims: { let mut v = ti4_engine_core::containers::SeatSet::EMPTY; v.insert(Seat(1)); v.insert(Seat(2)); v } }];
    let cursor = Cursor { planet_index: 0, planet: p1, groups: [1usize].into_iter().collect(), next_group: 0, taken: 2, held: 2, victims: plan[0].victims };
    let w = Bombardment::new(Seat(0), sys(), 1, true, Code(2), entries(&plan, &[&[1]]), 0).at_cursor(cursor, false);
    let mut rig = Rig::from_parts(active(s, p1), k, w);
    rig.run();
    assert!(rig.is_done() && !rig.driver.window.paused());
    assert_eq!(rig.driver.window.string_stage(), ti4_engine_core::rules::invasion::bombard::Where::Done);
    // Two players held ground forces there: the feat is for a sole victim, so none is scored (as the string).
    assert!(rig.driver.window.scoring.is_empty());
    assert!(rig.state.players().by_seat(Seat(0)).unwrap().misc.event_feats.is_empty());
}

// ---- the next fight ----------------------------------------------------------------------------------

#[test]
fn a_planet_where_the_invader_stands_with_coexisters_but_no_enemy_offers_the_next_combat() {
    covers!("rules::invasion::advance::Advance");
    let (p1, p2) = planets();
    let (mut s, k) = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec]).map_from(&[sys()]).unit(sys(), Some(p1), Unit::InfantryI, 0, 1).unit(sys(), Some(p2), Unit::InfantryI, 0, 1).build();
    let cell = s.board_mut().planet_mut(p1);
    cell.coexist.insert(Seat(1));
    cell.flags.set(PlanetFlags::COEXIST_ENTRY, true);
    let f = AdvFacts { invader: Seat(0), system: sys(), planets: vec![p1, p2], index: 0, coexisted: vec![] };
    let mut rig = Rig::from_parts(s.clone(), k.clone(), Advance::new(f));
    rig.run();
    assert_eq!(rig.driver.window.pick, Some(Upcoming::Coexisters { index: 0, remaining: vec![Seat(1)] }));
    // The invader that coexists itself is not offered a fight; neither is a planet already coexisted by choice.
    let mut mine = s.clone();
    mine.board_mut().planet_mut(p1).coexist.insert(Seat(0));
    let f = AdvFacts { invader: Seat(0), system: sys(), planets: vec![p1, p2], index: 0, coexisted: vec![] };
    let mut rig = Rig::from_parts(mine, k.clone(), Advance::new(f));
    rig.run();
    assert_eq!(rig.driver.window.pick, Some(Upcoming::Exhausted), "p1 is skipped (the invader coexists); p2 has nobody to fight");
    let f = AdvFacts { invader: Seat(0), system: sys(), planets: vec![p1, p2], index: 0, coexisted: vec![p1] };
    let mut rig = Rig::from_parts(s, k, Advance::new(f));
    rig.run();
    assert_eq!(rig.driver.window.pick, Some(Upcoming::Exhausted));
}

#[test]
fn the_search_starts_at_the_given_index_and_reports_the_index_of_the_planet_it_found() {
    covers!("rules::invasion::advance::Advance");
    let (p1, p2) = planets();
    let sc = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec]).map_from(&[sys()]).unit(sys(), Some(p1), Unit::InfantryI, 0, 1).unit(sys(), Some(p1), Unit::InfantryI, 1, 1).unit(sys(), Some(p2), Unit::InfantryI, 0, 1).unit(sys(), Some(p2), Unit::InfantryI, 1, 1);
    let f = |index| AdvFacts { invader: Seat(0), system: sys(), planets: vec![p1, p2], index, coexisted: vec![] };
    let mut first = Rig::new(sc.clone(), Advance::new(f(0)));
    first.run();
    assert_eq!(first.driver.window.pick, Some(Upcoming::Fight { index: 0, defender: Seat(1) }));
    let mut second = Rig::new(sc, Advance::new(f(1)));
    second.run();
    assert_eq!(second.driver.window.pick, Some(Upcoming::Fight { index: 1, defender: Seat(1) }));
}

// ---- landing ----------------------------------------------------------------------------------------

#[test]
fn a_landing_touches_the_tile_flags_the_planet_and_marks_a_ship_form_unit_as_not_followed() {
    covers!("rules::invasion::commit::CommitGround");
    covers!("rules::invasion::commit::LandOne");
    let (p1, _) = planets();
    let sc = || Scenario::new(&[Faction::TheNaazRokhaAlliance, Faction::TheArborec]).map_from(&[sys()]);
    let (mut s, k) = sc().build();
    s.board_mut().sys_mut(sys()).units.push(Piece::new(Unit::InfantryI, Seat(0)));
    let mut rig = Rig::from_parts(s.clone(), k.clone(), CommitGround::new(Seat(0), sys()));
    rig.run();
    rig.pick_opt(Opt::Invasion(IOpt::Commit { index: 0, planet: p1, unit: Unit::InfantryI, damaged: false, from: None }));
    assert!(!rig.driver.window.unsupported, "an infantry is a plain landing");
    assert!(rig.state.board().planet(p1).flags.has(PlanetFlags::UNITS_ENTRY));
    assert!(rig.state.board().sys(sys()).flags.has(SysFlags::TOUCHED));
    // The Eidolon, a mech in its ship form, flips on landing: the core does not follow it.
    let (mut s, k) = sc().build();
    let ship_mech = Unit::ALL.into_iter().find(|u| u.data().ship && u.data().base_type == ti4_engine_core::generated::UnitBaseType::Mech).expect("a mech with a ship form");
    s.board_mut().sys_mut(sys()).units.push(Piece::new(ship_mech, Seat(0)));
    let mut ship = Rig::from_parts(s.clone(), k.clone(), CommitGround::new(Seat(0), sys()));
    ship.run();
    ship.pick(0);
    assert!(ship.driver.window.unsupported);
    let mut one = Rig::from_parts(s, k, LandOne::new(Seat(0), sys(), true));
    one.run();
    one.pick(0);
    assert!(one.driver.window.unsupported, "LandOne says so too");
    assert_eq!(one.state.transient().combat.last_committed_unit.map(|(_, _, p, u)| (p, u.kind())), Some((p1, ship_mech)));
    // The commit question carries the usual fields.
    let ctx = rig.driver.window.question(&rig.state, &rig.kernel).ctx;
    assert!(!ctx.optional && !ctx.space_battle && ctx.invasion_seq.is_none() && ctx.outstanding.is_empty());
}

#[test]
fn a_crimson_deploy_is_announced_flags_the_tile_and_remembers_the_unit() {
    covers!("rules::invasion::commit::LandOne");
    let (p1, _) = planets();
    let sc = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec]).map_from(&[sys()]).control(p1, 1);
    let mut rig = Rig::new(sc.clone(), LandOne::new(Seat(0), sys(), true).with_deploy(vec![p1]));
    rig.run();
    rig.pick_opt(Opt::Invasion(IOpt::Deploy { planet: p1 }));
    assert!(rig.driver.window.deployed);
    assert!(rig.state.board().planet(p1).flags.has(PlanetFlags::UNITS_ENTRY));
    assert!(rig.state.board().sys(sys()).flags.has(SysFlags::TOUCHED));
    let ev = rig.driver.emitted.iter().find(|e| e.kind == EventKind::UnitsCommitted).expect("announced");
    assert_eq!((ev.payload.seat(Key::Player), ev.payload.planet(Key::Planet), ev.payload.seat(Key::Controller)), (Some(Seat(0)), Some(p1), Some(Seat(1))));
    assert_eq!(rig.state.transient().combat.last_committed_unit.map(|(s, y, p, u)| (s, y, p, u.kind())), Some((Seat(0), sys(), p1, Unit::Revenant)));
    // Without the timing machinery the deploy still happens, silently.
    let mut quiet = Rig::new(sc, LandOne::new(Seat(0), sys(), false).with_deploy(vec![p1]));
    quiet.run();
    quiet.pick(0);
    assert!(quiet.driver.window.deployed && quiet.driver.emitted.is_empty());
}

// ---- hits, defense and Harrow details -----------------------------------------------------------------

#[test]
fn a_galvanized_ground_force_destroyed_says_so_in_its_event_and_a_plain_one_does_not() {
    covers!("rules::invasion::hits::GroundHits");
    use ti4_engine_core::rules::invasion::hits::{Batch, GroundHits};
    let (p1, _) = planets();
    let (mut s, k) = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec]).map_from(&[sys()]).unit(sys(), Some(p1), Unit::InfantryI, 1, 2).control(p1, 1).build();
    let first = s.board_mut().planet_mut(p1).units.as_mut_slice().iter_mut().next().unwrap();
    *first = first.with_galvanized(true);
    let batch = Batch { victim: Seat(1), system: sys(), planet: p1, cause: Code(2), unit_ability: false, announce: true };
    let mut w = GroundHits::new(batch, 2, false, true);
    w.flush = true;
    let mut rig = Rig::from_parts(s, k, w);
    rig.run();
    let flags: Vec<Option<bool>> = rig.driver.emitted.iter().filter(|e| e.kind == EventKind::GroundForceDestroyed).map(|e| e.payload.flag(Key::Galvanized)).collect();
    assert_eq!(flags.len(), 2);
    assert_eq!(flags.iter().filter(|f| **f == Some(true)).count(), 1, "exactly the galvanized one carries the flag: {flags:?}");
    assert_eq!(flags.iter().filter(|f| f.is_none()).count(), 1);
}

#[test]
fn non_euclidean_shielding_makes_one_sustain_cancel_the_next_hit_of_a_batch_too() {
    covers!("rules::invasion::hits::GroundHits");
    use ti4_engine_core::rules::invasion::hits::{Batch, GroundHits};
    let (p1, _) = planets();
    let sc = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheBaronyOfLetnev]).map_from(&[sys()]).unit(sys(), Some(p1), Unit::FactionlessMech, 1, 1).unit(sys(), Some(p1), Unit::InfantryI, 1, 2).control(p1, 1).tech(1, Technology::NonEuclideanShielding);
    let batch = Batch { victim: Seat(1), system: sys(), planet: p1, cause: Code(2), unit_ability: false, announce: true };
    let mut w = GroundHits::new(batch, 2, false, true);
    w.flush = true;
    let mut rig = Rig::new(sc, w);
    rig.run();
    assert_eq!(rig.driver.window.destroyed, 0, "the sustain cancels both hits");
    let kinds: Vec<EventKind> = rig.driver.emitted.iter().map(|e| e.kind).collect();
    assert_eq!(kinds, vec![EventKind::GroundForceSustained]);
    assert_eq!(units_on(&rig, p1, 1), 3);
}

#[test]
fn the_defense_dice_carry_the_text_of_their_unit_type_and_the_hit_flag() {
    covers!("rules::invasion::defense::SpaceCannonDefense");
    use ti4_engine_core::rules::invasion::defense::{Facts, SpaceCannonDefense};
    let (p1, _) = planets();
    let f = Facts { invader: Seat(0), system: sys(), planets: vec![p1], announce: false, cause: Code(2), kind: Code(5), planets_key: Code(6), unit_codes: vec![(Unit::PDSI, Code(30)), (Unit::PDSII, Code(31))] };
    let sc = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec]).map_from(&[sys()]).unit(sys(), Some(p1), Unit::PDSI, 1, 1).unit(sys(), Some(p1), Unit::PDSII, 1, 1).unit(sys(), Some(p1), Unit::InfantryI, 0, 6).control(p1, 1);
    let (s, k) = sc.build();
    let s = active(s, p1);
    let mut rig = Rig::from_parts(s, k, SpaceCannonDefense::new(f)).seeded(4);
    rig.run();
    let step = rig.state.transient().invasion.active.as_ref().unwrap().last_step.clone().expect("dice were rolled");
    assert_eq!(step.dice.iter().map(|d| d.group).collect::<Vec<_>>(), vec![Code(30), Code(31)], "each die names the unit type that rolled it");
    assert!(step.dice.iter().all(|d| d.player == Seat(1) && d.hit == (d.face >= d.target)));
    assert_eq!(step.dice.iter().map(|d| d.target).collect::<Vec<_>>(), vec![6, 5]);
}

#[test]
fn harrow_gives_the_first_bombarding_ship_the_plasma_scoring_die() {
    covers!("rules::invasion::harrow::Harrow");
    use ti4_engine_core::rules::invasion::harrow::{Facts, Harrow};
    let (p1, _) = planets();
    let sc = |plasma: bool| {
        let mut sc = Scenario::new(&[Faction::TheL1Z1XMindnet, Faction::TheArborec]).map_from(&[sys()]).unit(sys(), None, Unit::DreadnoughtI, 0, 2).unit(sys(), Some(p1), Unit::InfantryI, 1, 4).control(p1, 1);
        if plasma {
            sc = sc.tech(0, Technology::PlasmaScoring);
        }
        sc
    };
    let f = Facts { invader: Seat(0), defender: Seat(1), system: sys(), planet: p1, announce: false, cause: Code(9) };
    let mut with = Rig::new(sc(true), Harrow::new(f)).seeded(2);
    with.run();
    assert_eq!(with.driver.window.drawn.iter().map(|(_, d)| d.len()).collect::<Vec<_>>(), vec![2, 1], "one die more for the first roller only");
    let mut without = Rig::new(sc(false), Harrow::new(f)).seeded(2);
    without.run();
    assert_eq!(without.driver.window.drawn.iter().map(|(_, d)| d.len()).collect::<Vec<_>>(), vec![1, 1]);
}
