//! Depth tests, group d2: Creuss, L1Z1X, Mentak, Naalu, Nekro Virus, Sardakk N'orr (see mod.rs and
//! docs/FACTION_EFFECT_COVERAGE.md). Each test drives the engine's own flow (a `Game` with a
//! scripted table, a whole space combat or invasion with loaded dice, or the armed resolver
//! emitting the real event) and asserts what the card promises, plus the opposite case where cheap.
//!
//! Test names: `<card id>_<effect no.>_<what>`; unit tests `unit_<unit id>_<n>_<what>`.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use ti4_content::galaxy::Galaxy;
use ti4_model::content_types::DEFAULT;
use ti4_model::id::{LeaderId, PlanetId, PlayerId, SystemId};
use ti4_model::state::{GameState, LeaderStatus, Phase};

use super::{content, emit_full, scripted, seated_all, strs};
use crate::choice::{AlwaysDecline, Choice, ChoiceOption, Decider, IllegalChoice, Table};
use crate::dice::Dice;
use crate::fixtures::{put, put_on_planet};
use crate::game::{Game, TACTICAL_ACTION_ID};
use crate::rng::GameRng;

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

/// What a [`Pick`] saw: the option ids of every question, in order.
pub(crate) type Seen = Rc<RefCell<Vec<Vec<String>>>>;

/// A decider answering from `rules`: the next rule is used at the first question that offers an
/// option with that id (a rule starting with `~` matches any id containing the rest). Questions
/// that offer nothing the next rule wants are declined (or, with no decline, `pass`, or the first
/// option). Every question's option ids are logged in [`Seen`].
pub(crate) struct Pick {
    rules: VecDeque<String>,
    seen: Seen,
    /// Unmatched questions take the first option that is not a decline, not the decline.
    eager: bool,
}

impl Pick {
    pub(crate) fn new(rules: &[&str]) -> (Self, Seen) {
        let seen: Seen = Rc::default();
        (
            Self {
                rules: rules.iter().map(|rule| (*rule).to_owned()).collect(),
                seen: seen.clone(),
                eager: false,
            },
            seen,
        )
    }
}

impl Pick {
    /// Like [`Pick::new`], but a question no rule wants takes its first option that is not a
    /// decline (forced assignments, payments, ...).
    pub(crate) fn eager(rules: &[&str]) -> (Self, Seen) {
        let (mut pick, seen) = Self::new(rules);
        pick.eager = true;
        (pick, seen)
    }
}

fn wanted<'a>(choice: &'a Choice, rule: &str) -> Option<&'a ChoiceOption> {
    rule.strip_prefix('~').map_or_else(
        || choice.options.iter().find(|option| option.id == rule),
        |part| choice.options.iter().find(|option| option.id.contains(part)),
    )
}

impl Decider for Pick {
    fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
        self.seen
            .borrow_mut()
            .push(choice.options.iter().map(|option| option.id.clone()).collect());
        if let Some(option) = self.rules.front().and_then(|rule| wanted(choice, rule)) {
            let option = option.clone();
            self.rules.pop_front();
            return Ok(option);
        }
        let eager = self.eager.then(|| choice.options.iter().find(|option| !option.is_decline())).flatten();
        eager
            .or_else(|| choice.options.iter().find(|option| option.is_decline()))
            .or_else(|| choice.options.iter().find(|option| option.id == "pass"))
            .or_else(|| choice.options.first())
            .cloned()
            .ok_or_else(|| IllegalChoice::NoOptions {
                player: choice.player.clone(),
                prompt: choice.prompt.clone(),
            })
    }
}

/// Did any question offer an option with this id?
pub(crate) fn was_offered(seen: &Seen, id: &str) -> bool {
    seen.borrow().iter().flatten().any(|offered| offered == id)
}

/// A `Game` of `state` in the action phase with `active` to act, driven by `rules`.
pub(crate) fn action_game(
    state: GameState,
    active: &str,
    galaxy: Option<Galaxy>,
    rules: &[&str],
) -> (Game<'static>, Seen) {
    let (pick, seen) = Pick::new(rules);
    (game_with(state, active, galaxy, pick), seen)
}

/// [`action_game`] whose unmatched questions take the first non-decline option.
pub(crate) fn eager_game(
    state: GameState,
    active: &str,
    galaxy: Option<Galaxy>,
    rules: &[&str],
) -> (Game<'static>, Seen) {
    let (pick, seen) = Pick::eager(rules);
    (game_with(state, active, galaxy, pick), seen)
}

fn game_with(mut state: GameState, active: &str, galaxy: Option<Galaxy>, pick: Pick) -> Game<'static> {
    state.phase = Phase::Action;
    state.active = Some(PlayerId::new(active));
    let table = Table::with_default(Box::new(pick));
    let mut game = Game::with_table(state, content(), table).with_sources(DEFAULT);
    if let Some(galaxy) = galaxy {
        game = game.with_galaxy(galaxy);
    }
    game
}

/// Step until the game logs `event` (at most `max` steps); every step must be legal.
pub(crate) fn step_until(game: &mut Game<'_>, event: &str, max: usize) {
    for _ in 0..max {
        let step = game.step();
        assert_eq!(step.error, None, "log: {:?}", game.events);
        if game.events.iter().any(|logged| logged == event) {
            return;
        }
    }
    panic!("{event} never happened in {max} steps; log: {:?}", game.events);
}

/// Give `who` the leader in this status.
pub(crate) fn set_leader(state: &mut GameState, who: &PlayerId, leader: &str, status: LeaderStatus) {
    state
        .player_mut(who)
        .expect("seat")
        .leaders
        .insert(LeaderId::new(leader), status);
}

pub(crate) fn leader_status(state: &GameState, who: &PlayerId, leader: &str) -> Option<LeaderStatus> {
    state.player(who)?.leaders.get(&LeaderId::new(leader)).copied()
}

/// Number of `kind` units `owner` has in the space area of `system`.
pub(crate) fn ships(state: &GameState, system: &SystemId, owner: &PlayerId, kind: &str) -> usize {
    state
        .system_state(system)
        .units
        .iter()
        .filter(|unit| unit.owner == *owner && unit.type_id.as_str() == kind)
        .count()
}

/// Number of `kind` units `owner` has on `planet`.
pub(crate) fn on_planet(
    state: &GameState,
    system: &SystemId,
    planet: &PlanetId,
    owner: &PlayerId,
    kind: &str,
) -> usize {
    state
        .system_state(system)
        .on_planet_of(planet, owner)
        .into_iter()
        .filter(|unit| unit.type_id.as_str() == kind)
        .count()
}

/// `faces`, then enough misses that a loaded `Dice` is never empty (an empty preload panics).
pub(crate) fn padded(faces: &[u32]) -> Vec<u32> {
    faces.iter().copied().chain(std::iter::repeat_n(1, 800)).collect()
}

