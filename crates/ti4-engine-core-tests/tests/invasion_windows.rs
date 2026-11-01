//! D4-2d: the windows of the invasion: commit and land (LRR 49.2), the custodians offer (27), space cannon defense (49.2/77.2), the rounds of a ground
//! combat (42: dice, hits, Harrow), assigning ground hits, the choice of the next planet. Dice are drawn from the seeded stream and derived from `drawn`.

use ti4_engine_core::choice::invasion::{Ask as IAsk, Opt as IOpt};
use ti4_engine_core::containers::{Cow, SmallList, SortedMap};
use ti4_engine_core::rules::invasion::absorb::AbsorbGround;
use ti4_engine_core::rules::invasion::advance::{Advance, Facts as AdvFacts, Upcoming};
use ti4_engine_core::rules::invasion::begin::{BeginGroundRound, Facts as BeginFacts};
use ti4_engine_core::rules::invasion::bombard::{Bombardment, Cursor, Entry};
use ti4_engine_core::rules::invasion::commit::{CommitGround, LandOne};
use ti4_engine_core::rules::invasion::defense::{defense_plan, Facts as DefFacts, GunRoll, SpaceCannonDefense};
use ti4_engine_core::rules::invasion::harrow::{Facts as HarrowFacts, Harrow};
use ti4_engine_core::rules::invasion::hits::{Batch, GroundHits};
use ti4_engine_core::rules::invasion::roll::{Facts as RollFacts, GroundRoll};
use ti4_engine_core::rules::invasion::round::{Facts as RoundFacts, GroundRoundHits};
use ti4_engine_core::rules::invasion::stages::{BombardTarget, Custodians, FightRound, NextCombat};
use ti4_engine_core::rules::invasion::tail::{Facts as TailFacts, GroundRollTail};
use ti4_engine_core::rules::invasion::{bombard_plan, Rd, Troop};
use ti4_engine_core::state::{ActiveInvasion, Code, EventKind, Feat, InvasionStep, Key, List, MetaFlags, Piece, RoundKind};
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::plain_systems;

fn sys() -> System {
    plain_systems().into_iter().find(|s| s.data().planets.len() >= 2).expect("a two-planet tile")
}

fn planets() -> (Planet, Planet) {
    let p = sys().data().planets;
    (p[0], p[1])
}

fn arena() -> Scenario {
    Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec]).map_from(&[sys()])
}

fn inv(o: IOpt) -> Opt {
    Opt::Invasion(o)
}

fn units_on(rig: &Rig<impl Window>, planet: Planet, seat: u8) -> Vec<(Unit, bool)> {
    rig.state.board().planet(planet).units.iter().filter(|p| p.owner() == Seat(seat)).map(|p| (p.kind(), p.sustained())).collect()
}

fn with_active(mut s: CoreState, planet: Planet, defender: u8) -> CoreState {
    s.transient_mut().invasion.active = Some(Cow::new(ActiveInvasion { system: sys(), invader: Seat(0), seq: 1, phase: Code(1), planet: Some(planet), defender: Some(Seat(defender)), ground_round: 0, last_step: None }));
    s
}

fn step_template(planet: Planet) -> InvasionStep {
    InvasionStep { planet, kind: Code(5), round: 0, before: vec![], after: vec![], dice: vec![], hits: SortedMap::default(), harrow_hits: 0 }
}

// ---- hits ----------------------------------------------------------------------------------------

fn batch(victim: u8, planet: Planet, ability: bool, announce: bool) -> Batch {
    Batch { victim: Seat(victim), system: sys(), planet, cause: Code(2), unit_ability: ability, announce }
}

#[test]
fn a_batch_of_ground_hits_opens_its_window_then_sustains_then_destroys_and_stages_the_events() {
    covers!("rules::invasion::hits::GroundHits");
    let (p1, _) = planets();
    let sc = arena().unit(sys(), Some(p1), Unit::FactionlessMech, 1, 1).unit(sys(), Some(p1), Unit::InfantryI, 1, 2).control(p1, 1);
    let mut w = GroundHits::new(batch(1, p1, false, true), 3, true, true);
    w.flush = true;
    let mut rig = Rig::new(sc, w);
    rig.run();
    assert!(rig.is_done());
    assert_eq!(rig.driver.window.destroyed, 2, "the mech sustains, two infantry are destroyed");
    assert_eq!(rig.driver.window.remaining, 3);
    assert_eq!(units_on(&rig, p1, 1), vec![(Unit::FactionlessMech, true)]);
    let kinds: Vec<EventKind> = rig.driver.emitted.iter().map(|e| e.kind).collect();
    assert_eq!(kinds, vec![EventKind::GroundHitsToAssign, EventKind::GroundForceSustained, EventKind::GroundForceDestroyed, EventKind::GroundForceDestroyed], "the window, then the hits in order");
    let first = &rig.driver.emitted[0].payload;
    assert_eq!((first.int(Key::Hits), first.seat(Key::Player), first.planet(Key::Planet), first.code(Key::Cause)), (Some(3), Some(Seat(1)), Some(p1), Some(Code(2))));
    let sustained = &rig.driver.emitted[1].payload;
    assert_eq!((sustained.unit(Key::Unit), sustained.flag(Key::Damaged)), (Some(Unit::FactionlessMech), None));
    let destroyed = &rig.driver.emitted[2].payload;
    assert_eq!((destroyed.unit(Key::Unit), destroyed.flag(Key::Damaged)), (Some(Unit::InfantryI), Some(false)));
}

/// Found by the whole-game shadow (jolnar 5p, saar 4p): a retreat stages its `COMMAND_TOKEN_PLACED`; the string announces it at the start of its next step. The
/// string's hit batch flushes a log of its own (the sustained and destroyed ground forces), the core's flush took the state's whole staged list and announced
/// the retreat's token right after `INVASION_BEGAN`.
#[test]
fn the_flush_of_a_hit_batch_announces_the_hit_events_only_and_leaves_the_other_staged_events_for_the_next_step() {
    covers!("rules::invasion::hits::GroundHits");
    let (p1, _) = planets();
    let sc = arena().unit(sys(), Some(p1), Unit::InfantryI, 1, 2).control(p1, 1);
    let mut w = GroundHits::new(batch(1, p1, false, true), 2, false, true);
    w.flush = true;
    let (mut state, kernel) = sc.build();
    let token = ti4_engine_core::kernel::fx::event(EventKind::CommandTokenPlaced, ti4_engine_core::state::Payload::new().with(Key::Player, ti4_engine_core::state::Val::Seat(Seat(0))).with(Key::System, ti4_engine_core::state::Val::System(sys())));
    ti4_engine_core::kernel::fx::stage(&mut state, token);
    let mut rig = Rig::from_parts(state, kernel, w);
    rig.run();
    let kinds: Vec<EventKind> = rig.driver.emitted.iter().map(|e| e.kind).collect();
    assert_eq!(kinds, vec![EventKind::GroundForceDestroyed, EventKind::GroundForceDestroyed], "only the hits of the batch are announced");
    let waiting: Vec<EventKind> = rig.state.pending().staged.iter().map(|e| e.kind).collect();
    assert_eq!(waiting, vec![EventKind::CommandTokenPlaced], "the retreat's token waits for the next step");
}

#[test]
fn cancellations_granted_in_the_hits_window_are_spent_first_and_the_open_and_apply_parts_are_independent() {
    covers!("rules::invasion::hits::GroundHits");
    let (p1, _) = planets();
    let sc = arena().unit(sys(), Some(p1), Unit::InfantryI, 1, 3).control(p1, 1);
    // `open` alone returns the hits that remain after the window; `apply` alone does not open.
    let mut open_only = Rig::new(sc.clone(), GroundHits::new(batch(1, p1, false, true), 3, true, false));
    open_only.run();
    assert_eq!(open_only.driver.window.remaining, 3);
    assert_eq!(open_only.driver.window.destroyed, 0);
    assert_eq!(units_on(&open_only, p1, 1).len(), 3, "nothing applied");
    let mut apply_only = Rig::new(sc.clone(), GroundHits::new(batch(1, p1, false, true), 2, false, true));
    apply_only.run();
    assert_eq!(apply_only.driver.window.destroyed, 2);
    assert!(apply_only.driver.emitted.is_empty(), "no window, and the staged events are not flushed without `flush`");
    // Without the timing machinery the window is not announced.
    let mut quiet = Rig::new(sc, GroundHits::new(batch(1, p1, false, false), 1, true, true));
    quiet.run();
    assert!(quiet.driver.emitted.is_empty());
    assert_eq!(quiet.driver.window.destroyed, 1);
}

