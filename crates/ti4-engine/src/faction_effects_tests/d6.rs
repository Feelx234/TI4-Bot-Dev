//! Depth tests, group d6: Crimson Rebellion, Last Bastion, Firmament / Obsidian, Deepwrought
//! Scholarate (see mod.rs and docs/FACTION_EFFECT_COVERAGE.md). Each test drives the engine's own
//! flow (a `Game` with a scripted table, a whole space combat or invasion with loaded dice, a use of
//! PRODUCTION, the status phase) and asserts what the card promises, plus the opposite case where
//! cheap.
//!
//! Test names: `<card id>_<effect no.>_<what>`; unit tests `unit_<unit id>_<n>_<what>`.
//!
//! Obsidian cannot be seated at setup (Nocturne), so its tests seat the Firmament and flip the seat
//! with the real "Puppets of the Blade" action (`firmament_flip`), which is also how the Obsidian
//! comes into a game.

use ti4_content::galaxy::Galaxy;
use ti4_model::content_types::DEFAULT;
use ti4_model::id::{PlanetId, PlayerId, SystemId};
use ti4_model::state::{GameState, LeaderStatus, Phase};

use super::d2::{
    Pick, Seen, action_game, eager_game, first_roll, leader_status, on_planet, padded_hits,
    play_status_phase, run_status_phase, score_objectives, set_leader, ships, space_fight,
    step_until, unlocks_in_the_game, was_offered, was_offered_like,
};
use super::{content, seated_all};
use crate::choice::Table;
use crate::fixtures::{put, put_on_planet};
use crate::game::{Game, TACTICAL_ACTION_ID};

fn a() -> PlayerId {
    PlayerId::new("a")
}
fn b() -> PlayerId {
    PlayerId::new("b")
}
fn c() -> PlayerId {
    PlayerId::new("c")
}
fn sys(id: &str) -> SystemId {
    SystemId::new(id)
}

// -- shared drivers -------------------------------------------------------------------------------

/// A table answering from `rules` (see [`Pick`]); unmatched questions decline.
pub(crate) fn table(rules: &[&str]) -> Table {
    Table::with_default(Box::new(Pick::new(rules).0))
}

/// [`table`] that also reports what it was asked.
pub(crate) fn table_seen(rules: &[&str]) -> (Table, Seen) {
    let (pick, seen) = Pick::new(rules);
    (Table::with_default(Box::new(pick)), seen)
}

/// [`table_seen`] whose unmatched questions take the first option that is not a decline.
pub(crate) fn eager_table_seen(rules: &[&str]) -> (Table, Seen) {
    let (pick, seen) = Pick::eager(rules);
    (Table::with_default(Box::new(pick)), seen)
}

/// Step `game` exactly `count` times; every step must be legal.
fn steps(game: &mut Game<'_>, count: usize) {
    for _ in 0..count {
        assert_eq!(game.step().error, None, "log: {:?}", game.events);
    }
}

fn logged(game: &Game<'_>, event: &str) -> bool {
    game.events.iter().any(|logged| logged == event)
}

/// Every question a table was asked, as `player: prompt [options]`, for failure messages.
fn asked(seen: &Seen) -> Vec<String> {
    seen.borrow()
        .iter()
        .map(|question| {
            format!(
                "{}: {} {:?}",
                question.player, question.prompt, question.options
            )
        })
        .collect()
}

/// Three real objectives scored through `objectives::award` (the scoring path every phase uses)
/// unlock a hero; two do not. Same as d4's `hero_unlocks_by_scoring`.
pub(crate) fn hero_unlocks_by_scoring(faction: &str, hero: &str) {
    use ti4_model::id::ObjectiveId;
    let mut state = seated_all(&[("a", faction), ("b", "hacan")]);
    assert_eq!(
        leader_status(&state, &a(), hero),
        Some(LeaderStatus::Locked)
    );
    let score = |state: &mut GameState, alias: &str| {
        crate::objectives::award(state, content(), DEFAULT, &a(), &ObjectiveId::new(alias))
            .expect("the objective scores");
    };
    score(&mut state, "corner");
    score(&mut state, "develop");
    assert_eq!(
        leader_status(&state, &a(), hero),
        Some(LeaderStatus::Locked),
        "two scored objectives"
    );
    score(&mut state, "diversify");
    assert_eq!(
        leader_status(&state, &a(), hero),
        Some(LeaderStatus::Unlocked),
        "the third scored objective unlocks the hero the moment it is scored"
    );
}

/// [`super::d2::space_fight`] with the typed timing windows open (a resolver armed as the game
/// arms it), so the reactions that hang on combat events fire as they do in a tactical action.
/// (A copy of d4's private `armed_fight`, kept here so the two groups cannot conflict.)
pub(crate) fn armed_fight(
    state: &mut GameState,
    system: &SystemId,
    faces: &[u32],
    table: &mut Table,
) -> (crate::combat::CombatOutcome, crate::dice::Dice) {
    let mut resolver = crate::fixtures::armed_resolver(state);
    let mut sequence = crate::event::EventSequence::new();
    let mut dice = crate::dice::Dice::from_faces(padded_hits(faces));
    let mut rng = crate::rng::GameRng::new(11);
    state.active_system = Some(system.clone());
    let outcome = {
        let mut ctx = crate::choice::Resolving {
            content: content(),
            sources: DEFAULT,
            dice: &mut dice,
            rng: &mut rng,
            table,
            timing: Some(crate::choice::TimingHandle {
                resolver: &mut resolver,
                sequence: &mut sequence,
                galaxy: None,
            }),
        };
        crate::combat::resolve_resolving(state, &mut ctx, system, None)
            .expect("the combat resolves")
    };
    (outcome, dice)
}