/// A whole space combat in `system` with these dice faces loaded first (then the seeded stream),
/// the table answering the questions. Returns the outcome and the dice that were rolled.
pub(crate) fn space_fight(
    state: &mut GameState,
    system: &SystemId,
    faces: &[u32],
    table: &mut Table,
) -> (crate::combat::CombatOutcome, Dice) {
    let mut dice = Dice::from_faces(padded(faces));
    let mut rng = GameRng::new(11);
    let outcome = crate::combat::resolve(state, content(), DEFAULT, table, &mut dice, &mut rng, system)
        .expect("the combat resolves");
    (outcome, dice)
}

/// The first roll `who` made in `dice`: `(hits_on, number of dice)`.
pub(crate) fn first_roll(dice: &Dice, who: &str) -> Option<(Option<u32>, usize)> {
    dice.history()
        .iter()
        .find(|roll| roll.by.as_deref() == Some(who))
        .map(|roll| (roll.hits_on, roll.faces.len()))
}

/// A whole invasion of `system` by `invader` with these faces loaded first.
pub(crate) fn invade(
    state: &mut GameState,
    system: &SystemId,
    invader: &PlayerId,
    faces: &[u32],
    table: &mut Table,
) -> (crate::invasion::InvasionReport, Dice) {
    let mut dice = Dice::from_faces(padded(faces));
    let mut rng = GameRng::new(11);
    let report = crate::invasion::resolve(
        state,
        content(),
        DEFAULT,
        table,
        &mut dice,
        &mut rng,
        system,
        invader,
    )
    .expect("the invasion resolves");
    (report, dice)
}

/// `state` stepped once in a `Game`: the engine's own unlock check (51.7) runs at the top of the
/// step. True when the game announced `LEADER_UNLOCKED:<leader>` and the seat holds it unlocked.
pub(crate) fn unlocks_in_the_game(state: &GameState, who: &PlayerId, leader: &str) -> bool {
    let mut state = state.clone();
    state.phase = Phase::Action;
    state.active = Some(who.clone());
    let mut game = Game::with_table(state, content(), Table::with_default(Box::new(AlwaysDecline)))
        .with_sources(DEFAULT);
    let _ = game.step();
    let announced = game
        .events
        .iter()
        .any(|event| *event == format!("LEADER_UNLOCKED:{leader}"));
    let unlocked = leader_status(&game.state, who, leader) == Some(LeaderStatus::Unlocked);
    assert_eq!(announced, unlocked, "log: {:?}", game.events);
    unlocked
}

/// Score `count` objectives for `who` (the third one unlocks the hero, 51.7).
pub(crate) fn score_objectives(state: &mut GameState, who: &PlayerId, count: usize) {
    for index in 0..count {
        state.record_score(who, ti4_model::id::ObjectiveId::new(format!("test_objective_{index}")));
    }
}

// -- Creuss ---------------------------------------------------------------------------------------

/// Alpha: 39 and 26; beta: 40 and 25; delta: the gate 17; plain: 18, 19, 20, 21.
fn ghost_galaxy() -> Galaxy {
    let mut galaxy = Galaxy::build(
        content(),
        &["18", "17", "39", "40", "26", "25", "19", "20", "21"],
        DEFAULT,
        3,
    )
    .expect("builds");
    crate::seating::place_creuss_home(&mut galaxy, content(), DEFAULT).expect("home");
    galaxy
}

fn ghost_game() -> GameState {
    seated_all(&[("a", "ghost"), ("b", "sol")])
}

/// `active` takes a tactical action into `to` and moves the first ship of `from` there. Returns
/// the game and what was offered (the move option is simply absent when the ship cannot reach).
fn tactical_move(
    state: GameState,
    active: &str,
    galaxy: Galaxy,
    from: &str,
    to: &str,
    extra: &[&str],
) -> (Game<'static>, Seen) {
    let mut rules = vec![TACTICAL_ACTION_ID.to_owned(), to.to_owned()];
    rules.extend(extra.iter().map(|rule| (*rule).to_owned()));
    rules.push(format!("move|{from}|0"));
    rules.push("done_moving".to_owned());
    let refs: Vec<&str> = rules.iter().map(String::as_str).collect();
    let (mut game, seen) = action_game(state, active, Some(galaxy), &refs);
    for _ in 0..30 {
        assert_eq!(game.step().error, None, "log: {:?}", game.events);
        if game.events.iter().any(|event| event == "TACTICAL_ACTION_COMPLETE") {
            break;
        }
    }
    (game, seen)
}

/// A Creuss carrier in `from` and a Sol carrier in `from` (Sol has no wormhole travel).
fn carriers_at(from: &str) -> GameState {
    let mut state = ghost_game();
    put(&mut state, &sys(from), "carrier", &a(), 1);
    put(&mut state, &sys(from), "carrier", &b(), 1);
    state
}

#[test]
fn quantum_entanglement_1_a_ship_crosses_between_far_alpha_and_beta_systems_for_creuss_only() {
    // 39 (alpha) and 25 (beta) are not hex neighbours; a carrier has move 1.
    assert!(!ghost_galaxy().adjacent("39").contains("25"));
    let (game, _) = tactical_move(carriers_at("39"), "a", ghost_galaxy(), "39", "25", &[]);
    assert_eq!(ships(&game.state, &sys("25"), &a(), "carrier"), 1, "Creuss arrived");
    assert_eq!(ships(&game.state, &sys("39"), &a(), "carrier"), 0);
    let (game, seen) = tactical_move(carriers_at("39"), "b", ghost_galaxy(), "39", "25", &[]);
    assert!(!was_offered(&seen, "move|39|0"), "Sol cannot reach it");
    assert_eq!(ships(&game.state, &sys("39"), &b(), "carrier"), 1);
    assert_eq!(ships(&game.state, &sys("25"), &b(), "carrier"), 0);
}

#[test]
fn quantum_entanglement_2_game_effects_cannot_stop_the_crossing() {
    let mut galaxy = ghost_galaxy();
    galaxy.wormholes_off = true;
    let (game, _) = tactical_move(carriers_at("39"), "a", galaxy, "39", "25", &[]);
    assert_eq!(
        ships(&game.state, &sys("25"), &a(), "carrier"),
        1,
        "wormholes are switched off for everyone else, not for Creuss"
    );
}

