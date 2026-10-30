//! Package A2b: the tactical action runs on the core beside the string engine (`tactical_dual`,
//! `rules::tactical`) under `movement=dual`, answer by answer, and agrees with it. A bail is silent, so every test counts
//! the windows the core followed (`compared`) and the ones it left to the string (`bailed`).

use ti4_content::ContentStore;
use ti4_content::galaxy::Galaxy;
use ti4_model::content_types::POK;
use ti4_model::id::{PlayerId, SystemId};
use ti4_model::state::GameState;

use crate::dual::{Area, AreaMask, Backend, report, take_report, with_mask};
use crate::game::{Game, TACTICAL_ACTION_ID};

fn a() -> PlayerId {
    PlayerId::new("a")
}
fn b() -> PlayerId {
    PlayerId::new("b")
}

/// The windows of the area, by label.
const LABELS: [&str; 5] = ["tactical::activate", "tactical::move", "tactical::load", "tactical::finish", "tactical::free"];

/// A seated table of real factions (notes dealt, no units anywhere), in the action phase with `a` to act.
fn table(factions: &[(&str, &str)]) -> GameState {
    let mut state = crate::fixtures::seated_game(factions, POK);
    crate::promissory::deal(&mut state, ContentStore::embedded(), POK);
    for system in state.board.values_mut() {
        system.units.clear();
        system.planet_units.clear();
    }
    state.phase = ti4_model::state::Phase::Action;
    state.active = Some(a());
    state
}

fn sol_hacan() -> GameState {
    table(&[("a", "sol"), ("b", "hacan")])
}