/// [`super::d2::invade`] with the typed timing windows open, as [`armed_fight`] does for a space
/// combat. (A copy of d4's private `armed_invade`.)
pub(crate) fn armed_invade(
    state: &mut GameState,
    system: &SystemId,
    invader: &PlayerId,
    faces: &[u32],
    table: &mut Table,
) -> (crate::invasion::InvasionReport, crate::dice::Dice) {
    use crate::choice::Window as _;
    let mut resolver = crate::fixtures::armed_resolver(state);
    let mut sequence = crate::event::EventSequence::new();
    let mut dice = crate::dice::Dice::from_faces(padded_hits(faces));
    let mut rng = crate::rng::GameRng::new(11);
    state.active_system = Some(system.clone());
    let report = {
        let mut window = crate::invasion::InvasionWindow::new(
            state,
            content(),
            DEFAULT,
            &mut dice,
            &mut rng,
            invader,
            system,
        );
        let mut ctx = crate::choice::Resolving {
            content: content(),
            sources: DEFAULT,
            dice: &mut dice,
            rng: &mut rng,
            table,
            timing: Some(crate::choice::TimingHandle {
                resolver: &mut resolver,
                sequence: &mut sequence,
                galaxy: None,
            }),
        };
        while !window.is_done() {
            window
                .drive(state, &mut ctx)
                .expect("the invasion resolves");
            while window.take_scoring_occurrence().is_some() {}
            window.settle(state, &mut ctx);
        }
        window.into_report()
    };
    (report, dice)
}

/// Every roll `who` made for `reason` in `dice`: `(hits_on, number of dice)`.
fn rolls(dice: &crate::dice::Dice, who: &str, reason: &str) -> Vec<(Option<u32>, usize)> {
    dice.history()
        .iter()
        .filter(|roll| roll.by.as_deref() == Some(who) && roll.reason == reason)
        .map(|roll| (roll.hits_on, roll.faces.len()))
        .collect()
}

/// `active` takes a tactical action into `target` on `galaxy`; `rules` answer what follows the
/// activation. Steps until the action completes.
fn tactical(
    state: GameState,
    active: &str,
    galaxy: Galaxy,
    target: &str,
    rules: &[&str],
) -> (Game<'static>, Seen) {
    let mut all = vec![TACTICAL_ACTION_ID, target];
    all.extend_from_slice(rules);
    let (mut game, seen) = action_game(state, active, Some(galaxy), &all);
    for _ in 0..60 {
        assert_eq!(game.step().error, None, "log: {:?}", game.events);
        if logged(&game, "TACTICAL_ACTION_COMPLETE") {
            break;
        }
    }
    (game, seen)
}

/// Number of `kind` units of `owner` on any board area, galvanized or not.
fn galvanized_of(state: &GameState, owner: &PlayerId) -> usize {
    state
        .board
        .values()
        .flat_map(|board| {
            board
                .units
                .iter()
                .chain(board.planet_units.values().flatten())
        })
        .filter(|unit| unit.owner == *owner && unit.galvanized)
        .count()
}

fn galvanized_in(state: &GameState, system: &SystemId, owner: &PlayerId, kind: &str) -> usize {
    let board = state.system_state(system);
    board
        .units
        .iter()
        .chain(board.planet_units.values().flatten())
        .filter(|unit| unit.owner == *owner && unit.type_id.as_str() == kind && unit.galvanized)
        .count()
}

/// An empty system with a planet nobody's home, as `(system, planet)`, cleared of units and
/// control.
fn arena(state: &mut GameState) -> (SystemId, PlanetId) {
    let (system, planet) = crate::fixtures::a_placed_planet();
    let board = state.system_mut(&system);
    board.units.clear();
    board.planet_units.clear();
    board.planet_control.clear();
    (system, planet)
}

// -- Last Bastion ---------------------------------------------------------------------------------

fn bastion_game() -> GameState {
    seated_all(&[("a", "bastion"), ("b", "sol")])
}

/// A non-home planet worth `resources` resources and no more than a planet's worth of ground
/// (not placed during play, not a space station).
fn planet_worth(resources: i64) -> (SystemId, PlanetId) {
    ti4_content::galaxy::all_planets(content(), DEFAULT)
        .iter()
        .find_map(|(id, record)| {
            let system = record.system_id()?;
            (record.resources() == resources
                && record.homeworld_of().is_none()
                && !record.is_placed_during_play()
                && !ti4_content::galaxy::is_space_station(content(), id, DEFAULT)
                && ti4_content::galaxy::planets_in(content(), system, DEFAULT).len() == 1)
                .then(|| (SystemId::new(system), PlanetId::new(*id)))
        })
        .expect("a planet with these resources")
}

/// `invader` (seat `a`) lands `landing` infantry from the space area of a system whose only planet
/// is worth 2 resources and held, empty, by seat `b`. Returns the state afterwards.
fn take_empty_planet(faction: &str, landing: usize) -> (GameState, SystemId, PlanetId) {
    let mut state = seated_all(&[("a", faction), ("b", "hacan")]);
    let (system, planet) = planet_worth(2);
    let board = state.system_mut(&system);
    board.units.clear();
    board.planet_units.clear();
    board.planet_control.clear();
    board.planet_control.insert(planet.clone(), b());
    state.exhausted_planets.remove(&planet);
    put(&mut state, &system, "carrier", &a(), 1);
    put(&mut state, &system, "infantry", &a(), landing);
    let _ = crate::invasion::resolve(
        &mut state,
        content(),
        DEFAULT,
        &mut Table::new(),
        &mut crate::dice::Dice::from_faces(padded_hits(&[])),
        &mut crate::rng::GameRng::new(5),
        &system,
        &a(),
    )
    .expect("the invasion resolves");
    (state, system, planet)
}