#[test]
fn slipstream_1_plus_one_move_from_home_and_from_alpha_but_not_from_a_plain_system() {
    // Target 20 is two hexes from 51 (via the gate), from alpha 39 and from plain 19. A carrier
    // has move 1, so only Slipstream's +1 reaches it.
    for (from, reaches) in [("51", true), ("39", true), ("19", false)] {
        let (game, seen) = tactical_move(carriers_at(from), "a", ghost_galaxy(), from, "20", &[]);
        assert_eq!(
            ships(&game.state, &sys("20"), &a(), "carrier"),
            usize::from(reaches),
            "from {from}"
        );
        assert_eq!(was_offered(&seen, &format!("move|{from}|0")), reaches, "from {from}");
    }
    let (game, seen) = tactical_move(carriers_at("39"), "b", ghost_galaxy(), "39", "20", &[]);
    assert!(!was_offered(&seen, "move|39|0"), "Sol gets no bonus");
    assert_eq!(ships(&game.state, &sys("20"), &b(), "carrier"), 0);
}

fn quiet_table(rules: &[&str]) -> Table {
    Table::with_default(Box::new(Pick::new(rules).0))
}

#[test]
fn ds_1_a_wormhole_system_combat_starts_with_a_hit_on_the_enemy() {
    // Cruiser (a) against destroyer (b); every die is a 10, so any round that is rolled kills.
    let fight = |tech: bool, system: &str| {
        let mut state = ghost_game();
        if tech {
            super::give(&mut state, "a", "ds");
        }
        put(&mut state, &sys(system), "cruiser", &a(), 1);
        put(&mut state, &sys(system), "destroyer", &b(), 1);
        let (outcome, dice) = space_fight(&mut state, &sys(system), &[10; 8], &mut quiet_table(&["produce_hit"]));
        (outcome.winner, first_roll(&dice, "b").is_some(), ships(&state, &sys(system), &b(), "destroyer"))
    };
    assert_eq!(fight(true, "39"), (Some(a()), false, 0), "the destroyer died before it could roll");
    assert_eq!(fight(true, "19").1, true, "no wormhole in the system: a normal first round");
    assert_eq!(fight(false, "39").1, true, "no technology: a normal first round");
}

#[test]
fn wg_1_the_component_action_places_a_creuss_token_and_exhausts_the_card() {
    for (tech, placed) in [(true, true), (false, false)] {
        let mut state = ghost_game();
        if tech {
            super::give(&mut state, "a", "wg");
        }
        let (mut game, seen) = action_game(state, "a", Some(ghost_galaxy()), &["faction|ghost|wg", "ALPHA|19", "pass"]);
        for _ in 0..6 {
            assert_eq!(game.step().error, None, "log: {:?}", game.events);
        }
        assert_eq!(was_offered(&seen, "faction|ghost|wg"), placed, "offered only with the card");
        assert_eq!(game.state.wormhole_tokens.get("ALPHA").cloned(), placed.then(|| sys("19")));
        if placed {
            assert!(
                !crate::factions::component_actions(&game.state, content(), &a())
                    .iter()
                    .any(|option| option.id == "faction|ghost|wg"),
                "exhausted: not offered again"
            );
        }
    }
}

/// The engine's own status phase (81) over `state`: leaders ready, units repair, planets ready.
pub(crate) fn run_status_phase(state: &mut GameState) {
    state.phase = Phase::Status;
    crate::status::resolve_status_phase(state).expect("the status phase resolves");
}

#[test]
fn ghostagent_1_taivra_links_the_activated_system_to_every_wormhole_system_and_readies_at_status() {
    // Sol activates 39 (alpha); its carrier sits in 25 (beta), two hexes away and out of reach
    // for move 1 -- unless the Creuss agent makes 39 adjacent to every wormhole system.
    let taivra = "leader:ghost:ghostagent:SYSTEM_ACTIVATED:after";
    let mut state = ghost_game();
    put(&mut state, &sys("25"), "carrier", &b(), 1);
    let (mut game, _) = tactical_move(state.clone(), "b", ghost_galaxy(), "25", "39", &[taivra]);
    assert_eq!(ships(&game.state, &sys("39"), &b(), "carrier"), 1, "the carrier reached 39");
    assert_eq!(leader_status(&game.state, &a(), "ghostagent"), Some(LeaderStatus::Exhausted));
    run_status_phase(&mut game.state);
    assert_eq!(leader_status(&game.state, &a(), "ghostagent"), Some(LeaderStatus::Readied));
    // Declined: no link, the carrier cannot move and the agent stays ready.
    let (game, seen) = tactical_move(state, "b", ghost_galaxy(), "25", "39", &[]);
    assert!(!was_offered(&seen, "move|25|0"));
    assert_eq!(ships(&game.state, &sys("25"), &b(), "carrier"), 1);
    assert_eq!(leader_status(&game.state, &a(), "ghostagent"), Some(LeaderStatus::Readied));
}

#[test]
fn unit_ghost_flagship_1_hil_colish_system_holds_a_delta_wormhole_for_every_player() {
    // Sol's carrier in 25 (beta) activates the gate 17 (delta): not adjacent, so it can only
    // go if a Creuss flagship in 25 makes that system a delta wormhole system too.
    assert!(!ghost_galaxy().adjacent("25").contains("17"));
    for (flagship, reaches) in [(true, true), (false, false)] {
        let mut state = ghost_game();
        put(&mut state, &sys("25"), "carrier", &b(), 1);
        if flagship {
            put(&mut state, &sys("25"), "ghost_flagship", &a(), 1);
        }
        let (game, seen) = tactical_move(state, "b", ghost_galaxy(), "25", "17", &[]);
        assert_eq!(was_offered(&seen, "move|25|0"), reaches, "flagship: {flagship}");
        assert_eq!(ships(&game.state, &sys("17"), &b(), "carrier"), usize::from(reaches));
    }
}

#[test]
fn unit_ghost_flagship_2_hil_colish_may_move_before_or_after_the_other_ships() {
    // The engine lets ships move one at a time in any order; both orders reach the target.
    for flagship_first in [true, false] {
        let mut state = ghost_game();
        put(&mut state, &sys("19"), "ghost_flagship", &a(), 1);
        put(&mut state, &sys("25"), "carrier", &a(), 1);
        let order: [&str; 2] = if flagship_first { ["move|19|0", "move|25|0"] } else { ["move|25|0", "move|19|0"] };
        let (mut game, _) = action_game(
            state,
            "a",
            Some(ghost_galaxy()),
            &[TACTICAL_ACTION_ID, "18", order[0], order[1], "done_moving"],
        );
        step_until(&mut game, "TACTICAL_ACTION_COMPLETE", 30);
        assert_eq!(ships(&game.state, &sys("18"), &a(), "ghost_flagship"), 1, "{flagship_first}");
        assert_eq!(ships(&game.state, &sys("18"), &a(), "carrier"), 1, "{flagship_first}");
    }
}

#[test]
fn ghostcommander_1_unlocks_in_the_game_with_units_in_three_alpha_or_beta_systems() {
    let mut state = ghost_game();
    put(&mut state, &sys("39"), "cruiser", &a(), 1);
    put(&mut state, &sys("40"), "cruiser", &a(), 1);
    assert!(!unlocks_in_the_game(&state, &a(), "ghostcommander"), "two systems");
    put(&mut state, &sys("25"), "cruiser", &a(), 1);
    assert!(unlocks_in_the_game(&state, &a(), "ghostcommander"), "three systems");
}

