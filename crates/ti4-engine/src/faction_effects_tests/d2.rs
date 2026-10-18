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

/// One question a [`Pick`] was asked.
#[derive(Clone, Debug)]
pub(crate) struct Asked {
    pub(crate) player: String,
    pub(crate) prompt: String,
    pub(crate) options: Vec<String>,
}

/// What a [`Pick`] saw: every question, in order.
pub(crate) type Seen = Rc<RefCell<Vec<Asked>>>;

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
        self.seen.borrow_mut().push(Asked {
            player: choice.player.to_string(),
            prompt: choice.prompt.clone(),
            options: choice.options.iter().map(|option| option.id.clone()).collect(),
        });
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
    seen.borrow().iter().flat_map(|asked| &asked.options).any(|offered| offered == id)
}

/// Did any question offer an option whose id contains `part`?
pub(crate) fn was_offered_like(seen: &Seen, part: &str) -> bool {
    seen.borrow().iter().flat_map(|asked| &asked.options).any(|offered| offered.contains(part))
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

/// `faces`, then hits: whatever fight is still going afterwards ends quickly (an invasion window
/// has no round cap, so a miss-padded stalemate would drain the dice).
pub(crate) fn padded_hits(faces: &[u32]) -> Vec<u32> {
    faces.iter().copied().chain(std::iter::repeat_n(10, 800)).collect()
}

/// A whole space combat in `system` with these dice faces loaded first (then the seeded stream),
/// the table answering the questions. Returns the outcome and the dice that were rolled.
pub(crate) fn space_fight(
    state: &mut GameState,
    system: &SystemId,
    faces: &[u32],
    table: &mut Table,
) -> (crate::combat::CombatOutcome, Dice) {
    space_fight_in(DEFAULT, state, system, faces, table)
}

/// [`space_fight`] under another content scope.
pub(crate) fn space_fight_in(
    sources: ti4_model::content_types::SourceSet,
    state: &mut GameState,
    system: &SystemId,
    faces: &[u32],
    table: &mut Table,
) -> (crate::combat::CombatOutcome, Dice) {
    let mut dice = Dice::from_faces(padded_hits(faces));
    let mut rng = GameRng::new(11);
    state.active_system = Some(system.clone());
    let outcome = crate::combat::resolve(state, content(), sources, table, &mut dice, &mut rng, system)
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
    invade_in(DEFAULT, state, system, invader, faces, table)
}

/// [`invade`] under another content scope.
pub(crate) fn invade_in(
    sources: ti4_model::content_types::SourceSet,
    state: &mut GameState,
    system: &SystemId,
    invader: &PlayerId,
    faces: &[u32],
    table: &mut Table,
) -> (crate::invasion::InvasionReport, Dice) {
    let mut dice = Dice::from_faces(padded_hits(faces));
    let mut rng = GameRng::new(11);
    state.active_system = Some(system.clone());
    let report = crate::invasion::resolve(
        state,
        content(),
        sources,
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
    unlocks_on_the_map(state, who, leader, None)
}

/// [`unlocks_in_the_game`] for a game with a map (Naalu's commander asks about Mecatol Rex).
pub(crate) fn unlocks_on_the_map(state: &GameState, who: &PlayerId, leader: &str, galaxy: Option<Galaxy>) -> bool {
    let mut state = state.clone();
    state.phase = Phase::Action;
    state.active = Some(who.clone());
    let mut game = Game::with_table(state, content(), Table::with_default(Box::new(AlwaysDecline)))
        .with_sources(DEFAULT);
    if let Some(galaxy) = galaxy {
        game = game.with_galaxy(galaxy);
    }
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
        state.system_mut(&system).set_control(planet.clone(), b());
        put(&mut state, &system, "dreadnought", &a(), 1);
        put(&mut state, &system, "infantry", &a(), 3);
        put_on_planet(&mut state, &system, &planet, "infantry", &b(), 1);
        // The opening bombardment misses, round one misses on both sides, the Harrow die hits.
        let (report, dice) = invade(&mut state, &system, &a(), &[1, 1, 1, 1, 1, 10], &mut Table::new());
        let harrow_rolls = dice.history().iter().filter(|roll| roll.reason == "harrow").count();
        if harrows {
            assert_eq!(harrow_rolls, 1, "one extra bombardment at the end of round one");
            assert_eq!(on_planet(&state, &system, &planet, &b(), "infantry"), 0, "it killed the last defender");
            assert_eq!(report.captured.len(), 1);
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
        let offered = was_offered_like(&seen, "ms|");
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

// -- Sardakk N'orr --------------------------------------------------------------------------------

fn sardakk_game() -> GameState {
    seated_all(&[("a", "sardakk"), ("b", "hacan")])
}

#[test]
fn unrelenting_1_every_norr_combat_roll_gets_plus_one_in_space_and_on_the_ground() {
    // Space: both sides' cruisers hit on 7; every die is a 6, so only the N'orr one hits.
    let mut state = sardakk_game();
    put(&mut state, &sys("19"), "cruiser", &a(), 1);
    put(&mut state, &sys("19"), "cruiser", &b(), 1);
    let (outcome, _) = space_fight(&mut state, &sys("19"), &[6, 6], &mut Table::new());
    assert_eq!(outcome.winner, Some(a()), "the N'orr 6 counts as a 7");
    assert_eq!(ships(&state, &sys("19"), &b(), "cruiser"), 0);
    // Ground: infantry hit on 8; a 7 hits for the N'orr player only.
    let mut state = sardakk_game();
    let (system, planet) = one_planet();
    state.system_mut(&system).set_control(planet.clone(), a());
    put_on_planet(&mut state, &system, &planet, "infantry", &a(), 1);
    put(&mut state, &system, "infantry", &b(), 1);
    let _ = invade(&mut state, &system, &b(), &[7, 7], &mut Table::new());
    assert_eq!(on_planet(&state, &system, &planet, &b(), "infantry"), 0, "the invader's 7 missed");
    assert_eq!(on_planet(&state, &system, &planet, &a(), "infantry"), 1, "the N'orr 7 hit and it survived");
}

#[test]
fn vpw_1_one_additional_ground_hit_when_the_opponent_produced_hits() {
    let run = |tech: bool| {
        let mut state = sardakk_game();
        if tech {
            super::give(&mut state, "a", "vpw");
        }
        let (system, planet) = one_planet();
        state.system_mut(&system).set_control(planet.clone(), b());
        put(&mut state, &system, "infantry", &a(), 1);
        put_on_planet(&mut state, &system, &planet, "infantry", &b(), 1);
        // Round one: a [1] misses, b [10] hits, so a is wiped out; with the weave a also scores.
        let _ = invade(&mut state, &system, &a(), &[1, 10], &mut Table::new());
        on_planet(&state, &system, &planet, &b(), "infantry")
    };
    assert_eq!(run(false), 1, "without the technology the defender survives");
    assert_eq!(run(true), 0, "with it the N'orr produces one additional hit");
}

/// A one-ring map with the system `x` in the middle.
fn hub_around(x: &SystemId) -> crate::fixtures::Hub {
    let mut ids = vec![x.to_string()];
    ids.extend(crate::fixtures::plain_systems(9).into_iter().filter(|id| id != x.as_str()).take(6));
    crate::fixtures::hub_from(&ids)
}

/// The N'orr player `a` takes a tactical action into the lone-planet system `x`, where it has a
/// carrier and no ground forces; its infantry waits on a planet of the adjacent system `near`.
fn commander_game(status: LeaderStatus, token_in_near: bool) -> (Game<'static>, Seen, SystemId, PlanetId, SystemId) {
    let (x, planet) = crate::fixtures::a_placed_planet();
    let hub = hub_around(&x);
    let near = sys(&hub.outer[0]);
    let mut state = sardakk_game();
    set_leader(&mut state, &a(), "sardakkcommander", status);
    put(&mut state, &x, "carrier", &a(), 1);
    put_on_planet(&mut state, &near, &PlanetId::new("test-near"), "infantry", &a(), 1);
    if token_in_near {
        state.system_mut(&near).place_token(a());
    }
    let (game, seen) = action_game(
        state,
        "a",
        Some(hub.galaxy),
        &[TACTICAL_ACTION_ID, x.as_str(), "done_moving", "~commit|", "done_committing"],
    );
    (game, seen, x, planet, near)
}

#[test]
fn sardakkcommander_2_commits_a_ground_force_from_an_adjacent_planet_without_the_invaders_token() {
    for (status, token, commits) in [
        (LeaderStatus::Unlocked, false, true),
        (LeaderStatus::Locked, false, false),
        (LeaderStatus::Unlocked, true, false),
    ] {
        let (mut game, seen, x, planet, near) = commander_game(status, token);
        for _ in 0..6 {
            assert_eq!(game.step().error, None, "log: {:?}", game.events);
        }
        let label = format!("{status:?}, token in the adjacent system: {token}");
        assert_eq!(was_offered(&seen, "commit|0|0.0.0"), commits, "{label}");
        assert_eq!(game.state.system_state(&x).planet_control.get(&planet).is_some(), commits, "{label}");
        assert_eq!(
            on_planet(&game.state, &near, &PlanetId::new("test-near"), &a(), "infantry"),
            usize::from(!commits),
            "{label}"
        );
    }
}

#[test]
fn sardakkcommander_1_unlocks_in_the_game_with_five_planets_outside_home_systems() {
    let mut state = sardakk_game();
    let planets: Vec<(SystemId, PlanetId)> = ti4_content::galaxy::all_planets(content(), DEFAULT)
        .iter()
        .filter(|(_, planet)| planet.homeworld_of().is_none() && !planet.is_placed_during_play())
        .filter_map(|(id, planet)| planet.system_id().map(|system| (sys(system), PlanetId::new(*id))))
        .filter(|(system, _)| !ti4_content::galaxy::is_home_system(content(), system.as_str(), DEFAULT))
        .take(5)
        .collect();
    for (system, planet) in &planets[..4] {
        state.system_mut(system).set_control(planet.clone(), a());
    }
    assert!(!unlocks_in_the_game(&state, &a(), "sardakkcommander"), "four planets");
    let (system, planet) = &planets[4];
    state.system_mut(system).set_control(planet.clone(), a());
    assert!(unlocks_in_the_game(&state, &a(), "sardakkcommander"), "five planets");
}

#[test]
fn sardakkhero_1_unlocks_in_the_game_on_the_third_scored_objective() {
    let mut state = sardakk_game();
    score_objectives(&mut state, &a(), 2);
    assert!(!unlocks_in_the_game(&state, &a(), "sardakkhero"));
    score_objectives(&mut state, &a(), 3);
    assert!(unlocks_in_the_game(&state, &a(), "sardakkhero"));
}

/// Hacan `b` invades the N'orr `a`'s planet with 3 infantry against 2 and holds Tekklar Legion when
/// `note` is set. Returns the `hits_on` of the first ground-combat roll of each side
/// `(invader b, defender a)` and the note's holder afterwards.
fn tekklar_fight(note: bool) -> ((Option<u32>, Option<u32>), Option<PlayerId>, Seen) {
    let (x, planet) = crate::fixtures::a_placed_planet();
    let mut state = sardakk_game();
    state.system_mut(&x).units.clear();
    state.system_mut(&x).planet_units.clear();
    state.system_mut(&x).set_control(planet.clone(), a());
    put_on_planet(&mut state, &x, &planet, "infantry", &a(), 2);
    put(&mut state, &x, "carrier", &b(), 1);
    put(&mut state, &x, "infantry", &b(), 3);
    let tekklar = "tekklar:sardakk";
    if note {
        state.promissory_notes.insert(tekklar.to_owned(), b());
    }
    let (mut game, seen) = action_game(
        state,
        "b",
        Some(hub_around(&x).galaxy),
        &[TACTICAL_ACTION_ID, x.as_str(), "done_moving", "~commit|", "~commit|", "~commit|", "~tekklar"],
    );
    for _ in 0..20 {
        assert_eq!(game.step().error, None, "log: {:?}", game.events);
        if game.rolls().iter().any(|roll| roll.reason == "ground combat") {
            break;
        }
    }
    let first = |who: &str| {
        game.rolls()
            .iter()
            .find(|roll| roll.reason == "ground combat" && roll.by.as_deref() == Some(who))
            .and_then(|roll| roll.hits_on)
    };
    ((first("b"), first("a")), game.state.promissory_notes.get(tekklar).cloned(), seen)
}

#[test]
fn tekklar_1_the_holder_rolls_one_better_the_norr_one_worse_and_the_note_goes_home() {
    // Hacan infantry hit on 8, N'orr infantry on 8 minus Unrelenting's 1.
    let (plain, _, _) = tekklar_fight(false);
    assert_eq!(plain, (Some(8), Some(7)), "no note: printed values and Unrelenting");
    let (played, holder, seen) = tekklar_fight(true);
    assert!(was_offered(&seen, "promissory:hacan:tekklar:GROUND_COMBAT_STARTED:after"), "offered to the holder");
    assert_eq!(played, (Some(7), Some(8)), "+1 for the holder, -1 for the N'orr player");
    assert_eq!(holder, Some(a()), "returned to the N'orr player");
}

#[test]
fn mentakcommander_1_unlocks_in_the_game_with_four_cruisers_on_the_board() {
    let mut state = mentak_game();
    state.board.values_mut().for_each(|board| board.units.retain(|unit| unit.type_id.as_str() != "cruiser"));
    put(&mut state, &sys("19"), "cruiser", &a(), 2);
    put(&mut state, &sys("20"), "cruiser", &a(), 1);
    put(&mut state, &sys("21"), "cruiser", &b(), 4);
    assert!(!unlocks_in_the_game(&state, &a(), "mentakcommander"), "three of its own");
    put(&mut state, &sys("25"), "cruiser", &a(), 1);
    assert!(unlocks_in_the_game(&state, &a(), "mentakcommander"), "four cruisers");
}

#[test]
fn mentakhero_1_unlocks_in_the_game_on_the_third_scored_objective() {
    let mut state = mentak_game();
    score_objectives(&mut state, &a(), 2);
    assert!(!unlocks_in_the_game(&state, &a(), "mentakhero"));
    score_objectives(&mut state, &a(), 3);
    assert!(unlocks_in_the_game(&state, &a(), "mentakhero"));
}

#[test]
fn naalucommander_1_unlocks_in_the_game_with_ground_forces_in_or_next_to_mecatol() {
    let galaxy = || ghost_galaxy();
    let infantry_in = |system: &str| {
        let mut state = naalu_game();
        let planet = ti4_content::galaxy::system(content(), system, DEFAULT)
            .and_then(|tile| tile.planets().into_iter().next())
            .map(PlanetId::new)
            .expect("a planet");
        put_on_planet(&mut state, &sys(system), &planet, "infantry", &a(), 1);
        state
    };
    assert!(!unlocks_on_the_map(&naalu_game(), &a(), "naalucommander", Some(galaxy())), "no ground forces");
    assert!(unlocks_on_the_map(&infantry_in("19"), &a(), "naalucommander", Some(galaxy())), "next to Mecatol");
    assert!(!unlocks_on_the_map(&infantry_in("20"), &a(), "naalucommander", Some(galaxy())), "two away");
}

#[test]
fn naaluhero_1_unlocks_in_the_game_on_the_third_scored_objective() {
    let mut state = naalu_game();
    score_objectives(&mut state, &a(), 2);
    assert!(!unlocks_in_the_game(&state, &a(), "naaluhero"));
    score_objectives(&mut state, &a(), 3);
    assert!(unlocks_in_the_game(&state, &a(), "naaluhero"));
}

#[test]
fn the_agents_of_these_factions_ready_in_the_status_phase() {
    for (faction, agent) in [
        ("ghost", "ghostagent"),
        ("l1z1x", "l1z1xagent"),
        ("mentak", "mentakagent"),
        ("naalu", "naaluagent-te"),
        ("nekro", "nekroagent"),
        ("sardakk", "sardakkagent"),
    ] {
        let mut state = seated_all(&[("a", faction), ("b", "sol")]);
        set_leader(&mut state, &a(), agent, LeaderStatus::Exhausted);
        run_status_phase(&mut state);
        assert_eq!(leader_status(&state, &a(), agent), Some(LeaderStatus::Readied), "{agent}");
    }
}

#[test]
fn vax_2_and_vay_2_an_assimilated_technology_is_played_as_text_exhausting_the_valefar_card() {
    // Yssaril's Mageon Implants ("ACTION: exhaust: look at another player's action cards and take
    // one") through the Nekro's X or Y token, in the action menu of a running game.
    for card in ["vax", "vay"] {
        let mut state = seated_all(&[("a", "nekro"), ("b", "yssaril")]);
        for tech in ["vax", "vay"] {
            super::give(&mut state, "a", tech);
        }
        super::give(&mut state, "b", "mi");
        state.player_mut(&b()).unwrap().action_cards = vec![ti4_model::id::ActionCardId::new("sabotage")];
        state.player_mut(&a()).unwrap().action_cards.clear();
        state.player_mut(&a()).unwrap().assimilated_technologies.insert(card.to_owned(), ti4_model::id::TechnologyId::new("mi"));
        let (mut game, seen) = eager_game(state, "a", None, &["~faction|yssaril|mi|"]);
        for _ in 0..3 {
            assert_eq!(game.step().error, None, "log: {:?}", game.events);
        }
        assert!(was_offered_like(&seen, "faction|yssaril|mi|"), "{card}: offered");
        let seat = game.state.player(&a()).unwrap();
        assert_eq!(seat.action_cards.len(), 1, "{card}: the Nekro took the Yssaril player's card");
        assert!(
            seat.exhausted_technologies.contains(&ti4_model::id::TechnologyId::new(card)),
            "{card}: the Valefar card is the one exhausted"
        );
    }
}

// -- unit abilities -------------------------------------------------------------------------------

/// What `kind` does in a real space combat against one Sol cruiser, every die a 10:
/// `(hits_on, dice, kept after the cruiser's hit with damage)` -- the last is a unit that used
/// SUSTAIN DAMAGE and is still standing.
fn space_profile(faction: &str, kind: &str) -> (Option<u32>, usize, bool) {
    let mut state = seated_all(&[("a", faction), ("b", "sol")]);
    put(&mut state, &sys("19"), kind, &a(), 1);
    put(&mut state, &sys("19"), "cruiser", &b(), 1);
    let (_, dice) = space_fight(&mut state, &sys("19"), &[10; 8], &mut Table::new());
    let (hits_on, count) = first_roll(&dice, "a").expect("the unit rolled");
    let sustained = state
        .system_state(&sys("19"))
        .units
        .iter()
        .any(|unit| unit.owner == a() && unit.type_id.as_str() == kind && unit.sustained_damage);
    (hits_on, count, sustained)
}

/// The first bombardment `kind` makes from space against a Sol infantry on a planet: `(hits_on, dice)`.
fn bombard_profile(faction: &str, kind: &str) -> (Option<u32>, usize) {
    let mut state = seated_all(&[("a", faction), ("b", "sol")]);
    // Plasma Scoring (a starting technology of some of them) adds a die; not under test here.
    state.player_mut(&a()).unwrap().technologies.remove(&ti4_model::id::TechnologyId::new("ps"));
    let (system, planet) = one_planet();
    state.system_mut(&system).set_control(planet.clone(), b());
    put_on_planet(&mut state, &system, &planet, "infantry", &b(), 1);
    put(&mut state, &system, kind, &a(), 1);
    let (_, dice) = invade(&mut state, &system, &a(), &[10; 8], &mut Table::new());
    dice.history()
        .iter()
        .find(|roll| roll.reason == "bombardment" && roll.by.as_deref() == Some("a"))
        .map(|roll| (roll.hits_on, roll.faces.len()))
        .expect("the unit bombarded")
}

#[test]
fn unit_ghost_flagship_0_hil_colish_fights_with_one_die_on_5_and_sustains() {
    assert_eq!(space_profile("ghost", "ghost_flagship"), (Some(5), 1, true));
}

#[test]
fn unit_l1z1x_dreadnought_0_super_dreadnought_i_fights_and_bombards_on_5_and_sustains() {
    assert_eq!(space_profile("l1z1x", "l1z1x_dreadnought"), (Some(5), 1, true));
    assert_eq!(bombard_profile("l1z1x", "l1z1x_dreadnought"), (Some(5), 1));
}

#[test]
fn unit_l1z1x_dreadnought2_0_super_dreadnought_ii_fights_and_bombards_on_4_and_sustains() {
    assert_eq!(space_profile("l1z1x", "l1z1x_dreadnought2"), (Some(4), 1, true));
    assert_eq!(bombard_profile("l1z1x", "l1z1x_dreadnought2"), (Some(4), 1));
}

#[test]
fn unit_l1z1x_flagship_0_fights_with_two_dice_on_5_and_sustains() {
    assert_eq!(space_profile("l1z1x", "l1z1x_flagship"), (Some(5), 2, true));
}

#[test]
fn unit_mentak_flagship_0_fourth_moon_fights_with_two_dice_on_7_and_sustains() {
    assert_eq!(space_profile("mentak", "mentak_flagship"), (Some(7), 2, true));
}

#[test]
fn unit_mentak_cruiser3_0_corsair_fights_on_6_and_cannot_sustain() {
    let (hits_on, dice, sustained) = space_profile("mentak", "mentak_cruiser3");
    assert_eq!((hits_on, dice, sustained), (Some(6), 1, false));
}

#[test]
fn unit_naalu_fighter_0_hybrid_crystal_fighter_i_hits_on_8() {
    let (hits_on, dice, _) = space_profile("naalu", "naalu_fighter");
    assert_eq!((hits_on, dice), (Some(8), 1), "a plain fighter hits on 9");
}

#[test]
fn unit_naalu_fighter2_0_hybrid_crystal_fighter_ii_hits_on_7() {
    let (hits_on, dice, _) = space_profile("naalu", "naalu_fighter2");
    assert_eq!((hits_on, dice), (Some(7), 1));
}

#[test]
fn unit_naalu_flagship_0_matriarch_fights_with_two_dice_on_9_and_sustains() {
    assert_eq!(space_profile("naalu", "naalu_flagship"), (Some(9), 2, true));
}

#[test]
fn unit_nekro_flagship_0_the_alastor_fights_with_two_dice_on_9_and_sustains() {
    assert_eq!(space_profile("nekro", "nekro_flagship"), (Some(9), 2, true));
}

#[test]
fn unit_sardakk_dreadnought_0_exotrireme_i_fights_on_5_bombards_two_dice_on_4_with_unrelenting() {
    // The printed values are 5 (combat) and 4 (bombardment); the N'orr's Unrelenting makes the
    // combat roll count one better.
    assert_eq!(space_profile("sardakk", "sardakk_dreadnought"), (Some(4), 1, true));
    let (_, dice) = bombard_profile("sardakk", "sardakk_dreadnought");
    assert_eq!(dice, 2, "two bombardment dice");
}

#[test]
fn unit_sardakk_dreadnought2_0_exotrireme_ii_fights_on_5_bombards_two_dice_on_4_with_unrelenting() {
    assert_eq!(space_profile("sardakk", "sardakk_dreadnought2"), (Some(4), 1, true));
    let (_, dice) = bombard_profile("sardakk", "sardakk_dreadnought2");
    assert_eq!(dice, 2, "two bombardment dice");
}

#[test]
fn unit_sardakk_flagship_0_c_morran_fights_with_two_dice_and_sustains() {
    // Printed 6; Unrelenting makes it 5.
    assert_eq!(space_profile("sardakk", "sardakk_flagship"), (Some(5), 2, true));
}

/// Sol (`b`, holding Direct Hit) attacks one `kind` of `faction` in system 19 with six cruisers.
/// The dreadnought uses SUSTAIN DAMAGE on the first hit it takes (dice are the game's own; six
/// cruisers make a hit in the first rounds all but certain, and a later round offers it again).
/// Returns whether Direct Hit was offered and whether the dreadnought was destroyed by the card.
fn direct_hit_on(faction: &str, kind: &str) -> (bool, bool) {
    let mut state = seated_all(&[("a", faction), ("b", "sol")]);
    put(&mut state, &sys("19"), kind, &a(), 1);
    put(&mut state, &sys("19"), "cruiser", &b(), 6);
    state.player_mut(&b()).unwrap().action_cards = vec![ti4_model::id::ActionCardId::new("dh1")];
    let card = "~:SUSTAIN_DAMAGE_USED:after";
    let (mut game, seen) = action_game(state, "b", Some(ghost_galaxy()), &[TACTICAL_ACTION_ID, "19", "done_moving", "sustain|0", card]);
    for _ in 0..30 {
        assert_eq!(game.step().error, None, "log: {:?}", game.events);
        if game.events.iter().any(|event| event == "SPACE_COMBAT_RESOLVED") {
            break;
        }
    }
    assert!(seen.borrow().iter().any(|asked| asked.options.iter().any(|id| id == "sustain|0")), "the unit sustained");
    let by_card = game
        .timing
        .applied_events()
        .iter()
        .any(|event| event.event_type == "SHIP_DESTROYED" && event.text("cause") == Some("action_card:direct_hit"));
    (was_offered_like(&seen, "SUSTAIN_DAMAGE_USED:after"), by_card)
}

#[test]
fn unit_l1z1x_dreadnought2_1_direct_hit_cannot_destroy_a_super_dreadnought_ii() {
    assert_eq!(direct_hit_on("l1z1x", "l1z1x_dreadnought"), (true, true), "Super-Dreadnought I can be hit");
    let (offered, destroyed) = direct_hit_on("l1z1x", "l1z1x_dreadnought2");
    assert!(!destroyed, "Super-Dreadnought II cannot be destroyed by Direct Hit (offered: {offered})");
}

#[test]
fn unit_sardakk_dreadnought2_1_direct_hit_cannot_destroy_an_exotrireme_ii() {
    assert_eq!(direct_hit_on("sardakk", "sardakk_dreadnought"), (true, true), "Exotrireme I can be hit");
    let (offered, destroyed) = direct_hit_on("sardakk", "sardakk_dreadnought2");
    assert!(!destroyed, "Exotrireme II cannot be destroyed by Direct Hit (offered: {offered})");
}

/// Records every question (with payloads) and answers with the first option.
struct Probe(Rc<RefCell<Vec<Choice>>>);

impl Decider for Probe {
    fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
        self.0.borrow_mut().push(choice.clone());
        choice.options.first().cloned().ok_or_else(|| IllegalChoice::NoOptions {
            player: choice.player.clone(),
            prompt: choice.prompt.clone(),
        })
    }
}

/// The unit types offered in the questions `player` was asked, one list per question.
fn offered_units(asked: &Rc<RefCell<Vec<Choice>>>, player: &str, prompt: &str) -> Vec<Vec<String>> {
    asked
        .borrow()
        .iter()
        .filter(|choice| choice.player.as_str() == player && choice.prompt.contains(prompt))
        .map(|choice| {
            choice
                .options
                .iter()
                .filter_map(|option| option.payload.get("unit").and_then(|unit| unit.as_str()).map(str::to_owned))
                .collect()
        })
        .collect()
}

#[test]
fn unit_l1z1x_flagship_1_zero_zero_one_forces_the_fleets_hits_onto_non_fighters() {
    // a: dreadnought (5, one die) and, in the first case, the flagship (5, two dice); every
    // die of the first round is a 5, which hits a 5 and misses Sol's 7 and 9.
    let hits_offered_to_sol = |flagship: bool| {
        let mut state = seated_all(&[("a", "l1z1x"), ("b", "sol")]);
        if flagship {
            put(&mut state, &sys("19"), "l1z1x_flagship", &a(), 1);
        }
        put(&mut state, &sys("19"), "l1z1x_dreadnought", &a(), 1);
        put(&mut state, &sys("19"), "cruiser", &b(), 1);
        put(&mut state, &sys("19"), "fighter", &b(), 3);
        let asked = Rc::new(RefCell::new(Vec::new()));
        let mut table = Table::with_default(Box::new(Probe(asked.clone())));
        let _ = space_fight(&mut state, &sys("19"), &[5, 5, 5, 1, 1, 1, 1], &mut table);
        offered_units(&asked, "b", "assign a hit")
    };
    let without = hits_offered_to_sol(false);
    assert!(without[0].contains(&"cruiser".to_owned()), "free choice: the fighters or the cruiser: {without:?}");
    let with = hits_offered_to_sol(true);
    assert!(
        with.iter().all(|units| !units.contains(&"cruiser".to_owned())),
        "the cruiser's hit was forced (no question about it), later hits fall on fighters: {with:?}"
    );
    assert!(!with.is_empty());
}

#[test]
fn unit_l1z1x_mech_1_anihilator_bombards_the_other_planets_of_its_system_but_not_its_own() {
    for (mech_on_target, kills) in [(false, 1), (true, 0)] {
        let mut state = l1z1x_game();
        state.player_mut(&a()).unwrap().technologies.remove(&ti4_model::id::TechnologyId::new("ps"));
        let (system, first, second) = two_planet_system();
        state.system_mut(&system).set_control(first.clone(), b());
        put_on_planet(&mut state, &system, &first, "infantry", &b(), 1);
        let mech_planet = if mech_on_target { &first } else { &second };
        put_on_planet(&mut state, &system, mech_planet, "l1z1x_mech", &a(), 1);
        let (report, dice) = invade(&mut state, &system, &a(), &[10; 4], &mut Table::new());
        assert_eq!(report.bombardment_kills, kills, "mech on the bombarded planet: {mech_on_target}");
        let bombard = dice.history().iter().find(|roll| roll.reason == "bombardment");
        assert_eq!(bombard.map(|roll| roll.hits_on), kills.eq(&1).then_some(Some(8)), "the mech bombards on 8");
    }
}

#[test]
fn unit_mentak_flagship_1_fourth_moon_stops_other_players_ships_using_sustain_damage() {
    // One hit each in round one. Sol's dreadnought can only absorb it by sustaining, which the
    // Fourth Moon forbids; with a Sol flagship in its place it sustains and fights on.
    let survivors = |faction: &str, flagship: &str| {
        let mut state = seated_all(&[("a", faction), ("b", "sol")]);
        put(&mut state, &sys("19"), flagship, &a(), 1);
        put(&mut state, &sys("19"), "dreadnought", &b(), 1);
        let (_, dice) = space_fight(&mut state, &sys("19"), &[10, 1, 10], &mut Table::new());
        let _ = dice;
        (ships(&state, &sys("19"), &a(), flagship), ships(&state, &sys("19"), &b(), "dreadnought"))
    };
    assert_eq!(survivors("mentak", "mentak_flagship"), (1, 0), "the dreadnought could not sustain and died in round one");
    assert_eq!(survivors("sol", "sol_flagship"), (0, 0), "an ordinary flagship: it sustained, so both fought another round");
}

#[test]
fn unit_mentak_mech_1_moll_terminus_stops_other_players_ground_forces_on_its_planet_sustaining() {
    // b invades; each side's mech scores one hit in round one. Sol's mech sustains the hit unless
    // the defender is Mentak's Moll Terminus.
    let survivors = |faction: &str, mech: &str| {
        let mut state = seated_all(&[("a", faction), ("b", "sol")]);
        let (system, planet) = one_planet();
        state.system_mut(&system).set_control(planet.clone(), a());
        put_on_planet(&mut state, &system, &planet, mech, &a(), 1);
        put(&mut state, &system, "sol_mech", &b(), 1);
        let _ = invade(&mut state, &system, &b(), &[10, 10], &mut Table::new());
        on_planet(&state, &system, &planet, &a(), mech)
    };
    assert_eq!(survivors("mentak", "mentak_mech"), 1, "the invader's mech could not sustain");
    assert_eq!(survivors("sol", "sol_mech"), 0, "both mechs sustained and finished each other in round two");
}

#[test]
fn unit_mentak_cruiser3_1_the_corsair_moves_through_other_players_ships_only_towards_non_fighter_ships() {
    let hub = crate::fixtures::plain_hub();
    let (origin, centre, target) = (hub.outer[0].clone(), hub.centre.clone(), hub.across(&hub.outer[0]));
    // (the moving ship, what Sol has in the target) -> is the move through the blocked centre offered?
    for (ship, defender, offered) in [
        ("mentak_cruiser3", "cruiser", true),
        ("mentak_cruiser3", "fighter", false),
        ("cruiser", "cruiser", false),
    ] {
        let mut state = mentak_game();
        state.board.clear();
        put(&mut state, &sys(&origin), ship, &a(), 1);
        // Every route out of the origin (the centre and both ring neighbours) holds a Sol ship.
        put(&mut state, &sys(&centre), "cruiser", &b(), 1);
        for next in hub.galaxy.adjacent(&origin) {
            if next != centre {
                put(&mut state, &sys(next), "cruiser", &b(), 1);
            }
        }
        put(&mut state, &sys(&target), defender, &b(), 1);
        let (_, seen) = tactical_move(state, "a", hub.galaxy.clone(), &origin, &target, &[]);
        assert_eq!(was_offered(&seen, &format!("move|{origin}|0")), offered, "{ship} against a {defender}");
    }
}

#[test]
fn unit_naalu_fighter2_1_hybrid_crystal_fighter_ii_moves_without_being_transported() {
    let hub = crate::fixtures::plain_hub();
    let (origin, target) = (hub.outer[0].clone(), hub.centre.clone());
    for (fighter, offered) in [("naalu_fighter2", true), ("naalu_fighter", false)] {
        let mut state = naalu_game();
        state.board.clear();
        put(&mut state, &sys(&origin), fighter, &a(), 1);
        let (game, seen) = tactical_move(state, "a", hub.galaxy.clone(), &origin, &target, &[]);
        assert_eq!(was_offered(&seen, &format!("move|{origin}|0")), offered, "{fighter}");
        assert_eq!(ships(&game.state, &sys(&target), &a(), fighter), usize::from(offered), "{fighter}");
    }
}

#[test]
fn unit_naalu_fighter2_2_excess_fighters_count_half_a_ship_against_the_fleet_pool() {
    // A destroyer (no capacity) and four fighter IIs: two ships' worth of fighters for Naalu, four
    // for anyone else. With three command tokens in the fleet pool only the second is over the
    // limit, and the turn's fleet-supply enforcement asks it to remove the (non-fighter) destroyer.
    for (faction, fighter, over) in [("naalu", "naalu_fighter2", false), ("sol", "fighter2", true)] {
        let mut state = seated_all(&[("a", faction), ("b", "hacan")]);
        state.board.clear();
        state.player_mut(&a()).unwrap().fleet_tokens = 3;
        put(&mut state, &sys("19"), "destroyer", &a(), 1);
        put(&mut state, &sys("19"), fighter, &a(), 4);
        let (mut game, seen) = action_game(state, "a", Some(ghost_galaxy()), &[TACTICAL_ACTION_ID, "19", "done_moving"]);
        for _ in 0..6 {
            assert_eq!(game.step().error, None, "log: {:?}", game.events);
        }
        let removal_asked = seen.borrow().iter().any(|asked| asked.prompt.contains("over fleet supply"));
        assert_eq!(removal_asked, over, "{faction}");
    }
}

#[test]
fn unit_nekro_mech_1_mordred_rolls_two_better_against_an_opponent_with_an_x_or_y_token() {
    // The Nekro invades a Sol-held planet; its mech lands from the space area. Sol owns Mageon
    // Implants, which carries an X token in one case.
    let mech_roll = |token: bool| {
        let mut state = nekro_game();
        super::give(&mut state, "b", "mi");
        if token {
            state.player_mut(&a()).unwrap().assimilated_technologies.insert("vax".to_owned(), ti4_model::id::TechnologyId::new("mi"));
        }
        let (system, planet) = one_planet();
        state.system_mut(&system).set_control(planet.clone(), b());
        put_on_planet(&mut state, &system, &planet, "infantry", &b(), 1);
        put(&mut state, &system, "nekro_mech", &a(), 1);
        let (_, dice) = invade(&mut state, &system, &a(), &[10, 10], &mut Table::new());
        first_roll(&dice, "a")
    };
    assert_eq!(mech_roll(false), Some((Some(6), 1)), "no token: the printed 6");
    assert_eq!(mech_roll(true), Some((Some(4), 1)), "+2 to the roll against an X token");
}

#[test]
fn unit_sardakk_flagship_1_c_morran_adds_one_to_every_other_ship_in_its_system() {
    // a's cruiser hits on 7; Unrelenting makes that 6, and the flagship (printed 6, Unrelenting
    // 5) makes the cruiser 5, so the cruiser's die joins the flagship's two on 5.
    let rolls = |flagship: bool| {
        let mut state = sardakk_game();
        put(&mut state, &sys("19"), "cruiser", &a(), 1);
        if flagship {
            put(&mut state, &sys("19"), "sardakk_flagship", &a(), 1);
        }
        put(&mut state, &sys("19"), "cruiser", &b(), 1);
        let (_, dice) = space_fight(&mut state, &sys("19"), &[10, 10, 10], &mut Table::new());
        let mut found: Vec<(Option<u32>, usize)> = dice
            .history()
            .iter()
            .filter(|roll| roll.by.as_deref() == Some("a") && roll.reason == "space combat")
            .take(1)
            .map(|roll| (roll.hits_on, roll.faces.len()))
            .collect();
        found.sort();
        found
    };
    assert_eq!(rolls(false), [(Some(6), 1)], "Unrelenting only");
    assert_eq!(rolls(true), [(Some(5), 3)], "the cruiser one better again, rolling with the flagship's two dice on 5");
}

/// Hacan `b` invades the planet of `a` (a `mech` standing on it) with two infantry in a running game,
/// on a seed whose first ground-combat dice are the wanted ones: both invaders score and the mech
/// misses. Returns the invaders left once the tactical action is over. (The game rolls its own dice;
/// the seed is searched for, so the test does not depend on which seed that is.)
fn invaders_left_after_two_hits(faction: &str, mech: &str) -> usize {
    for seed in 0..400u64 {
        let (x, planet) = crate::fixtures::a_placed_planet();
        let mut state = seated_all(&[("a", faction), ("b", "hacan")]);
        state.rng_seed = seed;
        state.system_mut(&x).units.clear();
        state.system_mut(&x).planet_units.clear();
        state.system_mut(&x).set_control(planet.clone(), a());
        put_on_planet(&mut state, &x, &planet, mech, &a(), 1);
        put(&mut state, &x, "carrier", &b(), 1);
        put(&mut state, &x, "infantry", &b(), 2);
        let (mut game, _) = action_game(
            state,
            "b",
            Some(hub_around(&x).galaxy),
            &[TACTICAL_ACTION_ID, x.as_str(), "done_moving", "~commit|", "~commit|", "~sustain"],
        );
        for _ in 0..20 {
            assert_eq!(game.step().error, None, "log: {:?}", game.events);
            if game.rolls().iter().filter(|roll| roll.reason == "ground combat").count() >= 2 {
                break;
            }
        }
        let first = |who: &str| {
            game.rolls().iter().find(|roll| roll.reason == "ground combat" && roll.by.as_deref() == Some(who)).cloned()
        };
        let (Some(invader), Some(defender)) = (first("b"), first("a")) else {
            continue;
        };
        let invader_hits = invader.faces.iter().filter(|face| **face >= 8).count();
        // The defender misses at 5 (Unrelenting's mech) and at 6 (an ordinary mech) when it rolls 1-4.
        if invader.faces.len() != 2 || invader_hits != 2 || defender.faces.iter().any(|face| *face > 4) {
            continue;
        }
        for _ in 0..30 {
            assert_eq!(game.step().error, None, "log: {:?}", game.events);
            if game.events.iter().any(|event| event == "INVASION_RESOLVED") {
                break;
            }
        }
        return on_planet(&game.state, &x, &planet, &b(), "infantry");
    }
    panic!("no seed gave the wanted first ground-combat round");
}

#[test]
fn unit_sardakk_mech_1_valkyrie_exoskeleton_produces_a_hit_after_it_sustains_in_ground_combat() {
    // Two hits land on the mech: it sustains the first, which produces one hit on the invaders,
    // and is destroyed by the second. An ordinary mech's sustain produces nothing.
    assert_eq!(invaders_left_after_two_hits("sardakk", "sardakk_mech"), 1, "the sustain produced a hit");
    assert_eq!(invaders_left_after_two_hits("sol", "sol_mech"), 2, "an ordinary mech: no hit");
}

// -- scopes where an older printing is dealt ------------------------------------------------------

fn pok_only() -> ti4_model::content_types::SourceSet {
    ti4_model::content_types::Source::Base | ti4_model::content_types::Source::Pok
}

#[test]
#[ignore = "MISSING/BUG: naalu_mech (Iconoclast, PoK printing) +2 to its rolls against an opponent with a relic fragment is not implemented"]
fn unit_naalu_mech_1_iconoclast_rolls_two_better_against_an_opponent_with_a_relic_fragment() {
    let mech_roll = |fragments: i32| {
        let mut state = crate::fixtures::seated_game(&[("a", "naalu"), ("b", "sol")], pok_only());
        if fragments > 0 {
            state.player_mut(&b()).unwrap().relic_fragments.insert("CULTURAL".to_owned(), fragments);
        }
        let (system, planet) = one_planet();
        state.system_mut(&system).set_control(planet.clone(), b());
        put_on_planet(&mut state, &system, &planet, "infantry", &b(), 1);
        put(&mut state, &system, "naalu_mech", &a(), 1);
        let (_, dice) = invade_in(pok_only(), &mut state, &system, &a(), &[10, 10], &mut Table::new());
        first_roll(&dice, "a")
    };
    assert_eq!(mech_roll(0), Some((Some(6), 1)));
    assert_eq!(mech_roll(1), Some((Some(4), 1)), "+2 against an opponent with a relic fragment");
}

#[test]
#[ignore = "MISSING/BUG: naalu_mech_omega (Iconoclast Omega) 'other players cannot use ANTI-FIGHTER BARRAGE against your units in this system' is not implemented"]
fn unit_naalu_mech_omega_1_other_players_cannot_barrage_the_naalus_units_in_its_system() {
    let barrage_rolls = |mech: bool| {
        let mut state = crate::fixtures::seated_game(&[("a", "naalu"), ("b", "sol")], ti4_model::content_types::POK);
        let (system, planet) = one_planet();
        if mech {
            put_on_planet(&mut state, &system, &planet, "naalu_mech_omega", &a(), 1);
        }
        put(&mut state, &system, "naalu_fighter", &a(), 2);
        put(&mut state, &system, "destroyer", &b(), 1);
        let (_, dice) = space_fight_in(ti4_model::content_types::POK, &mut state, &system, &[10; 6], &mut Table::new());
        dice.history().iter().filter(|roll| roll.reason == "anti_fighter_barrage" && roll.by.as_deref() == Some("b")).count()
    };
    assert_eq!(barrage_rolls(false), 1, "without the mech Sol's destroyer barrages");
    assert_eq!(barrage_rolls(true), 0, "with the Omega mech in the system it may not");
}
