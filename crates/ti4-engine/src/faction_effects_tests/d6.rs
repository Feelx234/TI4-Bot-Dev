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
    armed_fight_on(state, system, faces, table, None)
}

/// [`armed_fight`] on a map (abilities that reach "up to 2 systems away" need one).
pub(crate) fn armed_fight_on(
    state: &mut GameState,
    system: &SystemId,
    faces: &[u32],
    table: &mut Table,
    galaxy: Option<&Galaxy>,
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
                galaxy,
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

// -- Crimson Rebellion ----------------------------------------------------------------------------

use crate::factions::crimson::testkit::{plain, ring_map, wormhole_tiles};
use crate::factions::crimson::{flip_breach, has_breach, is_active};

fn crimson_game() -> GameState {
    seated_all(&[("a", "crimson"), ("b", "sol"), ("c", "hacan")])
}

/// Put a breach token in `system` (active or not) as the engine's own placement leaves it.
fn breach(state: &mut GameState, system: &SystemId, active: bool) {
    state.breach_tokens.insert(system.clone());
    if active {
        assert!(!is_active(state, system));
        flip_breach(state, system);
    }
}

/// The end of the action phase and the status phase after it, played by a `Game` driven by `rules`
/// (every seat has passed, so the game itself begins the status phase and announces
/// `STATUS_PHASE_BEGAN` before the scoring, upkeep and `STATUS_PHASE_ENDED`).
fn status_game(state: GameState, rules: &[&str]) -> (Game<'static>, Seen) {
    let (pick, seen) = Pick::new(rules);
    run_status(state, pick, seen)
}

/// [`status_game`] whose unmatched questions take the first option that is not a decline.
fn status_game_eager(state: GameState, rules: &[&str]) -> (Game<'static>, Seen) {
    let (pick, seen) = Pick::eager(rules);
    run_status(state, pick, seen)
}

fn run_status(mut state: GameState, pick: Pick, seen: Seen) -> (Game<'static>, Seen) {
    state.phase = Phase::Action;
    for seat in &mut state.players {
        seat.passed = true;
    }
    let mut game = Game::with_table(state, content(), Table::with_default(Box::new(pick)))
        .with_sources(DEFAULT);
    let mut in_status = false;
    for _ in 0..120 {
        assert_eq!(game.step().error, None, "log: {:?}", game.events);
        in_status |= game.state.phase == Phase::Status;
        if in_status && game.state.phase != Phase::Status {
            return (game, seen);
        }
    }
    panic!("the status phase never ended; log: {:?}", game.events);
}

/// Six plain ring systems around a plain centre with the Crimson home placed: `(galaxy, ring ids)`.
fn crimson_ring(first: Option<&str>) -> (Galaxy, Vec<String>) {
    let others = plain(7);
    let mut ring: Vec<String> = others[..6].to_vec();
    if let Some(first) = first {
        ring[0] = first.to_owned();
    }
    let refs: Vec<&str> = ring.iter().map(String::as_str).collect();
    (ring_map(&others[6], &refs), ring)
}

#[test]
fn sundered_1_the_crimson_cannot_use_a_wormhole_but_epsilon_in_a_real_movement() {
    // Alpha wormholes at ring positions 0 and 3; a carrier (move 1) waits at position 0.
    let alpha = wormhole_tiles("ALPHA");
    let others = plain(5);
    let ring = [
        alpha[0].as_str(),
        others[0].as_str(),
        others[1].as_str(),
        alpha[1].as_str(),
        others[2].as_str(),
        others[3].as_str(),
    ];
    let galaxy = ring_map(&others[4], &ring);
    let reaches = |mover: &str| {
        let mut state = crimson_game();
        put(
            &mut state,
            &sys(ring[0]),
            "carrier",
            &PlayerId::new(mover),
            1,
        );
        let (game, seen) = tactical(
            state,
            mover,
            galaxy.clone(),
            ring[3],
            &[&format!("move|{}|0", ring[0]), "done_moving"],
        );
        (
            was_offered(&seen, &format!("move|{}|0", ring[0])),
            ships(&game.state, &sys(ring[3]), &PlayerId::new(mover), "carrier"),
        )
    };
    assert_eq!(reaches("b"), (true, 1), "Sol uses the alpha wormhole");
    assert_eq!(
        reaches("a"),
        (false, 0),
        "the Crimson is never offered the move"
    );
    // Epsilon stays open: the Sorrow (94) to the home system (118), in both directions.
    let (galaxy, _) = crimson_ring(Some("94"));
    let mut state = crimson_game();
    put(&mut state, &sys("94"), "cruiser", &a(), 1);
    let (game, seen) = tactical(state, "a", galaxy, "118", &["move|94|0", "done_moving"]);
    assert!(was_offered(&seen, "move|94|0"), "{:?}", asked(&seen));
    assert_eq!(ships(&game.state, &sys("118"), &a(), "cruiser"), 1);
}

#[test]
fn incursion_1_activating_a_system_with_a_breach_may_flip_it_and_active_breaches_are_adjacent() {
    let (galaxy, ring) = crimson_ring(None);
    let flip = "ability:crimson:incursion:SYSTEM_ACTIVATED:when";
    for (answer, active) in [(flip, true), ("decline", false)] {
        let mut state = crimson_game();
        breach(&mut state, &sys(&ring[0]), false);
        let (game, seen) = tactical(
            state,
            "a",
            galaxy.clone(),
            &ring[0],
            &[answer, "done_moving"],
        );
        assert!(was_offered(&seen, flip), "{:?}", asked(&seen));
        assert_eq!(is_active(&game.state, &sys(&ring[0])), active, "{answer}");
    }
    // Another player activating the system gets no flip.
    let mut state = crimson_game();
    breach(&mut state, &sys(&ring[0]), false);
    let (game, seen) = tactical(state, "b", galaxy.clone(), &ring[0], &[flip, "done_moving"]);
    assert!(!was_offered(&seen, flip));
    assert!(!is_active(&game.state, &sys(&ring[0])));
    // "Systems that contain active breaches are adjacent", for every player: Sol's carrier (move 1)
    // reaches the system across the ring only while both hold active breaches.
    let (from, to) = (&ring[0], &ring[3]);
    let reaches = |both_active: bool| {
        let mut state = crimson_game();
        breach(&mut state, &sys(from), true);
        breach(&mut state, &sys(to), both_active);
        put(&mut state, &sys(from), "carrier", &b(), 1);
        let rule = format!("move|{from}|0");
        let (game, seen) = tactical(state, "b", galaxy.clone(), to, &[&rule, "done_moving"]);
        (
            was_offered(&seen, &rule),
            ships(&game.state, &sys(to), &b(), "carrier"),
        )
    };
    assert_eq!(reaches(true), (true, 1));
    assert_eq!(
        reaches(false),
        (false, 0),
        "an inactive breach links nothing"
    );
}

#[test]
fn incursion_2_any_player_with_ships_in_an_active_breach_may_remove_it_at_the_end_of_the_status_phase()
 {
    let remove = "ability:sol:incursion_remove:STATUS_PHASE_ENDED:after";
    let setup = || {
        let mut state = crimson_game();
        let (here, there) = (sys("25"), sys("26"));
        breach(&mut state, &here, true);
        breach(&mut state, &there, false);
        put(&mut state, &here, "cruiser", &b(), 1);
        put(&mut state, &there, "cruiser", &b(), 1);
        state
    };
    let (game, seen) = status_game(setup(), &[remove]);
    assert!(
        !has_breach(&game.state, &sys("25")),
        "Sol removed the active breach: {:?}",
        asked(&seen)
    );
    assert!(
        has_breach(&game.state, &sys("26")),
        "an inactive breach stays"
    );
    let (game, _) = status_game(setup(), &["decline"]);
    assert!(is_active(&game.state, &sys("25")), "the removal is a may");
    // Nobody has ships there: nobody may remove it.
    let mut empty = crimson_game();
    breach(&mut empty, &sys("25"), true);
    let (game, seen) = status_game(empty, &[remove]);
    assert!(is_active(&game.state, &sys("25")) && !was_offered_like(&seen, "incursion_remove"));
}

#[test]
fn subatomic_1_a_destroyed_ship_may_be_rebuilt_at_a_home_space_dock_in_a_real_combat() {
    let rebuilt = |tech: bool, decline: bool| {
        let mut state = crimson_game();
        if tech {
            super::give(&mut state, "a", "subatomic");
        }
        state.player_mut(&a()).unwrap().trade_goods = 12;
        let (system, _) = arena(&mut state);
        state.active = Some(a());
        put(&mut state, &system, "cruiser", &a(), 1);
        put(&mut state, &system, "cruiser", &b(), 1);
        let home = state.player(&a()).unwrap().home_system.clone().unwrap();
        let before = ships(&state, &home, &a(), "cruiser");
        let rule = if decline { "decline" } else { "~subatomic" };
        let (mut table, seen) = eager_table_seen(&[rule]);
        // a's cruiser misses, b's hits: the Crimson cruiser is destroyed.
        let _ = armed_fight(&mut state, &system, &[1, 10], &mut table);
        (
            ships(&state, &home, &a(), "cruiser") - before,
            was_offered_like(&seen, "subatomic"),
        )
    };
    assert_eq!(
        rebuilt(true, false),
        (1, true),
        "a cruiser appears at the home dock"
    );
    assert_eq!(rebuilt(true, true), (0, true), "declined");
    assert_eq!(rebuilt(false, false), (0, false), "no technology, no offer");
}

#[test]
fn crimsonagent_1_and_2_ahk_ravin_lets_a_player_swap_two_ships_with_their_cargo_and_readies_at_status()
 {
    let setup = || {
        let mut state = crimson_game();
        assert_eq!(
            leader_status(&state, &a(), "crimsonagent"),
            Some(LeaderStatus::Readied),
            "starts ready"
        );
        // Sol's carrier at 25 holds two infantry; its destroyer sits at 26.
        put(&mut state, &sys("25"), "sol_carrier", &b(), 1);
        put(&mut state, &sys("25"), "infantry", &b(), 2);
        put(&mut state, &sys("26"), "destroyer", &b(), 1);
        state
    };
    let swap = [
        "component|leader|crimsonagent",
        "25|sol_carrier|whole",
        "26|destroyer|whole",
        "~load|",
        "~load|",
    ];
    let (mut game, seen) = action_game(setup(), "a", None, &swap);
    steps(&mut game, 6);
    let state = &game.state;
    assert_eq!(
        ships(state, &sys("26"), &b(), "sol_carrier"),
        1,
        "{:?}",
        asked(&seen)
    );
    assert_eq!(ships(state, &sys("25"), &b(), "destroyer"), 1);
    assert_eq!(
        ships(state, &sys("26"), &b(), "infantry"),
        2,
        "the carrier took its cargo along: {:?}",
        asked(&seen)
    );
    assert_eq!(ships(state, &sys("25"), &b(), "infantry"), 0);
    assert_eq!(
        leader_status(state, &a(), "crimsonagent"),
        Some(LeaderStatus::Exhausted)
    );
    let mut after = game.state.clone();
    run_status_phase(&mut after);
    assert_eq!(
        leader_status(&after, &a(), "crimsonagent"),
        Some(LeaderStatus::Readied)
    );
    // The chosen player may decline: nothing moves.
    let (mut game, _) = action_game(
        setup(),
        "a",
        None,
        &["component|leader|crimsonagent", "decline"],
    );
    steps(&mut game, 3);
    assert_eq!(ships(&game.state, &sys("25"), &b(), "sol_carrier"), 1);
    assert_eq!(ships(&game.state, &sys("26"), &b(), "destroyer"), 1);
}

/// Seat a (Crimson) owns Exile II after a real research with the Technology card (two red
/// prerequisites given), returning the game after the research.
fn research_exile2() -> GameState {
    let mut state = crimson_game();
    super::give(&mut state, "a", "ps");
    super::give(&mut state, "a", "aida");
    state.player_mut(&a()).unwrap().strategy_cards =
        vec![ti4_model::id::StrategyCardId::new("pok7technology")];
    state.player_mut(&b()).unwrap().strategic_tokens = 0;
    state.player_mut(&c()).unwrap().strategic_tokens = 0;
    let (mut game, seen) = action_game(state, "a", None, &["strategic", "exile2"]);
    steps(&mut game, 8);
    assert!(
        super::owns(&game.state, "a", "exile2"),
        "{:?}",
        asked(&seen)
    );
    game.state
}

#[test]
fn exile2_1_researching_it_replaces_exile_i_and_the_destroyer_has_the_upgraded_stats_in_combat() {
    let state = research_exile2();
    let count = |state: &GameState, kind: &str| {
        state
            .board
            .values()
            .flat_map(|board| board.units.iter())
            .filter(|unit| unit.owner == a() && unit.type_id.as_str() == kind)
            .count()
    };
    assert_eq!(count(&state, "crimson_destroyer"), 0, "Exile I is gone");
    assert!(
        count(&state, "crimson_destroyer2") > 0,
        "the starting destroyers were upgraded"
    );
    // Combat 7, ANTI-FIGHTER BARRAGE 6 (x3), in a real combat against fighters.
    let mut fight = state.clone();
    let (system, _) = arena(&mut fight);
    fight.active = Some(a());
    put(&mut fight, &system, "crimson_destroyer2", &a(), 1);
    put(&mut fight, &system, "fighter", &b(), 4);
    let (_, dice) = space_fight(&mut fight, &system, &[1; 6], &mut Table::new());
    assert_eq!(rolls(&dice, "a", "anti-fighter barrage")[0], (Some(6), 3));
    assert_eq!(first_roll(&dice, "a").map(|(_, n)| n), Some(3));
    let combat = rolls(&dice, "a", "space combat");
    assert_eq!(combat[0], (Some(7), 1), "combat 7, one die");
}

/// A combat between Sol (b) and Hacan (c) in the first ring system, the Crimson (a) holding a
/// destroyer of kind `exile` in ring system `at`. `rules` answer a's questions.
fn exile_combat(exile: &str, at: usize, rules: &[&str]) -> (GameState, Vec<String>, Seen) {
    let (galaxy, ring) = crimson_ring(None);
    let mut state = crimson_game();
    let fought = sys(&ring[0]);
    state.active = Some(b());
    put(&mut state, &fought, "cruiser", &b(), 1);
    put(&mut state, &fought, "cruiser", &c(), 1);
    put(&mut state, &sys(&ring[at]), exile, &a(), 1);
    let (mut table, seen) = table_seen(rules);
    // b's die hits, c's misses: Sol keeps the system.
    let _ = armed_fight_on(&mut state, &fought, &[10, 1], &mut table, Some(&galaxy));
    (state, ring, seen)
}

#[test]
fn exile2_2_an_upgraded_destroyer_two_systems_away_may_place_an_active_or_inactive_breach() {
    // Exile II at ring[2], two steps from the combat in ring[0]: the Crimson may choose active.
    let (state, ring, seen) =
        exile_combat("crimson_destroyer2", 2, &["~crimson_destroyer", "active"]);
    assert!(is_active(&state, &sys(&ring[0])), "{:?}", asked(&seen));
    let (state, ring, _) =
        exile_combat("crimson_destroyer2", 2, &["~crimson_destroyer", "inactive"]);
    assert!(has_breach(&state, &sys(&ring[0])) && !is_active(&state, &sys(&ring[0])));
    let (state, ring, _) = exile_combat("crimson_destroyer2", 2, &["decline"]);
    assert!(
        !has_breach(&state, &sys(&ring[0])),
        "the placement is a may"
    );
    // Exile I reaches only one system, and only an inactive breach.
    let (state, ring, _) = exile_combat("crimson_destroyer", 2, &["~crimson_destroyer"]);
    assert!(
        !has_breach(&state, &sys(&ring[0])),
        "two systems away is out of reach for Exile I"
    );
    let (state, ring, seen) = exile_combat("crimson_destroyer", 1, &["~crimson_destroyer"]);
    assert!(
        has_breach(&state, &sys(&ring[0])) && !is_active(&state, &sys(&ring[0])),
        "{:?}",
        asked(&seen)
    );
    assert!(!was_offered(&seen, "active"));
}

#[test]
fn crimsoncommander_1_placing_a_breach_where_another_players_unit_stands_unlocks_ahk_siever() {
    // The breach goes into the combat system, where Sol's surviving cruiser stands.
    let (state, _, seen) = exile_combat("crimson_destroyer", 1, &["~crimson_destroyer"]);
    assert_eq!(
        leader_status(&state, &a(), "crimsoncommander"),
        Some(LeaderStatus::Unlocked),
        "{:?}",
        asked(&seen)
    );
    // Declining the breach leaves the commander locked.
    let (state, _, _) = exile_combat("crimson_destroyer", 1, &["decline"]);
    assert_eq!(
        leader_status(&state, &a(), "crimsoncommander"),
        Some(LeaderStatus::Locked)
    );
}

#[test]
fn crimsoncommander_2_ahk_siever_pays_a_commodity_at_the_end_of_any_combat_once_unlocked() {
    let paid = |unlocked: bool, commodities: i32| {
        let mut state = crimson_game();
        if unlocked {
            set_leader(&mut state, &a(), "crimsoncommander", LeaderStatus::Unlocked);
        }
        state.player_mut(&a()).unwrap().commodities = commodities;
        let (system, _) = arena(&mut state);
        state.active = Some(b());
        put(&mut state, &system, "cruiser", &b(), 1);
        put(&mut state, &system, "cruiser", &c(), 1);
        let goods = state.player(&a()).unwrap().trade_goods;
        let (mut table, seen) = table_seen(&["convert"]);
        let _ = armed_fight(&mut state, &system, &[10, 1], &mut table);
        let seat = state.player(&a()).unwrap();
        (seat.commodities, seat.trade_goods - goods, asked(&seen))
    };
    assert_eq!(
        paid(true, 0).0,
        1,
        "a combat between two other players: gain 1 commodity"
    );
    let (commodities, goods, log) = paid(true, 1);
    assert_eq!(
        (commodities, goods),
        (0, 1),
        "a commodity in hand and room: the holder converted it: {log:?}"
    );
    assert_eq!(paid(false, 0).0, 0, "locked: nothing");
}

#[test]
fn crimsonhero_1_unlocks_on_the_third_scored_objective() {
    hero_unlocks_by_scoring("crimson", "crimsonhero");
}

#[test]
fn crimsonhero_2_homesick_phantom_takes_produced_ships_onto_its_card_and_launches_them_into_a_combat()
 {
    let (galaxy, _) = crimson_ring(None);
    let mut state = crimson_game();
    set_leader(&mut state, &a(), "crimsonhero", LeaderStatus::Unlocked);
    state.player_mut(&a()).unwrap().trade_goods = 12;
    state.player_mut(&a()).unwrap().fleet_tokens = 12;
    let home = state.player(&a()).unwrap().home_system.clone().unwrap();
    let on_board = ships(&state, &home, &a(), "crimson_destroyer");
    // Produce a destroyer in the home system and place it onto the hero card.
    let (mut game, seen) = action_game(
        state,
        "a",
        Some(galaxy),
        &[
            TACTICAL_ACTION_ID,
            home.as_str(),
            "done_moving",
            "build|crimson_destroyer|1",
            "place|crimsoncard@space",
            "done_producing",
        ],
    );
    for _ in 0..10 {
        assert_eq!(game.step().error, None, "log: {:?}", game.events);
    }
    let card = crate::factions::crimson_cards::card_ships(&game.state, &a());
    assert_eq!(
        card,
        vec!["crimson_destroyer".to_owned()],
        "{:?}",
        asked(&seen)
    );
    assert_eq!(
        ships(&game.state, &home, &a(), "crimson_destroyer"),
        on_board,
        "it is on the card, not the board"
    );
    // At the start of a space combat of the owner's, the card is purged and the ship placed.
    let mut state = game.state.clone();
    let (system, _) = arena(&mut state);
    state.active = Some(a());
    put(&mut state, &system, "cruiser", &a(), 1);
    put(&mut state, &system, "cruiser", &b(), 1);
    let (mut table, seen) = table_seen(&["~crimsonhero"]);
    let (_, dice) = armed_fight(&mut state, &system, &[1, 1, 1, 1, 1], &mut table);
    assert!(was_offered_like(&seen, "crimsonhero"), "{:?}", asked(&seen));
    assert_eq!(
        first_batch(&dice, "a", "space combat"),
        2,
        "the launched destroyer rolled with the cruiser"
    );
    assert_eq!(
        leader_status(&state, &a(), "crimsonhero"),
        Some(LeaderStatus::Purged)
    );
    assert!(crate::factions::crimson_cards::card_ships(&state, &a()).is_empty());
}

#[test]
fn sever_1_and_2_the_played_note_silences_a_systems_wormholes_for_everyone_until_the_status_phase_ends()
 {
    let note = crate::promissory::note_id("sever", "crimson");
    let alpha = wormhole_tiles("ALPHA");
    let others = plain(5);
    let ring = [
        alpha[0].as_str(),
        others[0].as_str(),
        others[1].as_str(),
        alpha[1].as_str(),
        others[2].as_str(),
        others[3].as_str(),
    ];
    let galaxy = ring_map(&others[4], &ring);
    let mut state = crimson_game();
    crate::promissory::deal(&mut state, content(), DEFAULT);
    assert_eq!(
        state.promissory_notes.get(&note),
        Some(&a()),
        "dealt to the Rebellion"
    );
    crate::promissory::take(&mut state, content(), &b(), &note);
    put(&mut state, &sys(ring[0]), "carrier", &b(), 1);
    // b plays the note as an ACTION and puts the sever token in the alpha system holding its carrier.
    let (mut game, seen) = action_game(
        state,
        "b",
        Some(galaxy.clone()),
        &["faction|crimson|sever", ring[0]],
    );
    steps(&mut game, 3);
    assert!(
        game.state.promissory_faceup.contains(&note),
        "placed faceup: {:?}",
        asked(&seen)
    );
    assert_eq!(
        game.state.promissory_notes.get(&note),
        Some(&b()),
        "still in b's play area"
    );
    // The wormhole there has no effect during movement: even Sol's own carrier cannot use it.
    let mut after = game.state.clone();
    for seat in after.players.iter_mut() {
        seat.passed = false;
    }
    let rule = format!("move|{}|0", ring[0]);
    let (moved, seen) = tactical(
        after.clone(),
        "b",
        galaxy.clone(),
        ring[3],
        &[&rule, "done_moving"],
    );
    assert!(!was_offered(&seen, &rule), "{:?}", asked(&seen));
    assert_eq!(ships(&moved.state, &sys(ring[3]), &b(), "carrier"), 0);
    // Control: without the token the same carrier uses the wormhole.
    let mut control = after.clone();
    control
        .faction_marks
        .retain(|key, _| !key.starts_with("crimson:sever:"));
    let (_, seen) = tactical(control, "b", galaxy, ring[3], &[&rule, "done_moving"]);
    assert!(was_offered(&seen, &rule), "{:?}", asked(&seen));
    // At the end of the status phase the token goes and the card returns to the Rebellion player.
    let (done, _) = status_game(after, &[]);
    assert_eq!(
        done.state.promissory_notes.get(&note),
        Some(&a()),
        "returned to its owner"
    );
    assert!(
        done.state
            .faction_marks
            .keys()
            .all(|key| !key.starts_with("crimson:sever:"))
    );
    assert!(!done.state.promissory_faceup.contains(&note));
}

#[test]
fn crimsonbt_1_the_resonance_generator_adds_one_move_to_ships_starting_at_home_or_in_an_active_breach()
 {
    // A carrier (move 1) at the home system 118 reaches the system beside the Sorrow (two jumps,
    // epsilon then a hex); another starting in an active breach reaches a system two hexes away.
    let (galaxy, ring) = crimson_ring(Some("94"));
    let beside = ring[1].clone();
    let far = ring[2].clone();
    let reaches = |bt: bool, origin: &str, target: &str, active_breach: bool| {
        let mut state = crimson_game();
        if bt {
            state.player_mut(&a()).unwrap().breakthrough =
                Some(ti4_model::id::BreakthroughId::new("crimsonbt"));
        }
        if active_breach {
            breach(&mut state, &sys(origin), true);
        }
        put(&mut state, &sys(origin), "carrier", &a(), 1);
        let rule = format!("~move|{origin}|");
        let (game, _) = tactical(state, "a", galaxy.clone(), target, &[&rule, "done_moving"]);
        ships(&game.state, &sys(target), &a(), "carrier")
    };
    assert_eq!(
        reaches(true, "118", &beside, false),
        1,
        "starting at home: +1"
    );
    assert_eq!(
        reaches(false, "118", &beside, false),
        0,
        "no breakthrough: no bonus"
    );
    assert_eq!(
        reaches(true, &ring[3], &ring[5], true),
        1,
        "starting in an active breach: +1"
    );
    assert_eq!(
        reaches(true, &ring[3], &ring[5], false),
        0,
        "elsewhere: no bonus"
    );
    assert_eq!(
        reaches(false, &ring[3], &ring[5], true),
        0,
        "no breakthrough: no bonus"
    );
    let _ = far;
}

#[test]
fn crimsonbt_2_the_resonance_generator_flips_or_places_a_breach_exhausts_and_readies_at_status() {
    let generator = |state: &mut GameState| {
        state.player_mut(&a()).unwrap().breakthrough =
            Some(ti4_model::id::BreakthroughId::new("crimsonbt"));
    };
    let mut state = crimson_game();
    generator(&mut state);
    let (mut game, seen) = action_game(
        state.clone(),
        "a",
        None,
        &["faction|crimson|resonance", "flip|94"],
    );
    steps(&mut game, 3);
    assert!(
        is_active(&game.state, &sys("94")),
        "the Sorrow's breach flipped active: {:?}",
        asked(&seen)
    );
    // Exhausted: not offered again this round.
    assert!(
        !crate::factions::component_actions(&game.state, content(), &a())
            .iter()
            .any(|option| option.id == "faction|crimson|resonance")
    );
    // The status phase readies it.
    let (done, _) = status_game(game.state.clone(), &[]);
    assert!(
        crate::factions::component_actions(&done.state, content(), &a())
            .iter()
            .any(|option| option.id == "faction|crimson|resonance"),
        "ready again"
    );
    // Placing an active breach in a non-home system with the Crimson's units.
    put(&mut state, &sys("25"), "cruiser", &a(), 1);
    let (mut game, seen) =
        action_game(state, "a", None, &["faction|crimson|resonance", "place|25"]);
    steps(&mut game, 3);
    assert!(is_active(&game.state, &sys("25")), "{:?}", asked(&seen));
}

#[test]
fn unit_crimson_flagship_1_quietus_takes_sustain_damage_and_anti_fighter_barrage_from_other_players_in_active_breaches()
 {
    // The Quietus (a) fights in an active breach system; Sol's dreadnought (SUSTAIN DAMAGE) and
    // destroyer (ANTI-FIGHTER BARRAGE) stand in it too.
    let run = |breach_active: bool| {
        let mut state = crimson_game();
        let (system, _) = arena(&mut state);
        state.active = Some(a());
        breach(&mut state, &system, breach_active);
        put(&mut state, &system, "crimson_flagship", &a(), 1);
        put(&mut state, &system, "fighter", &a(), 2);
        put(&mut state, &system, "dreadnought", &b(), 1);
        put(&mut state, &system, "destroyer", &b(), 1);
        // Everything a rolls hits for one round (the flagship, two fighters) and Sol's dice miss.
        let (mut table, seen) = table_seen(&["sustain|0"]);
        let (_, dice) = armed_fight(&mut state, &system, &[10, 10, 10, 1, 1, 1, 1], &mut table);
        let sustain_asked = asked(&seen)
            .iter()
            .any(|line| line.starts_with("b: ") && line.contains("sustain"));
        (
            rolls(&dice, "b", "anti-fighter barrage").len(),
            sustain_asked,
        )
    };
    let (barrage, sustain) = run(false);
    assert_eq!(
        (barrage > 0, sustain),
        (true, true),
        "inactive breach: Sol keeps both abilities"
    );
    let (barrage, sustain) = run(true);
    assert_eq!(
        (barrage, sustain),
        (0, false),
        "active breach: Sol's destroyer cannot barrage, its dreadnought cannot sustain"
    );
}

#[test]
fn unit_crimson_flagship_2_quietus_stops_space_cannon_and_bombardment_in_active_breaches() {
    // Space cannon: the Crimson cruiser enters Sol's PDS system (an active breach); the Quietus
    // stands in another active breach.
    let hub = planet_hub();
    let centre = sys(&hub.centre);
    let planet = first_planet(&centre);
    let cannon = |active: bool| {
        let mut state = crimson_game();
        for id in hub.outer.iter().chain(std::iter::once(&hub.centre)) {
            let board = state.system_mut(&sys(id));
            board.units.clear();
            board.planet_units.clear();
        }
        state.system_mut(&centre).set_control(planet.clone(), b());
        put_on_planet(&mut state, &centre, &planet, "pds", &b(), 1);
        put(&mut state, &sys(&hub.outer[0]), "cruiser", &a(), 1);
        put(&mut state, &sys(&hub.outer[1]), "crimson_flagship", &a(), 1);
        breach(&mut state, &sys(&hub.outer[1]), active);
        breach(&mut state, &centre, active);
        let rule = format!("move|{}|0", hub.outer[0]);
        let (game, _) = tactical(
            state,
            "a",
            hub.galaxy.clone(),
            &hub.centre,
            &[&rule, "done_moving"],
        );
        game.rolls()
            .iter()
            .filter(|roll| roll.reason == "space cannon" && roll.by.as_deref() == Some("b"))
            .count()
    };
    assert!(cannon(false) > 0, "inactive breaches: Sol's PDS fires");
    assert_eq!(
        cannon(true),
        0,
        "active breaches: the PDS has lost SPACE CANNON"
    );
    // Bombardment: Sol's dreadnought in an active breach cannot bombard Hacan's planet.
    let bombard = |active: bool| {
        let mut state = crimson_game();
        let (system, planet) = arena(&mut state);
        state.active = Some(b());
        breach(&mut state, &system, active);
        put(&mut state, &system, "crimson_flagship", &a(), 1);
        state.system_mut(&system).set_control(planet.clone(), c());
        put_on_planet(&mut state, &system, &planet, "infantry", &c(), 2);
        put(&mut state, &system, "dreadnought", &b(), 1);
        put(&mut state, &system, "carrier", &b(), 1);
        put(&mut state, &system, "infantry", &b(), 1);
        let (_, dice) = armed_invade(
            &mut state,
            &system,
            &b(),
            &[1; 8],
            &mut table(&["~commit|"]),
        );
        rolls(&dice, "b", "bombardment").len()
    };
    assert!(bombard(false) > 0, "inactive: the dreadnought bombards");
    assert_eq!(
        bombard(true),
        0,
        "active breach and a Quietus: BOMBARDMENT is lost"
    );
}

// -- Firmament / Obsidian -------------------------------------------------------------------------

use crate::factions::firmament as fw;

fn firmament_game() -> GameState {
    seated_all(&[("a", "firmament"), ("b", "sol"), ("c", "hacan")])
}

/// A map whose ring holds the Firmament's home tile `96a` first, six plain systems around a plain
/// centre. Returns the map and the ring ids.
fn firmament_map() -> (Galaxy, Vec<String>) {
    let ids = fw::testkit::plain(7);
    let mut ring: Vec<String> = ids[1..].to_vec();
    ring[0] = "96a".to_owned();
    let refs: Vec<&str> = ring.iter().map(String::as_str).collect();
    (fw::testkit::ring(&ids[0], &refs), ring)
}

/// Seat a with a plot card (Sol's token) in play.
fn with_plot(state: &mut GameState) {
    fw::testkit::place_plot(state, &a(), &b());
}

#[test]
fn plotsplots_1_and_2_scoring_another_players_secret_places_a_facedown_plot_for_no_victory_point() {
    // Sol has scored Fuel the War Machine (3 space docks); the Firmament has 3 space docks.
    let mut state = firmament_game();
    state.record_score(&b(), ti4_model::id::ObjectiveId::new("fwm"));
    let home = state.player(&a()).unwrap().home_system.clone().unwrap();
    let planet = state.controlled_planets(&a())[0].1.clone();
    put_on_planet(&mut state, &home, &planet, "spacedock", &a(), 2);
    let points = state.player(&a()).unwrap().victory_points;
    let scored = state.scored_by(&a()).len();
    let (game, seen) = status_game_eager(state.clone(), &["plot|fwm|b"]);
    let seat = game.state.player(&a()).unwrap();
    assert_eq!(
        seat.victory_points,
        points,
        "no victory point: {:?}",
        asked(&seen)
    );
    assert_eq!(
        game.state.scored_by(&a()).len(),
        scored,
        "and not a scored objective"
    );
    let plots = fw::plots(&game.state, &a());
    assert_eq!(plots.len(), 1, "one plot card placed");
    assert!(!plots[0].faceup, "facedown");
    assert_eq!(
        plots[0].tokens,
        std::collections::BTreeSet::from([b()]),
        "with Sol's control token on it"
    );
    assert!(
        seat.plot_objectives
            .contains(&ti4_model::id::SecretObjectiveId::new("fwm"))
    );
    // A Firmament that does not meet the requirement is not offered it; nor is anyone else.
    let mut poor = state.clone();
    let home_planet = planet.clone();
    poor.system_mut(&home)
        .planet_units
        .get_mut(&home_planet)
        .unwrap()
        .retain(|unit| unit.type_id.as_str() != "spacedock");
    let (_, seen) = status_game_eager(poor, &["plot|fwm|b"]);
    assert!(!was_offered(&seen, "plot|fwm|b"), "{:?}", asked(&seen));
}

#[test]
fn puppetsoftheblade_1_the_action_purges_the_firmament_and_gains_the_obsidian_components() {
    let (galaxy, _) = firmament_map();
    let mut state = firmament_game();
    // Without a plot card the ability is not there.
    let (mut game, seen) = action_game(state.clone(), "a", Some(galaxy.clone()), &[]);
    steps(&mut game, 1);
    assert!(
        !was_offered(&seen, "faction|firmament|become_obsidian"),
        "{:?}",
        asked(&seen)
    );
    with_plot(&mut state);
    state
        .promissory_notes
        .insert("blackops:firmament".to_owned(), a());
    let (mut game, seen) = action_game(
        state,
        "a",
        Some(galaxy),
        &["faction|firmament|become_obsidian"],
    );
    steps(&mut game, 3);
    assert!(was_offered(&seen, "faction|firmament|become_obsidian"));
    let seat = game.state.player(&a()).unwrap();
    assert_eq!(seat.faction.as_str(), "obsidian");
    assert_eq!(
        seat.home_system,
        Some(sys("96b")),
        "the home system is flipped"
    );
    assert_eq!(
        leader_status(&game.state, &a(), "obsidianagent"),
        Some(LeaderStatus::Readied)
    );
    assert_eq!(
        leader_status(&game.state, &a(), "obsidiancommander"),
        Some(LeaderStatus::Locked)
    );
    assert_eq!(
        leader_status(&game.state, &a(), "obsidianhero"),
        Some(LeaderStatus::Locked)
    );
    assert_eq!(
        leader_status(&game.state, &a(), "firmamentagent"),
        None,
        "the Firmament's leaders are purged"
    );
    assert!(
        !game
            .state
            .promissory_notes
            .contains_key("blackops:firmament"),
        "the Firmament's promissory note is purged"
    );
    assert!(
        game.state
            .promissory_notes
            .contains_key("malevolency:obsidian"),
        "the Obsidian's note is gained"
    );
    assert!(
        fw::plots(&game.state, &a()).iter().all(|plot| plot.faceup),
        "plot cards flip faceup"
    );
}

/// The Firmament seat (a), flipped into the Obsidian by its real action in a running game, with the
/// map the game used. `setup` may change the state first (after the plot card is placed).
fn obsidian_game(setup: impl FnOnce(&mut GameState)) -> (GameState, Galaxy, Vec<String>) {
    let (galaxy, ring) = firmament_map();
    let mut state = firmament_game();
    with_plot(&mut state);
    setup(&mut state);
    let (mut game, _) = action_game(
        state,
        "a",
        Some(galaxy.clone()),
        &["faction|firmament|become_obsidian"],
    );
    steps(&mut game, 3);
    assert_eq!(
        game.state.player(&a()).unwrap().faction.as_str(),
        "obsidian"
    );
    (game.state, galaxy, ring)
}

#[test]
fn firmamentagent_2_myru_vos_moves_ships_through_other_players_ships_and_silences_space_cannon() {
    let vos = "leader:firmament:firmamentagent:SYSTEM_ACTIVATED:after";
    // Through a blockade: Sol's cruiser (move 2) goes through the centre where Hacan has a cruiser.
    let hub = crate::fixtures::plain_hub();
    let from = hub.outer[0].clone();
    let target = hub.across(&from);
    let run = |agent: bool| {
        let mut state = firmament_game();
        put(&mut state, &sys(&from), "cruiser", &b(), 1);
        put(&mut state, &sys(&hub.centre), "cruiser", &c(), 1);
        let rule = format!("move|{from}|0");
        let rules = [
            if agent { vos } else { "decline" },
            rule.as_str(),
            "done_moving",
        ];
        let (game, seen) = tactical(state, "b", hub.galaxy.clone(), &target, &rules);
        (
            ships(&game.state, &sys(&target), &b(), "cruiser"),
            leader_status(&game.state, &a(), "firmamentagent"),
            asked(&seen),
        )
    };
    let (arrived, status, log) = run(true);
    assert_eq!(
        (arrived, status),
        (1, Some(LeaderStatus::Exhausted)),
        "{log:?}"
    );
    let (arrived, status, _) = run(false);
    assert_eq!(
        (arrived, status),
        (0, Some(LeaderStatus::Readied)),
        "no agent: the blockade holds"
    );
    // Ready again after the status phase.
    let mut after = firmament_game();
    set_leader(&mut after, &a(), "firmamentagent", LeaderStatus::Exhausted);
    run_status_phase(&mut after);
    assert_eq!(
        leader_status(&after, &a(), "firmamentagent"),
        Some(LeaderStatus::Readied)
    );
    // SPACE CANNON cannot be used against the ships that moved: Hacan's PDS holds its fire.
    let planets = planet_hub();
    let centre = sys(&planets.centre);
    let planet = first_planet(&centre);
    let cannon = |agent: bool| {
        let mut state = firmament_game();
        for id in planets.outer.iter().chain(std::iter::once(&planets.centre)) {
            let board = state.system_mut(&sys(id));
            board.units.clear();
            board.planet_units.clear();
        }
        state.system_mut(&centre).set_control(planet.clone(), c());
        put_on_planet(&mut state, &centre, &planet, "pds", &c(), 1);
        put(&mut state, &sys(&planets.outer[0]), "cruiser", &b(), 1);
        let rule = format!("move|{}|0", planets.outer[0]);
        let rules = [
            if agent { vos } else { "decline" },
            rule.as_str(),
            "done_moving",
        ];
        let (game, _) = tactical(state, "b", planets.galaxy.clone(), &planets.centre, &rules);
        game.rolls()
            .iter()
            .filter(|roll| roll.reason == "space cannon" && roll.by.as_deref() == Some("c"))
            .count()
    };
    assert!(cannon(false) > 0, "without the agent the PDS fires");
    assert_eq!(
        cannon(true),
        0,
        "with Myru Vos, SPACE CANNON cannot be used against those ships"
    );
}

#[test]
fn firmamentcommander_1_unlocks_in_the_game_with_a_plot_card_in_play() {
    let mut state = firmament_game();
    assert!(
        !unlocks_in_the_game(&state, &a(), "firmamentcommander"),
        "no plot"
    );
    with_plot(&mut state);
    assert!(
        unlocks_in_the_game(&state, &a(), "firmamentcommander"),
        "a plot card in play"
    );
}

#[test]
fn firmamentcommander_2_captain_aroz_counts_planets_in_systems_with_the_owners_ships_for_secrets() {
    // Seize an Icon (control a legendary planet): the Firmament has only a ship in a legendary system.
    let legendary = {
        ti4_content::galaxy::all_systems(content(), DEFAULT)
            .into_iter()
            .find(|(id, _)| {
                ti4_content::galaxy::system(content(), id, DEFAULT).is_some_and(|system| {
                    system.planets().iter().any(|planet| {
                        ti4_content::galaxy::planet(content(), planet, DEFAULT)
                            .is_some_and(|planet| planet.is_legendary())
                    })
                })
            })
            .map(|(id, _)| sys(id))
            .expect("a legendary system")
    };
    let offered = |unlocked: bool| {
        let mut state = firmament_game();
        state.player_mut(&a()).unwrap().secret_objectives =
            vec![ti4_model::id::SecretObjectiveId::new("sai")];
        if unlocked {
            set_leader(
                &mut state,
                &a(),
                "firmamentcommander",
                LeaderStatus::Unlocked,
            );
        }
        put(&mut state, &legendary, "cruiser", &a(), 1);
        let (_, seen) = status_game_eager(state, &[]);
        was_offered(&seen, "sai")
    };
    assert!(
        !offered(false),
        "the planet is not the Firmament's: the secret cannot be scored"
    );
    assert!(
        offered(true),
        "Captain Aroz: planets in systems with its ships count as controlled"
    );
}

#[test]
fn firmamenthero_1_unlocks_on_the_third_scored_objective() {
    hero_unlocks_by_scoring("firmament", "firmamenthero");
}

#[test]
fn firmamenthero_2_sharsiss_places_a_plot_with_another_players_token_may_add_a_token_and_is_purged()
{
    let mut state = firmament_game();
    with_plot(&mut state);
    set_leader(&mut state, &a(), "firmamenthero", LeaderStatus::Unlocked);
    // Sol's token is already on the first plot; the hero places a new plot with Hacan's token, then
    // puts Hacan's token on the first plot too (index 0).
    let (mut game, seen) = action_game(
        state,
        "a",
        None,
        &["component|leader|firmamenthero", "c", "siphon", "token|c|0"],
    );
    steps(&mut game, 5);
    let plots = fw::plots(&game.state, &a());
    assert_eq!(
        plots.len(),
        2,
        "a second plot card is in play: {:?}",
        asked(&seen)
    );
    assert!(plots[1].tokens.contains(&c()) && !plots[1].tokens.contains(&b()));
    assert!(
        plots[0].tokens.contains(&b()) && plots[0].tokens.contains(&c()),
        "and a token was added to the first: {:?} {:?}",
        plots,
        asked(&seen)
    );
    assert_eq!(
        leader_status(&game.state, &a(), "firmamenthero"),
        Some(LeaderStatus::Purged)
    );
}

#[test]
fn blackops_1_and_2_the_note_is_dealt_lent_and_a_receiving_player_gains_two_tokens_and_trade_goods()
{
    let note = crate::promissory::note_id("blackops", "firmament");
    let hub = crate::fixtures::plain_hub();
    let mut state = firmament_game();
    crate::promissory::deal(&mut state, content(), DEFAULT);
    assert_eq!(
        state.promissory_notes.get(&note),
        Some(&a()),
        "dealt to the Firmament"
    );
    let centre = sys(&hub.centre);
    put(&mut state, &centre, "cruiser", &a(), 1);
    put(&mut state, &centre, "cruiser", &b(), 1);
    let goods = state.player(&b()).unwrap().trade_goods;
    let tokens = |state: &GameState| {
        let seat = state.player(&b()).unwrap();
        seat.tactic_tokens + seat.fleet_tokens + seat.strategic_tokens
    };
    let before = tokens(&state);
    let offer = format!("pn{note}:0");
    let (mut game, seen) = action_game(
        state,
        "a",
        Some(hub.galaxy),
        &[
            "component|trade|sol",
            offer.as_str(),
            "accept",
            "place",
            "enervate",
        ],
    );
    step_until(&mut game, "TRANSACTION_RESOLVED", 8);
    steps(&mut game, 3);
    assert!(was_offered(&seen, &offer), "{:?}", asked(&seen));
    assert_eq!(
        game.state.player(&b()).unwrap().trade_goods,
        goods + 2,
        "2 trade goods"
    );
    assert_eq!(tokens(&game.state), before + 2, "2 command tokens");
    assert!(
        !game.state.promissory_notes.contains_key(&note),
        "the card is purged"
    );
    let plots = fw::plots(&game.state, &a());
    assert_eq!(plots.len(), 1, "the Firmament placed a facedown plot");
    assert!(plots[0].tokens.contains(&b()) && !plots[0].faceup);
}

#[test]
fn firmamentbt_1_the_sowing_takes_up_to_three_trade_goods_when_gained_and_at_the_status_phase() {
    let mut state = firmament_game();
    state.player_mut(&a()).unwrap().trade_goods = 8;
    // Gaining the breakthrough in a running game opens the placement.
    let (mut game, seen) = action_game(state, "a", None, &["3"]);
    steps(&mut game, 1);
    game.state.player_mut(&a()).unwrap().breakthrough =
        Some(ti4_model::id::BreakthroughId::new("firmamentbt"));
    steps(&mut game, 2);
    assert_eq!(
        fw::goods_on_card(&game.state, &a()),
        3,
        "{:?}",
        asked(&seen)
    );
    assert_eq!(game.state.player(&a()).unwrap().trade_goods, 5);
    // At the start of the status phase, again, "up to 3": two this time.
    let (done, seen) = status_game(game.state.clone(), &["2"]);
    assert_eq!(
        fw::goods_on_card(&done.state, &a()),
        5,
        "{:?}",
        asked(&seen)
    );
    assert_eq!(done.state.player(&a()).unwrap().trade_goods, 3);
    // A may: declining places nothing.
    let (done, _) = status_game(game.state.clone(), &["decline"]);
    assert_eq!(fw::goods_on_card(&done.state, &a()), 3);
}

/// A research with the Technology strategy card played by seat `a` in a game of `state`.
fn research(state: GameState, tech: &str) -> (Game<'static>, Seen) {
    let mut state = state;
    state.player_mut(&a()).unwrap().strategy_cards =
        vec![ti4_model::id::StrategyCardId::new("pok7technology")];
    for other in [b(), c()] {
        if let Some(seat) = state.player_mut(&other) {
            seat.strategic_tokens = 0;
        }
    }
    let (mut game, seen) = eager_game(state, "a", None, &["strategic", tech]);
    steps(&mut game, 8);
    (game, seen)
}

#[test]
fn planesplitter_firm_1_gaining_it_puts_the_fracture_into_play_in_a_real_research() {
    let mut state = firmament_game();
    super::give(&mut state, "a", "st");
    super::give(&mut state, "a", "sdn");
    assert!(!state.fracture_in_play);
    let (game, seen) = research(state.clone(), "planesplitter-firm");
    assert!(
        super::owns(&game.state, "a", "planesplitter-firm"),
        "{:?}",
        asked(&seen)
    );
    assert!(game.state.fracture_in_play, "The Fracture is in play");
    assert!(
        !game.state.ingress_tokens.is_empty(),
        "with its ingress tokens"
    );
    // Researching an ordinary technology does not.
    super::give(&mut state, "a", "pa");
    let (game, _) = research(state, "gls");
    assert!(!game.state.fracture_in_play);
}

#[test]
fn parasite_firm_1_places_an_infantry_on_a_home_planet_at_the_start_of_the_status_phase() {
    let infantry = |state: &GameState| {
        let home = state.player(&a()).unwrap().home_system.clone().unwrap();
        state
            .controlled_planets(&a())
            .into_iter()
            .filter(|(system, _)| **system == home)
            .map(|(system, planet)| on_planet(state, system, planet, &a(), "infantry"))
            .sum::<usize>()
    };
    let run = |tech: bool, rule: &str| {
        let mut state = firmament_game();
        if tech {
            super::give(&mut state, "a", "parasite-firm");
        }
        let before = infantry(&state);
        let (game, _) = status_game(state, &[rule]);
        infantry(&game.state) - before
    };
    assert_eq!(
        run(true, "cronos"),
        1,
        "one infantry from reinforcements on Cronos"
    );
    assert_eq!(run(true, "decline"), 0, "a may");
    assert_eq!(run(false, "cronos"), 0, "no technology");
}

#[test]
fn planesplitter_firm_2_and_parasite_firm_2_the_cards_flip_when_the_faction_becomes_the_obsidian() {
    let (state, _, _) = obsidian_game(|state| {
        super::give(state, "a", "planesplitter-firm");
        super::give(state, "a", "parasite-firm");
        state
            .player_mut(&a())
            .unwrap()
            .exhausted_technologies
            .insert(ti4_model::id::TechnologyId::new("parasite-firm"));
    });
    assert!(
        super::owns(&state, "a", "planesplitter-obs") && super::owns(&state, "a", "parasite-obs")
    );
    assert!(
        !super::owns(&state, "a", "planesplitter-firm")
            && !super::owns(&state, "a", "parasite-firm")
    );
    assert!(
        state
            .player(&a())
            .unwrap()
            .exhausted_technologies
            .contains(&ti4_model::id::TechnologyId::new("parasite-obs")),
        "exhaustion is kept on the flipped card"
    );
}

#[test]
fn planesplitter_obs_1_a_strategic_action_may_move_an_ingress_token_next_to_the_owners_units() {
    let (mut state, galaxy, _) = obsidian_game(|_| {});
    super::give(&mut state, "a", "planesplitter-obs");
    state.fracture_in_play = true;
    state.ingress_tokens.insert(sys("19"));
    state.player_mut(&a()).unwrap().strategy_cards =
        vec![ti4_model::id::StrategyCardId::new("pok1leadership")];
    let (mut game, seen) = eager_game(
        state.clone(),
        "a",
        Some(galaxy.clone()),
        &["strategic", "~planesplitter-obs", "19>96b"],
    );
    steps(&mut game, 6);
    assert!(
        game.state.ingress_tokens.contains(&sys("96b")),
        "{:?}",
        asked(&seen)
    );
    assert!(
        !game.state.ingress_tokens.contains(&sys("19")),
        "moved, not copied"
    );
    // A may.
    let (mut game, _) = eager_game(state, "a", Some(galaxy), &["strategic", "decline"]);
    steps(&mut game, 6);
    assert!(game.state.ingress_tokens.contains(&sys("19")));
}

#[test]
fn planesplitter_obs_2_and_parasite_obs_2_the_flipped_technologies_cannot_be_researched() {
    let (state, _, _) = obsidian_game(|_| {});
    let (_, seen) = research(state, "aida");
    assert!(
        !was_offered(&seen, "planesplitter-obs") && !was_offered(&seen, "parasite-obs"),
        "{:?}",
        asked(&seen)
    );
    // The same Technology action offers a researchable card, so the question was asked.
    assert!(was_offered(&seen, "aida"), "{:?}", asked(&seen));
}

#[test]
fn parasite_obs_1_destroys_an_infantry_of_another_player_next_to_the_owners_infantry_at_turn_start()
{
    let infantry_left = |tech: bool| {
        let (mut state, galaxy, _) = obsidian_game(|_| {});
        if tech {
            super::give(&mut state, "a", "parasite-obs");
        }
        let centre = sys(&galaxy_centre(&galaxy));
        let planet = first_planet(&centre);
        state.system_mut(&centre).set_control(planet.clone(), b());
        put_on_planet(&mut state, &centre, &planet, "infantry", &b(), 2);
        let (mut game, seen) = action_game(state, "a", Some(galaxy), &[]);
        steps(&mut game, 1);
        let _ = seen;
        on_planet(&game.state, &centre, &planet, &b(), "infantry")
    };
    assert_eq!(
        infantry_left(true),
        1,
        "one Sol infantry in the system next to the Obsidian's home is destroyed"
    );
    assert_eq!(
        infantry_left(false),
        2,
        "without the technology nothing happens"
    );
}

/// The centre system of a one-ring map built from [`firmament_map`].
fn galaxy_centre(_galaxy: &Galaxy) -> String {
    fw::testkit::plain(7)[0].clone()
}

// -- Obsidian ---------------------------------------------------------------------------------------

use crate::factions::obsidian as ob;

#[test]
fn nocturne_1_the_obsidian_cannot_be_chosen_during_setup() {
    let players = [PlayerId::new("p0"), PlayerId::new("p1")];
    assert_eq!(
        crate::seating::seeded_faction_assignments(&["sol", "obsidian"], &players, 1),
        Err(crate::seating::FactionAssignmentError::NotChoosable(
            "obsidian".to_owned()
        ))
    );
    assert!(crate::seating::seeded_faction_assignments(&["sol", "hacan"], &players, 1).is_ok());
}

#[test]
fn bladesorchestra_1_coming_into_play_flips_the_home_the_plots_and_readies_the_hollows_it_controls()
{
    let (state, _, _) = obsidian_game(|state| {
        state.exhausted_planets.insert(PlanetId::new("cronos"));
        state.exhausted_planets.insert(PlanetId::new("tallin"));
        fw::testkit::place_card(state, &a(), "seethe", &c());
    });
    let seat = state.player(&a()).unwrap();
    assert_eq!(seat.home_system, Some(sys("96b")));
    assert_eq!(
        seat.home_planets,
        [PlanetId::new("cronoshollow"), PlanetId::new("tallinhollow")]
    );
    for hollow in ["cronoshollow", "tallinhollow"] {
        assert_eq!(
            state
                .system_state(&sys("96b"))
                .planet_control
                .get(&PlanetId::new(hollow)),
            Some(&a()),
            "{hollow}"
        );
        assert!(
            !state.exhausted_planets.contains(&PlanetId::new(hollow)),
            "{hollow} is ready"
        );
    }
    assert!(
        fw::plots(&state, &a()).iter().all(|plot| plot.faceup),
        "every plot is flipped"
    );
    assert!(
        state.board.get(&sys("96a")).is_none(),
        "the old home tile is gone"
    );
}

#[test]
fn marionettes_1_the_players_with_tokens_on_a_plot_are_its_puppeted_players() {
    // Assail: +1 to the Obsidian's combat rolls against the players with tokens on that plot.
    let (state, _, _) = obsidian_game(|state| {
        fw::testkit::place_card(state, &a(), "assail", &b());
    });
    assert_eq!(
        fw::puppets_of(&state, &a(), "assail"),
        std::collections::BTreeSet::from([b()])
    );
    let threshold = |against: PlayerId| {
        let mut state = state.clone();
        let (system, _) = arena(&mut state);
        state.active = Some(a());
        put(&mut state, &system, "cruiser", &a(), 1);
        put(&mut state, &system, "cruiser", &against, 1);
        let (_, dice) = armed_fight(&mut state, &system, &[1, 1], &mut table(&[]));
        first_roll(&dice, "a").and_then(|(hits_on, _)| hits_on)
    };
    assert_eq!(
        threshold(b()),
        Some(6),
        "against the puppeted player: 7 improved by 1"
    );
    assert_eq!(threshold(c()), Some(7), "against everyone else: unchanged");
}

#[test]
fn obsidianagent_1_and_2_vos_hollow_makes_the_opponent_destroy_a_ship_of_the_same_type_and_readies_at_status()
 {
    let run = |decline: bool| {
        let (mut state, _, _) = obsidian_game(|_| {});
        assert_eq!(
            leader_status(&state, &a(), "obsidianagent"),
            Some(LeaderStatus::Readied)
        );
        let (system, _) = arena(&mut state);
        state.active = Some(a());
        put(&mut state, &system, "cruiser", &a(), 1);
        put(&mut state, &system, "cruiser", &b(), 2);
        // a's cruiser misses, Sol's two hit: the Obsidian's cruiser is destroyed.
        let rule = if decline { "decline" } else { "~obsidianagent" };
        let (mut table, seen) = table_seen(&[rule]);
        let _ = armed_fight(&mut state, &system, &[1, 10, 10], &mut table);
        (state, system, asked(&seen))
    };
    let (state, system, log) = run(false);
    assert_eq!(
        ships(&state, &system, &b(), "cruiser"),
        1,
        "Sol had to destroy one of its own cruisers: {log:?}"
    );
    assert_eq!(
        leader_status(&state, &a(), "obsidianagent"),
        Some(LeaderStatus::Exhausted)
    );
    let mut after = state.clone();
    run_status_phase(&mut after);
    assert_eq!(
        leader_status(&after, &a(), "obsidianagent"),
        Some(LeaderStatus::Readied)
    );
    let (state, system, _) = run(true);
    assert_eq!(
        ships(&state, &system, &b(), "cruiser"),
        2,
        "declined: nothing is destroyed"
    );
    assert_eq!(
        leader_status(&state, &a(), "obsidianagent"),
        Some(LeaderStatus::Readied)
    );
}

#[test]
fn obsidiancommander_1_unlocks_in_the_game_with_units_in_the_fracture() {
    let (mut state, _, _) = obsidian_game(|_| {});
    state.fracture_in_play = true;
    assert!(!unlocks_in_the_game(&state, &a(), "obsidiancommander"));
    let fracture = crate::fracture::systems(content(), DEFAULT)
        .into_iter()
        .next()
        .expect("a Fracture system");
    // Another player's presence there does not unlock it.
    let mut other = state.clone();
    put(&mut other, &fracture, "cruiser", &b(), 1);
    assert!(!unlocks_in_the_game(&other, &a(), "obsidiancommander"));
    put(&mut state, &fracture, "cruiser", &a(), 1);
    assert!(unlocks_in_the_game(&state, &a(), "obsidiancommander"));
}

#[test]
fn obsidiancommander_2_aroz_hollow_adds_one_to_combat_rolls_in_the_fracture_only() {
    let (state, _, _) = obsidian_game(|_| {});
    let fracture = crate::fracture::systems(content(), DEFAULT)
        .into_iter()
        .find(|system| {
            ti4_content::galaxy::system(content(), system.as_str(), DEFAULT)
                .is_some_and(|tile| !tile.planets().is_empty())
        })
        .expect("a Fracture system with a planet");
    let threshold = |unlocked: bool, in_fracture: bool| {
        let mut state = state.clone();
        state.fracture_in_play = true;
        if unlocked {
            set_leader(
                &mut state,
                &a(),
                "obsidiancommander",
                LeaderStatus::Unlocked,
            );
        }
        let system = if in_fracture {
            let board = state.system_mut(&fracture);
            board.units.clear();
            fracture.clone()
        } else {
            arena(&mut state).0
        };
        state.active = Some(a());
        put(&mut state, &system, "cruiser", &a(), 1);
        put(&mut state, &system, "cruiser", &b(), 1);
        let (_, dice) = armed_fight(&mut state, &system, &[1, 1], &mut table(&[]));
        first_roll(&dice, "a").and_then(|(hits_on, _)| hits_on)
    };
    assert_eq!(threshold(true, true), Some(6), "+1 in the Fracture");
    assert_eq!(threshold(true, false), Some(7), "elsewhere: nothing");
    assert_eq!(threshold(false, true), Some(7), "locked: nothing");
}

#[test]
fn obsidianhero_1_unlocks_on_the_third_scored_objective() {
    let (mut state, _, _) = obsidian_game(|_| {});
    assert_eq!(
        leader_status(&state, &a(), "obsidianhero"),
        Some(LeaderStatus::Locked)
    );
    let score = |state: &mut GameState, alias: &str| {
        crate::objectives::award(
            state,
            content(),
            DEFAULT,
            &a(),
            &ti4_model::id::ObjectiveId::new(alias),
        )
        .expect("the objective scores");
    };
    score(&mut state, "corner");
    score(&mut state, "develop");
    assert_eq!(
        leader_status(&state, &a(), "obsidianhero"),
        Some(LeaderStatus::Locked)
    );
    score(&mut state, "diversify");
    assert_eq!(
        leader_status(&state, &a(), "obsidianhero"),
        Some(LeaderStatus::Unlocked)
    );
}

#[test]
fn obsidianhero_2_sharsiss_hollow_readies_every_planet_and_is_purged() {
    let (mut state, _, _) = obsidian_game(|_| {});
    set_leader(&mut state, &a(), "obsidianhero", LeaderStatus::Unlocked);
    let owned: Vec<PlanetId> = state
        .controlled_planets(&a())
        .into_iter()
        .map(|(_, planet)| planet.clone())
        .collect();
    assert!(owned.len() >= 2);
    for planet in &owned {
        state.exhausted_planets.insert(planet.clone());
    }
    state.exhausted_planets.insert(PlanetId::new("jord"));
    let (mut game, seen) = action_game(state, "a", None, &["component|leader|obsidianhero"]);
    steps(&mut game, 2);
    assert!(
        was_offered(&seen, "component|leader|obsidianhero"),
        "{:?}",
        asked(&seen)
    );
    for planet in &owned {
        assert!(
            !game.state.exhausted_planets.contains(planet),
            "{planet} is ready"
        );
    }
    assert!(
        game.state
            .exhausted_planets
            .contains(&PlanetId::new("jord")),
        "only its own planets"
    );
    assert_eq!(
        leader_status(&game.state, &a(), "obsidianhero"),
        Some(LeaderStatus::Purged)
    );
    // Nothing exhausted: not offered.
    let (mut idle, _, _) = obsidian_game(|_| {});
    set_leader(&mut idle, &a(), "obsidianhero", LeaderStatus::Unlocked);
    idle.exhausted_planets.clear();
    let (mut game, seen) = action_game(idle, "a", None, &[]);
    steps(&mut game, 1);
    assert!(!was_offered(&seen, "component|leader|obsidianhero"));
}

#[test]
fn malevolency_1_at_the_end_of_a_tactical_action_the_holder_may_pay_one_influence_to_pass_it_to_a_neighbour()
 {
    let (galaxy, ring) = firmament_map();
    let note = crate::promissory::note_id("malevolency", "obsidian");
    let (mut state, _, _) = obsidian_game(|state| {
        put(state, &sys(&ring[1]), "cruiser", &b(), 1);
    });
    // The note is the Obsidian's own, and it can use the ability itself.
    assert_eq!(state.promissory_notes.get(&note), Some(&a()));
    state.player_mut(&a()).unwrap().trade_goods = 3;
    let paid = |after: &GameState| {
        // The one influence is paid in trade goods or by exhausting a planet.
        (3 - after.player(&a()).unwrap().trade_goods)
            + i32::try_from(
                after
                    .exhausted_planets
                    .len()
                    .saturating_sub(state.exhausted_planets.len()),
            )
            .unwrap()
    };
    let (game, seen) = tactical(
        state.clone(),
        "a",
        galaxy.clone(),
        "96b",
        &["done_moving", "~malevolency", "b", "~trade"],
    );
    assert_eq!(
        game.state.promissory_notes.get(&note),
        Some(&b()),
        "given to the neighbour: {:?}",
        asked(&seen)
    );
    assert!(
        paid(&game.state) >= 1,
        "one influence was spent: {:?}",
        asked(&seen)
    );
    // Declining keeps the card.
    let (game, _) = tactical(state, "a", galaxy, "96b", &["done_moving", "decline"]);
    assert_eq!(game.state.promissory_notes.get(&note), Some(&a()));
}

#[test]
fn malevolency_2_a_holder_who_is_not_the_obsidian_loses_a_fleet_token_at_the_end_of_the_status_phase()
 {
    let note = crate::promissory::note_id("malevolency", "obsidian");
    let fleet = |holds: bool, who: &PlayerId| {
        let (mut state, _, _) = obsidian_game(|_| {});
        if holds {
            crate::promissory::take(&mut state, content(), &b(), &note);
        }
        let before = state.player(who).unwrap().fleet_tokens;
        let (game, _) = status_game(state, &[]);
        game.state.player(who).unwrap().fleet_tokens - before
    };
    let with_note = fleet(true, &b());
    let without = fleet(false, &b());
    assert_eq!(
        with_note,
        without - 1,
        "one fleet token fewer for the holder"
    );
    assert_eq!(
        fleet(true, &a()),
        fleet(false, &a()),
        "the Obsidian is never levied"
    );
}

#[test]
fn obsidianbt_1_the_reaping_collects_a_trade_good_per_combat_won_against_a_puppeted_player_and_doubles_them()
 {
    let (state, galaxy, ring) = obsidian_game(|state| {
        state.player_mut(&a()).unwrap().breakthrough =
            Some(ti4_model::id::BreakthroughId::new("firmamentbt"));
    });
    assert!(
        crate::breakthroughs::holds(&state, &a(), "obsidianbt"),
        "The Sowing flipped to The Reaping"
    );
    let centre = galaxy_centre(&galaxy);
    // A space combat in a tactical action: the Obsidian's cruiser hits, the other's misses.
    let win = |against: PlayerId| {
        let mut state = state.clone();
        // Four dreadnoughts against one destroyer: the Obsidian wins whatever the seeded dice say.
        put(&mut state, &sys(&ring[1]), "dreadnought", &a(), 4);
        put(&mut state, &sys(&centre), "destroyer", &against, 1);
        let rule = format!("~move|{}|", ring[1]);
        let mut all = vec![
            TACTICAL_ACTION_ID,
            centre.as_str(),
            rule.as_str(),
            rule.as_str(),
        ];
        all.extend([rule.as_str(), rule.as_str(), "done_moving"]);
        all.extend(["decline"; 6]);
        let (mut game, _) = action_game(state, "a", Some(galaxy.clone()), &all);
        for _ in 0..40 {
            assert_eq!(game.step().error, None, "log: {:?}", game.events);
            if logged(&game, "TACTICAL_ACTION_COMPLETE") {
                break;
            }
        }
        assert!(logged(&game, "SPACE_COMBAT_WON"), "{:?}", game.events);
        game.state
    };
    let won = win(b());
    assert_eq!(
        fw::goods_on_card(&won, &a()),
        1,
        "Sol is puppeted: a trade good on the card"
    );
    assert_eq!(fw::goods_on_card(&win(c()), &a()), 0, "Hacan is not");
    // A ground combat won against the puppeted player counts too.
    let mut ground = state.clone();
    let (system, planet) = arena(&mut ground);
    ground.active = Some(a());
    ground.system_mut(&system).set_control(planet.clone(), b());
    put_on_planet(&mut ground, &system, &planet, "infantry", &b(), 1);
    put(&mut ground, &system, "carrier", &a(), 1);
    put(&mut ground, &system, "infantry", &a(), 2);
    let _ = armed_invade(
        &mut ground,
        &system,
        &a(),
        &[10, 10, 1],
        &mut table(&["~commit|", "~commit|"]),
    );
    assert_eq!(
        fw::goods_on_card(&ground, &a()),
        1,
        "ground combat won against Sol"
    );
    // At the start of the status phase: the card's goods, then as many from the supply.
    let goods = won.player(&a()).unwrap().trade_goods;
    let (done, _) = status_game(won, &[]);
    assert_eq!(done.state.player(&a()).unwrap().trade_goods, goods + 2);
    assert_eq!(fw::goods_on_card(&done.state, &a()), 0);
}

// -- Deepwrought Scholarate -------------------------------------------------------------------------

use crate::factions::deepwrought as dw;

fn deepwrought_game() -> GameState {
    seated_all(&[("a", "deepwrought"), ("b", "sol"), ("c", "hacan")])
}

/// Deepwrought (a) lands `landing` of its ground forces (kinds listed) from a carrier on a planet
/// that Sol (b) holds with two infantry, in a tactical action on the one-ring planet map; `rules`
/// answer the commit step and the Research Team question.
fn research_team_landing(
    kinds: &[&str],
    rules: &[&str],
) -> (
    Game<'static>,
    Seen,
    SystemId,
    PlanetId,
    crate::fixtures::Hub,
) {
    let hub = planet_hub();
    let system = sys(&hub.centre);
    let planet = first_planet(&system);
    let mut state = deepwrought_game();
    for id in hub.outer.iter().chain(std::iter::once(&hub.centre)) {
        let board = state.system_mut(&sys(id));
        board.units.clear();
        board.planet_units.clear();
        board.planet_control.clear();
    }
    state.system_mut(&system).set_control(planet.clone(), b());
    put_on_planet(&mut state, &system, &planet, "infantry", &b(), 2);
    put(&mut state, &system, "carrier", &a(), 1);
    for kind in kinds {
        put(&mut state, &system, kind, &a(), 1);
    }
    let mut all = vec![TACTICAL_ACTION_ID, hub.centre.as_str(), "done_moving"];
    all.extend_from_slice(rules);
    let (mut game, seen) = action_game(state, "a", Some(hub.galaxy.clone()), &all);
    for _ in 0..40 {
        assert_eq!(game.step().error, None, "log: {:?}", game.events);
        if logged(&game, "TACTICAL_ACTION_COMPLETE") {
            break;
        }
    }
    (game, seen, system, planet, hub)
}

#[test]
fn researchteam_1_and_oceanbound_1_committed_forces_may_coexist_and_that_gains_a_ready_ocean_card()
{
    let commit = ["~commit|", "~commit|", "coexist", "done_committing"];
    let (game, seen, system, planet, _) = research_team_landing(&["infantry", "infantry"], &commit);
    assert!(
        crate::coexistence::is_coexisting(&game.state, &system, &planet, &a()),
        "{:?}",
        asked(&seen)
    );
    assert!(
        game.rolls()
            .iter()
            .all(|roll| roll.reason != "ground combat"),
        "nobody fought"
    );
    assert_eq!(
        game.state.system_state(&system).planet_control.get(&planet),
        Some(&b()),
        "Sol keeps the planet"
    );
    let oceans = dw::oceans(&game.state, &a());
    assert_eq!(
        oceans,
        vec![PlanetId::new("ocean1")],
        "Oceanbound: one ocean card"
    );
    assert!(
        !game.state.exhausted_planets.contains(&oceans[0]),
        "gained ready"
    );
    // Declining ("fight") starts the ground combat and gains no ocean.
    let fight = ["~commit|", "~commit|", "fight", "done_committing"];
    let (game, _, _, _, _) = research_team_landing(&["infantry", "infantry"], &fight);
    assert!(
        game.rolls()
            .iter()
            .any(|roll| roll.reason == "ground combat")
    );
    assert!(dw::oceans(&game.state, &a()).is_empty());
    // Another faction's committed forces are never asked.
    let mut other = deepwrought_game();
    other.player_mut(&a()).unwrap().faction = ti4_model::id::FactionId::new("sol");
    assert!(!crate::factions::deepwrought::has_research_team(
        &other,
        content(),
        &a()
    ));
}

#[test]
fn unit_deepwrought_mech_1_eanautic_leaves_for_home_with_infantry_and_oceanbound_2_discards_the_ocean()
 {
    // Land a mech and two infantry and coexist (an ocean card); then Hacan activates the system:
    // the Eanautic and one infantry go to a home planet, the coexistence ends, the ocean goes.
    let commit = [
        "~commit|",
        "~commit|",
        "~commit|",
        "coexist",
        "done_committing",
    ];
    let (game, _, system, planet, hub) =
        research_team_landing(&["deepwrought_mech", "infantry", "infantry"], &commit);
    assert!(crate::coexistence::is_coexisting(
        &game.state,
        &system,
        &planet,
        &a()
    ));
    assert_eq!(dw::oceans(&game.state, &a()).len(), 1);
    let home = game
        .state
        .player(&a())
        .unwrap()
        .home_system
        .clone()
        .unwrap();
    let home_planet = game.state.controlled_planets(&a())[0].1.clone();
    let mech_at_home =
        |state: &GameState| on_planet(state, &home, &home_planet, &a(), "deepwrought_mech");
    let mut state = game.state.clone();
    state.phase = Phase::Action;
    state
        .players
        .iter_mut()
        .for_each(|seat| seat.passed = false);
    let eanautic = "unit:deepwrought:deepwrought_mech:SYSTEM_ACTIVATED:when";
    let (mut next, seen) = action_game(
        state.clone(),
        "c",
        Some(hub.galaxy.clone()),
        &[
            TACTICAL_ACTION_ID,
            hub.centre.as_str(),
            eanautic,
            "infantry|1",
            "done_moving",
        ],
    );
    for _ in 0..30 {
        assert_eq!(next.step().error, None, "log: {:?}", next.events);
        if logged(&next, "TACTICAL_ACTION_COMPLETE") {
            break;
        }
    }
    assert_eq!(
        mech_at_home(&next.state),
        1,
        "the Eanautic moved to the home planet: {:?}",
        asked(&seen)
    );
    assert_eq!(
        on_planet(&next.state, &system, &planet, &a(), "deepwrought_mech"),
        0
    );
    assert_eq!(
        on_planet(&next.state, &system, &planet, &a(), "infantry"),
        1,
        "one infantry stayed behind: {:?}",
        asked(&seen)
    );
    // One infantry still coexists there, so the ocean stays; with every unit gone it would not.
    assert!(crate::coexistence::is_coexisting(
        &next.state,
        &system,
        &planet,
        &a()
    ));
    let (mut all_out, _) = action_game(
        state.clone(),
        "c",
        Some(hub.galaxy.clone()),
        &[
            TACTICAL_ACTION_ID,
            hub.centre.as_str(),
            eanautic,
            "infantry|2",
            "done_moving",
        ],
    );
    for _ in 0..30 {
        assert_eq!(all_out.step().error, None, "log: {:?}", all_out.events);
        if logged(&all_out, "TACTICAL_ACTION_COMPLETE") {
            break;
        }
    }
    assert!(
        !crate::coexistence::is_coexisting(&all_out.state, &system, &planet, &a()),
        "the coexistence ended"
    );
    assert!(
        dw::oceans(&all_out.state, &a()).is_empty(),
        "Oceanbound: no coexisting planet, no ocean card"
    );
    // Declined (a may): the mech stays.
    let (mut declined, _) = action_game(
        state,
        "c",
        Some(hub.galaxy),
        &[
            TACTICAL_ACTION_ID,
            hub.centre.as_str(),
            "decline",
            "done_moving",
        ],
    );
    for _ in 0..30 {
        assert_eq!(declined.step().error, None, "log: {:?}", declined.events);
        if logged(&declined, "TACTICAL_ACTION_COMPLETE") {
            break;
        }
    }
    assert_eq!(
        on_planet(&declined.state, &system, &planet, &a(), "deepwrought_mech"),
        1
    );
}

/// Sol (b) controls a planet outside every home system; returns its system and planet.
fn give_sol_a_planet(state: &mut GameState) -> (SystemId, PlanetId) {
    let (system, planet) = dw::testkit::plain_planet();
    let board = state.system_mut(&system);
    board.units.clear();
    board.planet_units.clear();
    board.planet_control.clear();
    state.system_mut(&system).set_control(planet.clone(), b());
    (system, planet)
}

fn technology_card(state: &mut GameState, who: &PlayerId) {
    state.player_mut(who).unwrap().strategy_cards =
        vec![ti4_model::id::StrategyCardId::new("pok7technology")];
    for seat in state.players.iter_mut().filter(|seat| seat.id != *who) {
        seat.strategic_tokens = 0;
    }
}

#[test]
fn deepwroughtagent_1_and_2_doctor_carrina_lets_a_researcher_ignore_a_prerequisite_and_places_infantry()
 {
    let setup = |own_green: bool| {
        let mut state = deepwrought_game();
        assert_eq!(
            leader_status(&state, &a(), "deepwroughtagent"),
            Some(LeaderStatus::Readied),
            "starts ready"
        );
        state.player_mut(&b()).unwrap().technologies.clear();
        if own_green {
            super::give(&mut state, "b", "nm");
        }
        let (system, planet) = give_sol_a_planet(&mut state);
        technology_card(&mut state, &b());
        (state, system, planet)
    };
    let (state, system, planet) = setup(false);
    let spot = format!("{system}|{planet}");
    let rules = ["strategic", "use", "dxa", spot.as_str()];
    let (mut game, seen) = action_game(state.clone(), "b", None, &rules);
    steps(&mut game, 8);
    assert!(
        super::owns(&game.state, "b", "dxa"),
        "Sol researched a green technology it had no prerequisite for: {:?}",
        asked(&seen)
    );
    assert_eq!(
        leader_status(&game.state, &a(), "deepwroughtagent"),
        Some(LeaderStatus::Exhausted)
    );
    assert!(
        crate::coexistence::is_coexisting(&game.state, &system, &planet, &a()),
        "an infantry coexists on Sol's planet: {:?}",
        asked(&seen)
    );
    assert_eq!(
        on_planet(&game.state, &system, &planet, &a(), "infantry"),
        1
    );
    assert_eq!(
        dw::oceans(&game.state, &a()).len(),
        1,
        "and Oceanbound answered"
    );
    let mut after = game.state.clone();
    run_status_phase(&mut after);
    assert_eq!(
        leader_status(&after, &a(), "deepwroughtagent"),
        Some(LeaderStatus::Readied)
    );
    // Declined: the technology is closed to Sol.
    let (mut game, seen) = action_game(state, "b", None, &["strategic", "decline"]);
    steps(&mut game, 8);
    assert!(!was_offered(&seen, "dxa"), "{:?}", asked(&seen));
    assert_eq!(
        leader_status(&game.state, &a(), "deepwroughtagent"),
        Some(LeaderStatus::Readied)
    );
    // A technology whose prerequisite is already met needs no waiver: researched plainly, the
    // agent stays ready and nothing is placed (even if it was exhausted for another card).
    let (state, system, planet) = setup(true);
    let (mut game, seen) = action_game(state, "b", None, &["strategic", "decline", "dxa"]);
    steps(&mut game, 8);
    assert!(super::owns(&game.state, "b", "dxa"), "{:?}", asked(&seen));
    assert_eq!(
        leader_status(&game.state, &a(), "deepwroughtagent"),
        Some(LeaderStatus::Readied)
    );
    assert!(!crate::coexistence::is_coexisting(
        &game.state,
        &system,
        &planet,
        &a()
    ));
}

#[test]
fn hydrothermal_1_gains_a_trade_good_per_ocean_card_in_play_at_the_start_of_the_status_phase() {
    let run = |tech: bool| {
        let mut state = deepwrought_game();
        if tech {
            super::give(&mut state, "a", "hydrothermal");
        }
        dw::testkit::coexisting_everywhere(&mut state, 3);
        assert_eq!(dw::oceans_in_play(&state), 3);
        let before = state.player(&a()).unwrap().trade_goods;
        let (game, _) = status_game(state, &[]);
        game.state.player(&a()).unwrap().trade_goods - before
    };
    assert_eq!(run(true), 3, "one trade good per ocean card in play");
    assert_eq!(run(false), 0, "no technology");
}

#[test]
fn radical_1_replaces_an_owned_technology_with_one_with_exactly_one_more_prerequisite() {
    let radical = "technology:deepwrought:radical:STATUS_PHASE_BEGAN:after";
    let setup = || {
        let mut state = deepwrought_game();
        state.player_mut(&a()).unwrap().technologies.clear();
        super::give(&mut state, "a", "radical");
        super::give(&mut state, "a", "nm");
        state
    };
    let (game, seen) = status_game(setup(), &[radical, "nm|dxa"]);
    assert!(
        super::owns(&game.state, "a", "dxa") && !super::owns(&game.state, "a", "nm"),
        "{:?}",
        asked(&seen)
    );
    assert!(super::owns(&game.state, "a", "radical"));
    // Two more prerequisites is not on offer for nm.
    assert!(!was_offered(&seen, "nm|hm"), "{:?}", asked(&seen));
    let (game, _) = status_game(setup(), &["decline"]);
    assert!(
        super::owns(&game.state, "a", "nm") && !super::owns(&game.state, "a", "dxa"),
        "a may"
    );
}

#[test]
fn oceanbound_ocean_cards_pay_for_things_like_planets_and_exhaust() {
    // The Deepwrought has no trade goods and every planet exhausted, but an ocean card in play (1
    // resource): it pays for a 1-cost infantry unit in a real use of PRODUCTION.
    let home = |state: &GameState| state.player(&a()).unwrap().home_system.clone().unwrap();
    let setup = |ocean: bool| {
        let mut state = deepwrought_game();
        state.player_mut(&a()).unwrap().trade_goods = 0;
        for (_, planet) in state
            .controlled_planets(&a())
            .into_iter()
            .map(|(s, p)| (s.clone(), p.clone()))
            .collect::<Vec<_>>()
        {
            state.exhausted_planets.insert(planet);
        }
        if ocean {
            dw::testkit::coexisting_everywhere(&mut state, 1);
        }
        state
    };
    let build = |ocean: bool| {
        let state = setup(ocean);
        let h = home(&state);
        let (mut game, seen) = action_game(
            state,
            "a",
            Some(crate::fixtures::plain_hub().galaxy),
            &[
                TACTICAL_ACTION_ID,
                h.as_str(),
                "done_moving",
                "build|infantry|2",
                "~ocean",
                "done_producing",
            ],
        );
        for _ in 0..8 {
            assert_eq!(game.step().error, None, "log: {:?}", game.events);
        }
        (
            game.state,
            was_offered(&seen, "build|infantry|2"),
            asked(&seen),
        )
    };
    let (state, offered, log) = build(true);
    assert!(offered, "{log:?}");
    assert!(
        state.exhausted_planets.contains(&PlanetId::new("ocean1")),
        "the ocean card was exhausted to pay: {log:?}"
    );
    let (_, offered, _) = build(false);
    assert!(!offered, "nothing to pay with");
}

#[test]
fn deepwroughtcommander_1_aello_unlocks_in_the_game_once_an_ocean_card_is_in_play() {
    let mut state = deepwrought_game();
    assert!(!unlocks_in_the_game(&state, &a(), "deepwroughtcommander"));
    dw::testkit::coexisting_everywhere(&mut state, 1);
    assert!(unlocks_in_the_game(&state, &a(), "deepwroughtcommander"));
}

#[test]
fn deepwroughtcommander_2_aello_cuts_one_off_another_players_paid_research_and_pays_its_holder() {
    // Sol plays Technology: the free research, then a second for 6 resources paid in trade goods.
    let run = |unlocked: bool| {
        let mut state = deepwrought_game();
        dw::testkit::coexisting_everywhere(&mut state, 1);
        if unlocked {
            set_leader(
                &mut state,
                &a(),
                "deepwroughtcommander",
                LeaderStatus::Unlocked,
            );
        }
        state.player_mut(&b()).unwrap().technologies.clear();
        state.player_mut(&b()).unwrap().trade_goods = 8;
        for (_, planet) in state
            .controlled_planets(&b())
            .into_iter()
            .map(|(s, p)| (s.clone(), p.clone()))
            .collect::<Vec<_>>()
        {
            state.exhausted_planets.insert(planet);
        }
        state.player_mut(&a()).unwrap().commodities = 0;
        technology_card(&mut state, &b());
        let (mut game, seen) = eager_game(state, "b", None, &["strategic", "nm", "pa", "~reduce"]);
        steps(&mut game, 8);
        (
            game.state.player(&b()).unwrap().trade_goods,
            game.state.player(&a()).unwrap().commodities,
            super::owns(&game.state, "b", "pa"),
            asked(&seen),
        )
    };
    let (goods, commodities, researched, log) = run(true);
    assert!(researched, "{log:?}");
    assert_eq!(
        (goods, commodities),
        (3, 1),
        "the second research cost 5 and the holder gained a commodity: {log:?}"
    );
    let (goods, commodities, researched, log) = run(false);
    assert!(researched, "{log:?}");
    assert_eq!((goods, commodities), (2, 0), "locked: the full 6");
}

#[test]
fn deepwroughthero_1_unlocks_on_the_third_scored_objective() {
    hero_unlocks_by_scoring("deepwrought", "deepwroughthero");
}

#[test]
fn deepwroughthero_2_ta_zern_purges_a_technology_everywhere_and_each_loser_researches_another() {
    let mut state = deepwrought_game();
    state.player_mut(&a()).unwrap().technologies.clear();
    state.player_mut(&b()).unwrap().technologies.clear();
    for who in ["a", "b"] {
        super::give(&mut state, who, "nm");
    }
    super::give(&mut state, "c", "amd");
    set_leader(&mut state, &a(), "deepwroughthero", LeaderStatus::Unlocked);
    let (mut game, seen) = eager_game(
        state,
        "a",
        None,
        &["component|leader|deepwroughthero", "nm", "pa", "aida"],
    );
    steps(&mut game, 6);
    assert!(
        !super::owns(&game.state, "a", "nm") && !super::owns(&game.state, "b", "nm"),
        "purged from both: {:?}",
        asked(&seen)
    );
    assert!(
        dw::technology_purged(&game.state, &ti4_model::id::TechnologyId::new("nm")),
        "and from the deck"
    );
    assert!(
        super::owns(&game.state, "a", "pa"),
        "the Deepwrought researched another: {:?}",
        asked(&seen)
    );
    assert!(super::owns(&game.state, "b", "aida"), "so did Sol");
    assert!(
        super::owns(&game.state, "c", "amd"),
        "a player without it is untouched"
    );
    assert_eq!(
        leader_status(&game.state, &a(), "deepwroughthero"),
        Some(LeaderStatus::Purged)
    );
}

#[test]
fn shareknowledge_1_and_2_the_holder_gains_a_technology_until_the_status_phase_ends_and_the_note_goes_home()
 {
    let note = crate::promissory::note_id("shareknowledge", "deepwrought");
    let mut state = deepwrought_game();
    crate::promissory::deal(&mut state, content(), DEFAULT);
    assert_eq!(
        state.promissory_notes.get(&note),
        Some(&a()),
        "dealt to the Deepwrought"
    );
    crate::promissory::take(&mut state, content(), &b(), &note);
    state.player_mut(&b()).unwrap().technologies.clear();
    state.player_mut(&a()).unwrap().technologies.clear();
    super::give(&mut state, "a", "nm");
    let (mut game, seen) = action_game(
        state,
        "b",
        None,
        &["faction|deepwrought|shareknowledge", "nm"],
    );
    steps(&mut game, 3);
    assert!(super::owns(&game.state, "b", "nm"), "{:?}", asked(&seen));
    assert!(
        game.state.promissory_faceup.contains(&note),
        "placed faceup"
    );
    let (done, _) = status_game(game.state.clone(), &[]);
    assert!(
        !super::owns(&done.state, "b", "nm"),
        "returned to the deck at the end of the status phase"
    );
    assert_eq!(
        done.state.promissory_notes.get(&note),
        Some(&a()),
        "and the card to the Deepwrought player"
    );
}

#[test]
fn deepwroughtbt_1_visionaria_select_sells_research_to_other_players_for_three_trade_goods_and_a_note()
 {
    let mut state = deepwrought_game();
    crate::promissory::deal(&mut state, content(), DEFAULT);
    state.player_mut(&b()).unwrap().trade_goods = 20;
    state.player_mut(&b()).unwrap().technologies.clear();
    state.player_mut(&a()).unwrap().technologies.clear();
    state.player_mut(&c()).unwrap().trade_goods = 0;
    state.player_mut(&a()).unwrap().breakthrough =
        Some(ti4_model::id::BreakthroughId::new("deepwroughtbt"));
    let notes_a = state
        .promissory_notes
        .values()
        .filter(|holder| **holder == a())
        .count();
    let (mut game, seen) = action_game(
        state,
        "a",
        None,
        &["faction|deepwrought|visionaria", "accept", "aida"],
    );
    steps(&mut game, 3);
    assert_eq!(
        game.state.player(&b()).unwrap().trade_goods,
        17,
        "Sol spent 3 trade goods: {:?}",
        asked(&seen)
    );
    assert!(super::owns(&game.state, "b", "aida"), "Sol researched");
    assert!(
        super::owns(&game.state, "a", "aida"),
        "and the Deepwrought gains it too"
    );
    let notes_after = game
        .state
        .promissory_notes
        .values()
        .filter(|holder| **holder == a())
        .count();
    assert_eq!(notes_after, notes_a + 1, "a promissory note changed hands");
    assert!(
        !crate::factions::component_actions(&game.state, content(), &a())
            .iter()
            .any(|option| option.id == "faction|deepwrought|visionaria"),
        "exhausted"
    );
    let (done, _) = status_game(game.state, &[]);
    assert!(
        crate::factions::component_actions(&done.state, content(), &a())
            .iter()
            .any(|option| option.id == "faction|deepwrought|visionaria"),
        "ready again after the status phase"
    );
}

#[test]
fn unit_deepwrought_flagship_1_luminous_moves_through_systems_with_its_owners_units_past_a_blockade()
 {
    let hub = crate::fixtures::plain_hub();
    let origin = hub.outer[0].clone();
    let target = hub.across(&origin);
    let centre = sys(&hub.centre);
    let run = |own_unit_in_centre: bool| {
        let mut state = deepwrought_game();
        put(&mut state, &sys(&origin), "deepwrought_flagship", &a(), 1);
        put(&mut state, &centre, "destroyer", &b(), 1);
        if own_unit_in_centre {
            put(&mut state, &centre, "infantry", &a(), 1);
        }
        let rule = format!("~move|{origin}|");
        let (game, seen) = tactical(
            state,
            "a",
            hub.galaxy.clone(),
            &target,
            &[&rule, "done_moving"],
        );
        (
            ships(&game.state, &sys(&target), &a(), "deepwrought_flagship"),
            asked(&seen),
        )
    };
    assert_eq!(
        run(true).0,
        1,
        "through a system holding its owner's unit, past Sol's destroyer: {:?}",
        run(true).1
    );
    assert_eq!(run(false).0, 0, "an empty-handed blockade holds");
}

// -- unit abilities ---------------------------------------------------------------------------------

/// `unit` of seat `a` fights one Sol cruiser (a pinned 1 and b pinned 10 for the first round): the
/// first space combat roll is `(hits_on, dice)`, and the flagship is asked about SUSTAIN DAMAGE when
/// Sol's hit lands.
fn fights_like(mut state: GameState, unit: &str, hits_on: u32, dice: usize) {
    let (system, _) = arena(&mut state);
    state.active = Some(a());
    put(&mut state, &system, unit, &a(), 1);
    put(&mut state, &system, "cruiser", &b(), 1);
    let (mut table, seen) = table_seen(&["sustain|0"]);
    let faces: Vec<u32> = vec![1; dice].into_iter().chain([10]).collect();
    let (_, rolled) = armed_fight(&mut state, &system, &faces, &mut table);
    let combat = rolls(&rolled, "a", "space combat");
    assert_eq!(
        combat[0],
        (Some(hits_on), dice),
        "{unit}: combat value and dice"
    );
    assert!(
        asked(&seen)
            .iter()
            .any(|line| line.starts_with("a: ") && line.contains("sustain|0")),
        "{unit} can SUSTAIN DAMAGE: {:?}",
        asked(&seen)
    );
}

#[test]
fn unit_bastion_flagship_0_the_egeiro_fights_on_9_with_one_die_and_sustains() {
    fights_like(bastion_game(), "bastion_flagship", 9, 1);
}

#[test]
fn unit_crimson_flagship_0_quietus_fights_with_two_dice_on_5_and_sustains() {
    fights_like(crimson_game(), "crimson_flagship", 5, 2);
}

#[test]
fn unit_deepwrought_flagship_0_luminous_fights_with_two_dice_on_7_and_sustains() {
    fights_like(deepwrought_game(), "deepwrought_flagship", 7, 2);
}

#[test]
fn unit_firmament_flagship_0_heavens_eye_fights_with_two_dice_on_5_and_sustains() {
    fights_like(firmament_game(), "firmament_flagship", 5, 2);
}

#[test]
fn unit_obsidian_flagship_0_heavens_hollow_fights_with_three_dice_on_5_and_sustains() {
    let (state, _, _) = obsidian_game(|_| {});
    fights_like(state, "obsidian_flagship", 5, 3);
}

#[test]
fn unit_crimson_destroyer_0_exile_i_fights_on_8_and_barrages_9_x2() {
    let mut state = crimson_game();
    let (system, _) = arena(&mut state);
    state.active = Some(a());
    put(&mut state, &system, "crimson_destroyer", &a(), 1);
    put(&mut state, &system, "fighter", &b(), 3);
    let (_, dice) = space_fight(&mut state, &system, &[1; 8], &mut Table::new());
    assert_eq!(rolls(&dice, "a", "anti-fighter barrage")[0], (Some(9), 2));
    assert_eq!(
        rolls(&dice, "a", "space combat")[0],
        (Some(8), 1),
        "the base destroyer fights on 9"
    );
}

/// Seat a activates `system` (no dock there) in a tactical action and is offered PRODUCTION; the
/// prompt reads "produce in <system> (<n> left)".
fn production_offered(state: GameState, system: &SystemId) -> Option<String> {
    let (mut game, seen) = action_game(
        state,
        "a",
        Some(crate::fixtures::plain_hub().galaxy),
        &[
            TACTICAL_ACTION_ID,
            system.as_str(),
            "done_moving",
            "done_producing",
        ],
    );
    for _ in 0..6 {
        assert_eq!(game.step().error, None, "log: {:?}", game.events);
    }
    asked(&seen)
        .into_iter()
        .find(|line| line.contains("produce in"))
}

#[test]
fn unit_bastion_flagship_2_the_egeiro_has_production_1_in_its_system() {
    // PRODUCTION 1 on the unit card: a system with the flagship and no space dock lets its owner
    // produce one unit.
    let mut state = bastion_game();
    let (system, _) = arena(&mut state);
    put(&mut state, &system, "bastion_flagship", &a(), 1);
    let found = production_offered(state.clone(), &system);
    assert!(
        found
            .as_deref()
            .is_some_and(|line| line.contains("(1 left)")),
        "{found:?}"
    );
    let mut without = state;
    without.system_mut(&system).units.clear();
    put(&mut without, &system, "cruiser", &a(), 1);
    assert_eq!(
        production_offered(without, &system),
        None,
        "no production unit, no production"
    );
}

#[test]
fn unit_deepwrought_mech_2_the_eanautic_has_production_1_on_its_planet() {
    let mut state = deepwrought_game();
    let (system, planet) = arena(&mut state);
    state.system_mut(&system).set_control(planet.clone(), a());
    put_on_planet(&mut state, &system, &planet, "deepwrought_mech", &a(), 1);
    let found = production_offered(state, &system);
    assert!(
        found
            .as_deref()
            .is_some_and(|line| line.contains("(1 left)")),
        "{found:?}"
    );
}

#[test]
fn unit_bastion_spacedock_1_up_to_three_fighters_in_a_helios_system_do_not_count_against_capacity()
{
    // A tactical action's end enforces capacity: four fighters with no carrier is one over, three
    // are free (V1 and V2 alike).
    let (system, planet) = crate::fixtures::a_placed_planet();
    let over = |dock: &str, fighters: usize| {
        let mut state = bastion_game();
        let board = state.system_mut(&system);
        board.units.clear();
        board.planet_units.clear();
        board.planet_control.clear();
        state.system_mut(&system).set_control(planet.clone(), a());
        put_on_planet(&mut state, &system, &planet, dock, &a(), 1);
        put(&mut state, &system, "fighter", &a(), fighters);
        let (mut game, seen) = action_game(
            state,
            "a",
            Some(crate::fixtures::plain_hub().galaxy),
            &[
                TACTICAL_ACTION_ID,
                system.as_str(),
                "done_moving",
                "done_producing",
            ],
        );
        for _ in 0..8 {
            assert_eq!(game.step().error, None, "log: {:?}", game.events);
        }
        asked(&seen)
            .iter()
            .any(|line| line.contains("over capacity"))
    };
    for dock in ["bastion_spacedock", "bastion_spacedock2"] {
        assert!(!over(dock, 3), "{dock}: three fighters are supported");
        assert!(over(dock, 4), "{dock}: the fourth is not");
    }
}

#[test]
fn unit_firmament_flagship_1_heavens_eye_moves_one_further_toward_a_puppeted_players_units() {
    // Move 1 reaches the system across the centre only when the active system holds units of a
    // player with a token on one of the Firmament's plots (Sol, not Hacan).
    let hub = crate::fixtures::plain_hub();
    let origin = hub.outer[0].clone();
    let target = hub.across(&origin);
    let reaches = |occupant: PlayerId| {
        let mut state = firmament_game();
        with_plot(&mut state);
        for id in std::iter::once(&hub.centre).chain(hub.outer.iter()) {
            state.system_mut(&sys(id)).units.clear();
        }
        put(&mut state, &sys(&origin), "firmament_flagship", &a(), 1);
        put(&mut state, &sys(&target), "cruiser", &occupant, 1);
        let rule = format!("~move|{origin}|");
        let rules = [
            rule.as_str(),
            "done_moving",
            "decline",
            "decline",
            "decline",
        ];
        let (game, seen) = tactical(state, "a", hub.galaxy.clone(), &target, &rules);
        (
            was_offered_like(&seen, &format!("move|{origin}|")),
            game.state.clone(),
            asked(&seen),
        )
    };
    let (offered, _, log) = reaches(b());
    assert!(
        offered,
        "Sol is puppeted: the flagship's move is 2: {log:?}"
    );
    let (offered, _, _) = reaches(c());
    assert!(!offered, "Hacan is not: the move stays 1");
}

#[test]
fn unit_firmament_flagship_2_heavens_eye_repairs_at_the_end_of_every_combat_round_against_a_puppet()
{
    let repaired = |opponent: PlayerId| {
        let mut state = firmament_game();
        with_plot(&mut state);
        let (system, _) = arena(&mut state);
        state.active = Some(a());
        let mut flagship =
            ti4_model::units::Unit::new(ti4_model::id::UnitTypeId::new("firmament_flagship"), a());
        flagship.sustained_damage = true;
        state.system_mut(&system).units.push(flagship);
        put(&mut state, &system, "cruiser", &opponent, 1);
        // Round one: both sides miss, so the fight goes on; round two: the flagship's dice hit.
        let _ = armed_fight(&mut state, &system, &[1, 1, 1, 10, 10, 1], &mut table(&[]));
        state
            .system_state(&system)
            .units
            .iter()
            .find(|unit| unit.type_id.as_str() == "firmament_flagship")
            .map(|unit| unit.sustained_damage)
    };
    assert_eq!(
        repaired(b()),
        Some(false),
        "repaired against Sol, whose token is on a plot"
    );
    assert_eq!(repaired(c()), Some(true), "not against Hacan");
}

#[test]
fn unit_firmament_mech_1_viper_ex_23_may_coexist_and_unit_obsidian_mech_1_viper_hollow_takes_the_planet_on_the_flip()
 {
    // The Firmament lands a Viper EX-23 on Sol's planet and chooses to coexist; Sol keeps the planet.
    let (galaxy, ring) = firmament_map();
    let centre = sys(&galaxy_centre(&galaxy));
    let planet = first_planet(&centre);
    let mut state = firmament_game();
    with_plot(&mut state);
    let board = state.system_mut(&centre);
    board.units.clear();
    board.planet_units.clear();
    board.planet_control.clear();
    state.system_mut(&centre).set_control(planet.clone(), b());
    put_on_planet(&mut state, &centre, &planet, "infantry", &b(), 2);
    put(&mut state, &sys(&ring[1]), "carrier", &a(), 1);
    put(&mut state, &sys(&ring[1]), "firmament_mech", &a(), 1);
    let (mut game, seen) = action_game(
        state,
        "a",
        Some(galaxy.clone()),
        &[
            TACTICAL_ACTION_ID,
            centre.as_str(),
            &format!("~move|{}|", ring[1]),
            "load|0",
            "done_moving",
            "~commit|",
            "coexist",
            "done_committing",
        ],
    );
    for _ in 0..40 {
        assert_eq!(game.step().error, None, "log: {:?}", game.events);
        if logged(&game, "TACTICAL_ACTION_COMPLETE") {
            break;
        }
    }
    assert!(
        crate::coexistence::is_coexisting(&game.state, &centre, &planet, &a()),
        "{:?}",
        asked(&seen)
    );
    assert_eq!(
        game.state.system_state(&centre).planet_control.get(&planet),
        Some(&b())
    );
    assert!(
        game.rolls()
            .iter()
            .all(|roll| roll.reason != "ground combat"),
        "nobody fought"
    );
    // Then the faction flips: the Viper Hollow gains control, Sol's units now coexist.
    let mut state = game.state.clone();
    state.phase = Phase::Action;
    for seat in &mut state.players {
        seat.passed = false;
    }
    let (mut flip, seen) = action_game(
        state,
        "a",
        Some(galaxy),
        &["faction|firmament|become_obsidian"],
    );
    steps(&mut flip, 3);
    assert_eq!(
        flip.state.player(&a()).unwrap().faction.as_str(),
        "obsidian",
        "{:?}",
        asked(&seen)
    );
    assert_eq!(
        flip.state.system_state(&centre).planet_control.get(&planet),
        Some(&a()),
        "the Obsidian controls the planet"
    );
    assert!(
        crate::coexistence::is_coexisting(&flip.state, &centre, &planet, &b()),
        "and Sol's units now coexist"
    );
    assert_eq!(
        on_planet(&flip.state, &centre, &planet, &a(), "obsidian_mech"),
        1,
        "the card flipped"
    );
}

#[test]
fn unit_crimson_flagship_3_quietus_strips_production_and_planetary_shield_in_active_breaches() {
    // PRODUCTION: Sol's home dock sits in an active breach, the Quietus in another.
    let produces = |active: bool| {
        let mut state = crimson_game();
        let (other, _) = arena(&mut state);
        put(&mut state, &other, "crimson_flagship", &a(), 1);
        breach(&mut state, &other, active);
        let home = state.player(&b()).unwrap().home_system.clone().unwrap();
        breach(&mut state, &home, active);
        let (mut game, seen) = action_game(
            state,
            "b",
            Some(crate::fixtures::plain_hub().galaxy),
            &[
                TACTICAL_ACTION_ID,
                home.as_str(),
                "done_moving",
                "done_producing",
            ],
        );
        for _ in 0..6 {
            assert_eq!(game.step().error, None, "log: {:?}", game.events);
        }
        was_offered_like(&seen, "build|")
    };
    assert!(produces(false), "inactive breaches: Sol's dock produces");
    assert!(
        !produces(true),
        "active breaches: Sol's space dock has lost PRODUCTION"
    );
    // PLANETARY SHIELD: the Crimson's dreadnought bombards Sol's planet, which a PDS shields; the
    // PDS (in an active breach beside the Quietus) has lost the shield.
    let bombards = |active: bool| {
        let mut state = crimson_game();
        let (system, planet) = arena(&mut state);
        state.active = Some(a());
        breach(&mut state, &system, active);
        put(&mut state, &system, "crimson_flagship", &a(), 1);
        state.system_mut(&system).set_control(planet.clone(), b());
        put_on_planet(&mut state, &system, &planet, "infantry", &b(), 2);
        put_on_planet(&mut state, &system, &planet, "pds", &b(), 1);
        put(&mut state, &system, "dreadnought", &a(), 1);
        put(&mut state, &system, "carrier", &a(), 1);
        put(&mut state, &system, "infantry", &a(), 1);
        let (_, dice) = armed_invade(
            &mut state,
            &system,
            &a(),
            &[1; 8],
            &mut table(&["~commit|"]),
        );
        rolls(&dice, "a", "bombardment").len()
    };
    assert_eq!(
        bombards(false),
        0,
        "the PDS's PLANETARY SHIELD stops BOMBARDMENT"
    );
    assert!(
        bombards(true) > 0,
        "in an active breach beside the Quietus the shield is gone"
    );
}

#[test]
fn unit_deepwrought_flagship_2_luminous_adds_one_move_for_each_system_it_passes_through_with_its_owners_units()
 {
    // Three jumps round the ring (outer 0 -> 1 -> 2 -> 3); the centre is blockaded, so the move-1
    // flagship needs +2: one for each intermediate system holding the Deepwrought's units.
    let hub = crate::fixtures::plain_hub();
    let (origin, first, second, target) = (
        hub.outer[0].clone(),
        hub.outer[1].clone(),
        hub.outer[2].clone(),
        hub.outer[3].clone(),
    );
    assert_eq!(
        hub.galaxy.distance(&origin, &target),
        Some(2),
        "through the centre it is two"
    );
    let reaches = |own_in_second: bool| {
        let mut state = deepwrought_game();
        for id in std::iter::once(&hub.centre).chain(hub.outer.iter()) {
            state.system_mut(&sys(id)).units.clear();
        }
        put(&mut state, &sys(&origin), "deepwrought_flagship", &a(), 1);
        put(&mut state, &sys(&hub.centre), "destroyer", &b(), 2);
        put(&mut state, &sys(&first), "destroyer", &b(), 1);
        put(&mut state, &sys(&first), "infantry", &a(), 1);
        put(&mut state, &sys(&second), "destroyer", &b(), 1);
        if own_in_second {
            put(&mut state, &sys(&second), "infantry", &a(), 1);
        }
        let rule = format!("~move|{origin}|");
        let (game, _) = tactical(
            state,
            "a",
            hub.galaxy.clone(),
            &target,
            &[&rule, "done_moving"],
        );
        ships(&game.state, &sys(&target), &a(), "deepwrought_flagship")
    };
    assert_eq!(reaches(true), 1, "two systems with its units: +2, move 3");
    assert_eq!(
        reaches(false),
        0,
        "one such system: +1, move 2, the second blockade holds"
    );
}
