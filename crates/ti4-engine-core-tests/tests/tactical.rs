//! D4-2b: `rules::tactical` (the `Machine` window: activation, the movement step, loading cargo, free tactical action and the close).
//! The machine is run `single()`, the way the driver of a turn runs it: one outcome per run.

use ti4_engine_core::choice::tactical::{Ask as TAsk, Opt as TOpt};
use ti4_engine_core::hex::Hex;
use ti4_engine_core::rules::tactical::{Facts, Machine, Plain, Step};
use ti4_engine_core::state::{Code, EventKind};
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::plain_systems;

fn facts() -> Facts {
    Facts { pool: Code(0) }
}

fn t(o: TOpt) -> Opt {
    Opt::Tactical(o)
}

/// A row A B C D on the hexes (0,0) .. (3,0); seat 0 plays Muaat, seat 1 the Arborec.
fn row() -> (Scenario, [System; 4]) {
    let p = plain_systems();
    let tiles = [p[0], p[1], p[2], p[3]];
    let sc = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec]).map_hexes(&[(tiles[0], Hex::new(0, 0)), (tiles[1], Hex::new(1, 0)), (tiles[2], Hex::new(2, 0)), (tiles[3], Hex::new(3, 0))]).tokens(0, 3, 3, 2);
    (sc, tiles)
}

fn token_pools(s: &CoreState) -> (i16, i16, i16) {
    let e = s.players().by_seat(Seat(0)).unwrap().econ;
    (e.tactic_tokens, e.fleet_tokens, e.strategic_tokens)
}

// ---- activation -------------------------------------------------------------------------------

#[test]
fn activation_offers_the_systems_without_our_token_in_hex_order_and_places_one() {
    covers!("rules::tactical::Machine");
    let (sc, tiles) = row();
    let sc = sc.command_token(tiles[1], 0);
    let mut rig = Rig::new(sc, Machine::activating(Seat(0), facts()).single());
    rig.run();
    assert_eq!(rig.asked().ask, Ask::Tactical(TAsk::Activate { seat: Seat(0) }));
    assert_eq!(rig.opts(), vec![t(TOpt::Activate(tiles[0])), t(TOpt::Activate(tiles[2])), t(TOpt::Activate(tiles[3]))], "the system with our token is not offered");
    rig.assert_options_sound();
    rig.expect_illegal(Pick::Opt(t(TOpt::Activate(tiles[1]))));
    rig.pick(2);
    assert!(rig.is_done());
    assert_eq!(rig.driver.window.step, Some(Step::Activated));
    assert_eq!(rig.state.meta().active_system, Some(tiles[3]));
    assert!(rig.state.board().sys(tiles[3]).cmd.contains(Seat(0)), "our token is on the system");
    assert_eq!(token_pools(&rig.state), (2, 3, 2), "from the tactic pool");
    assert_eq!(rig.state.meta().activation_seq, 1);
    assert!(rig.driver.window.plain.contains(&Plain::SystemActivated(tiles[3])));
    let kinds: Vec<EventKind> = rig.driver.emitted.iter().map(|e| e.kind).collect();
    assert_eq!(kinds, vec![EventKind::CommandTokenPlaced, EventKind::SystemActivated]);
    ti4_engine_core_tests::assert_snapshot("tactical/activate", &rig.snapshot());
}

#[test]
fn without_a_tactic_token_or_a_free_system_nothing_is_asked() {
    covers!("rules::tactical::Machine");
    let (sc, tiles) = row();
    let mut none = Rig::new(sc.clone().tokens(0, 0, 3, 2), Machine::activating(Seat(0), facts()).single());
    none.run();
    assert!(none.is_done() && none.driver.window.step.is_none());
    assert_eq!(delta(none.start_state(), &none.state), "");
    let mut all = sc;
    for tile in tiles {
        all = all.command_token(tile, 0);
    }
    let mut full = Rig::new(all, Machine::activating(Seat(0), facts()).single());
    full.run();
    assert!(full.is_done(), "a token everywhere: no system to activate");
}

#[test]
fn a_free_tactical_action_activates_without_a_token_and_without_a_question() {
    covers!("rules::tactical::Machine");
    let (sc, tiles) = row();
    let mut rig = Rig::new(sc, Machine::free(Seat(0), facts(), tiles[2]));
    rig.run();
    assert_eq!(rig.state.meta().active_system, Some(tiles[2]));
    assert!(!rig.state.board().sys(tiles[2]).cmd.contains(Seat(0)), "no token placed");
    assert_eq!(token_pools(&rig.state), (3, 3, 2));
    assert!(rig.driver.window.plain.contains(&Plain::FreeActivated) && rig.driver.window.plain.contains(&Plain::FreeTactical));
    assert_eq!(rig.asked().ask, Ask::Tactical(TAsk::Move { seat: Seat(0) }), "straight on to the movement step");
}

// ---- movement ---------------------------------------------------------------------------------

fn moving_scenario() -> (Scenario, [System; 4]) {
    let (sc, t) = row();
    // The active system is D; cruisers (move 2) stand on A, B and C; a carrier (move 1) stands on B.
    let sc = sc.active_system(t[3]).unit(t[0], None, Unit::CruiserI, 0, 1).unit(t[1], None, Unit::CruiserI, 0, 1).unit(t[1], None, Unit::CarrierI, 0, 1).unit(t[2], None, Unit::CruiserI, 0, 1);
    (sc, t)
}

fn moves(rig: &Rig<Machine>) -> Vec<(System, Unit)> {
    rig.opts().iter().filter_map(|o| if let Opt::Tactical(TOpt::Move { origin, kind, .. }) = o { Some((*origin, *kind)) } else { None }).collect()
}