#[test]
fn ghosthero_1_unlocks_in_the_game_on_the_third_scored_objective() {
    let mut state = ghost_game();
    score_objectives(&mut state, &a(), 2);
    assert!(!unlocks_in_the_game(&state, &a(), "ghosthero"));
    score_objectives(&mut state, &a(), 3);
    assert!(unlocks_in_the_game(&state, &a(), "ghosthero"));
}

#[test]
fn ghosthero_2_riftwalker_swaps_two_systems_on_the_map_and_is_purged() {
    let mut state = ghost_game();
    set_leader(&mut state, &a(), "ghosthero", LeaderStatus::Unlocked);
    put(&mut state, &sys("19"), "cruiser", &a(), 1);
    put(&mut state, &sys("40"), "cruiser", &a(), 1);
    let galaxy = ghost_galaxy();
    let (hex_19, hex_39) = (galaxy.coord_of("19").unwrap(), galaxy.coord_of("39").unwrap());
    let (mut game, _) = action_game(state, "a", Some(galaxy), &["component|leader|ghosthero", "19", "39"]);
    for _ in 0..4 {
        assert_eq!(game.step().error, None, "log: {:?}", game.events);
    }
    assert_eq!(leader_status(&game.state, &a(), "ghosthero"), Some(LeaderStatus::Purged));
    let mut map = ghost_galaxy();
    crate::movement::replay_map_edits(&game.state, &mut map, content(), DEFAULT).expect("replays");
    assert_eq!(map.coord_of("19"), Some(hex_39));
    assert_eq!(map.coord_of("39"), Some(hex_19));
}

// -- L1Z1X ----------------------------------------------------------------------------------------

fn l1z1x_game() -> GameState {
    seated_all(&[("a", "l1z1x"), ("b", "sol")])
}

/// A system with two ordinary planets: `(system, first, second)`.
fn two_planet_system() -> (SystemId, PlanetId, PlanetId) {
    let mut by_system: std::collections::BTreeMap<String, Vec<String>> = Default::default();
    for (id, planet) in ti4_content::galaxy::all_planets(content(), DEFAULT) {
        if planet.homeworld_of().is_none()
            && !planet.is_placed_during_play()
            && let Some(system) = planet.system_id()
        {
            by_system.entry(system.to_owned()).or_default().push((*id).to_owned());
        }
    }
    let (system, planets) = by_system
        .into_iter()
        .find(|(_, planets)| planets.len() >= 2)
        .expect("a system with two planets");
    (SystemId::new(system), PlanetId::new(&planets[0]), PlanetId::new(&planets[1]))
}

/// An ordinary planet that is nobody's home, alone in its system, not Mecatol Rex.
fn one_planet() -> (SystemId, PlanetId) {
    let all = ti4_content::galaxy::all_planets(content(), DEFAULT);
    all.iter()
        .find(|(_, planet)| {
            planet.homeworld_of().is_none()
                && !planet.is_placed_during_play()
                && planet.system_id().is_some_and(|system| {
                    system != "18"
                        && all.iter().filter(|(_, other)| other.system_id() == Some(system)).count() == 1
                })
        })
        .map(|(id, planet)| (SystemId::new(planet.system_id().unwrap_or("19")), PlanetId::new(*id)))
        .expect("an ordinary lone planet")
}

#[test]
fn assimilate_1_taking_a_planet_swaps_the_rivals_pds_and_dock_for_the_l1z1xs_own() {
    for (faction, swaps) in [("l1z1x", true), ("sol", false)] {
        let mut state = seated_all(&[("a", faction), ("b", "hacan")]);
        let (system, planet) = one_planet();
        state.system_mut(&system).set_control(planet.clone(), b());
        put_on_planet(&mut state, &system, &planet, "pds", &b(), 1);
        put_on_planet(&mut state, &system, &planet, "spacedock", &b(), 1);
        put(&mut state, &system, "infantry", &a(), 2);
        let (report, _) = invade(&mut state, &system, &a(), &[10; 8], &mut Table::new());
        assert_eq!(report.captured.len(), 1, "the planet changed hands ({faction})");
        for kind in ["pds", "spacedock"] {
            assert_eq!(on_planet(&state, &system, &planet, &a(), kind), usize::from(swaps), "{faction} {kind}");
            assert_eq!(on_planet(&state, &system, &planet, &b(), kind), 0, "{faction} {kind}: the rival's is gone");
        }
    }
}

#[test]
fn harrow_1_the_fleet_bombards_again_after_each_round_of_ground_combat() {
    for (faction, harrows) in [("l1z1x", true), ("sol", false)] {
        let mut state = seated_all(&[("a", faction), ("b", "hacan")]);
        let (system, planet) = one_planet();
        put(&mut state, &system, "dreadnought", &a(), 1);
        put_on_planet(&mut state, &system, &planet, "infantry", &a(), 3);
        put_on_planet(&mut state, &system, &planet, "infantry", &b(), 1);
        // Round one: a misses with 3 dice, b misses with 1, then the Harrow die is a 10.
        let mut dice = Dice::from_faces(padded(&[1, 1, 1, 1, 10]));
        let mut rng = GameRng::new(3);
        let winner = crate::invasion::ground_combat(
            &mut state, content(), DEFAULT, &mut Table::new(), &mut dice, &mut rng, &system, &planet, &a(),
        )
        .expect("resolves");
        let harrow_rolls = dice.history().iter().filter(|roll| roll.reason == "harrow").count();
        if harrows {
            assert_eq!(harrow_rolls, 1, "one extra bombardment at the end of round one");
            assert_eq!(winner, Some(a()), "it killed the last defender");
            assert_eq!(on_planet(&state, &system, &planet, &b(), "infantry"), 0);
        } else {
            assert_eq!(harrow_rolls, 0, "{faction} has no Harrow");
        }
    }
}