#[test]
fn a_destroyed_spec_ops_ii_is_counted_for_its_survival_roll_and_hits_on_nothing_are_wasted() {
    covers!("rules::invasion::hits::GroundHits");
    let (p1, _) = planets();
    let sc = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheNaaluCollective]).map_from(&[sys()]).unit(sys(), Some(p1), Unit::SpecOpsII, 1, 1).control(p1, 1);
    let mut rig = Rig::new(sc, GroundHits::new(batch(1, p1, false, false), 4, false, true));
    rig.run();
    assert_eq!(rig.driver.window.destroyed, 1, "one unit, four hits: three are wasted (15.2a)");
    assert_eq!(rig.state.players().by_seat(Seat(1)).unwrap().misc.spec_ops_destroyed, 1);
}

// ---- assigning ground hits ---------------------------------------------------------------------

#[test]
fn the_owner_chooses_which_ground_force_dies_after_the_sustains_and_a_single_candidate_goes_unasked() {
    covers!("rules::invasion::absorb::AbsorbGround");
    let (p1, _) = planets();
    let sc = arena().unit(sys(), Some(p1), Unit::FactionlessMech, 1, 1).unit(sys(), Some(p1), Unit::InfantryI, 1, 2).unit(sys(), Some(p1), Unit::InfantryII, 1, 1).control(p1, 1);
    let mut rig = Rig::new(sc, AbsorbGround::new(Seat(1), sys(), p1, 3, false, false));
    rig.run();
    // The mech took the first hit without a question; two hits remain for a choice among infantry I and II.
    assert_eq!(units_on(&rig, p1, 1)[0], (Unit::FactionlessMech, true));
    assert_eq!(rig.asked().actor, Seat(1));
    assert_eq!(rig.asked().ask, Ask::Invasion(IAsk::AssignGroundHit { system: sys(), planet: p1, staging: false }));
    let kinds: Vec<(Unit, bool)> = rig.opts().iter().map(|o| if let Opt::Invasion(IOpt::DestroyGround { unit, damaged, .. }) = o { (*unit, *damaged) } else { panic!("{o:?}") }).collect();
    assert_eq!(kinds, vec![(Unit::FactionlessMech, true), (Unit::InfantryI, false), (Unit::InfantryII, false)], "damaged mech, infantry I and II (the two infantry I are one option)");
    rig.assert_options_sound();
    rig.expect_illegal(Pick::Opt(Opt::Decline));
    rig.pick(2);
    assert_eq!(rig.asked().ask, Ask::Invasion(IAsk::AssignGroundHit { system: sys(), planet: p1, staging: false }), "one hit is still owed");
    rig.pick(0);
    assert!(rig.is_done());
    let left = units_on(&rig, p1, 1);
    assert_eq!(left, vec![(Unit::InfantryI, false), (Unit::InfantryI, false)], "the mech and infantry II were chosen");
}

#[test]
fn no_candidate_wastes_the_hits_and_a_lone_force_is_taken_without_a_question() {
    covers!("rules::invasion::absorb::AbsorbGround");
    let (p1, _) = planets();
    let mut one = Rig::new(arena().unit(sys(), Some(p1), Unit::InfantryI, 1, 1).control(p1, 1), AbsorbGround::new(Seat(1), sys(), p1, 5, false, false));
    one.run();
    assert!(one.is_done() && units_on(&one, p1, 1).is_empty());
    let mut none = Rig::new(arena().unit(sys(), Some(p1), Unit::PDSI, 1, 1).control(p1, 1), AbsorbGround::new(Seat(1), sys(), p1, 2, false, false));
    none.run();
    assert!(none.is_done(), "a PDS is not a ground force");
    assert_eq!(delta(none.start_state(), &none.state), "");
}

// ---- commit and land -------------------------------------------------------------------------

fn cargo(sc: Scenario, units: &[Unit], seat: u8) -> (CoreState, Kernel) {
    let (mut s, k) = sc.build();
    for u in units {
        s.board_mut().sys_mut(sys()).units.push(Piece::new(*u, Seat(seat)));
    }
    (s, k)
}

#[test]
fn committing_moves_ground_forces_from_space_to_a_planet_until_the_invader_is_done() {
    covers!("rules::invasion::commit::CommitGround");
    let (p1, p2) = planets();
    let (s, k) = cargo(arena().unit(sys(), None, Unit::CarrierI, 0, 1), &[Unit::InfantryI, Unit::InfantryI, Unit::FactionlessMech], 0);
    let mut rig = Rig::from_parts(s, k, CommitGround::new(Seat(0), sys()));
    rig.run();
    assert_eq!(rig.asked().ask, Ask::Invasion(IAsk::CommitGround { system: sys() }));
    assert_eq!(rig.asked().actor, Seat(0));
    let opts = rig.opts();
    assert_eq!(opts.len(), 2 * 2 + 1, "infantry and mech, two planets each, and the end");
    assert_eq!(*opts.last().unwrap(), inv(IOpt::DoneCommitting));
    rig.assert_options_sound();
    rig.pick_opt(inv(IOpt::Commit { index: 0, planet: p2, unit: Unit::InfantryI, damaged: false, from: None }));
    assert_eq!(rig.driver.window.committed.as_slice(), &[p2]);
    rig.pick_opt(inv(IOpt::Commit { index: 0, planet: p1, unit: Unit::InfantryI, damaged: false, from: None }));
    let mut want = vec![p1, p2];
    want.sort_unstable();
    assert_eq!(rig.driver.window.committed.as_slice(), want.as_slice(), "planet order");
    assert_eq!(rig.state.board().sys(sys()).units.iter().filter(|p| p.kind() == Unit::FactionlessMech).count(), 1, "the mech is still aboard");
    rig.pick_opt(inv(IOpt::DoneCommitting));
    assert!(rig.is_done());
    assert_eq!(units_on(&rig, p1, 0), vec![(Unit::InfantryI, false)]);
    assert_eq!(units_on(&rig, p2, 0), vec![(Unit::InfantryI, false)]);
    assert!(rig.state.board().sys(sys()).flags.has(ti4_engine_core::state::SysFlags::TOUCHED));
}

#[test]
fn nothing_to_land_or_no_planet_ends_the_commitment_and_a_bad_pick_is_illegal() {
    covers!("rules::invasion::commit::CommitGround");
    let mut nobody = Rig::new(arena().unit(sys(), None, Unit::CruiserI, 0, 1), CommitGround::new(Seat(0), sys()));
    nobody.run();
    assert!(nobody.is_done(), "no ground force in space");
    let (p1, _) = planets();
    let (s, k) = cargo(arena().unit(sys(), None, Unit::CarrierI, 0, 1), &[Unit::InfantryI], 0);
    let mut rig = Rig::from_parts(s, k, CommitGround::new(Seat(0), sys()));
    rig.run();
    rig.expect_illegal(Pick::Opt(inv(IOpt::Commit { index: 9, planet: p1, unit: Unit::InfantryI, damaged: false, from: None })));
    rig.expect_illegal(Pick::Opt(Opt::Decline));
    // Planetless systems end the commitment at once.
    let empty = plain_systems().into_iter().find(|s| s.data().planets.is_empty());
    if let Some(empty) = empty {
        let sc = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec]).map_from(&[empty]);
        let (mut s, k) = sc.build();
        s.board_mut().sys_mut(empty).units.push(Piece::new(Unit::InfantryI, Seat(0)));
        let mut rig = Rig::from_parts(s, k, CommitGround::new(Seat(0), empty));
        rig.run();
        assert!(rig.is_done());
    }
}

#[test]
fn land_one_lands_a_single_troop_remembers_it_and_announces_it_with_the_planets_controller() {
    covers!("rules::invasion::commit::LandOne");
    let (p1, _) = planets();
    let (s, k) = cargo(arena().unit(sys(), None, Unit::CarrierI, 0, 1).control(p1, 1), &[Unit::InfantryI, Unit::FactionlessMech], 0);
    let mut rig = Rig::from_parts(s, k, LandOne::new(Seat(0), sys(), true));
    rig.run();
    rig.pick_opt(inv(IOpt::Commit { index: 1, planet: p1, unit: Unit::FactionlessMech, damaged: false, from: None }));
    assert!(rig.is_done(), "one landing per window");
    assert_eq!(rig.driver.window.landed.map(|t| t.piece.kind()), Some(Unit::FactionlessMech));
    assert_eq!(units_on(&rig, p1, 0), vec![(Unit::FactionlessMech, false)]);
    assert_eq!(rig.state.transient().combat.last_committed_unit.map(|(s, sy, p, u)| (s, sy, p, u.kind())), Some((Seat(0), sys(), p1, Unit::FactionlessMech)));
    let ev = rig.driver.emitted.iter().find(|e| e.kind == EventKind::UnitsCommitted).expect("announced");
    assert_eq!((ev.payload.seat(Key::Player), ev.payload.planet(Key::Planet), ev.payload.seat(Key::Controller)), (Some(Seat(0)), Some(p1), Some(Seat(1))));
    // Quiet resolver: same landing, nothing announced.
    let (s, k) = cargo(arena().unit(sys(), None, Unit::CarrierI, 0, 1), &[Unit::InfantryI], 0);
    let mut quiet = Rig::from_parts(s, k, LandOne::new(Seat(0), sys(), false));
    quiet.run();
    quiet.pick(0);
    assert!(quiet.driver.emitted.is_empty());
}