#[test]
fn liberate_1_and_2_taking_a_planet_places_infantry_until_they_match_its_resources_then_readies_it()
{
    // Seat a (Last Bastion) lands on an empty 2-resource planet in a real invasion.
    // One infantry is fewer than the 2 resources: "otherwise place 1 infantry", the planet stays
    // exhausted (a planet taken in an invasion comes in exhausted).
    let (state, system, planet) = take_empty_planet("bastion", 1);
    assert_eq!(
        state.system_state(&system).planet_control.get(&planet),
        Some(&a())
    );
    assert_eq!(
        on_planet(&state, &system, &planet, &a(), "infantry"),
        2,
        "the lander plus the one Liberate placed"
    );
    assert!(
        state.exhausted_planets.contains(&planet),
        "fewer infantry than resources: not readied"
    );
    // Two infantry already match the 2 resources: the planet is readied, none are added.
    let (state, system, planet) = take_empty_planet("bastion", 2);
    assert_eq!(on_planet(&state, &system, &planet, &a(), "infantry"), 2);
    assert!(
        !state.exhausted_planets.contains(&planet),
        "enough infantry: Liberate readies the planet"
    );
    // Any other faction: no placement, the planet stays exhausted.
    let (state, system, planet) = take_empty_planet("sol", 1);
    assert_eq!(on_planet(&state, &system, &planet, &a(), "infantry"), 1);
    assert!(state.exhausted_planets.contains(&planet));
}

/// The dice `who` threw in their first batch of rolls for `reason` (consecutive rolls before any
/// other seat rolled): a whole round's worth of one side's dice.
fn first_batch(dice: &crate::dice::Dice, who: &str, reason: &str) -> usize {
    dice.history()
        .iter()
        .skip_while(|roll| roll.by.as_deref() != Some(who) || roll.reason != reason)
        .take_while(|roll| roll.by.as_deref() == Some(who) && roll.reason == reason)
        .map(|roll| roll.faces.len())
        .sum()
}

fn galvanized_unit(kind: &str, owner: &PlayerId) -> ti4_model::units::Unit {
    ti4_model::units::Unit::new(ti4_model::id::UnitTypeId::new(kind), owner.clone()).galvanized()
}

#[test]
fn galvanize_1_a_galvanized_ship_rolls_one_extra_combat_die_in_a_real_space_combat() {
    let batch = |galvanized: bool| {
        let mut state = bastion_game();
        let (system, _) = arena(&mut state);
        state.active = Some(a());
        put(&mut state, &system, "cruiser", &a(), 1);
        if galvanized {
            state
                .system_mut(&system)
                .units
                .push(galvanized_unit("cruiser", &a()));
        } else {
            put(&mut state, &system, "cruiser", &a(), 1);
        }
        put(&mut state, &system, "cruiser", &b(), 2);
        let (_, dice) = space_fight(&mut state, &system, &[1; 6], &mut Table::new());
        (
            first_batch(&dice, "a", "space combat"),
            first_batch(&dice, "b", "space combat"),
        )
    };
    assert_eq!(batch(false), (2, 2), "two plain cruisers roll two dice");
    assert_eq!(
        batch(true),
        (3, 2),
        "a galvanized cruiser rolls one additional die"
    );
}

#[test]
fn galvanize_1_b_galvanized_ground_forces_roll_one_extra_die_in_a_real_ground_combat() {
    let batch = |galvanized: bool| {
        let mut state = bastion_game();
        let (system, planet) = arena(&mut state);
        state.active = Some(a());
        state.system_mut(&system).set_control(planet.clone(), b());
        put_on_planet(&mut state, &system, &planet, "infantry", &b(), 1);
        put(&mut state, &system, "carrier", &a(), 1);
        put(&mut state, &system, "infantry", &a(), 1);
        if galvanized {
            state
                .system_mut(&system)
                .units
                .push(galvanized_unit("infantry", &a()));
        } else {
            put(&mut state, &system, "infantry", &a(), 1);
        }
        let (_, dice) = super::d2::invade(&mut state, &system, &a(), &[1; 8], &mut Table::new());
        (
            first_batch(&dice, "a", "ground combat"),
            first_batch(&dice, "b", "ground combat"),
        )
    };
    assert_eq!(batch(false), (2, 1));
    assert_eq!(
        batch(true),
        (3, 1),
        "the galvanized infantry throws two dice"
    );
}

#[test]
fn galvanize_1_c_a_galvanized_unit_rolls_one_extra_die_for_anti_fighter_barrage_and_bombardment() {
    // ANTI-FIGHTER BARRAGE: a destroyer rolls 2 dice, galvanized 3.
    let barrage = |galvanized: bool| {
        let mut state = bastion_game();
        let (system, _) = arena(&mut state);
        state.active = Some(a());
        if galvanized {
            state
                .system_mut(&system)
                .units
                .push(galvanized_unit("destroyer", &a()));
        } else {
            put(&mut state, &system, "destroyer", &a(), 1);
        }
        put(&mut state, &system, "fighter", &b(), 4);
        let (_, dice) = armed_fight(&mut state, &system, &[1; 6], &mut table(&[]));
        first_batch(&dice, "a", "anti-fighter barrage")
    };
    assert_eq!((barrage(false), barrage(true)), (2, 3));
    // BOMBARDMENT: a dreadnought rolls 1 die, galvanized 2, against the planet's ground forces.
    let bombard = |galvanized: bool| {
        let mut state = bastion_game();
        let (system, planet) = arena(&mut state);
        state.active = Some(a());
        state.system_mut(&system).set_control(planet.clone(), b());
        put_on_planet(&mut state, &system, &planet, "infantry", &b(), 3);
        if galvanized {
            state
                .system_mut(&system)
                .units
                .push(galvanized_unit("dreadnought", &a()));
        } else {
            put(&mut state, &system, "dreadnought", &a(), 1);
        }
        put(&mut state, &system, "infantry", &a(), 1);
        let (_, dice) = armed_invade(&mut state, &system, &a(), &[1; 8], &mut table(&[]));
        first_batch(&dice, "a", "bombardment")
    };
    assert_eq!((bombard(false), bombard(true)), (1, 2));
}