#[test]
fn l1z1xagent_1_i48s_turns_the_activators_infantry_into_a_mech_and_readies_at_status() {
    let agent = "leader:l1z1x:l1z1xagent:SYSTEM_ACTIVATED:after";
    let mut state = l1z1x_game();
    let home = state.player(&b()).unwrap().home_system.clone().unwrap();
    let infantry = |state: &GameState| {
        state
            .system_state(&home)
            .planet_units
            .values()
            .flatten()
            .filter(|unit| unit.owner == b() && unit.type_id.as_str().contains("infantry"))
            .count()
    };
    let mechs = |state: &GameState| {
        state
            .system_state(&home)
            .planet_units
            .values()
            .flatten()
            .filter(|unit| unit.owner == b() && unit.type_id.as_str().contains("mech"))
            .count()
    };
    let (before_infantry, before_mechs) = (infantry(&state), mechs(&state));
    assert!(before_infantry > 0);
    let (mut game, _) = action_game(state.clone(), "b", Some(ghost_galaxy()), &[TACTICAL_ACTION_ID, home.as_str(), agent, "~"]);
    for _ in 0..12 {
        assert_eq!(game.step().error, None, "log: {:?}", game.events);
        if leader_status(&game.state, &a(), "l1z1xagent") == Some(LeaderStatus::Exhausted) {
            break;
        }
    }
    assert_eq!(leader_status(&game.state, &a(), "l1z1xagent"), Some(LeaderStatus::Exhausted));
    assert_eq!(mechs(&game.state), before_mechs + 1, "one infantry became a mech");
    assert_eq!(infantry(&game.state), before_infantry - 1);
    run_status_phase(&mut game.state);
    assert_eq!(leader_status(&game.state, &a(), "l1z1xagent"), Some(LeaderStatus::Readied));
    // Declined: nothing changes.
    let (mut game, _) = action_game(state, "b", Some(ghost_galaxy()), &[TACTICAL_ACTION_ID, home.as_str()]);
    for _ in 0..12 {
        assert_eq!(game.step().error, None);
    }
    assert_eq!(mechs(&game.state), before_mechs);
    assert_eq!(leader_status(&game.state, &a(), "l1z1xagent"), Some(LeaderStatus::Readied));
}

#[test]
fn l1z1xcommander_1_unlocks_in_the_game_with_four_dreadnoughts_on_the_board() {
    let mut state = l1z1x_game();
    state.board.values_mut().for_each(|board| board.units.retain(|unit| unit.type_id.as_str() != "l1z1x_dreadnought"));
    put(&mut state, &sys("19"), "l1z1x_dreadnought", &a(), 2);
    put(&mut state, &sys("20"), "l1z1x_dreadnought", &a(), 1);
    assert!(!unlocks_in_the_game(&state, &a(), "l1z1xcommander"), "three dreadnoughts");
    put(&mut state, &sys("21"), "l1z1x_dreadnought", &a(), 1);
    assert!(unlocks_in_the_game(&state, &a(), "l1z1xcommander"), "four dreadnoughts");
}

#[test]
fn l1z1xcommander_2_bombardment_kills_through_planetary_shield_only_once_unlocked() {
    for (status, kills) in [(LeaderStatus::Locked, 0), (LeaderStatus::Unlocked, 1)] {
        let mut state = l1z1x_game();
        set_leader(&mut state, &a(), "l1z1xcommander", status);
        let (system, planet) = one_planet();
        put_on_planet(&mut state, &system, &planet, "pds", &b(), 1);
        put_on_planet(&mut state, &system, &planet, "infantry", &b(), 1);
        put(&mut state, &system, "l1z1x_dreadnought", &a(), 1);
        let (report, _) = invade(&mut state, &system, &a(), &[10, 10], &mut Table::new());
        assert_eq!(report.bombardment_kills, kills, "{status:?}");
        assert_eq!(on_planet(&state, &system, &planet, &b(), "infantry"), 1 - kills, "{status:?}");
    }
}

#[test]
fn l1z1xhero_1_unlocks_in_the_game_on_the_third_scored_objective() {
    let mut state = l1z1x_game();
    score_objectives(&mut state, &a(), 2);
    assert!(!unlocks_in_the_game(&state, &a(), "l1z1xhero"));
    score_objectives(&mut state, &a(), 3);
    assert!(unlocks_in_the_game(&state, &a(), "l1z1xhero"));
}

#[test]
fn l1z1xhero_2_the_helmsman_gathers_the_flagship_and_dreadnoughts_and_is_purged() {
    let hub = crate::fixtures::plain_hub();
    let mut state = l1z1x_game();
    state.board.clear();
    set_leader(&mut state, &a(), "l1z1xhero", LeaderStatus::Unlocked);
    let (far, other, target) = (
        sys(&hub.across(&hub.outer[0])),
        sys(&hub.outer[1]),
        sys(&hub.outer[2]),
    );
    put(&mut state, &far, "l1z1x_flagship", &a(), 1);
    put(&mut state, &far, "l1z1x_dreadnought", &a(), 1);
    put(&mut state, &far, "cruiser", &a(), 1);
    put(&mut state, &other, "l1z1x_dreadnought", &a(), 1);
    put(&mut state, &target, "cruiser", &a(), 1);
    let (mut game, _) = action_game(state, "a", Some(hub.galaxy), &["component|leader|l1z1xhero", target.as_str()]);
    for _ in 0..4 {
        assert_eq!(game.step().error, None, "log: {:?}", game.events);
    }
    assert_eq!(ships(&game.state, &target, &a(), "l1z1x_flagship"), 1);
    assert_eq!(ships(&game.state, &target, &a(), "l1z1x_dreadnought"), 2);
    assert_eq!(ships(&game.state, &far, &a(), "cruiser"), 1, "other ship types stay behind");
    assert_eq!(leader_status(&game.state, &a(), "l1z1xhero"), Some(LeaderStatus::Purged));
}

/// The Technology card's primary played by `a` in a running game, picking `tech`; returns the
/// game and what was offered.
fn play_technology(mut state: GameState, tech: &str) -> (Game<'static>, Seen) {
    state.player_mut(&a()).unwrap().strategy_cards = vec![ti4_model::id::StrategyCardId::new("pok7technology")];
    let (mut game, seen) = action_game(state, "a", None, &["strategic", tech]);
    for _ in 0..6 {
        assert_eq!(game.step().error, None, "log: {:?}", game.events);
    }
    (game, seen)
}

#[test]
fn is_1_inheritance_systems_researches_without_prerequisites_for_two_resources() {
    // The War Sun needs four prerequisites a fresh L1Z1X lacks.
    for (tech, researched) in [(true, true), (false, false)] {
        let mut state = l1z1x_game();
        if tech {
            super::give(&mut state, "a", "is");
        }
        let (game, seen) = play_technology(state, "ws");
        assert_eq!(was_offered(&seen, "ws"), researched, "offered only with Inheritance Systems");
        assert_eq!(super::owns(&game.state, "a", "ws"), researched);
        if researched {
            assert!(!game.state.exhausted_planets.is_empty(), "two resources were paid (a planet exhausted)");
        }
    }
}

/// The status phase played by a `Game` from its start until the next strategy phase; returns the
/// game. Token allocation questions get the first option.
pub(crate) fn play_status_phase(mut state: GameState) -> Game<'static> {
    state.phase = Phase::Status;
    let mut game = Game::with_table(state, content(), Table::with_default(Box::new(Pick::new(&[]).0)))
        .with_sources(DEFAULT);
    for _ in 0..40 {
        assert_eq!(game.step().error, None, "log: {:?}", game.events);
        if game.state.phase != Phase::Status {
            return game;
        }
    }
    panic!("the status phase never ended; log: {:?}", game.events);
}