#[test]
fn land_one_over_a_pool_takes_troops_from_their_origin_and_a_crimson_deploy_needs_no_ground_force() {
    covers!("rules::invasion::commit::LandOne");
    let (p1, p2) = planets();
    let piece = Piece::new(Unit::InfantryI, Seat(0));
    let (s, k) = arena().unit(sys(), Some(p2), Unit::InfantryI, 0, 1).control(p2, 0).build();
    let pool = vec![Troop { piece, origin: Some((sys(), p2)), temporary: false }];
    let mut rig = Rig::from_parts(s, k, LandOne::new(Seat(0), sys(), true).with_pool(pool));
    rig.run();
    assert_eq!(rig.opts().len(), 2, "p1 only (it stands on p2 already) and the end: {:?}", rig.opts());
    rig.pick(0);
    assert_eq!(units_on(&rig, p2, 0), vec![]);
    assert_eq!(units_on(&rig, p1, 0), vec![(Unit::InfantryI, false)]);
    let ev = rig.driver.emitted.iter().find(|e| e.kind == EventKind::UnitsCommitted).unwrap();
    assert_eq!((ev.payload.system(Key::FromSystem), ev.payload.planet(Key::FromPlanet)), (Some(sys()), Some(p2)));
    // The deploy: a Revenant from reinforcements, only to an offered planet.
    let mut deploy = Rig::new(arena().control(p1, 0), LandOne::new(Seat(0), sys(), true).with_deploy(vec![p1]));
    deploy.run();
    assert_eq!(deploy.opts(), vec![inv(IOpt::Deploy { planet: p1 }), inv(IOpt::DoneCommitting)]);
    deploy.expect_illegal(Pick::Opt(inv(IOpt::Deploy { planet: p2 })));
    deploy.pick(0);
    assert!(deploy.is_done() && deploy.driver.window.deployed);
    assert_eq!(units_on(&deploy, p1, 0), vec![(Unit::Revenant, false)]);
}

// ---- the custodians ------------------------------------------------------------------------------

fn rex(extra: Faction) -> Scenario {
    Scenario::new(&[Faction::TheEmbersOfMuaat, extra]).map_from(&[System::MecatolRex18])
}

#[test]
fn the_custodians_offer_is_asked_with_the_cost_and_keeping_the_token_costs_nothing() {
    covers!("rules::invasion::stages::Custodians");
    let (s, k) = rex(Faction::TheArborec).build();
    let mut s = s;
    s.board_mut().sys_mut(System::MecatolRex18).units.push(Piece::new(Unit::InfantryI, Seat(0)));
    let mut rig = Rig::from_parts(s.clone(), k.clone(), Custodians::new(Seat(0), System::MecatolRex18, true));
    rig.run();
    assert_eq!(rig.asked().ask, Ask::Invasion(IAsk::RemoveCustodians { system: System::MecatolRex18, cost: 6 }));
    assert_eq!(rig.opts(), vec![inv(IOpt::KeepCustodians), inv(IOpt::RemoveCustodians)]);
    rig.assert_options_sound();
    rig.pick(0);
    assert!(rig.is_done() && !rig.driver.window.removed && rig.driver.window.owes.is_none());
    assert!(!rig.state.meta().flags.has(MetaFlags::CUSTODIANS_REMOVED));
    // A paid "yes" does not lift the token itself: it owes six influence to the payment window.
    let mut pay = Rig::from_parts(s.clone(), k.clone(), Custodians::new(Seat(0), System::MecatolRex18, true));
    pay.run();
    pay.pick(1);
    assert!(pay.is_done());
    assert_eq!(pay.driver.window.owes, Some(6));
    assert!(!pay.state.meta().flags.has(MetaFlags::CUSTODIANS_REMOVED), "the token comes down after the payment");
    // Not asked when the invader cannot pay.
    let mut poor = Rig::from_parts(s, k, Custodians::new(Seat(0), System::MecatolRex18, false));
    poor.run();
    assert!(poor.is_done());
}

#[test]
fn winnu_blood_ties_remove_the_custodians_for_free_and_score_the_point_capped_at_ten() {
    covers!("rules::invasion::stages::Custodians");
    let sc = Scenario::new(&[Faction::TheWinnu, Faction::TheArborec]).map_from(&[System::MecatolRex18]);
    let (mut s, k) = sc.build();
    s.board_mut().sys_mut(System::MecatolRex18).units.push(Piece::new(Unit::InfantryI, Seat(0)));
    let mut rig = Rig::from_parts(s.clone(), k.clone(), Custodians::new(Seat(0), System::MecatolRex18, false));
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Invasion(IAsk::RemoveCustodians { cost: 0, .. })), "offered although the invader cannot pay: it costs nothing");
    rig.pick(1);
    assert!(rig.driver.window.removed && rig.state.meta().flags.has(MetaFlags::CUSTODIANS_REMOVED));
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().econ.victory_points, 1);
    s.players_mut().by_seat_mut(Seat(0)).unwrap().econ.victory_points = 10;
    let mut capped = Rig::from_parts(s, k, Custodians::new(Seat(0), System::MecatolRex18, false));
    capped.run();
    capped.pick(1);
    assert_eq!(capped.state.players().by_seat(Seat(0)).unwrap().econ.victory_points, 10, "no more than ten points");
    // The tail helper does the same for a paid yes.
    let (mut s2, _) = rex(Faction::TheArborec).build();
    ti4_engine_core::rules::invasion::stages::lift(&mut s2, Seat(1));
    assert!(s2.meta().flags.has(MetaFlags::CUSTODIANS_REMOVED));
    assert_eq!(s2.players().by_seat(Seat(1)).unwrap().econ.victory_points, 1);
}

#[test]
fn the_custodians_window_is_not_offered_without_the_gate_and_refuses_foreign_picks() {
    covers!("rules::invasion::stages::Custodians");
    let (s, k) = rex(Faction::TheArborec).build();
    let mut none = Rig::from_parts(s.clone(), k.clone(), Custodians::new(Seat(0), System::MecatolRex18, true));
    none.run();
    assert!(none.is_done(), "no ground force in space on Mecatol Rex");
    let mut s = s;
    s.board_mut().sys_mut(System::MecatolRex18).units.push(Piece::new(Unit::InfantryI, Seat(0)));
    let mut rig = Rig::from_parts(s, k, Custodians::new(Seat(0), System::MecatolRex18, true));
    rig.run();
    rig.expect_illegal(Pick::Opt(Opt::Decline));
    rig.expect_illegal(Pick::Number(1));
}

// ---- the questions that are only questions --------------------------------------------------------

#[test]
fn fight_a_round_and_start_another_combat_are_single_option_questions_of_the_invader() {
    covers!("rules::invasion::stages::FightRound");
    covers!("rules::invasion::stages::NextCombat");
    let (p1, _) = planets();
    let mut fight = Rig::new(arena(), FightRound::new(Seat(0), sys(), p1));
    fight.run();
    assert_eq!(fight.asked().ask, Ask::Invasion(IAsk::FightRound { system: sys(), planet: p1 }));
    assert_eq!(fight.opts(), vec![inv(IOpt::FightRound { planet: p1 })]);
    fight.expect_illegal(Pick::Opt(inv(IOpt::FightRound { planet: p1 })));
    let mut next = Rig::new(arena(), NextCombat::new(Seat(0), sys(), p1, Seat(1)));
    next.run();
    assert_eq!(next.opts(), vec![inv(IOpt::FightNext { next: Seat(1) }), inv(IOpt::LeaveCoexisting { next: Seat(1) })]);
    // `FightNext` is offered but refused by the window: starting the combat is the string side's (documented on the type), so the options probe does not apply.
    next.expect_illegal(Pick::Opt(inv(IOpt::FightNext { next: Seat(1) })));
    next.pick(1);
    assert!(next.is_done(), "leaving ends the chain");
}