/// The dice `who` threw for `reason`, one total per batch of consecutive rolls (one per round).
fn batches(dice: &crate::dice::Dice, who: &str, reason: &str) -> Vec<usize> {
    let mut found: Vec<usize> = Vec::new();
    let mut inside = false;
    for roll in dice.history() {
        let mine = roll.by.as_deref() == Some(who) && roll.reason == reason;
        if mine {
            if inside {
                *found.last_mut().expect("a batch") += roll.faces.len();
            } else {
                found.push(roll.faces.len());
            }
        }
        inside = mine;
    }
    found
}

const PHOENIX_SPACE: &str = "ability:bastion:phoenixstandard:SPACE_COMBAT_ENDED:after";
const PHOENIX_GROUND: &str = "ability:bastion:phoenixstandard:GROUND_COMBAT_ENDED:after";

/// A real space combat: Bastion (a) has 2 cruisers and a destroyer, Sol (b) a destroyer; every die
/// hits, so each side loses one ship. `rules` answer the Last Bastion's questions.
fn bastion_wins_a_space_combat(
    rules: &[&str],
    galvanized_cruisers: usize,
) -> (GameState, SystemId, Seen) {
    let mut state = bastion_game();
    let (system, _) = arena(&mut state);
    state.active = Some(a());
    put(
        &mut state,
        &system,
        "cruiser",
        &a(),
        2 - galvanized_cruisers,
    );
    for _ in 0..galvanized_cruisers {
        state
            .system_mut(&system)
            .units
            .push(galvanized_unit("cruiser", &a()));
    }
    put(&mut state, &system, "destroyer", &a(), 1);
    put(&mut state, &system, "destroyer", &b(), 1);
    let (mut table, seen) = table_seen(rules);
    let _ = armed_fight(&mut state, &system, &[], &mut table);
    (state, system, seen)
}

#[test]
fn phoenixstandard_1_galvanizes_one_participating_unit_after_a_real_space_combat() {
    let cruiser = |system: &SystemId| format!("{system}|space|cruiser|undamaged");
    let system = bastion_wins_a_space_combat(&[], 0).1;
    let (state, system, seen) =
        bastion_wins_a_space_combat(&["~phoenixstandard", &cruiser(&system)], 0);
    assert_eq!(
        galvanized_in(&state, &system, &a(), "cruiser"),
        1,
        "{:?}",
        asked(&seen)
    );
    assert_eq!(galvanized_of(&state, &a()), 1, "exactly one unit");
    // "May": declining the ability leaves everything as it was.
    let (state, _, seen) = bastion_wins_a_space_combat(&["decline"], 0);
    assert!(
        was_offered_like(&seen, "phoenixstandard"),
        "{:?}",
        asked(&seen)
    );
    assert_eq!(galvanized_of(&state, &a()), 0);
}

#[test]
fn phoenixstandard_1_galvanizes_a_ground_force_after_a_real_ground_combat() {
    let mut state = bastion_game();
    let (system, planet) = arena(&mut state);
    state.active = Some(a());
    state.system_mut(&system).set_control(planet.clone(), b());
    put_on_planet(&mut state, &system, &planet, "infantry", &b(), 1);
    put(&mut state, &system, "carrier", &a(), 1);
    put(&mut state, &system, "infantry", &a(), 3);
    // a's three dice hit, b's one die misses: the Last Bastion wins the planet with all three.
    let (mut table, seen) = table_seen(&["~commit|", "~commit|", "~commit|", "~phoenixstandard"]);
    let _ = armed_invade(&mut state, &system, &a(), &[10, 10, 10, 1], &mut table);
    assert!(
        was_offered_like(&seen, "phoenixstandard"),
        "{:?}",
        asked(&seen)
    );
    assert_eq!(
        galvanized_in(&state, &system, &a(), "infantry"),
        1,
        "{:?}",
        asked(&seen)
    );
    assert_eq!(
        galvanized_in(&state, &system, &a(), "carrier"),
        0,
        "the ship did not fight on the ground: {:?}",
        asked(&seen)
    );
}

#[test]
fn galvanize_2_a_unit_takes_one_token_and_no_unit_can_be_galvanized_once_seven_are_out() {
    // All seven tokens are beneath Hacan ships elsewhere: nothing is offered after the combat.
    let mut state = bastion_game();
    let (system, _) = arena(&mut state);
    state.active = Some(a());
    let far = sys("19");
    for _ in 0..7 {
        state
            .system_mut(&far)
            .units
            .push(galvanized_unit("cruiser", &c()));
    }
    put(&mut state, &system, "cruiser", &a(), 2);
    put(&mut state, &system, "destroyer", &b(), 1);
    let (mut table, seen) = table_seen(&["~phoenixstandard"]);
    let _ = armed_fight(&mut state, &system, &[], &mut table);
    assert!(
        !was_offered_like(&seen, "phoenixstandard"),
        "{:?}",
        asked(&seen)
    );
    assert_eq!(galvanized_of(&state, &a()), 0);
    // An already galvanized unit takes no second token: with only galvanized participants left
    // (the plain cruiser died), Phoenix Standard has nobody to offer.
    let mut state = bastion_game();
    let (system, _) = arena(&mut state);
    state.active = Some(a());
    state
        .system_mut(&system)
        .units
        .push(galvanized_unit("cruiser", &a()));
    put(&mut state, &system, "cruiser", &b(), 1);
    let (mut table, seen) = table_seen(&["~phoenixstandard"]);
    let _ = armed_fight(&mut state, &system, &[10, 10, 1], &mut table);
    assert!(
        !was_offered_like(&seen, "phoenixstandard"),
        "{:?}",
        asked(&seen)
    );
    assert_eq!(galvanized_of(&state, &a()), 1, "still just the one token");
}

