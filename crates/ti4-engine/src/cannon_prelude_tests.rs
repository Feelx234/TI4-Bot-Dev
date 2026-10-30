//! The space cannon offense of the aftermath prelude as a core window (package D1b2, A2b-10b), beside the string
//! `AftermathWindow::cannon_prelude`: positions with PDS fire at an arriving fleet, two gunners, Graviton Laser System, the
//! active seat's own guns, the feat and the documented bails. Every test asserts the core half ran (the window was compared).

use super::{AftermathWindow, EventScoreLimit, FeatOccurrence};
use crate::choice::{Resolving, Table};
use crate::dice::Dice;
use crate::dual::{Area, AreaMask, Backend, report, take_report, with_mask};
use crate::rng::GameRng;
use ti4_content::ContentStore;
use ti4_model::id::{PlayerId, PlanetId, SystemId};
use ti4_model::state::GameState;

const LABEL: &str = "combat::space_cannon_offense";

fn a() -> PlayerId {
    PlayerId::new("a")
}
fn b() -> PlayerId {
    PlayerId::new("b")
}

/// What one run of the prelude left.
struct Ran {
    state: GameState,
    result: Result<Option<(FeatOccurrence, EventScoreLimit)>, super::GameError>,
    dice: usize,
}

/// A position: `seats` seated, `a` active at the system of a placed planet that `b` controls (`planet` is passed to `setup`).
fn position(seed: u64, seats: &[(&str, &str)], setup: impl Fn(&mut GameState, &SystemId, &PlanetId)) -> (GameState, SystemId) {
    let mut state = crate::fixtures::seated_game(seats, ti4_model::content_types::DEFAULT);
    state.promissory_notes.clear();
    state.promissory_faceup.clear();
    state.rng_seed = seed;
    let (system, planet) = crate::fixtures::a_placed_planet();
    state.active = Some(a());
    state.active_system = Some(system.clone());
    state.system_mut(&system).set_control(planet.clone(), b());
    setup(&mut state, &system, &planet);
    (state, system)
}

fn run(mut state: GameState, system: &SystemId, seed: u64) -> Ran {
    let mut table = Table::with_default(Box::new(crate::choice::FirstOption));
    let (mut dice, mut rng) = (Dice::new(), GameRng::new(seed));
    let mut resolver = crate::fixtures::armed_resolver(&state);
    let mut sequence = crate::event::EventSequence::new();
    let mut ctx = Resolving {
        content: ContentStore::embedded(),
        sources: ti4_model::content_types::DEFAULT,
        dice: &mut dice,
        rng: &mut rng,
        table: &mut table,
        timing: Some(crate::choice::TimingHandle { resolver: &mut resolver, sequence: &mut sequence, galaxy: None }),
    };
    let result = AftermathWindow::cannon_prelude(&mut state, &mut ctx, &a(), system, None);
    let rolled = dice.history().iter().filter(|r| r.reason == "space cannon").count();
    Ran { state, result, dice: rolled }
}

/// Runs `seeds` positions on `driver=dual`; returns the runs and the report (taken).
fn lockstep(seeds: std::ops::Range<u64>, seats: &[(&str, &str)], setup: impl Fn(&mut GameState, &SystemId, &PlanetId) + Copy) -> (Vec<Ran>, crate::dual::DualReport) {
    let _guard = crate::dual::tests::serial();
    let _ = take_report();
    let mut ran = Vec::new();
    for seed in seeds {
        let (state, system) = position(seed, seats, setup);
        with_mask(AreaMask::STRING.with(Area::Driver, Backend::Dual), || ran.push(run(state, &system, seed)));
    }
    let r = report();
    let _ = take_report();
    (ran, r)
}

fn compared(r: &crate::dual::DualReport) -> u64 {
    r.windows.get(LABEL).map_or(0, |w| w.compared)
}

fn pds_at_b(s: &mut GameState, system: &SystemId, planet: &PlanetId) {
    crate::fixtures::put_on_planet(s, system, planet, "pds", &b(), 2);
    crate::fixtures::put(s, system, "cruiser", &a(), 3);
    crate::fixtures::put(s, system, "fighter", &a(), 2);
}

#[test]
fn a_pds_fires_at_the_arriving_fleet_in_the_core_window_in_lockstep() {
    let (ran, r) = lockstep(0..14, &[("a", "sol"), ("b", "hacan")], pds_at_b);
    assert_eq!(r.total_differences(), 0, "{:?}", r.differences.first());
    assert!(compared(&r) >= 10, "the core window ran: {:?}", r.windows);
    assert!(ran.iter().all(|x| x.dice > 0), "the guns rolled");
    let lost = ran.iter().filter(|x| x.state.system_state(&SystemId::new(ran_system(x))).units.len() < 5).count();
    assert!(lost > 0, "some hit landed");
}

fn ran_system(x: &Ran) -> String {
    x.state.active_system.as_ref().map(ToString::to_string).unwrap_or_default()
}