#[test]
fn the_bombardment_target_question_lists_the_other_players_holding_units_on_the_planet() {
    covers!("rules::invasion::stages::BombardTarget");
    let (p1, _) = planets();
    let sc = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec, Faction::TheNaaluCollective]).map_from(&[sys()]).unit(sys(), Some(p1), Unit::InfantryI, 0, 1).unit(sys(), Some(p1), Unit::InfantryI, 1, 1).unit(sys(), Some(p1), Unit::InfantryI, 2, 1);
    let mut rig = Rig::new(sc, BombardTarget::new(Seat(0), sys(), p1, 2));
    rig.run();
    assert_eq!(rig.asked().ask, Ask::Invasion(IAsk::BombardTarget { system: sys(), planet: p1, hits: 2 }));
    assert_eq!(rig.opts(), vec![inv(IOpt::BombardTarget { seat: Seat(1) }), inv(IOpt::BombardTarget { seat: Seat(2) })], "never the invader itself");
    rig.expect_illegal(Pick::Opt(inv(IOpt::BombardTarget { seat: Seat(1) })));
    let mut empty = Rig::new(arena(), BombardTarget::new(Seat(0), sys(), p1, 1));
    empty.run();
    assert!(empty.is_done(), "nobody holds units there");
}

// ---- space cannon defense ---------------------------------------------------------------------

fn def_facts(planets: Vec<Planet>, announce: bool) -> DefFacts {
    DefFacts { invader: Seat(0), system: sys(), planets, announce, cause: Code(2), kind: Code(5), planets_key: Code(6), unit_codes: vec![(Unit::PDSI, Code(30)), (Unit::PDSII, Code(31))] }
}

#[test]
fn the_plan_of_the_defense_is_the_printed_cannon_of_every_gun_with_plasma_assail_and_disable() {
    covers!("rules::invasion::defense::SpaceCannonDefense");
    let (p1, _) = planets();
    let (s, k) = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec]).map_from(&[sys()]).tech(1, Technology::PlasmaScoring).build();
    let guns = [Piece::new(Unit::PDSI, Seat(1)), Piece::new(Unit::PDSII, Seat(1)), Piece::new(Unit::InfantryI, Seat(1)), Piece::new(Unit::PDSI, Seat(1)).with_galvanized(true)];
    let plan = defense_plan(Rd::new(&s, &k), sys(), p1, Seat(0), &guns);
    assert_eq!(
        plan,
        vec![
            GunRoll { owner: Seat(1), unit: Unit::PDSI, hits_on: 6, dice: 1 },
            GunRoll { owner: Seat(1), unit: Unit::PDSII, hits_on: 5, dice: 2 },
            GunRoll { owner: Seat(1), unit: Unit::PDSI, hits_on: 6, dice: 2 },
        ],
        "Plasma Scoring: the easiest gun (PDS II) rolls a die more; a galvanized PDS one more; infantry have no cannon"
    );
    let (mut s, k) = arena().build();
    s.meta_mut().activation_seq = 2;
    s.players_mut().by_seat_mut(Seat(0)).unwrap().round_marks.push(RoundKind::DisableInvasion, 2);
    assert!(defense_plan(Rd::new(&s, &k), sys(), p1, Seat(0), &guns).is_empty(), "the invader's Disable silences the PDS");
}

#[test]
fn the_defense_fires_at_the_committed_ground_forces_and_records_the_step() {
    covers!("rules::invasion::defense::SpaceCannonDefense");
    let (p1, _) = planets();
    let mut counts = [0usize; 4];
    for seed in 0..40u64 {
        let sc = arena().unit(sys(), Some(p1), Unit::PDSI, 1, 3).control(p1, 1).unit(sys(), Some(p1), Unit::InfantryI, 0, 3);
        let (s, k) = sc.build();
        let s = with_active(s, p1, 1);
        let mut rig = Rig::from_parts(s, k, SpaceCannonDefense::new(def_facts(vec![p1], true))).seeded(seed);
        rig.run();
        assert!(rig.is_done());
        let drawn = rig.driver.window.drawn.clone();
        assert_eq!(drawn.len(), 3, "three PDS");
        assert!(drawn.iter().all(|(on, f)| *on == 6 && f.len() == 1));
        let hits: usize = drawn.iter().map(|(on, f)| f.iter().filter(|x| **x >= *on).count()).sum();
        counts[hits] += 1;
        assert_eq!(units_on(&rig, p1, 0).len(), 3usize.saturating_sub(hits), "seed {seed}: one invading ground force per hit");
        assert_eq!(units_on(&rig, p1, 1).len(), 3, "the defenders' own units are not hit");
        let first = rig.driver.emitted.first().expect("announced");
        assert_eq!(first.kind, EventKind::GroundCommitmentFinished);
        assert_eq!(first.payload.seat(Key::Player), Some(Seat(0)));
        let Some(List::Planets(listed)) = first.payload.list(Key::Other(Code(6))) else { panic!("planets list") };
        assert_eq!(listed.as_slice(), &[p1]);
        let destroyed = rig.driver.emitted.iter().filter(|e| e.kind == EventKind::GroundForceDestroyed).count();
        assert_eq!(destroyed, hits.min(3));
        let step = rig.state.transient().invasion.active.as_ref().and_then(|a| a.last_step.clone()).expect("a step is recorded when dice were rolled");
        assert_eq!((step.planet, step.kind, step.dice.len()), (p1, Code(5), 3));
        assert_eq!(step.hits.get(&Seat(1)).copied().unwrap_or(0) as usize, hits);
        assert_eq!(step.before.len(), 6);
        assert_eq!(rig.state.ledger().dice.len(), 3);
    }
    assert!(counts.iter().all(|c| *c > 0), "the seeds cover 0..3 hits: {counts:?}");
}

#[test]
fn a_planet_without_guns_rolls_nothing_and_leaves_no_step_and_a_quiet_resolver_announces_nothing() {
    covers!("rules::invasion::defense::SpaceCannonDefense");
    let (p1, _) = planets();
    let (s, k) = arena().unit(sys(), Some(p1), Unit::InfantryI, 0, 2).unit(sys(), Some(p1), Unit::InfantryI, 1, 2).control(p1, 1).build();
    let s = with_active(s, p1, 1);
    let mut rig = Rig::from_parts(s, k, SpaceCannonDefense::new(def_facts(vec![p1], false))).seeded(1);
    rig.run();
    assert!(rig.is_done() && rig.driver.window.drawn.is_empty() && rig.driver.emitted.is_empty());
    assert!(rig.state.transient().invasion.active.as_ref().unwrap().last_step.is_none());
    assert_eq!(delta(rig.start_state(), &rig.state).lines().filter(|l| l.contains("planet")).count(), 0);
}

// ---- the start of a ground round ------------------------------------------------------------------

fn begin(copied: bool, numbered: bool) -> BeginFacts {
    let (p1, _) = planets();
    BeginFacts { system: sys(), planet: p1, invader: Seat(0), defender: Seat(1), copied, numbered, can_pay: [false, false], pay_env: None, dunlain: Code(9) }
}

#[test]
fn a_ground_round_counts_up_on_the_active_fight_and_numbers_the_combat_round_from_version_16() {
    covers!("rules::invasion::begin::BeginGroundRound");
    let (p1, p2) = planets();
    let (s, k) = arena().build();
    let s = with_active(s, p1, 1);
    let mut rig = Rig::from_parts(s.clone(), k.clone(), BeginGroundRound::new(begin(false, false)));
    rig.run();
    assert_eq!(rig.state.transient().invasion.active.as_ref().unwrap().ground_round, 1);
    assert_eq!(rig.state.meta().combat_round_seq, 0, "an old rules version does not number the round");
    let mut numbered = Rig::from_parts(s.clone(), k.clone(), BeginGroundRound::new(begin(false, true)));
    numbered.run();
    assert_eq!(numbered.state.meta().combat_round_seq, 1);
    assert!(numbered.driver.emitted.is_empty());
    // Another planet or another defender is another fight: the counter stays.
    let other = with_active(s, p2, 1);
    let mut rig = Rig::from_parts(other, k, BeginGroundRound::new(begin(false, false)));
    rig.run();
    assert_eq!(rig.state.transient().invasion.active.as_ref().unwrap().ground_round, 0);
}