#[test]
fn bastionagent_2_dame_briar_exhausts_to_galvanize_another_unit_of_the_destroyed_units_owner() {
    // a (Bastion) attacks with a dreadnought; Sol (b) has a cruiser and a destroyer. The dreadnought
    // kills one of them in round one; Dame Briar galvanizes the other, which then throws two dice
    // in round two (a plain one throws one).
    let run = |rules: &[&str]| {
        let mut state = bastion_game();
        let (system, _) = arena(&mut state);
        state.active = Some(a());
        put(&mut state, &system, "dreadnought", &a(), 1);
        put(&mut state, &system, "cruiser", &b(), 1);
        put(&mut state, &system, "carrier", &b(), 1);
        let (mut table, seen) = table_seen(rules);
        // round 1: a hits, b misses twice; round 2: a misses, b misses twice; round 3: a hits.
        let (_, dice) = armed_fight(&mut state, &system, &[10, 1, 1, 1, 1, 1, 10], &mut table);
        (state, dice, seen)
    };
    let (mut state, dice, seen) = run(&["~bastionagent"]);
    assert_eq!(
        batches(&dice, "b", "space combat")[..2],
        [2, 2],
        "{:?}",
        asked(&seen)
    );
    assert_eq!(
        leader_status(&state, &a(), "bastionagent"),
        Some(LeaderStatus::Exhausted)
    );
    run_status_phase(&mut state);
    assert_eq!(
        leader_status(&state, &a(), "bastionagent"),
        Some(LeaderStatus::Readied),
        "readies in the status phase"
    );
    // Declined: the survivor stays plain and throws one die; the agent stays ready.
    let (state, dice, _) = run(&["decline"]);
    assert_eq!(batches(&dice, "b", "space combat")[..2], [2, 1]);
    assert_eq!(
        leader_status(&state, &a(), "bastionagent"),
        Some(LeaderStatus::Readied)
    );
}

#[test]
fn bastionagent_1_starts_ready_and_the_commander_hero_start_locked() {
    let state = bastion_game();
    assert_eq!(
        leader_status(&state, &a(), "bastionagent"),
        Some(LeaderStatus::Readied)
    );
    assert_eq!(
        leader_status(&state, &a(), "bastioncommander"),
        Some(LeaderStatus::Locked)
    );
    assert_eq!(
        leader_status(&state, &a(), "bastionhero"),
        Some(LeaderStatus::Locked)
    );
}

#[test]
fn bastionhero_1_unlocks_on_the_third_scored_objective() {
    hero_unlocks_by_scoring("bastion", "bastionhero");
}

#[test]
fn bastionhero_2_lyra_keen_purges_to_destroy_rival_units_in_the_system_of_a_destroyed_galvanized_unit()
 {
    // a's galvanized cruiser (two dice) misses; b's three cruisers destroy it. Lyra Keen rolls
    // one die per Sol unit in the system against the cruiser's combat value 7: [10, 1, 10] kills
    // two of them.
    let run = |unlocked: bool| {
        let mut state = bastion_game();
        let (system, _) = arena(&mut state);
        state.active = Some(a());
        if unlocked {
            set_leader(&mut state, &a(), "bastionhero", LeaderStatus::Unlocked);
        }
        state
            .system_mut(&system)
            .units
            .push(galvanized_unit("cruiser", &a()));
        put(&mut state, &system, "cruiser", &b(), 3);
        let mut table = table(&["~bastionhero"]);
        let (_, dice) = armed_fight(
            &mut state,
            &system,
            &[1, 1, 10, 10, 10, 10, 1, 10, 1, 1, 1],
            &mut table,
        );
        let left = ships(&state, &system, &b(), "cruiser");
        (state, dice, left)
    };
    let (state, dice, left) = run(true);
    assert_eq!(rolls(&dice, "a", "lyra keen"), vec![(Some(7), 3)]);
    assert_eq!(left, 1, "two of the three Sol cruisers were destroyed");
    assert_eq!(
        leader_status(&state, &a(), "bastionhero"),
        Some(LeaderStatus::Purged)
    );
    let (state, _, left) = run(false);
    assert_eq!(left, 3, "a locked hero does nothing");
    assert_eq!(
        leader_status(&state, &a(), "bastionhero"),
        Some(LeaderStatus::Locked)
    );
}

#[test]
fn bastioncommander_1_unlocks_in_the_game_with_three_galvanized_units_on_the_board() {
    let mut state = bastion_game();
    state
        .system_mut(&sys("19"))
        .units
        .push(galvanized_unit("cruiser", &b()));
    state
        .system_mut(&sys("19"))
        .units
        .push(galvanized_unit("cruiser", &c()));
    assert!(
        !unlocks_in_the_game(&state, &a(), "bastioncommander"),
        "two"
    );
    state
        .system_mut(&sys("19"))
        .units
        .push(galvanized_unit("cruiser", &a()));
    assert!(
        unlocks_in_the_game(&state, &a(), "bastioncommander"),
        "three, any player's"
    );
}