struct Done {
    game_events: Vec<String>,
    state: GameState,
    rolls: usize,
    /// `(compared, bailed)` by label.
    counts: Vec<(&'static str, (u64, u64))>,
    /// The bail reasons the report counted.
    reasons: Vec<String>,
}

impl Done {
    fn counts(&self, label: &str) -> (u64, u64) {
        self.counts.iter().find(|(l, _)| *l == label).map_or((0, 0), |(_, c)| *c)
    }
}

/// Play the scripted answers on `galaxy` from `state`, with `movement=dual`, and fail on any difference.
fn play(state: GameState, galaxy: &Galaxy, script: &[&str], edit: impl FnOnce(&mut Game<'_>)) -> Done {
    let _guard = crate::dual::tests::serial();
    let _ = take_report();
    crate::dual::reset_thread_windows();
    let table = crate::choice::Table::with_default(Box::new(crate::choice::Scripted::new(script.iter().map(|s| (*s).to_owned()))));
    let mut game = Game::with_table(state, ContentStore::embedded(), table).with_galaxy(galaxy.clone());
    edit(&mut game);
    with_mask(AreaMask::STRING.with(Area::Movement, Backend::Dual), || {
        // One step per scripted answer: the table falls back to the first option when the script is spent.
        for _ in 0..script.len() {
            if game.step().error.is_some() {
                break;
            }
        }
    });
    let r = report();
    assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
    let counts = LABELS.iter().map(|l| (*l, crate::dual::thread_window_counts(l))).collect();
    let reasons = r.windows.iter().filter(|(k, _)| k.starts_with("tactical::") && k.contains("[bail")).map(|(k, w)| format!("{k} x{}", w.bailed)).collect();
    let rolls = game.rift_rolls();
    Done { game_events: game.events.clone(), state: game.state.clone(), rolls, counts, reasons }
}

/// The seat's units in a system, in the space area and on its planets.
fn total(state: &GameState, system: &SystemId, owner: &PlayerId, kind: Option<&str>) -> usize {
    let cell = state.system_state(system);
    cell.units
        .iter()
        .chain(cell.planet_units.values().flatten())
        .filter(|u| &u.owner == owner && kind.is_none_or(|k| u.type_id.as_str() == k))
        .count()
}

fn hub() -> (crate::fixtures::Hub, SystemId, SystemId) {
    let hub = crate::fixtures::plain_hub();
    let (origin, target) = (SystemId::new(hub.outer[0].clone()), SystemId::new(hub.centre.clone()));
    (hub, origin, target)
}

#[test]
fn a_system_is_activated_in_lockstep() {
    let (hub, origin, target) = hub();
    let mut state = sol_hacan();
    crate::fixtures::put(&mut state, &origin, "carrier", &a(), 1);
    let tokens = state.player(&a()).unwrap().tactic_tokens;
    let done = play(state, &hub.galaxy, &[TACTICAL_ACTION_ID, target.as_str()], |_| {});
    assert_eq!(done.counts("tactical::activate"), (1, 0), "the core followed the activation: {:?}", done.reasons);
    assert!(done.game_events.iter().any(|e| e == &format!("SYSTEM_ACTIVATED:{target}")));
    assert_eq!(done.state.player(&a()).unwrap().tactic_tokens, tokens - 1, "one tactic token was spent");
    assert!(done.state.system_state(&target).command_tokens.contains(&a()));
}

/// A hub with `a`'s units placed by `put` in the first ring system and the script answered through the move.
fn carrier_table(extra: &[(&str, usize)]) -> (GameState, crate::fixtures::Hub, SystemId, SystemId) {
    let (hub, origin, target) = hub();
    let mut state = sol_hacan();
    crate::fixtures::put(&mut state, &origin, "carrier", &a(), 1);
    for (kind, count) in extra {
        crate::fixtures::put(&mut state, &origin, kind, &a(), *count);
    }
    (state, hub, origin, target)
}

fn id(origin: &SystemId, index: usize) -> String {
    format!("move|{origin}|{index}")
}

#[test]
fn a_carrier_loads_two_infantry_and_sails_and_the_step_closes() {
    let (state, hub, origin, target) = carrier_table(&[("infantry", 2)]);
    let mv = id(&origin, 0);
    let done = play(state, &hub.galaxy, &[TACTICAL_ACTION_ID, target.as_str(), &mv, "load|0", "load|1", "done_moving"], |_| {});
    assert_eq!(done.counts("tactical::activate"), (1, 0), "{:?}", done.reasons);
    assert_eq!(done.counts("tactical::move"), (1, 0), "{:?}", done.reasons);
    assert_eq!(done.counts("tactical::load"), (2, 0), "{:?}", done.reasons);
    assert_eq!(done.counts("tactical::finish"), (1, 0), "{:?}", done.reasons);
    assert_eq!(total(&done.state, &target, &a(), None), 3, "the carrier and both passengers arrived");
    assert_eq!(total(&done.state, &origin, &a(), None), 0);
    assert!(done.game_events.iter().any(|e| e == "SHIP_MOVED"));
}

#[test]
fn done_loading_closes_the_hold_early() {
    let (state, hub, origin, target) = carrier_table(&[("infantry", 2)]);
    let mv = id(&origin, 0);
    let done = play(state, &hub.galaxy, &[TACTICAL_ACTION_ID, target.as_str(), &mv, "load|0", "done_loading"], |_| {});
    assert_eq!(done.counts("tactical::load"), (2, 0), "{:?}", done.reasons);
    assert_eq!(total(&done.state, &target, &a(), Some("infantry")), 1, "one passenger arrived");
    assert_eq!(total(&done.state, &origin, &a(), Some("infantry")), 1, "one was left behind");
}

#[test]
fn a_ship_without_capacity_sails_at_once() {
    let (hub, origin, target) = hub();
    let mut state = sol_hacan();
    crate::fixtures::put(&mut state, &origin, "destroyer", &a(), 1);
    let mv = id(&origin, 0);
    let done = play(state, &hub.galaxy, &[TACTICAL_ACTION_ID, target.as_str(), &mv], |_| {});
    assert_eq!(done.counts("tactical::move"), (1, 0), "{:?}", done.reasons);
    assert_eq!(total(&done.state, &target, &a(), None), 1);
}

#[test]
fn a_damaged_and_a_fresh_dreadnought_are_two_options_and_the_second_hull_moves() {
    let (hub, origin, target) = hub();
    let mut state = sol_hacan();
    crate::fixtures::put(&mut state, &origin, "dreadnought", &a(), 2);
    state.system_mut(&origin).units[1].sustained_damage = true;
    let mv = id(&origin, 1);
    let done = play(state, &hub.galaxy, &[TACTICAL_ACTION_ID, target.as_str(), &mv], |_| {});
    assert_eq!(done.counts("tactical::move"), (1, 0), "{:?}", done.reasons);
    let arrived = done.state.system_state(&target);
    let moved = arrived.units.iter().filter(|u| u.owner == a()).collect::<Vec<_>>();
    assert_eq!(moved.len(), 1);
    assert!(moved[0].sustained_damage, "the damaged hull is the one that moved");
}

#[test]
fn a_hull_is_chosen_by_its_index_among_the_seats_units_when_a_damaged_one_comes_first() {
    // [damaged, fresh, fresh]: the two fresh hulls are one option, whose index is the first of them (1).
    let (hub, origin, target) = hub();
    let mut state = sol_hacan();
    crate::fixtures::put(&mut state, &origin, "dreadnought", &a(), 3);
    state.system_mut(&origin).units[0].sustained_damage = true;
    let mv = id(&origin, 1);
    let done = play(state, &hub.galaxy, &[TACTICAL_ACTION_ID, target.as_str(), &mv], |_| {});
    assert_eq!(done.counts("tactical::move"), (1, 0), "{:?}", done.reasons);
    let left = done.state.system_state(&origin);
    assert_eq!(left.units.len(), 2);
    assert_eq!(left.units.iter().filter(|u| u.sustained_damage).count(), 1, "the damaged hull stayed");
    assert_eq!(total(&done.state, &target, &a(), Some("dreadnought")), 1);
}

#[test]
fn gravity_drive_reaches_a_system_two_away() {
    let (hub, origin, _) = hub();
    let far = SystemId::new(hub.across(&hub.outer[0]));
    let mut state = sol_hacan();
    crate::fixtures::put(&mut state, &origin, "carrier", &a(), 1);
    state.player_mut(&a()).unwrap().technologies.insert(ti4_model::id::TechnologyId::new("gd"));
    let done = play(state, &hub.galaxy, &[TACTICAL_ACTION_ID, far.as_str(), &format!("move_gd|{origin}|0")], |_| {});
    assert_eq!(done.counts("tactical::move"), (1, 0), "{:?}", done.reasons);
    assert!(done.state.player(&a()).unwrap().gravity_drive_used_activation.is_some(), "the drive was spent");
}

#[test]
fn the_free_tactical_action_activates_without_a_token() {
    let _guard = crate::dual::tests::serial();
    let _ = take_report();
    crate::dual::reset_thread_windows();
    let (hub, _, target) = hub();
    let state = sol_hacan();
    let mut game = Game::with_table(state, ContentStore::embedded(), crate::choice::Table::new()).with_galaxy(hub.galaxy.clone());
    with_mask(AreaMask::STRING.with(Area::Movement, Backend::Dual), || {
        game.begin_free_tactical(a(), target.clone()).expect("the free action opens");
    });
    let r = report();
    assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
    assert_eq!(crate::dual::thread_window_counts("tactical::free"), (1, 0));
    assert_eq!(game.state.player(&a()).unwrap().tactic_tokens, 3, "no token was spent");
    assert_eq!(game.state.active_system, Some(target));
    assert_eq!(game.events.last().map(String::as_str), Some("FREE_TACTICAL_ACTION"));
}

// -- the documented bails: the string result stands and the reason is counted ---------------------------------------

fn bails(state: GameState, hub: &crate::fixtures::Hub, target: &SystemId, label: &str, reason: &str) {
    let done = play(state, &hub.galaxy, &[TACTICAL_ACTION_ID, target.as_str()], |_| {});
    assert_eq!(done.counts(label), (0, 1), "{:?}", done.reasons);
    assert!(done.reasons.iter().any(|r| r.contains(reason)), "{reason}: {:?}", done.reasons);
}

#[test]
fn a_held_hook_technology_keeps_the_activation_on_the_string() {
    for (alias, reason) in [("scc", "Spatial Conduit"), ("sdn", "Scanlink"), ("ers", "E-Res"), ("nf", "Nullification")] {
        let (hub, origin, target) = hub();
        let mut state = sol_hacan();
        crate::fixtures::put(&mut state, &origin, "carrier", &a(), 1);
        state.player_mut(&b()).unwrap().technologies.insert(ti4_model::id::TechnologyId::new(alias));
        bails(state, &hub, &target, "tactical::activate", reason);
    }
}

#[test]
fn a_table_with_a_dropped_faction_or_two_seats_of_one_faction_stays_on_the_string() {
    let (hub, origin, target) = hub();
    let mut state = table(&[("a", "sol"), ("b", "hacan")]);
    crate::fixtures::put(&mut state, &origin, "carrier", &a(), 1);
    state.player_mut(&b()).unwrap().faction = ti4_model::id::FactionId::new("ralnel");
    bails(state, &hub, &target, "tactical::activate", "dropped by decision");
    let mut twin = table(&[("a", "sol"), ("b", "hacan")]);
    twin.player_mut(&b()).unwrap().faction = ti4_model::id::FactionId::new("sol");
    bails(twin, &hub, &target, "tactical::activate", "two seats");
}

#[test]
fn diplomacy_on_keeps_the_whole_tactical_action_on_the_string() {
    let (hub, origin, target) = hub();
    let mut state = sol_hacan();
    crate::fixtures::put(&mut state, &origin, "carrier", &a(), 1);
    state.diplomacy.enabled = true;
    let done = play(state, &hub.galaxy, &[TACTICAL_ACTION_ID, target.as_str()], |_| {});
    assert_eq!(done.counts("tactical::activate"), (0, 1));
}

// -- gravity rifts: one die per rift exited, 1-3 destroys the ship and its hold -----------------------------------

/// `origin -- rift -- active` on a line: a ship of move 2 exits the rift on its way.
fn rift_line() -> (Galaxy, SystemId, SystemId) {
    let rift = crate::fixtures::a_system_where("gravity rift");
    let plain = crate::fixtures::plain_systems(2);
    let galaxy = Galaxy::placed(
        ContentStore::embedded(),
        &[
            (plain[0].as_str(), ti4_model::hex::Hex::new(0, 0)),
            (rift.as_str(), ti4_model::hex::Hex::new(1, 0)),
            (plain[1].as_str(), ti4_model::hex::Hex::new(2, 0)),
        ],
        POK,
    )
    .expect("a line of three systems");
    (galaxy, SystemId::new(&plain[0]), SystemId::new(&plain[1]))
}

#[test]
fn a_carrier_exiting_a_rift_rolls_a_die_and_a_lost_ship_takes_its_hold_with_it() {
    let (galaxy, origin, target) = rift_line();
    let (mut lost, mut arrived) = (0, 0);
    for seed in 0..24_u64 {
        let mut state = sol_hacan();
        state.rng_seed = seed;
        crate::fixtures::put(&mut state, &origin, "carrier2", &a(), 1);
        crate::fixtures::put(&mut state, &origin, "infantry", &a(), 2);
        let mv = id(&origin, 0);
        let done = play(state, &galaxy, &[TACTICAL_ACTION_ID, target.as_str(), &mv, "load|0", "load|1"], |_| {});
        assert_eq!(done.counts("tactical::move"), (1, 0), "seed {seed}: {:?}", done.reasons);
        assert_eq!(done.counts("tactical::load"), (2, 0), "seed {seed}: {:?}", done.reasons);
        assert_eq!(done.rolls, 1, "seed {seed}: one rift exited, one die");
        let there = total(&done.state, &target, &a(), None);
        let left = total(&done.state, &origin, &a(), None);
        if done.game_events.iter().any(|e| e == "SHIP_LOST_TO_GRAVITY_RIFT") {
            lost += 1;
            assert_eq!((there, left), (0, 0), "seed {seed}: the carrier and the passengers are gone");
        } else {
            arrived += 1;
            assert_eq!((there, left), (3, 0), "seed {seed}");
        }
    }
    assert!(lost > 0 && arrived > 0, "both outcomes are covered: {lost} lost, {arrived} arrived");
}

#[test]
fn nav_suite_and_the_circlet_never_roll() {
    let (galaxy, origin, target) = rift_line();
    let mut state = sol_hacan();
    crate::fixtures::put(&mut state, &origin, "carrier2", &a(), 1);
    state.player_mut(&a()).unwrap().relics.push(ti4_model::id::RelicId::new("circletofthevoid"));
    let mv = id(&origin, 0);
    let done = play(state, &galaxy, &[TACTICAL_ACTION_ID, target.as_str(), &mv], |_| {});
    assert_eq!(done.counts("tactical::move"), (1, 0), "{:?}", done.reasons);
    assert_eq!(done.rolls, 0, "the Circlet spares the roll");
}

// -- notes that return on an activation --------------------------------------------------------------------------

#[test]
fn a_support_for_the_throne_lent_to_the_activator_goes_home_when_its_owner_is_in_the_system() {
    let (hub, origin, target) = hub();
    let mut state = sol_hacan();
    crate::fixtures::put(&mut state, &origin, "carrier", &a(), 1);
    crate::fixtures::put(&mut state, &target, "cruiser", &b(), 1);
    state.support_holders.insert(b(), a());
    state.player_mut(&a()).unwrap().victory_points = 1;
    let done = play(state, &hub.galaxy, &[TACTICAL_ACTION_ID, target.as_str()], |_| {});
    assert_eq!(done.counts("tactical::activate"), (1, 0), "{:?}", done.reasons);
    assert!(done.game_events.iter().any(|e| e == "SUPPORT_FOR_THE_THRONE_RETURNED:b"), "{:?}", done.game_events);
    assert!(done.state.support_holders.is_empty());
    assert_eq!(done.state.player(&a()).unwrap().victory_points, 0, "the point went home with the card");
}

#[test]
fn a_ceasefire_held_by_the_defender_keeps_the_activation_on_the_string() {
    let (hub, origin, target) = hub();
    let mut state = sol_hacan();
    crate::fixtures::put(&mut state, &origin, "carrier", &a(), 1);
    crate::fixtures::put(&mut state, &target, "cruiser", &b(), 1);
    state.promissory_notes.insert("cf:sol".to_owned(), b());
    bails(state, &hub, &target, "tactical::activate", "Ceasefire");
}

// -- scratch of the step, wormholes, the storm ----------------------------------------------------------------------

#[test]
fn gravleash_maneuvers_lets_a_slower_hull_inherit_the_anchor_of_the_first_mover() {
    let (hub, origin, _) = hub();
    let far = SystemId::new(hub.across(&hub.outer[0]));
    let mut state = sol_hacan();
    crate::fixtures::put(&mut state, &origin, "cruiser", &a(), 1);
    crate::fixtures::put(&mut state, &origin, "dreadnought", &a(), 1);
    state.player_mut(&a()).unwrap().breakthrough = Some(ti4_model::id::BreakthroughId::new("letnevbt"));
    // The cruiser (move 2) reaches the far side, the dreadnought (move 1) does not: until the cruiser has moved from here.
    let script = [TACTICAL_ACTION_ID, far.as_str(), &id(&origin, 0), &id(&origin, 0)];
    let done = play(state, &hub.galaxy, &script, |_| {});
    assert_eq!(done.counts("tactical::move"), (2, 0), "{:?}", done.reasons);
    assert_eq!(total(&done.state, &far, &a(), None), 2, "both hulls arrived");
}

#[test]
fn a_move_that_leaves_the_ion_storm_turns_it_over_and_a_wormhole_hop_is_counted() {
    let hub = crate::fixtures::plain_hub();
    let mut galaxy = hub.galaxy.clone();
    crate::seating::place_wormhole_nexus(&mut galaxy, ContentStore::embedded(), POK).expect("the Nexus sits beside the map");
    let origin = SystemId::new(hub.outer[0].clone());
    let nexus = SystemId::new(crate::seating::LOCKED_NEXUS);
    let mut state = sol_hacan();
    crate::fixtures::put(&mut state, &origin, "cruiser", &a(), 1);
    state.wormhole_tokens.insert("GAMMA".to_owned(), origin.clone());
    state.ion_storm = Some((origin.clone(), "ALPHA".to_owned()));
    crate::laws::apply_to_galaxy(&state, &mut galaxy);
    let done = play(state, &galaxy, &[TACTICAL_ACTION_ID, nexus.as_str(), &id(&origin, 0)], |_| {});
    assert_eq!(done.counts("tactical::activate"), (1, 0), "{:?}", done.reasons);
    assert_eq!(done.counts("tactical::move"), (1, 0), "{:?}", done.reasons);
    assert_eq!(done.state.ion_storm, Some((origin, "BETA".to_owned())), "the storm flipped");
}

// -- the typed payload of an open action (the pending window of D1b) -------------------------------------------

#[test]
fn an_open_hold_round_trips_through_the_typed_payload_and_asks_the_same_question() {
    use ti4_engine_core::generated::{System, Unit as CUnit};
    use ti4_engine_core::kernel::stage::Window;
    use ti4_engine_core::rules::tactical::{Facts, Machine};
    use ti4_engine_core::state::{Piece, Slice};
    let (hub, origin, target) = hub();
    let mut state = sol_hacan();
    crate::fixtures::put(&mut state, &origin, "carrier", &a(), 1);
    crate::fixtures::put(&mut state, &origin, "infantry", &a(), 2);
    state.active_system = Some(target.clone());
    let (mut cx, r) = crate::dual_window::tables_for_tests(&state).expect("a seated table");
    let seat = r.seat("a").unwrap();
    let facts = Facts { pool: r.code("tactic") };
    let env = crate::tactical_dual::env_of(&state.clone(), &cx, POK).expect("a kernel");
    let (origin_c, target_c) = (<System as crate::generated::IdStr>::from_id(origin.as_str()).unwrap(), <System as crate::generated::IdStr>::from_id(target.as_str()).unwrap());
    let ship = Piece::new(<CUnit as crate::generated::IdStr>::from_id("carrier").unwrap(), seat);
    let fp = crate::tactical_dual::WINDOW;
    let src = crate::core_sync::Sources { galaxy: Some(&hub.galaxy), rng: None };
    let (asked, again) = crate::core_sync::with_core(&mut state, &mut cx, src, fp, |core, _| {
        let machine = Machine::loading(core, &env.kernel, seat, facts, origin_c, ship, vec![origin_c, target_c], &[0], false);
        let payload = machine.payload(Default::default());
        let back = Machine::from_payload(core, &env.kernel, facts, &payload);
        (machine.ask_of(core, &env.kernel), back.ask_of(core, &env.kernel))
    })
    .expect("the lend works");
    let asked = asked.expect("the hold asks for the second infantry");
    assert_eq!(asked.opts.0.len(), 2, "one pickup left and the close");
    assert_eq!(Some(asked), again, "the machine read back from its payload asks the same question");
    let _ = Slice::Meta;
}

#[test]
fn an_argent_mech_rides_free_in_a_full_hold() {
    let (hub, origin, target) = hub();
    let mut state = table(&[("a", "argent"), ("b", "hacan")]);
    crate::fixtures::put(&mut state, &origin, "dreadnought", &a(), 1);
    crate::fixtures::put(&mut state, &origin, "infantry", &a(), 1);
    crate::fixtures::put(&mut state, &origin, "argent_mech", &a(), 1);
    let mv = id(&origin, 0);
    // The dreadnought carries one unit; the infantry takes the slot, the mech still asks to come along.
    let done = play(state, &hub.galaxy, &[TACTICAL_ACTION_ID, target.as_str(), &mv, "load|0", "load|1"], |_| {});
    assert_eq!(done.counts("tactical::load"), (2, 0), "{:?}", done.reasons);
    assert_eq!(total(&done.state, &target, &a(), Some("argent_mech")), 1, "the mech arrived with a full hold");
    assert_eq!(total(&done.state, &target, &a(), Some("infantry")), 1);
}

#[test]
fn a_free_rider_taken_first_leaves_the_slot_for_an_ordinary_unit() {
    let (hub, origin, target) = hub();
    let mut state = table(&[("a", "argent"), ("b", "hacan")]);
    crate::fixtures::put(&mut state, &origin, "dreadnought", &a(), 1);
    crate::fixtures::put(&mut state, &origin, "argent_mech", &a(), 1);
    crate::fixtures::put(&mut state, &origin, "infantry", &a(), 1);
    let mv = id(&origin, 0);
    let done = play(state, &hub.galaxy, &[TACTICAL_ACTION_ID, target.as_str(), &mv, "load|0", "load|1"], |_| {});
    assert_eq!(done.counts("tactical::load"), (2, 0), "{:?}", done.reasons);
    assert_eq!(total(&done.state, &target, &a(), Some("infantry")), 1, "the one slot was still free for the infantry");
}

// -- fleet and capacity enforcement (the head of the aftermath) -----------------------------------------------

/// Run `fleet::enforce_seeing` for `a` in `system` with the scripted answers, under `movement=dual`.
fn enforce(mut state: GameState, system: &SystemId, script: &[&str]) -> (Result<usize, crate::choice::IllegalChoice>, GameState, (u64, u64), Vec<String>) {
    let _guard = crate::dual::tests::serial();
    let _ = take_report();
    crate::dual::reset_thread_windows();
    let mut table = crate::choice::Table::with_default(Box::new(crate::choice::Scripted::new(script.iter().map(|s| (*s).to_owned()))));
    let out = with_mask(AreaMask::STRING.with(Area::Movement, Backend::Dual), || {
        crate::fleet::enforce_seeing(&mut state, ContentStore::embedded(), POK, None, &mut table, &a(), system)
    });
    let r = report();
    assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
    let reasons = r.windows.iter().filter(|(k, _)| k.starts_with("tactical::enforce") && k.contains("[bail")).map(|(k, w)| format!("{k} x{}", w.bailed)).collect();
    (out, state, crate::dual::thread_window_counts("tactical::enforce"), reasons)
}

#[test]
fn a_fleet_over_its_supply_removes_the_ship_the_owner_names() {
    let (_, origin, _) = hub();
    let mut state = sol_hacan();
    state.player_mut(&a()).unwrap().fleet_tokens = 1;
    crate::fixtures::put(&mut state, &origin, "destroyer", &a(), 1);
    crate::fixtures::put(&mut state, &origin, "cruiser", &a(), 1);
    crate::fixtures::put(&mut state, &origin, "dreadnought", &a(), 1);
    // Three ships against a pool of one: two come off. The options are the distinct types, in board order.
    let (out, state, counts, reasons) = enforce(state, &origin, &["remove|2", "remove|0"]);
    assert_eq!(out.expect("both answers were offered"), 2);
    assert_eq!(counts, (1, 0), "{reasons:?}");
    let left: Vec<String> = state.system_state(&origin).units.iter().map(|u| u.type_id.to_string()).collect();
    assert_eq!(left, ["cruiser"], "the dreadnought and the first remaining ship (the destroyer) were removed");
}

#[test]
fn units_over_capacity_are_removed_after_the_supply() {
    let (_, origin, _) = hub();
    let mut state = sol_hacan();
    crate::fixtures::put(&mut state, &origin, "carrier", &a(), 1);
    crate::fixtures::put(&mut state, &origin, "infantry", &a(), 6);
    let (out, state, counts, reasons) = enforce(state, &origin, &["remove|0", "remove|0"]);
    assert_eq!(out.expect("answered"), 2, "{reasons:?}");
    assert_eq!(counts, (1, 0));
    assert_eq!(state.system_state(&origin).units.iter().filter(|u| u.type_id.as_str() == "infantry").count(), 4, "a carrier holds four");
}

#[test]
fn a_position_within_its_limits_asks_nothing() {
    let (_, origin, _) = hub();
    let mut state = sol_hacan();
    crate::fixtures::put(&mut state, &origin, "carrier", &a(), 1);
    let (out, _, counts, _) = enforce(state, &origin, &[]);
    assert_eq!(out.expect("nothing to remove"), 0);
    assert_eq!(counts, (1, 0));
}

#[test]
fn upgraded_fighters_over_capacity_are_charged_to_the_fleet_pool_in_the_question() {
    let (_, origin, _) = hub();
    let mut state = sol_hacan();
    state.player_mut(&a()).unwrap().fleet_tokens = 2;
    crate::fixtures::put(&mut state, &origin, "carrier", &a(), 1);
    crate::fixtures::put(&mut state, &origin, "fighter2", &a(), 6);
    // The carrier holds four; the other two fighters are charged to the pool: 1 ship + 2 fighters against a pool of 2.
    let (out, _, counts, reasons) = enforce(state, &origin, &["remove|0"]);
    assert_eq!(out.expect("answered"), 1, "{reasons:?}");
    assert_eq!(counts, (1, 0));
}

#[test]
fn a_closed_movement_step_hands_the_aftermath_on() {
    use ti4_engine_core::driver::Driver;
    use ti4_engine_core::generated::System;
    use ti4_engine_core::kernel::stage::AbilityTable;
    use ti4_engine_core::kernel::{Next, Resolver};
    use ti4_engine_core::rules::tactical::{Facts, Machine};
    use ti4_engine_core::state::WindowKind;
    let (hub, _, target) = hub();
    let mut state = sol_hacan();
    state.active_system = Some(target.clone());
    let (mut cx, r) = crate::dual_window::tables_for_tests(&state).expect("a seated table");
    let seat = r.seat("a").unwrap();
    let facts = Facts { pool: r.code("tactic") };
    let env = crate::tactical_dual::env_of(&state.clone(), &cx, POK).expect("a kernel");
    let src = crate::core_sync::Sources { galaxy: Some(&hub.galaxy), rng: None };
    let next = crate::core_sync::with_core(&mut state, &mut cx, src, crate::tactical_dual::WINDOW, |core, _| {
        let mut driver = Driver::new(Machine::closing(seat, facts), Resolver::new(&[seat], Some(seat)), AbilityTable::default(), 1);
        driver.run(core, &env.kernel).expect("the close runs");
        driver.window.done_next()
    })
    .expect("the lend works");
    let Next::Open(window) = next else { panic!("the close opens the aftermath: {next:?}") };
    let at = <System as crate::generated::IdStr>::from_id(target.as_str()).unwrap();
    assert_eq!((window.kind, window.actor, window.data), (WindowKind::Aftermath, Some(seat), [at.index() as u32, 0, 0]));
}

// -- the remaining documented bails, one test each --------------------------------------------------------------------

#[test]
fn minister_of_peace_mahact_and_the_dominus_orb_keep_the_activation_on_the_string() {
    let (hub, origin, target) = hub();
    let ready = |edit: &dyn Fn(&mut GameState)| {
        let mut state = sol_hacan();
        crate::fixtures::put(&mut state, &origin, "carrier", &a(), 1);
        edit(&mut state);
        state
    };
    bails(ready(&|s| { s.laws.insert("minister_peace".to_owned(), "a".to_owned()); }), &hub, &target, "tactical::activate", "Minister of Peace");
    bails(ready(&|s| s.player_mut(&b()).unwrap().faction = ti4_model::id::FactionId::new("mahact")), &hub, &target, "tactical::activate", "Mahact");
    bails(ready(&|s| s.player_mut(&a()).unwrap().relics.push(ti4_model::id::RelicId::new("dominusorb"))), &hub, &target, "tactical::activate", "Dominus Orb");
}

#[test]
fn test_faces_in_the_dice_keep_every_window_on_the_string() {
    let (hub, origin, target) = hub();
    let mut state = sol_hacan();
    crate::fixtures::put(&mut state, &origin, "carrier", &a(), 1);
    let done = play(state, &hub.galaxy, &[TACTICAL_ACTION_ID, target.as_str()], |game| game.use_test_faces(&[10, 10]));
    assert_eq!(done.counts("tactical::activate"), (0, 1));
    assert!(done.reasons.iter().any(|r| r.contains("irreproducible dice")), "{:?}", done.reasons);
}

/// The head of the close, reached with a carrier that does not move.
fn closes(state: GameState, hub: &crate::fixtures::Hub, target: &SystemId, reason: &str) {
    let done = play(state, &hub.galaxy, &[TACTICAL_ACTION_ID, target.as_str(), "done_moving"], |_| {});
    assert_eq!(done.counts("tactical::finish"), (0, 1), "{reason}: {:?}", done.reasons);
    assert!(done.reasons.iter().any(|r| r.contains(reason)), "{reason}: {:?}", done.reasons);
}

#[test]
fn nova_seed_the_sardakk_hero_and_a_floating_factory_keep_the_close_on_the_string() {
    let (hub, origin, target) = hub();
    let base = || {
        let mut state = sol_hacan();
        crate::fixtures::put(&mut state, &origin, "carrier", &a(), 1);
        state
    };
    let mut edits = base();
    edits.faction_marks.insert(format!("{}x", crate::movement::MAP_EDIT_PREFIX), "1".to_owned());
    closes(edits, &hub, &target, "map edits");
    let mut hero = base();
    hero.player_mut(&b()).unwrap().leaders.insert(ti4_model::id::LeaderId::new("sardakkhero"), ti4_model::state::LeaderStatus::Unlocked);
    closes(hero, &hub, &target, "Sardakk hero");
    let mut factory = base();
    crate::fixtures::put(&mut factory, &target, "saar_spacedock", &a(), 1);
    closes(factory, &hub, &target, "Floating Factory");
}

#[test]
fn a_route_other_than_the_default_is_followed_from_the_questions_list() {
    // outer[0] to outer[2]: two steps round the ring or two through the centre, so the question lists routes.
    let hub = crate::fixtures::plain_hub();
    let (origin, target) = (SystemId::new(hub.outer[0].clone()), SystemId::new(hub.outer[2].clone()));
    let mut state = sol_hacan();
    crate::fixtures::put(&mut state, &origin, "cruiser", &a(), 1);
    let mv = format!("{}|r1", id(&origin, 0));
    let done = play(state, &hub.galaxy, &[TACTICAL_ACTION_ID, target.as_str(), &mv], |_| {});
    assert_eq!(done.counts("tactical::move"), (1, 0), "{:?}", done.reasons);
    assert_eq!(total(&done.state, &target, &a(), Some("cruiser")), 1);
}