#[test]
fn a_copied_round_agent_numbers_the_round_in_any_version_and_announces_it() {
    covers!("rules::invasion::begin::BeginGroundRound");
    let (p1, _) = planets();
    let (s, k) = arena().build();
    let s = with_active(s, p1, 1);
    let mut rig = Rig::from_parts(s, k, BeginGroundRound::new(begin(true, false)));
    rig.run();
    assert_eq!(rig.state.meta().combat_round_seq, 1);
    let ev = &rig.driver.emitted[0];
    assert_eq!(ev.kind, EventKind::GroundCombatRoundStarted);
    assert_eq!((ev.payload.int(Key::RoundSeq), ev.payload.seat(Key::Attacker), ev.payload.seat(Key::Defender), ev.payload.planet(Key::Planet)), (Some(1), Some(Seat(0)), Some(Seat(1)), Some(p1)));
}

/// Package W13: a fight of many rounds reads what the Letnev can spend in each round (`Facts::pay_env`): the Reaper is offered while the seat can
/// pay two resources and is not once it cannot, whatever `can_pay` said at the start.
#[test]
fn the_dunlain_reaper_reads_what_the_letnev_can_spend_when_the_fight_gives_the_environment() {
    covers!("rules::invasion::begin::BeginGroundRound");
    let (p1, _) = planets();
    let letnev = |goods: i16| {
        Scenario::new(&[Faction::TheBaronyOfLetnev, Faction::TheArborec])
            .kernel(kernel_all())
            .map_from(&[sys()])
            .unit(sys(), Some(p1), Unit::InfantryI, 0, 1)
            .unit(sys(), Some(p1), Unit::InfantryI, 1, 1)
            .goods(0, goods)
    };
    let env = ti4_engine_core::kernel::fx_pay::PayEnv { scope: ti4_engine_core::tables::Scope(u32::from(kernel_all().sources.0)), blanked: false };
    let asked = |goods: i16, can_pay: bool, env: Option<ti4_engine_core::kernel::fx_pay::PayEnv>| {
        let (s, k) = letnev(goods).build();
        let s = with_active(s, p1, 1);
        let facts = BeginFacts { can_pay: [can_pay, false], pay_env: env, ..begin(false, true) };
        let mut rig = Rig::from_parts(s, k, BeginGroundRound::new(facts));
        rig.run();
        !rig.is_done()
    };
    assert!(asked(3, false, Some(env)), "three trade goods pay the Reaper: offered although the static flag says no");
    assert!(!asked(0, true, Some(env)), "nothing to spend: not offered although the static flag says yes");
    assert!(asked(0, true, None), "without the environment the static flag stands");
    assert!(!asked(3, false, None));
}

// ---- which planet is fought next --------------------------------------------------------------------

fn adv(index: usize, coexisted: Vec<Planet>) -> AdvFacts {
    let (p1, p2) = planets();
    AdvFacts { invader: Seat(0), system: sys(), planets: vec![p1, p2], index, coexisted }
}

#[test]
fn the_next_fight_is_the_first_planet_where_the_invader_and_another_player_both_have_ground_forces() {
    covers!("rules::invasion::advance::Advance");
    let (p1, p2) = planets();
    let sc = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec, Faction::TheNaaluCollective])
        .map_from(&[sys()])
        .unit(sys(), Some(p1), Unit::InfantryI, 0, 1)
        .unit(sys(), Some(p2), Unit::InfantryI, 0, 1)
        .unit(sys(), Some(p2), Unit::InfantryI, 1, 1)
        .unit(sys(), Some(p2), Unit::InfantryI, 2, 1)
        .control(p2, 2);
    let mut rig = Rig::new(sc.clone(), Advance::new(adv(0, vec![])));
    rig.run();
    assert!(rig.is_done());
    assert_eq!(rig.driver.window.pick, Some(Upcoming::Fight { index: 1, defender: Seat(2) }), "p1 has no one to fight; on p2 the controller defends before another owner");
    // Starting after p2: nothing left.
    let mut after = Rig::new(sc.clone(), Advance::new(adv(2, vec![])));
    after.run();
    assert_eq!(after.driver.window.pick, Some(Upcoming::Exhausted));
    // A planet the invader chose to coexist on is skipped.
    let mut skip = Rig::new(sc.clone(), Advance::new(adv(0, vec![p2])));
    skip.run();
    assert_eq!(skip.driver.window.pick, Some(Upcoming::Exhausted));
    // Without a controller with ground forces there, the first other owner defends.
    let sc2 = sc.control(p2, 0);
    let mut first_other = Rig::new(sc2, Advance::new(adv(1, vec![])));
    first_other.run();
    assert_eq!(first_other.driver.window.pick, Some(Upcoming::Fight { index: 1, defender: Seat(1) }));
}

#[test]
fn a_planet_where_only_the_invader_has_ground_forces_is_passed_over() {
    covers!("rules::invasion::advance::Advance");
    let (p1, _) = planets();
    let mut rig = Rig::new(arena().unit(sys(), Some(p1), Unit::InfantryI, 0, 2).unit(sys(), Some(p1), Unit::PDSI, 1, 1).control(p1, 1), Advance::new(adv(0, vec![])));
    rig.run();
    assert_eq!(rig.driver.window.pick, Some(Upcoming::Exhausted), "a PDS is no ground force: nothing to fight");
}

// ---- the dice of a ground round ----------------------------------------------------------------------

fn roll_facts(planet: Planet) -> RollFacts {
    RollFacts { attacker: Seat(0), defender: Seat(1), system: sys(), planet, kind: Code(40), names: (0..12).map(|i| Code(50 + i)).collect(), unit_codes: vec![(Unit::InfantryI, Code(70)), (Unit::InfantryII, Code(71)), (Unit::FactionlessMech, Code(72))], x89: [false, false] }
}

fn fighting() -> (Scenario, Planet) {
    let (p1, _) = planets();
    (arena().unit(sys(), Some(p1), Unit::InfantryI, 0, 3).unit(sys(), Some(p1), Unit::InfantryII, 1, 2).control(p1, 1), p1)
}

#[test]
fn both_sides_roll_a_die_per_ground_force_attacker_first_and_the_hits_are_the_dice_at_or_above_the_number() {
    covers!("rules::invasion::roll::GroundRoll");
    let mut seen = (false, false);
    for seed in 0..40u64 {
        let (sc, p1) = fighting();
        let mut rig = Rig::new(sc, GroundRoll::new(roll_facts(p1))).seeded(seed);
        rig.run();
        assert!(rig.is_done());
        let w = &rig.driver.window;
        let shape: Vec<(u32, usize)> = w.drawn.iter().map(|(on, f)| (*on, f.len())).collect();
        assert_eq!(shape, vec![(8, 3), (7, 2)], "infantry I hit on 8, infantry II on 7");
        assert!(!w.unsupported);
        let want = (w.drawn[0].1.iter().filter(|f| **f >= 8).count() as u32, w.drawn[1].1.iter().filter(|f| **f >= 7).count() as u32);
        assert_eq!(w.hits, want);
        seen = (seen.0 || want.0 > 0, seen.1 || want.1 > 0);
        let combat = &rig.state.transient().combat;
        assert_eq!(combat.last_reroll_player, Some(Seat(1)), "the defender rolled last");
        let staged: Vec<Seat> = combat.reroll_staging.iter().map(|(s, _)| *s).collect();
        assert_eq!(staged, vec![Seat(0), Seat(1)], "each side's dice are staged for the reroll windows, by seat");
        let set = &combat.reroll_staging.iter().next().unwrap().1;
        assert_eq!((set.kind, set.system, set.rolls.len()), (Code(40), sys(), 1));
        assert_eq!((set.rolls[0].unit, set.rolls[0].hits_on, set.rolls[0].planet), (Code(57), Some(8), Some(p1)), "the pool is named `combat value 8`");
        assert_eq!(rig.state.ledger().dice.len(), 2);
    }
    assert!(seen.0 && seen.1);
}

#[test]
fn a_pool_the_window_cannot_name_is_flagged_for_the_adapter() {
    covers!("rules::invasion::roll::GroundRoll");
    let (sc, p1) = fighting();
    let mut facts = roll_facts(p1);
    facts.unit_codes.clear();
    let mut rig = Rig::new(sc.clone(), GroundRoll::new(facts)).seeded(1);
    rig.run();
    assert!(rig.driver.window.unsupported, "no text for the unit type");
    let mut facts = roll_facts(p1);
    facts.names.clear();
    let mut rig = Rig::new(sc, GroundRoll::new(facts)).seeded(1);
    rig.run();
    assert!(rig.driver.window.unsupported, "no name for the pool");
    assert!(rig.state.transient().combat.reroll_staging.is_empty());
}