#[test]
fn bastioncommander_2_an_unlocked_commander_stops_sabotage_from_cancelling_the_action_cards() {
    // The Last Bastion plays Frontline Deployment; Sol holds Sabotage.
    let run = |unlocked: bool| {
        let mut state = seated_all(&[("a", "bastion"), ("b", "sol")]);
        if unlocked {
            set_leader(&mut state, &a(), "bastioncommander", LeaderStatus::Unlocked);
        }
        state.player_mut(&a()).unwrap().action_cards =
            vec![ti4_model::id::ActionCardId::new("f_deployment")];
        state.player_mut(&b()).unwrap().action_cards =
            vec![ti4_model::id::ActionCardId::new("sabo1")];
        let planets_infantry = |state: &GameState| {
            state
                .controlled_planets(&a())
                .into_iter()
                .map(|(system, planet)| on_planet(state, system, planet, &a(), "infantry"))
                .sum::<usize>()
        };
        let before = planets_infantry(&state);
        let (mut game, seen) = eager_game(
            state,
            "a",
            None,
            &["action_card|0", "~reaction:sol:ACTION_CARD_PLAYED"],
        );
        steps(&mut game, 6);
        (
            planets_infantry(&game.state) > before,
            was_offered_like(&seen, "reaction:sol:ACTION_CARD_PLAYED"),
            asked(&seen),
        )
    };
    let (resolved, offered, log) = run(false);
    assert!(
        offered && !resolved,
        "locked: Sabotage is offered and cancels the card: {log:?}"
    );
    let (resolved, offered, log) = run(true);
    assert!(
        !offered && resolved,
        "unlocked: no Sabotage window, the card resolves: {log:?}"
    );
}

/// Seat a (Last Bastion) takes a tactical action into its own home system on a blank map and
/// produces with `rules`; returns the game and what was asked.
fn bastion_produces(state: GameState, rules: &[&str]) -> (Game<'static>, Seen) {
    let home = state.player(&a()).unwrap().home_system.clone().unwrap();
    let mut all = vec![TACTICAL_ACTION_ID, home.as_str(), "done_moving"];
    all.extend_from_slice(rules);
    let (mut game, seen) = action_game(state, "a", Some(crate::fixtures::plain_hub().galaxy), &all);
    for _ in 0..14 {
        assert_eq!(game.step().error, None, "log: {:?}", game.events);
    }
    (game, seen)
}

fn ship_count(state: &GameState, system: &SystemId, owner: &PlayerId, kind: &str) -> usize {
    ships(state, system, owner, kind)
}

#[test]
fn helios2_1_to_3_the_upgraded_dock_produces_four_more_than_the_resources_and_adds_two() {
    // Bastion's home planet is worth 0 resources: the V1 dock produces 3 units (2 more than the
    // planet's resource value, which the dock itself raises by 1) and V2 produces 6 (4 more than
    // the planet's value, raised by 2). Every unit is a 1-cost destroyer paid with trade goods.
    let produced = |upgrade: bool| {
        let mut state = bastion_game();
        state.player_mut(&a()).unwrap().trade_goods = 30;
        state.player_mut(&a()).unwrap().fleet_tokens = 14;
        if upgrade {
            super::give(&mut state, "a", "helios2");
            crate::technology::apply_unit_upgrades(&mut state, content(), DEFAULT, &a());
        }
        let home = state.player(&a()).unwrap().home_system.clone().unwrap();
        let before = ship_count(&state, &home, &a(), "destroyer");
        let rules = ["build|destroyer|1"; 8];
        let mut all = rules.to_vec();
        all.push("done_producing");
        let (game, seen) = bastion_produces(state, &all);
        let prompt = asked(&seen)
            .into_iter()
            .find(|line| line.contains("produce in"))
            .unwrap();
        (
            ship_count(&game.state, &home, &a(), "destroyer") - before,
            prompt,
        )
    };
    let (v1, prompt) = produced(false);
    assert_eq!(v1, 3, "{prompt}");
    let (v2, prompt) = produced(true);
    assert_eq!(v2, 6, "{prompt}");
}

/// Seat a holds The Icon and has a command token, an infantry and no rival ship in a second system.
fn icon_game() -> (GameState, SystemId) {
    let mut state = bastion_game();
    state.player_mut(&a()).unwrap().trade_goods = 30;
    state.player_mut(&a()).unwrap().fleet_tokens = 14;
    state.player_mut(&a()).unwrap().breakthrough =
        Some(ti4_model::id::BreakthroughId::new("bastionbt"));
    let (target, _) = arena(&mut state);
    state.system_mut(&target).place_token(a());
    put(&mut state, &target, "infantry", &a(), 1);
    (state, target)
}

#[test]
fn bastionbt_1_the_icon_places_the_produced_ships_in_a_system_with_a_token_and_ground_force_then_readies()
 {
    let (state, target) = icon_game();
    let home = state.player(&a()).unwrap().home_system.clone().unwrap();
    let remote = format!("place|{target}@space");
    let (game, seen) = bastion_produces(
        state.clone(),
        &[
            "build|destroyer|1",
            &remote,
            "build|destroyer|1",
            "done_producing",
        ],
    );
    assert_eq!(
        ship_count(&game.state, &target, &a(), "destroyer"),
        2,
        "{:?}",
        asked(&seen)
    );
    assert_eq!(ship_count(&game.state, &home, &a(), "destroyer"), 0);
    assert!(
        !crate::factions::bastion::icon_ready(&game.state, &a()),
        "the card is exhausted"
    );
    // Exhausted: the next production offers no remote system; the card readies after the status phase.
    let game = play_status_phase(game.state);
    assert!(
        crate::factions::bastion::icon_ready(&game.state, &a()),
        "readied in the status phase"
    );
    // A rival ship in the system, or no ground force there, and the Icon is not offered.
    let mut rival = state.clone();
    put(&mut rival, &target, "cruiser", &b(), 1);
    let mut no_ground = state;
    no_ground.system_mut(&target).units.clear();
    for blocked in [rival, no_ground] {
        let (game, seen) = bastion_produces(blocked, &["build|destroyer|1", "done_producing"]);
        assert!(!was_offered_like(&seen, "@space"), "{:?}", asked(&seen));
        assert_eq!(ship_count(&game.state, &target, &a(), "destroyer"), 0);
    }
}