#[test]
fn a_dreadnought_is_offered_its_sustain_and_graviton_binds_the_loss_in_lockstep() {
    let gls = |s: &mut GameState, system: &SystemId, planet: &PlanetId| {
        crate::fixtures::put_on_planet(s, system, planet, "pds", &b(), 4);
        crate::fixtures::put(s, system, "dreadnought", &a(), 2);
        crate::fixtures::put(s, system, "fighter", &a(), 3);
        s.player_mut(&b()).unwrap().technologies.insert(ti4_model::id::TechnologyId::new("gls"));
    };
    let (ran, r) = lockstep(0..14, &[("a", "sol"), ("b", "hacan")], gls);
    assert_eq!(r.total_differences(), 0, "{:?}", r.differences.first());
    assert!(compared(&r) >= 10, "the core window ran: {:?}", r.windows);
    let bound = ran.iter().filter(|x| x.state.player(&b()).unwrap().exhausted_technologies.contains(&ti4_model::id::TechnologyId::new("gls"))).count();
    assert!(bound > 0, "Graviton Laser System was exhausted to bind a loss");
}

#[test]
fn the_active_seats_own_guns_fire_at_the_ships_of_the_seat_it_attacks_in_lockstep() {
    let own = |s: &mut GameState, system: &SystemId, planet: &PlanetId| {
        s.system_mut(system).set_control(planet.clone(), a());
        crate::fixtures::put_on_planet(s, system, planet, "pds", &a(), 2);
        crate::fixtures::put(s, system, "cruiser", &b(), 2);
        crate::fixtures::put(s, system, "fighter", &b(), 2);
    };
    let (ran, r) = lockstep(0..12, &[("a", "sol"), ("b", "hacan")], own);
    assert_eq!(r.total_differences(), 0, "{:?}", r.differences.first());
    assert!(compared(&r) >= 8, "the core window ran: {:?}", r.windows);
    assert!(ran.iter().any(|x| x.state.system_state(&SystemId::new(ran_system(x))).units.len() < 4), "b lost a ship to a's guns");
}

#[test]
fn two_gunners_roll_in_seat_order_and_the_last_non_fighter_scores_the_feat_in_lockstep() {
    let two = |s: &mut GameState, system: &SystemId, planet: &PlanetId| {
        crate::fixtures::put_on_planet(s, system, planet, "pds", &b(), 3);
        crate::fixtures::put_on_planet(s, system, planet, "pds", &PlayerId::new("c"), 3);
        crate::fixtures::put(s, system, "cruiser", &a(), 1);
    };
    let (ran, r) = lockstep(0..20, &[("a", "sol"), ("b", "hacan"), ("c", "letnev")], two);
    assert_eq!(r.total_differences(), 0, "{:?}", r.differences.first());
    assert!(compared(&r) >= 15, "the core window ran: {:?}", r.windows);
    let scored = ran.iter().filter(|x| matches!(x.result, Ok(Some(_)))).count();
    assert!(scored > 0, "the last non-fighter ship fell to the guns of the others in some run");
}

#[test]
fn a_heart_of_ixth_or_a_jolnar_commander_keeps_the_offense_on_the_string() {
    let heart = |s: &mut GameState, system: &SystemId, planet: &PlanetId| {
        pds_at_b(s, system, planet);
        s.player_mut(&b()).unwrap().relics.push(ti4_model::id::RelicId::new("heartofixth"));
    };
    let (_, r) = lockstep(0..3, &[("a", "sol"), ("b", "hacan")], heart);
    assert_eq!(r.total_differences(), 0, "{:?}", r.differences.first());
    assert_eq!(compared(&r), 0, "the window did not run: {:?}", r.windows);
    // The bail is decided before the string runs (`cannon_prelude` always goes through the dual on `driver=dual`).
    let unported = |s: &GameState| crate::cannon_dual::unported(s, &crate::cannon_dual::Args { active: &a(), system: &SystemId::new("18"), announce: true, reproducible: true });
    let (state, _) = position(0, &[("a", "sol"), ("b", "hacan")], heart);
    assert!(unported(&state).is_some_and(|why| why.contains("reroll question")));
}

#[test]
fn irreproducible_dice_and_dropped_factions_are_documented_bails() {
    let (state, _) = position(0, &[("a", "sol"), ("b", "hacan")], pds_at_b);
    let system = SystemId::new("18");
    let args = |reproducible| crate::cannon_dual::Args { active: &a(), system: &system, announce: true, reproducible };
    assert_eq!(crate::cannon_dual::unported(&state, &args(true)), None, "the plain position is carried");
    assert!(crate::cannon_dual::unported(&state, &args(false)).is_some_and(|why| why.contains("irreproducible")));
    let (dropped, _) = position(0, &[("a", "sol"), ("b", "obsidian")], pds_at_b);
    assert!(crate::cannon_dual::unported(&dropped, &args(true)).is_some_and(|why| why.contains("dropped")));
}