fn tail_facts(rolled: (u32, u32), bump: bool, planet: Planet) -> TailFacts {
    TailFacts { invader: Seat(0), defender: Seat(1), system: sys(), planet, rolled, bump, announce: true, x89: [false, false] }
}

#[test]
fn the_tail_reads_the_hits_back_from_the_staged_dice_lists_the_dice_and_clears_the_staging() {
    covers!("rules::invasion::tail::GroundRollTail");
    let (sc, p1) = fighting();
    let mut roll = Rig::new(sc, GroundRoll::new(roll_facts(p1))).seeded(3);
    roll.run();
    let rolled = roll.driver.window.hits;
    let mut tail = Rig::from_parts(roll.state.clone(), roll.kernel.clone(), GroundRollTail::new(tail_facts(rolled, true, p1)));
    tail.run();
    assert!(tail.is_done());
    assert_eq!(tail.driver.window.hits, rolled, "no reroll happened: the staged dice give the same hits");
    assert_eq!(tail.driver.window.dice.len(), 5, "three and two dice");
    assert!(tail.driver.window.dice.iter().all(|(side, name, face, target, hit)| (*side == Seat(0) && *name == Code(57) && *target == 8 || *side == Seat(1) && *name == Code(56) && *target == 7) && *hit == (*face >= *target)));
    let kinds: Vec<EventKind> = tail.driver.emitted.iter().map(|e| e.kind).collect();
    assert_eq!(kinds, vec![EventKind::GroundRollsMade, EventKind::GroundRollsMade]);
    assert_eq!((tail.driver.emitted[0].payload.seat(Key::Player), tail.driver.emitted[0].payload.int(Key::Hits)), (Some(Seat(0)), Some(i64::from(rolled.0))));
    assert_eq!(tail.state.meta().combat_round_seq, roll.state.meta().combat_round_seq + 1, "the round number moves on");
    assert!(tail.state.transient().combat.reroll_staging.is_empty());
    assert_eq!(tail.state.transient().combat.last_reroll_player, None);
}

#[test]
fn a_reroll_that_changed_a_die_changes_the_hits_the_tail_reports_and_a_numbered_round_is_not_bumped_twice() {
    covers!("rules::invasion::tail::GroundRollTail");
    let (sc, p1) = fighting();
    let mut roll = Rig::new(sc, GroundRoll::new(roll_facts(p1))).seeded(3);
    roll.run();
    // A reroll window (Fire Team, a card) raised every die of the attacker's pool by 3.
    let mut state = roll.state.clone();
    let staging = state.transient_mut().combat.reroll_staging.edit();
    for (seat, set) in staging.iter_mut() {
        if *seat == Seat(0) {
            for e in &mut set.rolls {
                for i in 0..e.faces.len() {
                    e.deltas.insert(i as u8, 10);
                }
            }
        }
    }
    let mut tail = Rig::from_parts(state, roll.kernel.clone(), GroundRollTail::new(tail_facts(roll.driver.window.hits, false, p1)));
    tail.run();
    assert_eq!(tail.driver.window.hits.0, 3, "every one of the three attacking dice hits now");
    assert!(tail.driver.window.dice.iter().filter(|d| d.0 == Seat(0)).all(|d| d.4));
    assert_eq!(tail.state.meta().combat_round_seq, roll.state.meta().combat_round_seq, "the round was numbered already (a copied round agent)");
    let mut quiet = TailFacts { announce: false, ..tail_facts((0, 0), false, p1) };
    quiet.rolled = (1, 1);
    let mut rig = Rig::new(fighting().0, GroundRollTail::new(quiet));
    rig.run();
    assert!(rig.driver.emitted.is_empty());
    assert_eq!(rig.driver.window.hits, (1, 1), "nothing staged: the hits are the rolled ones");
    assert!(rig.driver.window.dice.is_empty());
}

// ---- the hits of a round ---------------------------------------------------------------------------------

fn round_facts(hits: (u32, u32), harrow: Option<HarrowFacts>) -> RoundFacts {
    let (p1, _) = planets();
    RoundFacts { invader: Seat(0), defender: Seat(1), system: sys(), planet: p1, hits, announce: true, cause: Code(2), template: step_template(p1), harrow }
}

#[test]
fn a_ground_round_removes_the_hits_of_both_sides_announces_them_after_the_round_and_records_the_step() {
    covers!("rules::invasion::round::GroundRoundHits");
    let (sc, p1) = fighting();
    let (s, k) = sc.build();
    let mut s = with_active(s, p1, 1);
    s.transient_mut().invasion.active.as_mut().unwrap().edit().ground_round = 2;
    let mut rig = Rig::from_parts(s, k, GroundRoundHits::new(round_facts((2, 1), None)));
    rig.run();
    assert!(rig.is_done());
    assert_eq!(rig.driver.window.assigned, (2, 1));
    assert_eq!(units_on(&rig, p1, 1).len(), 0, "two hits on two defenders");
    assert_eq!(units_on(&rig, p1, 0).len(), 2, "one hit on three attackers");
    let kinds: Vec<EventKind> = rig.driver.emitted.iter().map(|e| e.kind).collect();
    assert_eq!(kinds, vec![EventKind::GroundHitsToAssign, EventKind::GroundHitsToAssign, EventKind::GroundForceDestroyed, EventKind::GroundForceDestroyed, EventKind::GroundForceDestroyed], "both windows first, then the removals' events");
    assert_eq!(rig.driver.emitted[0].payload.seat(Key::Player), Some(Seat(1)), "the attacker's hits land on the defender first");
    let step = rig.state.transient().invasion.active.as_ref().unwrap().last_step.clone().unwrap();
    assert_eq!((step.round, step.planet, step.kind, step.harrow_hits), (2, p1, Code(5), 0));
    assert_eq!(step.hits.get(&Seat(0)).copied(), Some(2), "hits the invader produced");
    assert_eq!(step.hits.get(&Seat(1)).copied(), Some(1));
    assert_eq!(step.after.len(), 2);
}

/// Package W13: the hits a faction adds to its own roll (Valkyrie Particle Weave: one more when the opponent produced any) are the kernel's hook,
/// read once both sides' dice are final: the invader's two hits and the defender's one become two and one + 1.
#[test]
fn the_hits_a_faction_adds_to_its_roll_are_assigned_by_the_opponent_like_any_other() {
    covers!("rules::invasion::round::GroundRoundHits");
    fn weave(_: &ti4_engine_core::state::CoreState, _: &ti4_engine_core::kernel::Kernel, roller: Seat, _: System, _: Planet, own: usize, opponent: usize) -> usize {
        usize::from(roller == Seat(1) && opponent >= 1 && own < 9)
    }
    let (sc, p1) = fighting();
    let (s, mut k) = sc.build();
    k.hooks.ground.ground_rolls_extra_hits.push(weave);
    let s = with_active(s, p1, 1);
    // The defender (seat 1) rolled none of its own: the weave adds one to the invader's two.
    let mut rig = Rig::from_parts(s.clone(), k.clone(), GroundRoundHits::new(round_facts((2, 0), None)));
    rig.run();
    assert_eq!(rig.driver.window.assigned, (2, 1), "the defender's weave hit lands on the invader");
    assert_eq!(units_on(&rig, p1, 0).len(), 2);
    // The invader rolled nothing: the defender's hook reads the opponent's hits as zero and adds nothing.
    let mut quiet = Rig::from_parts(s, k, GroundRoundHits::new(round_facts((0, 1), None)));
    quiet.run();
    assert_eq!(quiet.driver.window.assigned, (0, 1));
}

#[test]
fn a_round_with_no_hits_changes_nothing_and_a_quiet_resolver_drops_the_events() {
    covers!("rules::invasion::round::GroundRoundHits");
    let (sc, p1) = fighting();
    let (s, k) = sc.build();
    let s = with_active(s, p1, 1);
    let mut rig = Rig::from_parts(s.clone(), k.clone(), GroundRoundHits::new(round_facts((0, 0), None)));
    rig.run();
    assert!(rig.driver.emitted.is_empty());
    assert_eq!(units_on(&rig, p1, 0).len() + units_on(&rig, p1, 1).len(), 5);
    let mut quiet_facts = round_facts((1, 1), None);
    quiet_facts.announce = false;
    let mut quiet = Rig::from_parts(s, k, GroundRoundHits::new(quiet_facts));
    quiet.run();
    assert!(quiet.driver.emitted.is_empty());
    assert_eq!(units_on(&quiet, p1, 0).len() + units_on(&quiet, p1, 1).len(), 3, "the hits still land");
    assert!(quiet.state.transient().combat.pending_destructions.is_empty());
}