#[test]
fn raisethestandard_1_and_2_the_holder_galvanizes_a_unit_after_its_combat_and_the_note_goes_home() {
    let note = crate::promissory::note_id("raisethestandard", "bastion");
    let run = |holder_fights: bool| {
        let mut state = seated_all(&[("a", "bastion"), ("b", "sol"), ("c", "hacan")]);
        crate::promissory::deal(&mut state, content(), DEFAULT);
        assert_eq!(
            state.promissory_notes.get(&note),
            Some(&a()),
            "dealt to the Last Bastion"
        );
        crate::promissory::take(&mut state, content(), &b(), &note);
        assert_eq!(state.promissory_notes.get(&note), Some(&b()), "lent to b");
        let (system, _) = arena(&mut state);
        // b (the holder) or a attacks c's cruiser; the attacker's die hits, c's misses.
        let attacker = if holder_fights { b() } else { a() };
        state.active = Some(attacker.clone());
        put(&mut state, &system, "cruiser", &attacker, 1);
        put(&mut state, &system, "cruiser", &c(), 1);
        let _ = armed_fight(&mut state, &system, &[10, 1], &mut table(&[]));
        state
    };
    let state = run(true);
    assert_eq!(
        galvanized_of(&state, &b()),
        1,
        "the holder's surviving ship"
    );
    assert_eq!(
        galvanized_of(&state, &a()),
        0,
        "the Last Bastion galvanized nothing itself"
    );
    assert_eq!(
        state.promissory_notes.get(&note),
        Some(&a()),
        "then it returns to the Last Bastion player"
    );
    let state = run(false);
    assert_eq!(
        galvanized_of(&state, &b()),
        0,
        "b did not fight: nothing happens"
    );
    assert_eq!(
        state.promissory_notes.get(&note),
        Some(&b()),
        "and the card stays with b"
    );
}

/// Seven ordinary systems that all hold a planet and are nobody's home, as a one-ring map.
fn planet_hub() -> crate::fixtures::Hub {
    let ids: Vec<String> = crate::fixtures::plain_systems(120)
        .into_iter()
        .filter(|id| {
            !crate::seating::is_mecatol(id)
                && !id.starts_with("fracture")
                && !ti4_content::galaxy::is_home_system(content(), id, DEFAULT)
                && !ti4_content::galaxy::planets_in(content(), id, DEFAULT).is_empty()
        })
        .take(7)
        .collect();
    crate::fixtures::hub_from(&ids)
}

fn first_planet(system: &SystemId) -> PlanetId {
    PlanetId::new(ti4_content::galaxy::planets_in(content(), system.as_str(), DEFAULT)[0].id())
}

#[test]
fn galvanize_1_d_a_galvanized_pds_rolls_one_extra_space_cannon_die_in_a_real_movement() {
    // Sol moves a cruiser into the Last Bastion's PDS system: SPACE CANNON 6 once, twice galvanized.
    let dice = |galvanized: bool| {
        let hub = planet_hub();
        let centre = sys(&hub.centre);
        let planet = first_planet(&centre);
        let mut state = bastion_game();
        for id in hub.outer.iter().chain(std::iter::once(&hub.centre)) {
            let board = state.system_mut(&sys(id));
            board.units.clear();
            board.planet_units.clear();
        }
        state.system_mut(&centre).set_control(planet.clone(), a());
        if galvanized {
            state
                .system_mut(&centre)
                .planet_units
                .entry(planet.clone())
                .or_default()
                .push(galvanized_unit("pds", &a()));
        } else {
            put_on_planet(&mut state, &centre, &planet, "pds", &a(), 1);
        }
        put(&mut state, &sys(&hub.outer[0]), "cruiser", &b(), 1);
        let rule = format!("move|{}|0", hub.outer[0]);
        let (game, seen) = tactical(
            state,
            "b",
            hub.galaxy.clone(),
            &hub.centre,
            &[&rule, "done_moving"],
        );
        let cannon: Vec<usize> = game
            .rolls()
            .iter()
            .filter(|roll| roll.reason == "space cannon" && roll.by.as_deref() == Some("a"))
            .map(|roll| roll.faces.len())
            .collect();
        (cannon, asked(&seen))
    };
    let (plain, log) = dice(false);
    assert_eq!(plain.iter().sum::<usize>(), 1, "{log:?}");
    let (galvanized, log) = dice(true);
    assert_eq!(galvanized.iter().sum::<usize>(), 2, "{log:?}");
}