fn tokens_held(state: &GameState, who: &PlayerId) -> i32 {
    let seat = state.player(who).unwrap();
    seat.tactic_tokens + seat.fleet_tokens + seat.strategic_tokens
}

#[test]
fn ce_1_and_2_the_holder_gains_one_more_status_token_and_the_note_goes_home() {
    let note = crate::promissory::note_id("ce", "l1z1x");
    let plain = play_status_phase(l1z1x_game());
    let mut state = l1z1x_game();
    state.promissory_notes.insert(note.clone(), b());
    let held = play_status_phase(state);
    let before = tokens_held(&l1z1x_game(), &b());
    assert_eq!(
        tokens_held(&held.state, &b()) - before,
        tokens_held(&plain.state, &b()) - before + 1,
        "one additional token for the holder"
    );
    assert_eq!(tokens_held(&held.state, &a()), tokens_held(&plain.state, &a()), "the L1Z1X player gains no extra");
    assert_eq!(held.state.promissory_notes.get(&note), Some(&a()), "returned to the L1Z1X player");
}

// -- Mentak ---------------------------------------------------------------------------------------

fn mentak_game() -> GameState {
    seated_all(&[("a", "mentak"), ("b", "sol")])
}

#[test]
fn mc_1_each_trade_good_pays_two_when_building_a_ship() {
    // Every planet is exhausted, so only trade goods pay; a dreadnought costs 4.
    for (tech, builds) in [(true, true), (false, false)] {
        let mut state = mentak_game();
        if tech {
            super::give(&mut state, "a", "mc");
        }
        let home = state.player(&a()).unwrap().home_system.clone().unwrap();
        for (_, planet) in state.controlled_planets(&a()).into_iter().map(|(s, p)| (s.clone(), p.clone())).collect::<Vec<_>>() {
            state.exhausted_planets.insert(planet);
        }
        state.player_mut(&a()).unwrap().trade_goods = 2;
        let (mut game, seen) = action_game(
            state,
            "a",
            Some(ghost_galaxy()),
            &[TACTICAL_ACTION_ID, home.as_str(), "done_moving", "build|dreadnought|1", "done_producing"],
        );
        for _ in 0..8 {
            assert_eq!(game.step().error, None, "log: {:?}", game.events);
        }
        assert_eq!(was_offered(&seen, "build|dreadnought|1"), builds, "tech: {tech}");
        assert_eq!(ships(&game.state, &home, &a(), "dreadnought"), usize::from(builds), "tech: {tech}");
        if builds {
            assert_eq!(game.state.player(&a()).unwrap().trade_goods, 0, "both goods were spent");
        }
    }
}

#[test]
fn ambush_1_two_cruisers_or_destroyers_roll_at_the_start_and_each_hit_is_assigned_by_the_opponent() {
    let fight = |faction: &str, faces: &[u32], choice: &str| {
        let mut state = seated_all(&[("a", faction), ("b", "sol")]);
        put(&mut state, &sys("19"), "cruiser", &a(), 1);
        put(&mut state, &sys("19"), "destroyer", &a(), 2);
        put(&mut state, &sys("19"), "carrier", &b(), 1);
        let (_, dice) = space_fight(&mut state, &sys("19"), faces, &mut quiet_table(&[choice]));
        (first_roll(&dice, "b").is_some(), ships(&state, &sys("19"), &b(), "carrier"))
    };
    // Cruiser hits on 7, destroyer on 9: a 7 and an 8 give one hit, which kills the lone carrier
    // before the first round, so the Sol player never rolls.
    assert_eq!(fight("mentak", &[7, 8], "cruiser+destroyer"), (false, 0));
    // A 6 misses a 7: no hit, an ordinary first round (the carrier rolls and survives or not by dice).
    assert!(fight("mentak", &[6, 1, 1, 1, 1], "cruiser").0);
    // Another faction has no Ambush: its first round is rolled normally.
    assert!(fight("sol", &[7, 8], "cruiser+destroyer").0);
}

#[test]
fn pop_1_the_holder_places_promise_of_protection_with_an_action_and_mentak_cannot_pillage_them() {
    let note = crate::promissory::note_id("pop", "mentak");
    let mut state = mentak_game();
    state.rules_version = ti4_model::state::RULES_FACTION_EFFECTS;
    crate::promissory::take(&mut state, content(), &b(), &note);
    assert!(!state.promissory_faceup.contains(&note), "waits in hand");
    let (mut game, seen) = action_game(state, "b", Some(ghost_galaxy()), &["faction|mentak|pop"]);
    for _ in 0..2 {
        assert_eq!(game.step().error, None, "log: {:?}", game.events);
    }
    assert!(was_offered(&seen, "faction|mentak|pop"), "offered to the holder in the action menu");
    assert!(game.state.promissory_faceup.contains(&note), "placed faceup in the holder's play area");
    assert_eq!(game.state.promissory_notes.get(&note), Some(&b()), "still the holder's");
    // The Mentak player is not offered the holder's copy.
    let mut state = mentak_game();
    state.rules_version = ti4_model::state::RULES_FACTION_EFFECTS;
    crate::promissory::take(&mut state, content(), &b(), &note);
    let (mut game, seen) = action_game(state, "a", Some(ghost_galaxy()), &[]);
    assert_eq!(game.step().error, None);
    assert!(!seen.borrow().is_empty() && !was_offered(&seen, "faction|mentak|pop"));
}

// -- Naalu ----------------------------------------------------------------------------------------

fn naalu_game() -> GameState {
    let mut state = seated_all(&[("a", "naalu"), ("b", "sol")]);
    crate::promissory::deal(&mut state, content(), DEFAULT);
    state
}

#[test]
fn telepathic_1_naalu_acts_first_in_the_action_phase_whatever_card_it_took() {
    // Sol takes Leadership (1), Naalu takes the second card: initiative would put Sol first.
    for (faction, naalu_first) in [("naalu", true), ("hacan", false)] {
        let state = seated_all(&[("a", "sol"), ("b", faction)]);
        let (pick, _) = Pick::new(&["pok1leadership", "pok2diplomacy"]);
        let mut game = Game::with_table(state, content(), Table::with_default(Box::new(pick))).with_sources(DEFAULT);
        for _ in 0..8 {
            assert_eq!(game.step().error, None, "log: {:?}", game.events);
            if game.state.phase == Phase::Action {
                break;
            }
        }
        assert_eq!(game.state.phase, Phase::Action);
        assert_eq!(game.state.active, Some(if naalu_first { b() } else { a() }), "{faction}");
    }
}