// ---- Harrow --------------------------------------------------------------------------------------

fn harrow_facts() -> HarrowFacts {
    let (p1, _) = planets();
    HarrowFacts { invader: Seat(0), defender: Seat(1), system: sys(), planet: p1, announce: true, cause: Code(9) }
}

fn l1z1x() -> Scenario {
    let (p1, _) = planets();
    Scenario::new(&[Faction::TheL1Z1XMindnet, Faction::TheArborec]).map_from(&[sys()]).unit(sys(), None, Unit::DreadnoughtI, 0, 2).unit(sys(), Some(p1), Unit::InfantryI, 0, 1).unit(sys(), Some(p1), Unit::InfantryI, 1, 3).control(p1, 1)
}

#[test]
fn harrow_rolls_the_bombarding_ships_after_the_removals_and_lands_the_hits_on_the_defender() {
    covers!("rules::invasion::harrow::Harrow");
    let (p1, _) = planets();
    let mut got = [false; 3];
    for seed in 0..40u64 {
        let mut rig = Rig::new(l1z1x(), Harrow::new(harrow_facts())).seeded(seed);
        rig.run();
        assert!(rig.is_done());
        let drawn = rig.driver.window.drawn.clone();
        assert_eq!(drawn.iter().map(|(on, f)| (*on, f.len())).collect::<Vec<_>>(), vec![(5, 1), (5, 1)], "each dreadnought rolls its BOMBARDMENT 5 once");
        let hits = drawn.iter().map(|(on, f)| f.iter().filter(|x| **x >= *on).count()).sum::<usize>();
        got[hits] = true;
        assert_eq!(rig.driver.window.assigned as usize, hits, "no card cancelled any");
        assert_eq!(units_on(&rig, p1, 1).len(), 3usize.saturating_sub(hits), "seed {seed}");
        assert_eq!(units_on(&rig, p1, 0).len(), 1, "the invader's own forces are safe");
        assert_eq!(rig.state.ledger().dice.len(), 2);
    }
    assert!(got.iter().all(|g| *g), "0, 1 and 2 hits all occur");
}

#[test]
fn harrow_does_nothing_when_the_planet_cannot_be_bombarded_or_nothing_can_bombard() {
    covers!("rules::invasion::harrow::Harrow");
    let (p1, _) = planets();
    let mut shielded = Rig::new(l1z1x().unit(sys(), Some(p1), Unit::PDSI, 1, 1), Harrow::new(harrow_facts())).seeded(1);
    shielded.run();
    assert!(shielded.is_done() && shielded.driver.window.drawn.is_empty(), "a planetary shield stops Harrow too");
    let mut cruisers = Rig::new(Scenario::new(&[Faction::TheL1Z1XMindnet, Faction::TheArborec]).map_from(&[sys()]).unit(sys(), None, Unit::CruiserI, 0, 2).unit(sys(), Some(p1), Unit::InfantryI, 1, 1).control(p1, 1), Harrow::new(harrow_facts())).seeded(1);
    cruisers.run();
    assert!(cruisers.driver.window.drawn.is_empty() && cruisers.driver.window.assigned == 0);
    // Through the round window: the Harrow facts make its hits part of the step report.
    let (s, k) = l1z1x().build();
    let s = with_active(s, p1, 1);
    let mut round = Rig::from_parts(s, k, GroundRoundHits::new(round_facts((0, 0), Some(harrow_facts())))).seeded(2);
    round.run();
    assert!(round.driver.window.harrow.is_some());
    let step = round.state.transient().invasion.active.as_ref().unwrap().last_step.clone().unwrap();
    assert_eq!(step.harrow_hits, round.driver.window.harrow.as_ref().unwrap().assigned);
}

// ---- the bombardment window ---------------------------------------------------------------------

fn entry_from(plan: &[ti4_engine_core::rules::invasion::PlanetBombard], hits: &[usize]) -> Vec<Entry> {
    plan.iter().map(|p| Entry { planet: p.planet, template: step_template(p.planet), groups: hits.iter().copied().collect::<SmallList<usize, 8>>(), held: p.held, victims: p.victims }).collect()
}

#[test]
fn a_bombardment_with_one_victim_lands_every_group_and_bombarding_the_last_ground_forces_out_scores() {
    covers!("rules::invasion::bombard::Bombardment");
    let (p1, _) = planets();
    let sc = arena().unit(sys(), None, Unit::DreadnoughtI, 0, 2).unit(sys(), Some(p1), Unit::InfantryI, 1, 2).control(p1, 1);
    let (s, k) = sc.clone().build();
    let plan = bombard_plan(Rd::new(&s, &k), sys(), Seat(0));
    let s = with_active(s, p1, 1);
    let mut rig = Rig::from_parts(s, k.clone(), Bombardment::new(Seat(0), sys(), 4, true, Code(2), entry_from(&plan, &[1, 1]), 0));
    rig.run();
    assert!(rig.is_done());
    assert_eq!(rig.driver.window.kills, 2);
    assert!(units_on(&rig, p1, 1).is_empty());
    let feats = rig.state.players().by_seat(Seat(0)).unwrap().misc.event_feats.as_slice().to_vec();
    assert_eq!(feats, vec![(Feat::BombardedOutTheLastGroundForces, 4)]);
    assert_eq!(rig.driver.window.scoring, vec![(4, false)]);
    assert!(!rig.driver.window.paused());
    let destroyed = rig.driver.emitted.iter().filter(|e| e.kind == EventKind::GroundForceDestroyed).count();
    assert_eq!(destroyed, 2);
    assert!(rig.state.transient().invasion.active.as_ref().unwrap().last_step.is_some(), "the planet's step is recorded");
    // Hits that do not clear the planet score nothing.
    let (s, _) = sc.build();
    let mut some = Rig::from_parts(with_active(s, p1, 1), k, Bombardment::new(Seat(0), sys(), 4, false, Code(2), entry_from(&plan, &[1, 0]), 0));
    some.run();
    assert_eq!(some.driver.window.kills, 1);
    assert!(some.state.players().by_seat(Seat(0)).unwrap().misc.event_feats.is_empty() && some.driver.window.scoring.is_empty());
    assert!(some.driver.emitted.is_empty(), "a quiet resolver announces nothing");
}

#[test]
fn a_planet_with_two_victims_pauses_on_the_target_question_and_the_answer_lands_the_hits_on_the_chosen_player() {
    covers!("rules::invasion::bombard::Bombardment");
    let (p1, _) = planets();
    let sc = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec, Faction::TheNaaluCollective])
        .map_from(&[sys()])
        .unit(sys(), None, Unit::DreadnoughtI, 0, 1)
        .unit(sys(), Some(p1), Unit::InfantryI, 1, 1)
        .unit(sys(), Some(p1), Unit::InfantryI, 2, 2)
        .control(p1, 1);
    let (s, k) = sc.build();
    let plan = bombard_plan(Rd::new(&s, &k), sys(), Seat(0));
    assert_eq!(plan[0].victims.len(), 2);
    let mut rig = Rig::from_parts(s.clone(), k.clone(), Bombardment::new(Seat(0), sys(), 1, true, Code(2), entry_from(&plan, &[2]), 0));
    rig.run();
    assert!(rig.is_done(), "the window stops where the string waits for the answer");
    assert!(rig.driver.window.paused());
    assert_eq!(units_on(&rig, p1, 1).len() + units_on(&rig, p1, 2).len(), 3, "nothing landed yet");
    let q = rig.driver.window.ask_of(&rig.state, &rig.kernel).expect("the target question");
    assert_eq!(q.opts.as_slice(), &[inv(IOpt::BombardTarget { seat: Seat(1) }), inv(IOpt::BombardTarget { seat: Seat(2) })]);
    assert_eq!(q.ask, Ask::Invasion(IAsk::BombardTarget { system: sys(), planet: p1, hits: 2 }));
    // Answer: the window rebuilt at its cursor, asking.
    let cursor = Cursor { planet_index: 0, planet: p1, groups: [2usize].into_iter().collect(), next_group: 0, taken: 0, held: plan[0].held, victims: plan[0].victims };
    let at = Bombardment::new(Seat(0), sys(), 1, true, Code(2), entry_from(&plan, &[2]), 0).at_cursor(cursor, true);
    let mut ans = Rig::from_parts(with_active(s, p1, 1), k, at);
    ans.run();
    assert_eq!(ans.asked().actor, Seat(0));
    ans.expect_illegal(Pick::Opt(inv(IOpt::BombardTarget { seat: Seat(0) })).clone());
    ans.pick_opt(inv(IOpt::BombardTarget { seat: Seat(2) }));
    assert_eq!(units_on(&ans, p1, 2).len(), 0, "both hits landed on the chosen player");
    assert_eq!(units_on(&ans, p1, 1).len(), 1);
    assert_eq!(ans.driver.window.kills, 2);
    assert!(ans.state.players().by_seat(Seat(0)).unwrap().misc.event_feats.is_empty(), "a victim is left: no feat");
}