#[test]
fn only_ships_that_can_reach_the_active_system_are_offered_and_done_moving_closes_the_step() {
    covers!("rules::tactical::Machine");
    let (sc, t) = moving_scenario();
    let mut rig = Rig::new(sc, Machine::moving(Seat(0), facts()).single());
    rig.run();
    assert_eq!(rig.asked().ask, Ask::Tactical(TAsk::Move { seat: Seat(0) }));
    let m = moves(&rig);
    assert!(m.contains(&(t[1], Unit::CruiserI)) && m.contains(&(t[2], Unit::CruiserI)), "cruisers within two steps");
    assert!(!m.iter().any(|(o, _)| *o == t[0]), "three steps away: out of range");
    assert!(!m.contains(&(t[1], Unit::CarrierI)), "a carrier moves one: B is two steps from D");
    assert!(matches!(rig.opts().last(), Some(Opt::Tactical(TOpt::DoneMoving))));
    rig.assert_options_sound();
    rig.pick_where(|o| matches!(o, Opt::Tactical(TOpt::DoneMoving)));
    assert_eq!(rig.driver.window.step, Some(Step::Finished));
    assert!(rig.is_done());
    assert_eq!(delta(rig.start_state(), &rig.state), "", "finishing moves nothing");
}

#[test]
fn a_ship_that_is_chosen_sails_into_the_active_system_and_the_arrival_is_announced() {
    covers!("rules::tactical::Machine");
    let (sc, t) = moving_scenario();
    let mut rig = Rig::new(sc, Machine::moving(Seat(0), facts()).single());
    rig.run();
    rig.pick_where(|o| matches!(o, Opt::Tactical(TOpt::Move { origin, kind: Unit::CruiserI, .. }) if *origin == t[2]));
    assert!(rig.is_done());
    assert_eq!(rig.driver.window.step, Some(Step::Sailed { lost: false }));
    assert_eq!(rig.state.board().sys(t[2]).units.len(), 0, "left C");
    assert_eq!(rig.state.board().sys(t[3]).units.len(), 1, "arrived in D");
    assert!(rig.driver.window.plain.contains(&Plain::ShipMoved));
    assert!(rig.driver.emitted.iter().any(|e| e.kind == EventKind::ShipMoved));
    ti4_engine_core_tests::assert_snapshot("tactical/sail_one", &rig.snapshot());
}

#[test]
fn a_ship_out_of_range_or_a_move_that_was_not_offered_is_illegal() {
    covers!("rules::tactical::Machine");
    let (sc, t) = moving_scenario();
    let mut rig = Rig::new(sc, Machine::moving(Seat(0), facts()).single());
    rig.run();
    let far = TOpt::Move { origin: t[0], index: 0, kind: Unit::CruiserI, damaged: false, capacity: 0, gd: false, ionian: false, route: 0 };
    rig.expect_illegal(Pick::Opt(t(far)));
    rig.expect_illegal(Pick::Opt(t(TOpt::Activate(t[0]))));
}

#[test]
fn another_players_ship_in_the_way_blocks_the_route() {
    covers!("rules::tactical::Machine");
    let (sc, t) = moving_scenario();
    let sc = sc.unit(t[2], None, Unit::CruiserI, 1, 1);
    let mut rig = Rig::new(sc, Machine::moving(Seat(0), facts()).single());
    rig.run();
    let m = moves(&rig);
    assert!(m.contains(&(t[2], Unit::CruiserI)), "the rival's ship does not stop a ship that starts in its system");
    assert!(!m.iter().any(|(o, _)| *o == t[1]), "but B cannot pass C");
}

#[test]
fn a_ship_carrying_cargo_asks_what_to_load_before_it_sails() {
    covers!("rules::tactical::Machine");
    let (sc, t) = row();
    let p = t[2].data().planets[0];
    let sc = sc.active_system(t[3]).unit(t[2], None, Unit::CarrierI, 0, 1).unit(t[2], Some(p), Unit::InfantryI, 0, 2).control(p, 0);
    let mut rig = Rig::new(sc, Machine::moving(Seat(0), facts()).single());
    rig.run();
    rig.pick_where(|o| matches!(o, Opt::Tactical(TOpt::Move { kind: Unit::CarrierI, .. })));
    assert!(!rig.is_done(), "the carrier has room: load first");
    assert_eq!(rig.driver.window.step, Some(Step::Selected));
}

#[test]
fn closing_the_movement_emits_the_finish_and_opens_the_aftermath() {
    covers!("rules::tactical::Machine");
    let (sc, t) = moving_scenario();
    let mut rig = Rig::new(sc, Machine::closing(Seat(0), facts()));
    rig.run();
    assert!(rig.is_done());
    let closed = rig.driver.window.closed.expect("closed");
    assert_eq!((closed.player, closed.system, closed.ships_moved), (Seat(0), t[3], 0));
    assert!(rig.driver.emitted.iter().any(|e| e.kind == EventKind::MovementFinished));
}

#[test]
fn the_machine_state_round_trips_through_its_payload() {
    covers!("rules::tactical::Machine");
    let (sc, _) = moving_scenario();
    let (state, kernel) = sc.build();
    for m in [Machine::activating(Seat(0), facts()), Machine::moving(Seat(0), facts())] {
        let payload = m.payload(Default::default());
        let back = Machine::from_payload(&state, &kernel, facts(), &payload);
        assert_eq!(format!("{:?}", back.payload(Default::default())), format!("{payload:?}"));
    }
}

#[test]
fn tactical_smoke() {
    smoke("tactical/activation_smoke", || {
        let (sc, _) = row();
        Rig::new(sc, Machine::activating(Seat(0), facts()).single())
    });
    smoke("tactical/movement_smoke", || {
        let (sc, _) = moving_scenario();
        Rig::new(sc, Machine::moving(Seat(0), facts()).single())
    });
}