#[test]
fn naalubt_1_mindsieve_follows_a_secondary_for_a_promissory_note_instead_of_a_token() {
    let run = |breakthrough: bool| {
        let mut state = naalu_game();
        if breakthrough {
            state.player_mut(&a()).unwrap().breakthrough = Some(ti4_model::id::BreakthroughId::new("naalubt"));
        }
        state.player_mut(&a()).unwrap().strategic_tokens = 0;
        state.player_mut(&a()).unwrap().commodities = 0;
        state.player_mut(&b()).unwrap().strategy_cards = vec![ti4_model::id::StrategyCardId::new("pok5trade")];
        let notes = crate::promissory::held_by(&state, &a()).len();
        let (mut game, seen) = action_game(state, "b", Some(ghost_galaxy()), &["strategic", "~ms|"]);
        for _ in 0..40 {
            assert_eq!(game.step().error, None, "log: {:?}", game.events);
            if game.events.iter().any(|event| event == "STRATEGIC_ACTION_COMPLETE") {
                break;
            }
        }
        let offered = seen.borrow().iter().flatten().any(|id| id.contains("ms|"));
        (offered, game.state.player(&a()).unwrap().commodities, notes - crate::promissory::held_by(&game.state, &a()).len())
    };
    let (offered, commodities, given) = run(true);
    assert!(offered, "a waiver per note in hand is offered to a follower with no token");
    assert!(commodities > 0, "the Trade secondary resolved: commodities replenished");
    assert_eq!(given, 1, "one promissory note went to the primary player");
    let (offered, commodities, given) = run(false);
    assert!(!offered && commodities == 0 && given == 0, "without Mindsieve a follower with no token cannot follow");
}

// -- Nekro Virus ----------------------------------------------------------------------------------

fn nekro_game() -> GameState {
    seated_all(&[("a", "nekro"), ("b", "sol")])
}

#[test]
fn nekrocommander_2_draws_an_action_card_after_a_technology_is_gained_only_once_unlocked() {
    for (status, draws) in [(LeaderStatus::Unlocked, true), (LeaderStatus::Locked, false)] {
        let mut state = nekro_game();
        set_leader(&mut state, &a(), "nekrocommander", status);
        state.action_card_deck = vec![ti4_model::id::ActionCardId::new("sabotage")];
        let hand = state.player(&a()).unwrap().action_cards.len();
        let ability = "leader:nekro:nekrocommander:TECHNOLOGY_GAINED:after";
        let (mut game, seen) = action_game(state, "a", None, &[ability]);
        assert_eq!(game.step().error, None, "the first look records the starting technologies");
        super::give(&mut game.state, "a", "gd");
        for _ in 0..3 {
            assert_eq!(game.step().error, None, "log: {:?}", game.events);
        }
        assert_eq!(was_offered(&seen, ability), draws, "{status:?}");
        assert_eq!(game.state.player(&a()).unwrap().action_cards.len(), hand + usize::from(draws), "{status:?}");
    }
}

/// Records who was asked what and always answers with the first option.
struct Spy(Rc<RefCell<Vec<(String, String)>>>);

impl Decider for Spy {
    fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
        self.0.borrow_mut().push((choice.player.to_string(), choice.prompt.clone()));
        choice.options.first().cloned().ok_or_else(|| IllegalChoice::NoOptions {
            player: choice.player.clone(),
            prompt: choice.prompt.clone(),
        })
    }
}

#[test]
fn galactic_threat_1_the_nekro_is_never_asked_to_vote_on_an_agenda() {
    let law = content()
        .records(ti4_model::content_types::ContentType::Agendas)
        .iter()
        .find(|record| record.text("type") == Some("Law") && record.text("target") == Some("For/Against"))
        .and_then(|record| record.text("alias"))
        .expect("a For/Against law")
        .to_owned();
    let mut state = nekro_game();
    state.phase = Phase::Agenda;
    state.custodians_removed = true;
    state.agenda_deck = vec![law];
    // Both seats control a planet with influence, so both could vote.
    let planets = ti4_content::galaxy::all_planets(content(), DEFAULT);
    for (seat, (id, planet)) in [a(), b()].iter().zip(
        planets.iter().filter(|(_, p)| p.influence() > 0 && !p.is_placed_during_play() && p.homeworld_of().is_none()),
    ) {
        state
            .system_mut(&sys(planet.system_id().unwrap_or("18")))
            .set_control(PlanetId::new(*id), seat.clone());
    }
    let asked = Rc::new(RefCell::new(Vec::new()));
    let mut game = Game::with_table(state, content(), Table::with_default(Box::new(Spy(asked.clone()))))
        .with_sources(DEFAULT);
    for _ in 0..80 {
        if game.state.phase != Phase::Agenda {
            break;
        }
        assert_eq!(game.step().error, None, "log: {:?}", game.events);
    }
    assert!(game.events.iter().any(|event| event.starts_with("AGENDA_RESOLVED:")), "the agenda was decided");
    let asked = asked.borrow();
    assert!(asked.iter().any(|(who, prompt)| who == "b" && prompt.contains("vote")), "Sol voted: {asked:?}");
    assert!(!asked.iter().any(|(who, prompt)| who == "a" && prompt.contains("vote")), "the Nekro was asked: {asked:?}");
}

#[test]
fn nekroagent_1_malleon_pays_the_chosen_player_two_trade_goods_for_a_token_and_readies_at_status() {
    let mut state = nekro_game();
    state.player_mut(&b()).unwrap().trade_goods = 0;
    let tactic = state.player(&b()).unwrap().tactic_tokens;
    let (mut game, _) = action_game(state, "a", None, &["component|leader|nekroagent", "b", "token|tactic"]);
    for _ in 0..3 {
        assert_eq!(game.step().error, None, "log: {:?}", game.events);
    }
    let seat = game.state.player(&b()).unwrap();
    assert_eq!((seat.trade_goods, seat.tactic_tokens), (2, tactic - 1));
    assert_eq!(game.state.player(&a()).unwrap().trade_goods, 0, "the Nekro gains nothing");
    assert_eq!(leader_status(&game.state, &a(), "nekroagent"), Some(LeaderStatus::Exhausted));
    run_status_phase(&mut game.state);
    assert_eq!(leader_status(&game.state, &a(), "nekroagent"), Some(LeaderStatus::Readied));
}

#[test]
fn nekrocommander_1_unlocks_in_the_game_with_three_technologies() {
    let mut state = nekro_game();
    state.player_mut(&a()).unwrap().technologies.clear();
    for tech in ["gd", "dxa"] {
        super::give(&mut state, "a", tech);
    }
    assert!(!unlocks_in_the_game(&state, &a(), "nekrocommander"), "two technologies");
    super::give(&mut state, "a", "ac2");
    assert!(unlocks_in_the_game(&state, &a(), "nekrocommander"), "three technologies");
}