#[test]
fn proxima_1_each_galvanized_unit_on_the_planet_cancels_one_bombardment_hit_in_a_real_invasion() {
    // Sol's three dreadnoughts bombard the Bastion's planet: three hits, one cancelled by the
    // galvanized infantry present; without the technology all three land.
    let left = |tech: bool| {
        let mut state = bastion_game();
        let (system, planet) = arena(&mut state);
        state.active = Some(b());
        state.system_mut(&system).set_control(planet.clone(), a());
        if tech {
            super::give(&mut state, "a", "proxima");
        }
        put_on_planet(&mut state, &system, &planet, "infantry", &a(), 4);
        state
            .system_mut(&system)
            .planet_units
            .entry(planet.clone())
            .or_default()
            .push(galvanized_unit("infantry", &a()));
        put(&mut state, &system, "dreadnought", &b(), 3);
        let (_, dice) = armed_invade(&mut state, &system, &b(), &[10, 10, 10], &mut table(&[]));
        (
            on_planet(&state, &system, &planet, &a(), "infantry"),
            rolls(&dice, "b", "bombardment"),
        )
    };
    let (with, bombard) = left(true);
    assert_eq!(bombard.iter().map(|(_, n)| n).sum::<usize>(), 3);
    assert_eq!(with, 5 - 2, "three hits, one cancelled");
    assert_eq!(left(false).0, 5 - 3, "no technology: all three land");
}

#[test]
fn unit_bastion_mech_1_a3_valiance_galvanizes_up_to_three_infantry_when_a_galvanized_mech_is_destroyed()
 {
    // Sol lands one infantry on the Bastion's planet, where only a damaged mech stands; the hit
    // destroys it. Four Bastion infantry wait in the space area of the same system.
    let run = |galvanized: bool| {
        let mut state = bastion_game();
        let (system, planet) = arena(&mut state);
        state.active = Some(b());
        state.system_mut(&system).set_control(planet.clone(), a());
        let mut mech =
            ti4_model::units::Unit::new(ti4_model::id::UnitTypeId::new("bastion_mech"), a());
        mech.sustained_damage = true;
        mech.galvanized = galvanized;
        state
            .system_mut(&system)
            .planet_units
            .entry(planet.clone())
            .or_default()
            .push(mech);
        put(&mut state, &system, "infantry", &a(), 4);
        put(&mut state, &system, "carrier", &b(), 1);
        put(&mut state, &system, "infantry", &b(), 1);
        let (mut table, seen) = table_seen(&["~commit|"]);
        let _ = armed_invade(&mut state, &system, &b(), &[10], &mut table);
        assert_eq!(
            on_planet(&state, &system, &planet, &a(), "bastion_mech"),
            0,
            "{:?}",
            asked(&seen)
        );
        galvanized_in(&state, &system, &a(), "infantry")
    };
    assert_eq!(
        run(true),
        3,
        "up to three of the four infantry in the system"
    );
    assert_eq!(run(false), 0, "a plain mech does nothing");
}

#[test]
fn unit_bastion_flagship_1_the_egeiro_gains_plus_one_per_non_home_system_with_a_controlled_planet()
{
    // The Egeiro hits on 9; each non-home system with a planet the Bastion controls adds 1.
    let threshold = |systems: usize| {
        let mut state = bastion_game();
        let (system, _) = arena(&mut state);
        state.active = Some(a());
        let owned: Vec<(SystemId, PlanetId)> = ti4_content::galaxy::all_planets(content(), DEFAULT)
            .iter()
            .filter(|(_, p)| p.homeworld_of().is_none() && !p.is_placed_during_play())
            .filter_map(|(id, p)| Some((sys(p.system_id()?), PlanetId::new(*id))))
            .filter(|(s, _)| *s != system)
            .collect::<Vec<_>>();
        let mut seen_systems = std::collections::BTreeSet::new();
        for (s, p) in owned
            .into_iter()
            .filter(|(s, _)| seen_systems.insert(s.clone()))
            .take(systems)
        {
            state.system_mut(&s).set_control(p, a());
        }
        put(&mut state, &system, "bastion_flagship", &a(), 1);
        put(&mut state, &system, "cruiser", &b(), 1);
        let (_, dice) = space_fight(&mut state, &system, &[1, 1], &mut Table::new());
        first_roll(&dice, "a").and_then(|(hits_on, _)| hits_on)
    };
    assert_eq!(threshold(0), Some(9));
    assert_eq!(threshold(2), Some(7), "two systems");
}

#[test]
fn bastioncommander_3_nip_and_tuck_bars_the_nekro_from_placing_assimilator_tokens_on_its_components()
 {
    // The Nekro (a) destroys a Bastion ship (b) in a real combat; Technological Singularity then
    // offers its gains. Bastion's faction technology can take a token only while the commander is locked.
    let offered = |unlocked: bool| {
        let mut state = seated_all(&[("a", "nekro"), ("b", "bastion")]);
        super::give(&mut state, "a", "vax");
        super::give(&mut state, "b", "proxima");
        super::give(&mut state, "b", "aida");
        state
            .player_mut(&a())
            .unwrap()
            .technologies
            .remove(&ti4_model::id::TechnologyId::new("aida"));
        if unlocked {
            set_leader(&mut state, &b(), "bastioncommander", LeaderStatus::Unlocked);
        }
        let (system, _) = arena(&mut state);
        state.active = Some(a());
        put(&mut state, &system, "cruiser", &a(), 1);
        put(&mut state, &system, "cruiser", &b(), 1);
        let (mut table, seen) = table_seen(&["~technological_singularity"]);
        let _ = armed_fight(&mut state, &system, &[10, 1], &mut table);
        (
            was_offered(&seen, "gain|b|aida"),
            was_offered(&seen, "x|b|proxima"),
            asked(&seen),
        )
    };
    let (gain, token, log) = offered(false);
    assert!(
        gain && token,
        "locked: both the gain and the token: {log:?}"
    );
    let (gain, token, log) = offered(true);
    assert!(
        gain && !token,
        "unlocked: the gain stays, no token: {log:?}"
    );
}