// ---- X-89 Bacterial Weapon (package W10) ------------------------------------------------------------

#[test]
fn x89_doubles_the_hits_of_its_holders_ground_roll_and_the_tail_and_leaves_the_staged_dice_raw() {
    covers!("rules::invasion::roll::GroundRoll");
    covers!("rules::invasion::tail::GroundRollTail");
    for x89 in [[false, false], [true, false], [false, true], [true, true]] {
        for seed in 0..12u64 {
            let (sc, p1) = fighting();
            let mut rig = Rig::new(sc, GroundRoll::new(RollFacts { x89, ..roll_facts(p1) })).seeded(seed);
            rig.run();
            let w = &rig.driver.window;
            let raw = (w.drawn[0].1.iter().filter(|f| **f >= 8).count() as u32, w.drawn[1].1.iter().filter(|f| **f >= 7).count() as u32);
            let want = (raw.0 * if x89[0] { 2 } else { 1 }, raw.1 * if x89[1] { 2 } else { 1 });
            assert_eq!(w.hits, want, "seed {seed} {x89:?}: the holder's hits are doubled, the other's are not");
            // The dice a reroll window sees are the raw faces.
            let staged_faces: usize = rig.state.transient().combat.reroll_staging.iter().map(|(_, set)| set.rolls.iter().map(|r| r.faces.len()).sum::<usize>()).sum();
            assert_eq!(staged_faces, 5);
            // The tail recomputes the hits from the staged dice and doubles them again for the holder.
            let mut tail = Rig::from_parts(rig.state.clone(), rig.kernel.clone(), GroundRollTail::new(TailFacts { x89, ..tail_facts(w.hits, true, p1) }));
            tail.run();
            assert_eq!(tail.driver.window.hits, want, "seed {seed} {x89:?}");
        }
    }
}

#[test]
fn x89_exhausts_each_planet_it_bombards_where_the_planets_turn_begins() {
    covers!("rules::invasion::bombard::Bombardment");
    let (p1, _) = planets();
    let sc = arena().unit(sys(), None, Unit::DreadnoughtI, 0, 2).unit(sys(), Some(p1), Unit::InfantryI, 1, 2).control(p1, 1);
    let (s, k) = sc.build();
    let plan = bombard_plan(Rd::new(&s, &k), sys(), Seat(0));
    let mut plain = Rig::from_parts(with_active(s.clone(), p1, 1), k.clone(), Bombardment::new(Seat(0), sys(), 4, true, Code(2), entry_from(&plan, &[1, 1]), 0));
    plain.run();
    assert!(!plain.state.board().planet(p1).flags.has(ti4_engine_core::state::PlanetFlags::EXHAUSTED), "without the technology the planet stays ready");
    let mut window = Bombardment::new(Seat(0), sys(), 4, true, Code(2), entry_from(&plan, &[1, 1]), 0);
    window.x89 = true;
    let mut rig = Rig::from_parts(with_active(s, p1, 1), k, window);
    rig.run();
    assert!(rig.state.board().planet(p1).flags.has(ti4_engine_core::state::PlanetFlags::EXHAUSTED), "the bombarded planet is exhausted");
}

// ---- the space cannon defense where the guns are silenced or an attachment fires (package X) ----------------------------

fn defense_in(system: System, planet: Planet, seed: u64, tech: Option<Technology>, attachment: Option<Attachment>, pds: usize) -> Rig<SpaceCannonDefense> {
    let mut sc = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec]).kernel(kernel_all()).map_from(&[system]).control(planet, 1).unit(system, Some(planet), Unit::InfantryI, 0, 4);
    if pds > 0 {
        sc = sc.unit(system, Some(planet), Unit::PDSI, 1, pds);
    }
    if let Some(t) = tech {
        sc = sc.tech(0, t);
    }
    let (mut s, k) = sc.build();
    if let Some(a) = attachment {
        s.board_mut().planet_mut(planet).attach.push(a);
        s.board_mut().planet_mut(planet).flags.set(ti4_engine_core::state::PlanetFlags::ATTACH_ENTRY, true);
    }
    let mut s = s;
    s.transient_mut().invasion.active = Some(Cow::new(ActiveInvasion { system, invader: Seat(0), seq: 1, phase: Code(1), planet: Some(planet), defender: Some(Seat(1)), ground_round: 0, last_step: None }));
    let facts = DefFacts { invader: Seat(0), system, planets: vec![planet], announce: true, cause: Code(2), kind: Code(5), planets_key: Code(6), unit_codes: vec![(Unit::PDSI, Code(30))] };
    Rig::from_parts(s, k, SpaceCannonDefense::new(facts)).seeded(seed)
}

#[test]
fn an_entropic_scar_and_l4_disruptors_silence_every_gun_but_the_commitment_is_still_announced() {
    covers!("rules::invasion::defense::SpaceCannonDefense");
    let (lemox_sys, lemox) = (System::Lemox, Planet::Lemox);
    let mut scar = defense_in(lemox_sys, lemox, 3, None, None, 3);
    scar.run();
    assert!(scar.is_done() && scar.driver.window.drawn.is_empty(), "no gun fires in an entropic scar");
    assert_eq!(scar.driver.emitted.first().map(|e| e.kind), Some(EventKind::GroundCommitmentFinished));
    assert_eq!(scar.state.board().planet(lemox).units.as_slice().iter().filter(|p| p.owner() == Seat(0)).count(), 4, "nobody is hit");
    assert!(scar.state.ledger().dice.is_empty());
    // L4 Disruptors on the invader: a plain system's PDS do not fire either; without it they do.
    let (system, planet) = (sys(), planets().0);
    let mut l4 = defense_in(system, planet, 3, Some(Technology::L4Disruptors), None, 3);
    l4.run();
    assert!(l4.is_done() && l4.driver.window.drawn.is_empty() && l4.state.ledger().dice.is_empty());
    let mut plain = defense_in(system, planet, 3, None, None, 3);
    plain.run();
    assert_eq!(plain.driver.window.drawn.len(), 3, "the same position without L4 Disruptors fires three PDS");
}

#[test]
fn an_attachment_that_gives_space_cannon_rolls_for_the_planets_controller_after_the_guns_and_is_not_a_die_of_the_step() {
    covers!("rules::invasion::defense::SpaceCannonDefense");
    let (system, planet) = (sys(), planets().0);
    let mut hits_seen = 0;
    for seed in 0..40u64 {
        let mut rig = defense_in(system, planet, seed, None, Some(Attachment::Geoform), 1);
        rig.run();
        let cannon = Attachment::Geoform.data().space_cannon.expect("Geoform gives SPACE CANNON");
        let drawn = rig.driver.window.drawn.clone();
        assert_eq!(drawn.len(), 2, "the PDS, then the attachment");
        assert_eq!((drawn[1].0, drawn[1].1.len()), (u32::from(cannon.hits_on), usize::from(cannon.dice)));
        let total: usize = drawn.iter().map(|(on, f)| f.iter().filter(|x| **x >= *on).count()).sum();
        hits_seen += total;
        assert_eq!(rig.state.board().planet(planet).units.as_slice().iter().filter(|p| p.owner() == Seat(0)).count(), 4usize.saturating_sub(total), "seed {seed}");
        let step = rig.state.transient().invasion.active.as_ref().and_then(|a| a.last_step.clone()).expect("the PDS die is recorded");
        assert_eq!(step.dice.len(), 1, "only the unit's die is a die of the step");
        assert_eq!(rig.state.ledger().dice.len(), 2);
        assert_eq!(rig.state.ledger().dice.iter().filter(|d| d.unit.is_none()).count(), 1, "the attachment's roll names no unit");
        assert!(rig.state.ledger().dice.iter().all(|d| d.rerolled == 0), "nothing was rerolled");
    }
    assert!(hits_seen > 0);
    // The invader's own planet does not fire at its owner: the attachment rolls for the controller only.
    let mut own = defense_in(system, planet, 1, None, Some(Attachment::Geoform), 0);
    own.state.board_mut().planet_mut(planet).control = Some(Seat(0));
    own.run();
    assert!(own.driver.window.drawn.is_empty());
}