#[test]
fn nekrohero_1_unlocks_in_the_game_on_the_third_scored_objective() {
    let mut state = nekro_game();
    score_objectives(&mut state, &a(), 2);
    assert!(!unlocks_in_the_game(&state, &a(), "nekrohero"));
    score_objectives(&mut state, &a(), 3);
    assert!(unlocks_in_the_game(&state, &a(), "nekrohero"));
}

/// A system whose only technology-specialty planet is `planet`, and the colour of that specialty.
fn specialty_planet() -> (SystemId, PlanetId, &'static str) {
    for (id, record) in ti4_content::galaxy::all_planets(content(), DEFAULT) {
        let (Some(specialty), Some(tile)) = (record.tech_specialties().first().copied(), record.system_id()) else {
            continue;
        };
        if ti4_content::galaxy::planets_in(content(), tile, DEFAULT)
            .iter()
            .filter(|planet| !planet.tech_specialties().is_empty())
            .count()
            != 1
        {
            continue;
        }
        let colour = match specialty.to_ascii_uppercase().as_str() {
            "BIOTIC" => "BIOTIC",
            "CYBERNETIC" => "CYBERNETIC",
            "PROPULSION" => "PROPULSION",
            "WARFARE" => "WARFARE",
            _ => continue,
        };
        return (SystemId::new(tile), PlanetId::new(id), colour);
    }
    panic!("a planet with a technology specialty");
}

#[test]
fn nekrohero_2_mallice_devours_a_specialty_planet_for_goods_and_a_technology_and_is_purged() {
    let (system, planet, colour) = specialty_planet();
    let mut state = nekro_game();
    set_leader(&mut state, &a(), "nekrohero", LeaderStatus::Unlocked);
    put(&mut state, &system, "cruiser", &a(), 1);
    put_on_planet(&mut state, &system, &planet, "infantry", &b(), 2);
    put_on_planet(&mut state, &system, &planet, "pds", &b(), 1);
    put_on_planet(&mut state, &system, &planet, "infantry", &a(), 1);
    let worth = |spend| crate::production::planet_value_now(&state, content(), DEFAULT, &planet, spend);
    let value = worth(crate::production::Spend::Resources) + worth(crate::production::Spend::Influence);
    let (goods, owned) = {
        let seat = state.player(&a()).unwrap();
        (seat.trade_goods, seat.technologies.clone())
    };
    let (mut game, _) = eager_game(state, "a", None, &["component|leader|nekrohero"]);
    for _ in 0..3 {
        assert_eq!(game.step().error, None, "log: {:?}", game.events);
    }
    let seat = game.state.player(&a()).unwrap();
    assert_eq!(i64::from(seat.trade_goods - goods), value, "goods equal resources plus influence");
    let gained: Vec<_> = seat.technologies.difference(&owned).collect();
    assert_eq!(gained.len(), 1, "exactly one technology");
    assert_eq!(crate::technology::colour_type(content(), gained[0]), Some(colour), "of the planet's specialty");
    assert_eq!(on_planet(&game.state, &system, &planet, &b(), "infantry"), 0, "their units are destroyed");
    assert_eq!(on_planet(&game.state, &system, &planet, &b(), "pds"), 0, "structures too");
    assert_eq!(on_planet(&game.state, &system, &planet, &a(), "infantry"), 1, "its own are untouched");
    assert_eq!(leader_status(&game.state, &a(), "nekrohero"), Some(LeaderStatus::Purged));
}

#[test]
fn antivirus_3_activating_a_system_with_the_nekros_units_returns_the_faceup_note() {
    let note = "antivirus:nekro";
    for (target, returns) in [("19", true), ("20", false)] {
        let mut state = nekro_game();
        put(&mut state, &sys("19"), "cruiser", &a(), 1);
        put(&mut state, &sys("20"), "cruiser", &b(), 1);
        state.promissory_notes.insert(note.to_owned(), b());
        state.promissory_faceup.insert(note.to_owned());
        let (mut game, _) = action_game(state, "b", Some(ghost_galaxy()), &[TACTICAL_ACTION_ID, target]);
        for _ in 0..4 {
            assert_eq!(game.step().error, None, "log: {:?}", game.events);
        }
        assert_eq!(game.state.promissory_faceup.contains(note), !returns, "activated {target}");
        assert_eq!(game.state.promissory_notes.get(note), Some(&if returns { a() } else { b() }));
    }
}

/// Fighters committed through Matriarch fight the defender's infantry on a planet and go back to
/// the space area; the invader holds no ground force afterwards (the engine returns them before
/// control is checked).
fn matriarch_fight(mut state: GameState, flagship_present: bool) -> (usize, usize) {
    let (system, planet) = one_planet();
    // The Nekro starts with Dacxive Animators (an infantry for the winner), which is not under test.
    state.player_mut(&a()).unwrap().technologies.remove(&ti4_model::id::TechnologyId::new("dxa"));
    state.system_mut(&system).set_control(planet.clone(), b());
    put_on_planet(&mut state, &system, &planet, "infantry", &b(), 1);
    let fighter = if state.player(&a()).unwrap().faction.as_str() == "naalu" { "naalu_fighter" } else { "fighter" };
    let flagship = if fighter == "fighter" { "nekro_flagship" } else { "naalu_flagship" };
    if flagship_present {
        put(&mut state, &system, flagship, &a(), 1);
    }
    put(&mut state, &system, fighter, &a(), 2);
    // The commit step is answered with the first option (commit the fighters), then done.
    let (report, _) = invade(&mut state, &system, &a(), &[10; 6], &mut Table::new());
    assert!(report.captured.is_empty(), "the fighters are back in space before control is checked: {report:?}");
    (on_planet(&state, &system, &planet, &b(), "infantry"), ships(&state, &system, &a(), fighter))
}

#[test]
fn nekrobt_1_the_flagship_gains_the_text_of_the_flagship_a_z_token_sits_on() {
    // Matriarch lets fighters be committed to planets as if they were ground forces.
    let with = crate::fixtures::nekro_with_z(&[("a", "nekro"), ("b", "sol")], &["naalu"]);
    assert_eq!(matriarch_fight(with, true), (0, 2), "the fighters killed the defender and returned");
    let without = crate::fixtures::nekro_with_z(&[("a", "nekro"), ("b", "sol")], &[]);
    assert_eq!(matriarch_fight(without, true), (1, 2), "no Z token, no commit: the defender is untouched");
}

#[test]
fn unit_naalu_flagship_1_matriarch_commits_fighters_to_planets_as_ground_forces() {
    assert_eq!(matriarch_fight(seated_all(&[("a", "naalu"), ("b", "sol")]), true), (0, 2));
    // Control: the fighters alone, without the flagship in the system, are not ground forces.
    assert_eq!(matriarch_fight(seated_all(&[("a", "naalu"), ("b", "sol")]), false), (1, 2));
}
