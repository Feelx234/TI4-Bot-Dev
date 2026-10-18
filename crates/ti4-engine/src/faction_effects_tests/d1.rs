//! Depth tests, group d1: Arborec, Letnev, Saar, Muaat, Hacan and Sol (see mod.rs and
//! docs/FACTION_EFFECT_COVERAGE.md, "Depth" column and "Unit abilities D1").
//!
//! These drive the real engine flow: a `Game` stepped through a scripted `choice::Table` (turn
//! start, tactical action, movement, production, status phase), the real combat and invasion
//! resolvers with loaded dice, the real scoring path (`objectives::award`) and the real
//! component-action entry points. A test that cannot reach an effect that way says so.
//!
//! Test names: `<card id>_<effect no.>_<what>`; unit tests `<unit id>_<n>_<what>`. An effect the
//! engine does not implement keeps its test, marked `#[ignore = "MISSING/BUG: ..."]`.

#![allow(dead_code)]

use ti4_content::galaxy::Galaxy;
use ti4_model::content_types::{DEFAULT, SourceSet};
use ti4_model::id::{LeaderId, ObjectiveId, PlanetId, PlayerId, SystemId};
use ti4_model::state::{GameState, LeaderStatus, Phase};

use super::{content, pid, scripted, seated_all};
use crate::choice::{Scripted, Table, Window as _};
use crate::fixtures::{put, put_on_planet};
use crate::game::{Game, TACTICAL_ACTION_ID};

const SOURCES: SourceSet = DEFAULT;

fn a() -> PlayerId {
    pid("a")
}
fn b() -> PlayerId {
    pid("b")
}

fn set_leader(state: &mut GameState, who: &PlayerId, leader: &str, status: LeaderStatus) {
    state
        .player_mut(who)
        .expect("seat")
        .leaders
        .insert(LeaderId::new(leader), status);
}

fn leader_status(state: &GameState, who: &PlayerId, leader: &str) -> Option<LeaderStatus> {
    crate::leaders::status(state, who, &LeaderId::new(leader))
}

fn count(state: &GameState, system: &SystemId, kind: &str, owner: &PlayerId) -> usize {
    state
        .system_state(system)
        .units
        .iter()
        .filter(|unit| &unit.owner == owner && unit.type_id.as_str() == kind)
        .count()
}

fn count_on(
    state: &GameState,
    system: &SystemId,
    planet: &PlanetId,
    kind: &str,
    owner: &PlayerId,
) -> usize {
    state
        .system_state(system)
        .on_planet_of(planet, owner)
        .into_iter()
        .filter(|unit| unit.type_id.as_str() == kind)
        .count()
}

fn home_of(state: &GameState, who: &PlayerId) -> SystemId {
    state.player(who).unwrap().home_system.clone().unwrap()
}

fn home_planet(state: &GameState, who: &PlayerId) -> (SystemId, PlanetId) {
    let home = home_of(state, who);
    let planet = state
        .controlled_planets(who)
        .into_iter()
        .find(|(system, _)| **system == home)
        .map(|(_, planet)| planet.clone())
        .expect("a home planet");
    (home, planet)
}

/// A table that answers with `answers` in order, then takes the first option offered.
fn table_of(answers: &[&str]) -> Table {
    Table::with_default(Box::new(Scripted::new(answers.iter().map(|a| (*a).to_owned()))))
}

/// Step `game` until `done` holds (at most `limit` steps); every step must succeed.
fn drive(game: &mut Game<'_>, limit: usize, done: impl Fn(&Game<'_>) -> bool) {
    for _ in 0..limit {
        let result = game.step();
        assert_eq!(result.error, None, "events: {:?}", game.events);
        if done(game) {
            return;
        }
    }
    panic!("not finished within {limit} steps; events: {:?}", game.events);
}

/// Step `game` until it is about to ask a question whose prompt starts with `prefix`; return it.
fn until_prompt(game: &mut Game<'_>, prefix: &str) -> crate::choice::Choice {
    for _ in 0..60 {
        if let Some(choice) = game.legal_options()
            && choice.prompt.starts_with(prefix)
        {
            return choice;
        }
        assert_eq!(game.step().error, None, "events: {:?}", game.events);
    }
    panic!("no question starting {prefix:?}; events: {:?}", game.events);
}

fn tactical_done(game: &Game<'_>) -> bool {
    game.events.iter().any(|e| e == "TACTICAL_ACTION_COMPLETE")
}

/// A seven-system map of ordinary systems and a game of `seats` (seat `a` active, action phase).
/// `hub.centre` and `hub.outer[..]` are the systems to use.
fn action_board(seats: &[(&str, &str)]) -> (GameState, crate::fixtures::Hub) {
    let mut state = seated_all(seats);
    let hub = crate::fixtures::plain_hub();
    state.phase = Phase::Action;
    state.active = Some(a());
    (state, hub)
}

fn game_of(state: GameState, galaxy: Galaxy, table: Table) -> Game<'static> {
    Game::with_table(state, content(), table)
        .with_sources(SOURCES)
        .with_galaxy(galaxy)
}

// -- Unlocks through the real flow ---------------------------------------------------------------

/// Three real objectives scored through `objectives::award` (the scoring path every phase uses)
/// unlock a hero; two do not.
fn hero_unlocks_by_scoring(faction: &str, hero: &str) {
    let mut state = seated_all(&[("a", faction), ("b", "hacan")]);
    assert_eq!(leader_status(&state, &a(), hero), Some(LeaderStatus::Locked));
    let score = |state: &mut GameState, alias: &str| {
        crate::objectives::award(state, content(), SOURCES, &a(), &ObjectiveId::new(alias))
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

#[test]
fn arborechero_1_scoring_a_third_objective_unlocks_the_hero() {
    hero_unlocks_by_scoring("arborec", "arborechero");
}
#[test]
fn letnevhero_1_scoring_a_third_objective_unlocks_the_hero() {
    hero_unlocks_by_scoring("letnev", "letnevhero");
}
#[test]
fn muaathero_1_scoring_a_third_objective_unlocks_the_hero() {
    hero_unlocks_by_scoring("muaat", "muaathero");
}
#[test]
fn hacanhero_1_scoring_a_third_objective_unlocks_the_hero() {
    hero_unlocks_by_scoring("hacan", "hacanhero");
}
#[test]
fn saarhero_1_scoring_a_third_objective_unlocks_the_hero() {
    hero_unlocks_by_scoring("saar", "saarhero");
}
#[test]
fn solhero_1_scoring_a_third_objective_unlocks_the_hero() {
    hero_unlocks_by_scoring("sol", "solhero");
}

/// A commander unlocks at the start of its owner's turn (the game re-checks before the seat
/// decides) once `arrange` has made its condition true, and not while it is false.
fn commander_unlocks_at_turn_start(
    faction: &str,
    commander: &str,
    arrange: impl Fn(&mut GameState),
) {
    let run = |met: bool| {
        let mut state = seated_all(&[("a", faction), ("b", "hacan")]);
        state.phase = Phase::Action;
        state.active = Some(a());
        if met {
            arrange(&mut state);
        }
        let mut game =
            Game::with_table(state, content(), table_of(&["pass"])).with_sources(SOURCES);
        assert_eq!(game.step().error, None, "{:?}", game.events);
        (
            leader_status(&game.state, &a(), commander),
            game.events
                .iter()
                .any(|e| e == &format!("LEADER_UNLOCKED:{commander}")),
        )
    };
    assert_eq!(run(false), (Some(LeaderStatus::Locked), false), "condition not met");
    assert_eq!(
        run(true),
        (Some(LeaderStatus::Unlocked), true),
        "condition met: unlocked at the start of the turn"
    );
}

#[test]
fn arboreccommander_1_twelve_ground_forces_unlock_it_at_the_start_of_the_turn() {
    commander_unlocks_at_turn_start("arborec", "arboreccommander", |state| {
        let (home, planet) = home_planet(state, &a());
        put_on_planet(state, &home, &planet, "arborec_infantry", &a(), 12);
    });
}

#[test]
fn hacancommander_1_ten_trade_goods_unlock_it_at_the_start_of_the_turn() {
    commander_unlocks_at_turn_start("hacan", "hacancommander", |state| {
        state.player_mut(&a()).unwrap().trade_goods = 10;
    });
}

#[test]
fn letnevcommander_1_five_ships_in_one_system_unlock_it_at_the_start_of_the_turn() {
    commander_unlocks_at_turn_start("letnev", "letnevcommander", |state| {
        let system = SystemId::new(crate::fixtures::plain_systems(1)[0].as_str());
        put(state, &system, "cruiser", &a(), 5);
    });
}

#[test]
fn saarcommander_1_three_space_docks_unlock_it_at_the_start_of_the_turn() {
    commander_unlocks_at_turn_start("saar", "saarcommander", |state| {
        let system = SystemId::new(crate::fixtures::plain_systems(1)[0].as_str());
        put(state, &system, "saar_spacedock", &a(), 3);
    });
}

#[test]
fn solcommander_1_twelve_resources_of_planets_unlock_it_at_the_start_of_the_turn() {
    commander_unlocks_at_turn_start("sol", "solcommander", |state| {
        let mut total: i64 = state
            .controlled_planets(&a())
            .iter()
            .map(|(_, planet)| planet_resources(planet))
            .sum();
        let owned: Vec<PlanetId> = state
            .controlled_planets(&a())
            .iter()
            .map(|(_, planet)| (*planet).clone())
            .collect();
        for (id, planet) in ti4_content::galaxy::all_planets(content(), SOURCES) {
            if total >= 12 {
                break;
            }
            let Some(system) = planet.system_id() else { continue };
            let planet_id = PlanetId::new(id);
            if planet.homeworld_of().is_some()
                || planet.is_placed_during_play()
                || owned.contains(&planet_id)
            {
                continue;
            }
            state.system_mut(&SystemId::new(system)).set_control(planet_id, a());
            total += i64::from(planet.resources());
        }
        assert!(total >= 12);
    });
}

fn planet_resources(planet: &PlanetId) -> i64 {
    ti4_content::galaxy::all_planets(content(), SOURCES)
        .get(planet.as_str())
        .map_or(0, |p| i64::from(p.resources()))
}

// -- Shared drivers: production, preferred answers ----------------------------------------------

/// Answers the first option whose id starts with one of `prefixes`, else the first option.
struct Prefer(Vec<String>);

impl crate::choice::Decider for Prefer {
    fn choose(
        &mut self,
        choice: &crate::choice::Choice,
    ) -> Result<crate::choice::ChoiceOption, crate::choice::IllegalChoice> {
        let wanted = choice
            .options
            .iter()
            .find(|option| self.0.iter().any(|prefix| option.id.starts_with(prefix.as_str())));
        Ok(wanted.unwrap_or(&choice.options[0]).clone())
    }
}

fn preferring(prefixes: &[&str]) -> Table {
    Table::with_default(Box::new(Prefer(prefixes.iter().map(|p| (*p).to_owned()).collect())))
}

/// Builds `count` of `kind` (an id such as `build|destroyer|1`) when offered, pays with trade goods,
/// and stops producing when nothing is left to build.
struct Builds {
    option: String,
    left: usize,
}

impl crate::choice::Decider for Builds {
    fn choose(
        &mut self,
        choice: &crate::choice::Choice,
    ) -> Result<crate::choice::ChoiceOption, crate::choice::IllegalChoice> {
        if self.left > 0
            && let Some(build) = choice.option(&self.option)
        {
            self.left -= 1;
            return Ok(build.clone());
        }
        Ok(choice
            .option("trade_good")
            .or_else(|| choice.option("done_producing"))
            .unwrap_or(&choice.options[0])
            .clone())
    }
}

/// Builds the `wanted` options in order (each when it is offered), pays with trade goods, and
/// stops producing when the list is used up.
struct BuildOrder(std::collections::VecDeque<String>);

impl crate::choice::Decider for BuildOrder {
    fn choose(
        &mut self,
        choice: &crate::choice::Choice,
    ) -> Result<crate::choice::ChoiceOption, crate::choice::IllegalChoice> {
        if let Some(next) = self.0.front()
            && let Some(build) = choice.option(next)
        {
            self.0.pop_front();
            return Ok(build.clone());
        }
        Ok(choice
            .option("trade_good")
            .or_else(|| choice.option("done_producing"))
            .unwrap_or(&choice.options[0])
            .clone())
    }
}

/// The scripted `opening` answers first (they must be offered), then the `prefer`red ones.
struct Opening {
    queue: std::collections::VecDeque<String>,
    then: Prefer,
    builds: Option<Builds>,
    order: Option<BuildOrder>,
}

impl crate::choice::Decider for Opening {
    fn choose(
        &mut self,
        choice: &crate::choice::Choice,
    ) -> Result<crate::choice::ChoiceOption, crate::choice::IllegalChoice> {
        if let Some(wanted) = self.queue.pop_front() {
            return choice.option(&wanted).cloned().ok_or_else(|| {
                crate::choice::IllegalChoice::ScriptDiverged {
                    player: choice.player.clone(),
                    wanted,
                    offered: choice.ids().into_iter().map(str::to_owned).collect(),
                }
            });
        }
        if let Some(order) = &mut self.order {
            return order.choose(choice);
        }
        match &mut self.builds {
            Some(builds) => builds.choose(choice),
            None => self.then.choose(choice),
        }
    }
}

fn opening_then(opening: &[&str], prefixes: &[&str]) -> Table {
    Table::with_default(Box::new(Opening {
        queue: opening.iter().map(|a| (*a).to_owned()).collect(),
        then: Prefer(prefixes.iter().map(|p| (*p).to_owned()).collect()),
        builds: None,
        order: None,
    }))
}

/// Seven ordinary systems that all hold a planet and are nobody's home. Seat `a` controls the
/// first planet of the centre system and has a `dock` there, ten trade goods, and it is a's turn.
fn production_board(
    seats: &[(&str, &str)],
    dock: &str,
) -> (GameState, crate::fixtures::Hub, SystemId, PlanetId) {
    let mut state = seated_all(seats);
    let ids: Vec<String> = crate::fixtures::plain_systems(120)
        .into_iter()
        .filter(|id| {
            !crate::seating::is_mecatol(id)
                && !id.starts_with("fracture")
                && !ti4_content::galaxy::is_home_system(content(), id, SOURCES)
                && !ti4_content::galaxy::planets_in(content(), id, SOURCES).is_empty()
        })
        .take(7)
        .collect();
    let hub = crate::fixtures::hub_from(&ids);
    let centre = SystemId::new(hub.centre.clone());
    let planet = PlanetId::new(
        ti4_content::galaxy::planets_in(content(), centre.as_str(), SOURCES)[0].id(),
    );
    state.phase = Phase::Action;
    state.active = Some(a());
    state.system_mut(&centre).set_control(planet.clone(), a());
    put_on_planet(&mut state, &centre, &planet, dock, &a(), 1);
    state.player_mut(&a()).unwrap().trade_goods = 10;
    (state, hub, centre, planet)
}

/// Activate `centre` from seat `a` (no movement), then answer with `then` until the action ends.
fn activate_and_produce(
    state: GameState,
    hub: crate::fixtures::Hub,
    centre: &SystemId,
    then: &[&str],
) -> Game<'static> {
    let mut answers = vec![TACTICAL_ACTION_ID.to_owned(), centre.to_string(), "done_moving".to_owned()];
    answers.extend(then.iter().map(|a| (*a).to_owned()));
    let table = Table::with_default(Box::new(Scripted::new(answers)));
    let mut game = game_of(state, hub.galaxy, table);
    drive(&mut game, 80, tactical_done);
    game
}

// -- Hacan ---------------------------------------------------------------------------------------

/// Seat `a` (Sol) takes Trade and declines to replenish anyone; every follower says yes. Returns
/// the follower's `(commodities, strategy tokens)` before and after.
fn trade_followed_by(follower_faction: &str) -> ((i32, i32), (i32, i32)) {
    let mut state = seated_all(&[("a", "sol"), ("b", follower_faction)]);
    state.phase = Phase::Action;
    state.active = Some(a());
    let trade = state
        .unclaimed_strategy_cards
        .iter()
        .find(|card| card.as_str().contains("trade"))
        .cloned()
        .expect("the Trade card");
    state.player_mut(&a()).unwrap().strategy_cards = vec![trade];
    let seat = state.player_mut(&b()).unwrap();
    seat.commodities = 0;
    seat.strategic_tokens = 2;
    let before = (seat.commodities, seat.strategic_tokens);
    let mut game = Game::with_table(state, content(), table_of(&["strategic", "done", "yes"]))
        .with_sources(SOURCES);
    drive(&mut game, 12, |game| {
        game.events.iter().any(|e| e == "STRATEGIC_ACTION_COMPLETE")
    });
    let seat = game.state.player(&b()).unwrap();
    (before, (seat.commodities, seat.strategic_tokens))
}

#[test]
fn master_of_trade_1_the_trade_secondary_costs_hacan_no_strategy_token() {
    let (before, after) = trade_followed_by("hacan");
    assert_eq!(after.1, before.1, "Hacan follows Trade without paying a token");
    assert!(after.0 > before.0, "and still replenishes its commodities");
    let (before, after) = trade_followed_by("jolnar");
    assert_eq!(after.1, before.1 - 1, "another faction pays the token");
    assert!(after.0 > before.0);
}

/// Seats `a`/`b` of the given factions, `a` to act, on an ordinary map. With `together`, both have
/// a ship in the centre system (neighbours); otherwise their only units are at home, which the
/// map does not link (not neighbours).
fn trading_game(a_faction: &str, b_faction: &str, together: bool, answers: &[&str]) -> Game<'static> {
    let mut state = seated_all(&[("a", a_faction), ("b", b_faction)]);
    state.phase = Phase::Action;
    state.active = Some(a());
    let hub = crate::fixtures::plain_hub();
    if together {
        let centre = SystemId::new(hub.centre.clone());
        put(&mut state, &centre, "cruiser", &a(), 1);
        put(&mut state, &centre, "cruiser", &b(), 1);
    }
    for who in [a(), b()] {
        let seat = state.player_mut(&who).unwrap();
        seat.trade_goods = 3;
        seat.action_cards.push(ti4_model::id::ActionCardId::new("emergency"));
    }
    game_of(state, hub.galaxy, table_of(answers))
}

fn action_menu(game: &Game<'_>) -> Vec<String> {
    game.legal_options()
        .expect("a decision")
        .ids()
        .into_iter()
        .map(str::to_owned)
        .collect()
}

#[test]
fn guild_ships_1_hacan_opens_a_transaction_with_a_player_who_is_not_a_neighbour() {
    // Hacan and Sol are nowhere near each other: the action menu still offers the transaction,
    // the deal is proposed, accepted and settled.
    let mut game = trading_game("hacan", "sol", false, &["component|trade|sol", "ss", "accept"]);
    assert!(action_menu(&game).contains(&"component|trade|sol".to_owned()));
    drive(&mut game, 6, |g| g.events.iter().any(|e| e == "TRANSACTION"));
    assert!(game.events.iter().any(|e| e == "TRANSACTION_RESOLVED"));
    // Two factions without the ability and without a shared system or a link have no partner.
    let game = trading_game("jolnar", "sol", false, &[]);
    assert!(
        !action_menu(&game).iter().any(|id| id.starts_with("component|trade|")),
        "no neighbours, no transaction"
    );
}

#[test]
fn arbiters_1_hacan_can_sell_an_action_card_in_a_transaction() {
    let mut game = trading_game("hacan", "sol", true, &["component|trade|sol", "acemergency:1", "accept"]);
    let (goods_a, goods_b) = (
        game.state.player(&a()).unwrap().trade_goods,
        game.state.player(&b()).unwrap().trade_goods,
    );
    drive(&mut game, 6, |g| g.events.iter().any(|e| e == "TRANSACTION"));
    let hand = |who: &PlayerId| game.state.player(who).unwrap().action_cards.len();
    assert_eq!(hand(&a()), 0, "the card left Hacan's hand");
    assert_eq!(hand(&b()), 2, "and joined Sol's");
    assert_eq!(game.state.player(&a()).unwrap().trade_goods, goods_a + 1);
    assert_eq!(game.state.player(&b()).unwrap().trade_goods, goods_b - 1);
    // Neighbours without Arbiters are not offered an action card deal at all.
    let (cap, seen) = crate::choice::Capturing::new(Box::new(Scripted::new(["component|trade|letnev"])));
    let mut plain = trading_game("jolnar", "letnev", true, &[]);
    plain.install_table(Table::with_default(Box::new(cap)));
    let _ = plain.step();
    let _ = plain.step();
    let offered: Vec<String> = seen
        .borrow()
        .iter()
        .flat_map(|c| c.ids().into_iter().map(str::to_owned).collect::<Vec<_>>())
        .collect();
    assert!(offered.iter().any(|id| id == "ss") || offered.iter().any(|id| id.contains(':')), "a deal menu was shown: {offered:?}");
    assert!(
        !offered.iter().any(|id| id.starts_with("ac") || id.starts_with("cn")),
        "no action card terms without Arbiters: {offered:?}"
    );
}

/// The units of `kind` owned by `a` in the system `centre`.
fn built(game: &Game<'_>, centre: &SystemId, kind: &str) -> usize {
    count(&game.state, centre, kind, &a())
}

#[test]
fn hacanhero_2_harrugh_makes_a_use_of_production_free_and_is_purged() {
    let run = |hero: bool| {
        let (mut state, hub, centre, _planet) =
            production_board(&[("a", "hacan"), ("b", "sol")], "spacedock");
        if hero {
            set_leader(&mut state, &a(), "hacanhero", LeaderStatus::Unlocked);
        }
        let table = opening_then(
            &[TACTICAL_ACTION_ID, centre.as_str(), "done_moving"],
            &["use", "build|dreadnought|1", "trade_good", "done_producing"],
        );
        let mut game = game_of(state, hub.galaxy, table);
        drive(&mut game, 60, tactical_done);
        (game, centre)
    };
    let (free, centre) = run(true);
    let (paid, _) = run(false);
    assert!(built(&free, &centre, "dreadnought") >= 1, "built under the hero");
    assert!(built(&paid, &centre, "dreadnought") >= 1, "built without it");
    let spent = |game: &Game<'_>| {
        10 - game.state.player(&a()).unwrap().trade_goods
            + i32::try_from(game.state.exhausted_planets.len()).unwrap()
    };
    assert_eq!(spent(&free), 0, "every unit cost 0: nothing was spent");
    assert!(spent(&paid) > 0, "the same build costs something without the hero");
    assert_eq!(leader_status(&free.state, &a(), "hacanhero"), Some(LeaderStatus::Purged));
    assert_eq!(leader_status(&paid.state, &a(), "hacanhero"), Some(LeaderStatus::Locked));
}

#[test]
fn qdn_1_quantum_datahub_node_swaps_strategy_cards_at_the_end_of_the_strategy_phase() {
    let run = |tech: bool| {
        let mut state = seated_all(&[("a", "hacan"), ("b", "sol")]);
        if tech {
            super::give(&mut state, "a", "qdn");
        }
        let seat = state.player_mut(&a()).unwrap();
        seat.trade_goods = 5;
        seat.strategic_tokens = 1;
        state.player_mut(&b()).unwrap().trade_goods = 0;
        let mut game =
            Game::with_table(state, content(), preferring(&["qdn|"])).with_sources(SOURCES);
        drive(&mut game, 40, |g| g.state.phase != Phase::Strategy);
        game
    };
    let swapped = run(true);
    let plain = run(false);
    assert!(
        swapped.events.iter().any(|e| e == "QUANTUM_DATAHUB_SWAP:a:b"),
        "{:?}",
        swapped.events
    );
    let cards = |game: &Game<'_>, who: &PlayerId| game.state.player(who).unwrap().strategy_cards.clone();
    // The same picks, then the exchange: each side now holds what the other picked.
    assert_ne!(cards(&swapped, &a()), cards(&plain, &a()));
    let mut both: Vec<_> = cards(&swapped, &a()).into_iter().chain(cards(&swapped, &b())).collect();
    let mut both_plain: Vec<_> = cards(&plain, &a()).into_iter().chain(cards(&plain, &b())).collect();
    both.sort();
    both_plain.sort();
    assert_eq!(both, both_plain, "the same cards, exchanged");
    let (a_now, b_now) = (
        swapped.state.player(&a()).unwrap(),
        swapped.state.player(&b()).unwrap(),
    );
    assert_eq!((a_now.trade_goods, a_now.strategic_tokens, b_now.trade_goods), (2, 0, 3));
    assert!(!plain.events.iter().any(|e| e.starts_with("QUANTUM_DATAHUB_SWAP")));
}

#[test]
fn pm_1_production_biomes_is_an_action_paying_four_trade_goods_and_two_to_another_player() {
    let mut state = seated_all(&[("a", "hacan"), ("b", "sol")]);
    state.phase = Phase::Action;
    state.active = Some(a());
    super::give(&mut state, "a", "pm");
    state.player_mut(&a()).unwrap().strategic_tokens = 1;
    state.player_mut(&a()).unwrap().trade_goods = 0;
    state.player_mut(&b()).unwrap().trade_goods = 0;
    let mut game = Game::with_table(
        state,
        content(),
        table_of(&["faction|production_biomes", "b", "pass"]),
    )
    .with_sources(SOURCES);
    assert!(action_menu(&game).contains(&"faction|production_biomes".to_owned()));
    drive(&mut game, 8, |g| g.state.player(&a()).unwrap().strategic_tokens == 0);
    let seat = game.state.player(&a()).unwrap();
    assert_eq!((seat.trade_goods, seat.strategic_tokens), (4, 0));
    assert_eq!(game.state.player(&b()).unwrap().trade_goods, 2);
    assert!(
        !action_menu(&game).contains(&"faction|production_biomes".to_owned()),
        "exhausted until the status phase"
    );
}

/// Three seats: Hacan (a), Sol (b) holding Trade Convoys, Jol-Nar (c). Nobody is anyone's
/// neighbour. `b` is to act.
fn convoys_game(answers: &[&str]) -> Game<'static> {
    let mut state = seated_all(&[("a", "hacan"), ("b", "sol"), ("c", "jolnar")]);
    state.phase = Phase::Action;
    state.active = Some(b());
    crate::promissory::take(&mut state, content(), &b(), &crate::promissory::note_id("convoys", "hacan"));
    game_of(state, crate::fixtures::plain_hub().galaxy, table_of(answers))
}

#[test]
fn convoys_1_action_places_trade_convoys_faceup_and_opens_transactions_with_non_neighbours() {
    let mut game = convoys_game(&["faction|trade_convoys", "pass"]);
    let trade_with_jolnar = "component|trade|jolnar".to_owned();
    let menu = action_menu(&game);
    assert!(menu.contains(&"faction|trade_convoys".to_owned()), "{menu:?}");
    assert!(!menu.contains(&trade_with_jolnar), "not a neighbour yet");
    drive(&mut game, 6, |g| g.state.promissory_faceup.contains("convoys:hacan"));
    assert!(
        action_menu(&game).contains(&trade_with_jolnar),
        "faceup in the play area, the whole table can be negotiated with"
    );
    assert!(!action_menu(&game).contains(&"faction|trade_convoys".to_owned()));
}

#[test]
fn convoys_2_activating_a_system_with_hacan_units_returns_the_note() {
    let activate = |hacan_present: bool| {
        let mut game = convoys_game(&[]);
        let hub = crate::fixtures::plain_hub();
        let target = SystemId::new(hub.centre.clone());
        if hacan_present {
            put(&mut game.state, &target, "cruiser", &a(), 1);
        }
        // Played earlier (the ACTION of the first test), now in the holder's play area.
        assert!(crate::promissory::play_convoys(&mut game.state, &b()));
        game.install_table(table_of(&[TACTICAL_ACTION_ID, target.as_str(), "done_moving"]));
        drive(&mut game, 30, tactical_done);
        game
    };
    let returned = activate(true);
    assert_eq!(crate::promissory::holder_of(&returned.state, "convoys", &a()), None, "back with Hacan");
    assert!(!returned.state.promissory_faceup.contains("convoys:hacan"));
    let kept = activate(false);
    assert_eq!(crate::promissory::holder_of(&kept.state, "convoys", &a()), Some(b()));
    assert!(kept.state.promissory_faceup.contains("convoys:hacan"));
}

#[test]
fn hacanbt_1_auto_factories_three_non_fighter_ships_put_a_token_in_the_fleet_pool() {
    let run = |ships: usize| {
        let (mut state, hub, centre, planet) =
            production_board(&[("a", "hacan"), ("b", "sol")], "spacedock");
        put_on_planet(&mut state, &centre, &planet, "spacedock", &a(), 1);
        state.player_mut(&a()).unwrap().breakthrough =
            Some(ti4_model::id::BreakthroughId::new("hacanbt"));
        let tokens = state.player(&a()).unwrap().fleet_tokens;
        let answers = [TACTICAL_ACTION_ID, centre.as_str(), "done_moving"];
        let table = Table::with_default(Box::new(Opening {
            queue: answers.iter().map(|a| (*a).to_owned()).collect(),
            then: Prefer(Vec::new()),
            builds: Some(Builds { option: "build|destroyer|1".to_owned(), left: ships }),
            order: None,
        }));
        let mut game = game_of(state, hub.galaxy, table);
        drive(&mut game, 80, tactical_done);
        assert_eq!(count(&game.state, &centre, "destroyer", &a()), ships);
        game.state.player(&a()).unwrap().fleet_tokens - tokens
    };
    assert_eq!(run(3), 1, "three non-fighter ships: one token");
    assert_eq!(run(2), 0, "two are not enough");
}

/// The options of the first production prompt of a tactical action on `centre` (no movement).
fn production_menu(state: GameState, hub: crate::fixtures::Hub, centre: &SystemId) -> Vec<String> {
    let table = opening_then(&[TACTICAL_ACTION_ID, centre.as_str(), "done_moving"], &[]);
    let mut game = game_of(state, hub.galaxy, table);
    for _ in 0..40 {
        if let Some(choice) = game.legal_options()
            && choice.prompt.starts_with("produce in ")
        {
            return choice.ids().into_iter().map(str::to_owned).collect();
        }
        assert_eq!(game.step().error, None, "{:?}", game.events);
    }
    panic!("no production prompt: {:?}", game.events);
}

#[test]
fn mitosis_1_arborec_space_docks_cannot_produce_infantry() {
    // Only a space dock stands in the activated system: Sol could build infantry there, the
    // Arborec cannot (their Letani Warriors, not the dock, are the infantry source).
    let menu = |faction: &str| {
        let (mut state, hub, centre, planet) =
            production_board(&[("a", faction), ("b", "hacan")], "spacedock");
        // Clear the home-system leftovers: the dock is the only unit of a anywhere.
        for system in state.board.keys().cloned().collect::<Vec<_>>() {
            if system != centre {
                let board = state.system_mut(&system);
                board.units.retain(|u| u.owner != a());
                board.planet_units.values_mut().for_each(|units| units.retain(|u| u.owner != a()));
            }
        }
        let _ = planet;
        // Magen Defense Grid (an Arborec starting technology) would place a free infantry on
        // activation; without it the dock is the only thing on the planet.
        state
            .player_mut(&a())
            .unwrap()
            .technologies
            .remove(&ti4_model::id::TechnologyId::new("md"));
        production_menu(state, hub, &centre)
    };
    let sol = menu("sol");
    assert!(sol.iter().any(|id| id.starts_with("build|") && id.contains("infantry")), "{sol:?}");
    let arborec = menu("arborec");
    assert!(arborec.iter().any(|id| id.starts_with("build|cruiser|")), "ships are fine: {arborec:?}");
    assert!(
        !arborec.iter().any(|id| id.starts_with("build|") && id.contains("infantry")),
        "a space dock cannot produce Arborec infantry: {arborec:?}"
    );
}

// -- Letnev --------------------------------------------------------------------------------------

/// Non-fighter ships of `a` in `system`.
fn non_fighters(state: &GameState, system: &SystemId) -> usize {
    state
        .system_state(system)
        .units
        .iter()
        .filter(|unit| {
            unit.owner == a() && matches!(unit.type_id.as_str(), "cruiser" | "destroyer" | "carrier" | "dreadnought")
        })
        .count()
}

#[test]
fn armada_1_letnev_may_hold_two_more_ships_than_its_fleet_pool_has_tokens() {
    // Three tokens in the fleet pool and three cruisers in the system: production keeps adding
    // destroyers until the system holds the limit. Armada's limit is 3 + 2; anyone else's is 3.
    let run = |faction: &str| {
        let (mut state, hub, centre, _planet) =
            production_board(&[("a", faction), ("b", "hacan")], "spacedock");
        state.player_mut(&a()).unwrap().fleet_tokens = 3;
        put(&mut state, &centre, "cruiser", &a(), 3);
        let table = Table::with_default(Box::new(Opening {
            queue: [TACTICAL_ACTION_ID, centre.as_str(), "done_moving"]
                .iter()
                .map(|a| (*a).to_owned())
                .collect(),
            then: Prefer(Vec::new()),
            builds: Some(Builds { option: "build|destroyer|1".to_owned(), left: 4 }),
            order: None,
        }));
        let mut game = game_of(state, hub.galaxy, table);
        drive(&mut game, 80, tactical_done);
        let produced = non_fighters(&game.state, &centre);
        // The limit is enforced when the turn ends (37.3): whatever is over it is removed.
        drive(&mut game, 20, |g| g.events.iter().any(|e| e == "TURN_PASSED"));
        (produced, non_fighters(&game.state, &centre))
    };
    assert_eq!(run("letnev").1, 5, "three tokens plus Armada's two");
    assert_eq!(run("sol").1, 3, "nobody else may exceed the pool");
    assert!(run("sol").0 > 3, "the excess is only removed at the end of the turn");
}

/// An ordinary system that is nobody's home (and not Mecatol Rex), the `n`th such.
fn open_system(n: usize) -> SystemId {
    let id = crate::fixtures::plain_systems(120)
        .into_iter()
        .filter(|id| {
            !crate::seating::is_mecatol(id)
                && !id.starts_with("fracture")
                && !ti4_content::galaxy::is_home_system(content(), id, SOURCES)
        })
        .nth(n)
        .expect("an open system");
    SystemId::new(id)
}

/// Resolve the space combat in `system` (seat `a` is the active attacker) with these faces loaded;
/// a question is answered with the first option whose id starts with one of `answers`, else the
/// first option. Returns the state and the dice rolled.
fn space_combat(
    mut state: GameState,
    system: &SystemId,
    faces: &[u32],
    answers: &[&str],
) -> (GameState, crate::dice::Dice) {
    state.active = Some(a());
    let mut table = preferring(answers);
    let mut dice = crate::dice::Dice::from_faces(faces.iter().copied());
    let mut rng = crate::rng::GameRng::new(3);
    crate::combat::resolve(&mut state, content(), SOURCES, &mut table, &mut dice, &mut rng, system)
        .expect("the combat resolves");
    (state, dice)
}

fn faces_of(dice: &crate::dice::Dice, reason_prefix: &str) -> Vec<Vec<u32>> {
    dice.history()
        .iter()
        .filter(|roll| roll.reason.starts_with(reason_prefix))
        .map(|roll| roll.faces.clone())
        .collect()
}

#[test]
fn munitions_1_letnev_pays_two_trade_goods_to_reroll_its_dice_in_a_space_combat() {
    let run = |answer: &str| {
        let mut state = seated_all(&[("a", "letnev"), ("b", "sol")]);
        let system = open_system(0);
        put(&mut state, &system, "cruiser", &a(), 3);
        put(&mut state, &system, "destroyer", &b(), 1);
        state.player_mut(&a()).unwrap().trade_goods = 4;
        // Letnev rolls three misses first; the re-roll is the only thing that can kill the ship.
        let faces = [1, 1, 1, 1, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10];
        let (state, dice) = space_combat(state, &system, &faces, &[answer]);
        (
            state.player(&a()).unwrap().trade_goods,
            dice.history().iter().any(|roll| roll.reason == "munitions:a"),
        )
    };
    assert_eq!(run("munitions"), (2, true), "two trade goods paid, the dice rolled again");
    assert_eq!(run("decline"), (4, false), "declined: nothing paid, nothing rolled");
}

/// A tactical action in which `a` moves everything from `hub.outer[0]` into the centre system,
/// where `b` has ships. The table answers `window` prefixes when offered. Runs to the end of the
/// action.
fn attack_game(
    mut state: GameState,
    hub: crate::fixtures::Hub,
    window_answers: &[&str],
) -> (Game<'static>, SystemId, SystemId) {
    let centre = SystemId::new(hub.centre.clone());
    let origin = SystemId::new(hub.outer[0].clone());
    state.phase = Phase::Action;
    state.active = Some(a());
    let mut prefixes = vec!["move|", "done_loading", "done_moving"];
    prefixes.extend_from_slice(window_answers);
    let table = opening_then(&[TACTICAL_ACTION_ID, centre.as_str()], &prefixes);
    let mut game = game_of(state, hub.galaxy, table);
    drive(&mut game, 120, tactical_done);
    (game, centre, origin)
}

/// A seven-system map of open systems (nobody's home, not Mecatol Rex).
fn open_hub() -> crate::fixtures::Hub {
    let ids: Vec<String> = (0..7).map(|n| open_system(n).to_string()).collect();
    crate::fixtures::hub_from(&ids)
}

/// The dice (count of faces) of the first `space combat` roll of `by`.
fn first_roll_len(game: &Game<'_>, by: &str, reason: &str) -> usize {
    game.rolls()
        .iter()
        .find(|roll| roll.reason == reason && roll.by.as_deref() == Some(by))
        .map_or(0, |roll| roll.faces.len())
}

#[test]
#[ignore = "MISSING/BUG: letnevagent the owner is never offered the agent in a space combat round (only Ssruu's copy and, from version 16, Sol's own agent have a window)"]
fn letnevagent_1_exhausts_to_give_one_ship_an_additional_die_in_a_space_combat_round() {
    let run = |use_agent: bool| {
        let mut state = seated_all(&[("a", "letnev"), ("b", "sol")]);
        let hub = open_hub();
        put(&mut state, &SystemId::new(hub.outer[0].clone()), "cruiser", &a(), 2);
        put(&mut state, &SystemId::new(hub.centre.clone()), "cruiser", &b(), 1);
        set_leader(&mut state, &a(), "letnevagent", LeaderStatus::Readied);
        let answers: &[&str] = if use_agent { &["leader:letnev:letnevagent"] } else { &[] };
        let (game, _, _) = attack_game(state, hub, answers);
        game
    };
    let with_agent = run(true);
    let without = run(false);
    assert_eq!(first_roll_len(&without, "a", "space combat"), 2, "two cruisers, two dice");
    assert_eq!(leader_status(&with_agent.state, &a(), "letnevagent"), Some(LeaderStatus::Exhausted));
    assert_eq!(first_roll_len(&with_agent, "a", "space combat"), 3, "the chosen ship rolls one more die");
    // Only that round: the extra die is not a permanent upgrade.
    assert_eq!(leader_status(&without.state, &a(), "letnevagent"), Some(LeaderStatus::Readied));
}

#[test]
#[ignore = "MISSING/BUG: letnevcommander (Rear Admiral Farran) no trade good is offered when a ship sustains damage in a real space combat; only combat::absorb_hits asks, the combat window's sustain step does not"]
fn letnevcommander_2_a_sustain_in_a_space_combat_offers_a_trade_good_only_once_unlocked() {
    // One round: Letnev's dreadnought and Sol's cruiser both hit. The dreadnought sustains.
    for (status, gain) in [(LeaderStatus::Locked, 0), (LeaderStatus::Unlocked, 1)] {
        let mut state = seated_all(&[("a", "letnev"), ("b", "sol")]);
        let system = open_system(0);
        put(&mut state, &system, "dreadnought", &a(), 1);
        put(&mut state, &system, "cruiser", &b(), 1);
        set_leader(&mut state, &a(), "letnevcommander", status);
        let before = state.player(&a()).unwrap().trade_goods;
        let (state, _) = space_combat(state, &system, &[10, 10, 10, 10, 10, 10], &["sustain", "gain"]);
        assert!(state.system_state(&system).units.iter().any(|u| u.owner == a() && u.sustained_damage), "the dreadnought sustained");
        assert_eq!(state.player(&a()).unwrap().trade_goods, before + gain, "{status:?}");
    }
}

#[test]
#[ignore = "MISSING/BUG: nes (Non-Euclidean Shielding) a sustain in a real space combat cancels one hit, not two; only combat::absorb_hits applies the card, the combat window's sustain step does not"]
fn nes_1_non_euclidean_shielding_one_sustain_cancels_two_hits_in_a_space_combat() {
    let ships_left = |nes: bool| {
        let mut state = seated_all(&[("a", "letnev"), ("b", "sol")]);
        let system = open_system(0);
        put(&mut state, &system, "dreadnought", &a(), 1);
        put(&mut state, &system, "cruiser", &a(), 1);
        put(&mut state, &system, "cruiser", &b(), 2);
        if nes {
            super::give(&mut state, "a", "nes");
        }
        // One round: both of Sol's cruisers hit, and both of Letnev's ships hit back.
        let (state, _) = space_combat(state, &system, &[10, 10, 10, 10, 10, 10, 10, 10], &["sustain"]);
        state.system_state(&system).units.iter().filter(|u| u.owner == a()).count()
    };
    assert_eq!(ships_left(true), 2, "the sustain cancels both hits");
    assert_eq!(ships_left(false), 1, "without the card the second hit costs a ship");
}

#[test]
#[ignore = "MISSING/BUG: letnevhero (Darktalon Treilla) fleet::over_supply (the enforcement after production and at turn end) reads limit() and ignores fleet::is_unlimited, so ships over the pool are still removed during her round"]
fn letnevhero_2_darktalon_lifts_the_fleet_limit_for_the_rest_of_the_round() {
    let ships_after = |use_hero: bool| {
        let (mut state, hub, centre, _planet) =
            production_board(&[("a", "letnev"), ("b", "sol")], "spacedock");
        state.player_mut(&a()).unwrap().fleet_tokens = 3;
        put(&mut state, &centre, "cruiser", &a(), 3);
        set_leader(&mut state, &a(), "letnevhero", LeaderStatus::Unlocked);
        let mut queue: Vec<&str> = Vec::new();
        if use_hero {
            // The hero is an ACTION: it takes the turn. Sol passes, and Letnev acts again.
            queue.extend(["component|leader|letnevhero", "pass"]);
        }
        queue.extend([TACTICAL_ACTION_ID, centre.as_str(), "done_moving"]);
        let table = Table::with_default(Box::new(Opening {
            queue: queue.iter().map(|a| (*a).to_owned()).collect(),
            then: Prefer(Vec::new()),
            builds: Some(Builds { option: "build|destroyer|1".to_owned(), left: 4 }),
            order: None,
        }));
        let mut game = game_of(state, hub.galaxy, table);
        drive(&mut game, 120, tactical_done);
        let produced = non_fighters(&game.state, &centre);
        drive(&mut game, 30, |g| g.events.iter().filter(|e| *e == "TURN_PASSED").count() >= 3);
        let _ = produced;
        (non_fighters(&game.state, &centre), leader_status(&game.state, &a(), "letnevhero"))
    };
    assert_eq!(ships_after(false).0, 5, "without her the limit is the three tokens plus Armada's two");
    let (with_hero, status) = ships_after(true);
    assert_eq!(with_hero, 6, "three cruisers and the three destroyers produced: the pool no longer limits the fleet");
    assert_eq!(status, Some(LeaderStatus::Unlocked), "she stays until the end of the round");
}

#[test]
fn letnevbt_1_gravleash_maneuvers_slower_ships_move_at_the_fastest_moving_ship_value() {
    // Origin and target are two apart. The cruiser (move 2) can go; the carrier (move 1) can only
    // follow when the breakthrough lends it the cruiser's value.
    let run = |breakthrough: bool| {
        let mut state = seated_all(&[("a", "letnev"), ("b", "sol")]);
        let hub = open_hub();
        let origin = SystemId::new(hub.outer[0].clone());
        let target = SystemId::new(hub.across(&hub.outer[0]));
        put(&mut state, &origin, "cruiser", &a(), 1);
        put(&mut state, &origin, "carrier", &a(), 1);
        if breakthrough {
            state.player_mut(&a()).unwrap().breakthrough =
                Some(ti4_model::id::BreakthroughId::new("letnevbt"));
        }
        state.phase = Phase::Action;
        state.active = Some(a());
        let table = opening_then(
            &[TACTICAL_ACTION_ID, target.as_str()],
            &["move|", "done_loading", "done_moving"],
        );
        let mut game = game_of(state, hub.galaxy, table);
        drive(&mut game, 60, tactical_done);
        (
            count(&game.state, &target, "cruiser", &a()),
            count(&game.state, &target, "carrier", &a()),
        )
    };
    assert_eq!(run(true), (1, 1), "the carrier moves two systems");
    assert_eq!(run(false), (1, 0), "without it the carrier stays");
}

#[test]
fn war_funding_1_rerolls_all_of_the_opponents_dice_and_the_holders_misses_then_returns_home() {
    let run = |answer: &str| {
        let mut state = seated_all(&[("a", "hacan"), ("b", "letnev")]);
        let system = open_system(0);
        put(&mut state, &system, "cruiser", &a(), 2);
        put(&mut state, &system, "cruiser", &b(), 2);
        let note = crate::promissory::note_id("war_funding", "letnev");
        state.promissory_notes.insert(note.clone(), a());
        // The holder rolls a hit and a miss; the Letnev player rolls two hits.
        let mut faces = vec![10, 1, 10, 10];
        faces.extend([10; 40]);
        let (state, dice) = space_combat(state, &system, &faces, &[answer]);
        let rerolled = dice.history().iter().filter(|roll| roll.reason == "war_funding").count();
        (rerolled, state.promissory_notes.get(&note).cloned())
    };
    assert_eq!(run("use"), (3, Some(b())), "both opposing dice and the one miss; the note goes home");
    assert_eq!(run("decline"), (0, Some(a())), "declined: nothing is rolled again and the holder keeps it");
}

/// A seven-system map of systems that hold a planet and are nobody's home; `b` controls the first
/// planet of the centre system with a PDS and `n` infantry on it.
fn defended_planet_board(
    seats: &[(&str, &str)],
    infantry: usize,
) -> (GameState, crate::fixtures::Hub, SystemId, PlanetId) {
    let (mut state, hub, centre, planet) = production_board(seats, "spacedock");
    // The dock fixture belongs to a: take it off, the planet is b's.
    state.system_mut(&centre).planet_units.clear();
    state.system_mut(&centre).planet_control.clear();
    state.system_mut(&centre).set_control(planet.clone(), b());
    put_on_planet(&mut state, &centre, &planet, "pds", &b(), 1);
    put_on_planet(&mut state, &centre, &planet, "infantry", &b(), infantry);
    (state, hub, centre, planet)
}

/// `a` (of `faction`) moves one `unit` from the ring into the centre, where Sol holds a planet with
/// a PDS and three infantry. Returns Sol's infantry left and the bombardment roll `a` made, if any.
fn bombard_with(faction: &str, unit: &str, upgrade: Option<&str>) -> (usize, Option<(usize, Option<u32>)>) {
    let (mut state, hub, centre, planet) = defended_planet_board(&[("a", faction), ("b", "sol")], 3);
    let origin = SystemId::new(hub.outer[0].clone());
    // Only the moved ship matters: take the home fleet out of the way.
    for system in state.board.keys().cloned().collect::<Vec<_>>() {
        if system != centre {
            let board = state.system_mut(&system);
            board.units.retain(|u| u.owner != a());
        }
    }
    put(&mut state, &origin, unit, &a(), 1);
    if let Some(tech) = upgrade {
        super::give(&mut state, "a", tech);
    }
    // Plasma Scoring (Letnev's starting technology) adds a bombardment die; keep the printed count.
    state
        .player_mut(&a())
        .unwrap()
        .technologies
        .remove(&ti4_model::id::TechnologyId::new("ps"));
    let table = opening_then(
        &[TACTICAL_ACTION_ID, centre.as_str()],
        &["move|", "done_loading", "done_moving"],
    );
    let mut game = game_of(state, hub.galaxy, table);
    drive(&mut game, 80, tactical_done);
    let roll = game
        .rolls()
        .iter()
        .find(|roll| roll.reason == "bombardment" && roll.by.as_deref() == Some("a"))
        .map(|roll| (roll.faces.len(), roll.hits_on));
    (count_on(&game.state, &centre, &planet, "infantry", &b()), roll)
}

fn bombard_a_shielded_planet(unit: &str, upgrade: Option<&str>) -> (usize, bool) {
    let (left, roll) = bombard_with("muaat", unit, upgrade);
    (left, roll.is_some())
}

#[test]
fn muaat_warsun_1_prototype_war_sun_i_bombards_a_planet_under_a_planetary_shield() {
    let (left, rolled) = bombard_a_shielded_planet("muaat_warsun", None);
    assert!(rolled, "the war sun bombards although a PDS shields the planet");
    assert!(left < 3, "and kills infantry: {left} left");
    let (left, rolled) = bombard_a_shielded_planet("dreadnought", None);
    assert!(!rolled, "a dreadnought is stopped by PLANETARY SHIELD");
    assert_eq!(left, 3);
}

#[test]
fn pws2_3_prototype_war_sun_ii_bombards_a_planet_under_a_planetary_shield() {
    let (left, rolled) = bombard_a_shielded_planet("muaat_warsun2", Some("pws2"));
    assert!(rolled, "the upgraded war sun still removes the shield");
    assert!(left < 3, "{left} infantry left");
}

/// A supernova in the middle of a ring of open systems; `a` (the given faction) has a cruiser in
/// the first ring system. A tactical action activates `target` and moves everything it can there.
/// Returns the game and the supernova.
fn supernova_game(
    faction: &str,
    target: impl Fn(&crate::fixtures::Hub) -> String,
    tech: Option<&str>,
    builds: usize,
) -> (Game<'static>, SystemId, SystemId) {
    let supernova = crate::fixtures::a_system_where("supernova");
    let hub = crate::fixtures::hub_with_centre(&supernova);
    let mut state = seated_all(&[("a", faction), ("b", "hacan")]);
    let origin = SystemId::new(hub.outer[0].clone());
    let target = SystemId::new(target(&hub));
    put(&mut state, &origin, "cruiser", &a(), 1);
    if let Some(tech) = tech {
        super::give(&mut state, "a", tech);
    }
    state.player_mut(&a()).unwrap().trade_goods = 10;
    state.phase = Phase::Action;
    state.active = Some(a());
    let table = Table::with_default(Box::new(Opening {
        queue: [TACTICAL_ACTION_ID, target.as_str()].iter().map(|a| (*a).to_owned()).collect(),
        then: Prefer(vec!["move|".to_owned(), "done_loading".to_owned(), "done_moving".to_owned()]),
        builds: (builds > 0).then(|| Builds { option: "build|destroyer|1".to_owned(), left: builds }),
        order: None,
    }));
    let mut game = game_of(state, hub.galaxy, table);
    drive(&mut game, 80, tactical_done);
    (game, target, SystemId::new(supernova))
}

#[test]
fn gashlai_physiology_1_muaat_ships_move_through_a_supernova_but_other_factions_do_not() {
    let arrives = |faction: &str| {
        let (game, target, _) = supernova_game(faction, |hub| hub.across(&hub.outer[0]), None, 0);
        count(&game.state, &target, "cruiser", &a())
    };
    assert_eq!(arrives("muaat"), 1, "through the supernova to the far side");
    assert_eq!(arrives("sol"), 0, "the supernova blocks everyone else");
}

#[test]
fn mr_1_magmus_reactor_lets_ships_move_into_a_supernova() {
    let ends_in_nova = |tech: Option<&str>| {
        let (game, target, _) = supernova_game("muaat", |hub| hub.centre.clone(), tech, 0);
        count(&game.state, &target, "cruiser", &a())
    };
    assert_eq!(ends_in_nova(Some("mr")), 1, "with the reactor the cruiser stops in the supernova");
    assert_eq!(ends_in_nova(None), 0, "without it a ship cannot end its move there");
}

#[test]
fn mr_2_magmus_reactor_gives_a_supernova_holding_your_ships_production_five() {
    // Six destroyers are asked for; PRODUCTION 5 allows five.
    let (game, nova, _) = supernova_game("muaat", |hub| hub.centre.clone(), Some("mr"), 6);
    assert_eq!(count(&game.state, &nova, "destroyer", &a()), 5);
    assert!(game.events.iter().any(|e| e == "PRODUCTION_USED"));
}

// -- Saar ----------------------------------------------------------------------------------------

#[test]
fn nomadic_1_saar_may_score_without_controlling_the_planets_in_their_home_system() {
    // Both seats lose their home planets and hold the five trade goods "Trade Routes" asks for.
    // In the status phase's scoring step only the Saar player is offered the objective.
    let mut state = seated_all(&[("a", "saar"), ("b", "sol")]);
    for who in [a(), b()] {
        let (home, _) = home_planet(&state, &who);
        state.system_mut(&home).planet_control.clear();
        state.player_mut(&who).unwrap().trade_goods = 5;
    }
    state.revealed_objectives.push(ObjectiveId::new("trade_routes"));
    state.phase = Phase::Action;
    state.active = Some(a());
    let table = preferring(&["pass", "trade_routes"]);
    let mut game = Game::with_table(state, content(), table).with_sources(SOURCES);
    drive(&mut game, 200, |g| g.state.round >= 2);
    assert!(game.state.scored_by(&a()).contains(&ObjectiveId::new("trade_routes")));
    assert!(game.state.scored_by(&b()).is_empty(), "Sol still needs its home planets (61.16)");
}

#[test]
fn saaragent_1_raises_one_ship_of_the_activating_player_to_the_fastest_move_on_the_board() {
    // Sol's carrier (move 1) is two systems from the target; a Saar cruiser (move 2) sits elsewhere.
    let carrier_arrives = |agent: bool| {
        let mut state = seated_all(&[("a", "saar"), ("b", "sol")]);
        let hub = open_hub();
        let origin = SystemId::new(hub.outer[0].clone());
        let target = SystemId::new(hub.across(&hub.outer[0]));
        // Take every other ship off the board, so the carrier is the only choice for the agent.
        for system in state.board.keys().cloned().collect::<Vec<_>>() {
            state.system_mut(&system).units.clear();
        }
        put(&mut state, &origin, "carrier", &b(), 1);
        put(&mut state, &SystemId::new(hub.outer[2].clone()), "cruiser", &a(), 1);
        if !agent {
            set_leader(&mut state, &a(), "saaragent", LeaderStatus::Exhausted);
        } else {
            set_leader(&mut state, &a(), "saaragent", LeaderStatus::Readied);
        }
        state.phase = Phase::Action;
        state.active = Some(b());
        let table = opening_then(
            &[TACTICAL_ACTION_ID, target.as_str()],
            &["leader:saar:saaragent", "ship|", "move|", "done_loading", "done_moving"],
        );
        let mut game = game_of(state, hub.galaxy, table);
        drive(&mut game, 80, tactical_done);
        (
            count(&game.state, &target, "carrier", &b()),
            leader_status(&game.state, &a(), "saaragent"),
        )
    };
    assert_eq!(carrier_arrives(true), (1, Some(LeaderStatus::Exhausted)), "the carrier moves 2");
    assert_eq!(carrier_arrives(false).0, 0, "without the agent it is too slow");
}

#[test]
fn saarcommander_2_fighters_may_be_placed_at_any_unblockaded_space_dock() {
    let fighters = |status: LeaderStatus, blockaded: bool| {
        let mut state = seated_all(&[("a", "saar"), ("b", "sol")]);
        let hub = open_hub();
        let centre = SystemId::new(hub.centre.clone());
        let other = SystemId::new(hub.outer[1].clone());
        for system in state.board.keys().cloned().collect::<Vec<_>>() {
            let board = state.system_mut(&system);
            board.units.retain(|u| u.owner != a());
            board.planet_units.values_mut().for_each(|units| units.retain(|u| u.owner != a()));
        }
        put(&mut state, &centre, "saar_spacedock", &a(), 1);
        put(&mut state, &other, "saar_spacedock", &a(), 1);
        if blockaded {
            put(&mut state, &other, "cruiser", &b(), 1);
        }
        set_leader(&mut state, &a(), "saarcommander", status);
        state.player_mut(&a()).unwrap().trade_goods = 10;
        state.phase = Phase::Action;
        state.active = Some(a());
        let place = format!("place|{other}@");
        let table = opening_then(
            &[TACTICAL_ACTION_ID, centre.as_str(), "done_moving"],
            &["build|fighter", "trade_good", place.as_str(), "done_producing"],
        );
        let mut game = game_of(state, hub.galaxy, table);
        drive(&mut game, 80, tactical_done);
        (
            count(&game.state, &centre, "fighter", &a()),
            count(&game.state, &other, "fighter", &a()),
        )
    };
    let (here, there) = fighters(LeaderStatus::Unlocked, false);
    assert!(there > 0, "placed at the other dock: {here} here, {there} there");
    let (here, there) = fighters(LeaderStatus::Locked, false);
    assert!(here > 0 && there == 0, "locked: only where they were produced ({here}, {there})");
    let (here, there) = fighters(LeaderStatus::Unlocked, true);
    assert!(here > 0 && there == 0, "a blockaded dock is not an option ({here}, {there})");
}

#[test]
fn ffac2_1_floating_factory_ii_moves_two_systems_like_a_ship() {
    let arrives = |tech: bool| {
        let mut state = seated_all(&[("a", "saar"), ("b", "sol")]);
        let hub = open_hub();
        let origin = SystemId::new(hub.outer[0].clone());
        let target = SystemId::new(hub.across(&hub.outer[0]));
        put(&mut state, &origin, "saar_spacedock", &a(), 1);
        if tech {
            super::give(&mut state, "a", "ffac2");
            crate::technology::apply_unit_upgrades(&mut state, content(), SOURCES, &a());
        }
        state.phase = Phase::Action;
        state.active = Some(a());
        let table = opening_then(
            &[TACTICAL_ACTION_ID, target.as_str()],
            &["move|", "done_loading", "done_moving"],
        );
        let mut game = game_of(state, hub.galaxy, table);
        // Take the Saar home fleet out of the way: only the floating factory can move.
        drive(&mut game, 60, tactical_done);
        (
            count(&game.state, &target, "saar_spacedock2", &a()),
            count(&game.state, &target, "saar_spacedock", &a()),
        )
    };
    assert_eq!(arrives(true), (1, 0), "Floating Factory II has move 2");
    assert_eq!(arrives(false), (0, 0), "Floating Factory I (move 1) cannot make the two systems");
}

#[test]
fn ffac2_2_floating_factory_ii_has_production_seven_in_the_space_area() {
    let left = |tech: bool| {
        let mut state = seated_all(&[("a", "saar"), ("b", "sol")]);
        let hub = open_hub();
        let centre = SystemId::new(hub.centre.clone());
        for system in state.board.keys().cloned().collect::<Vec<_>>() {
            let board = state.system_mut(&system);
            board.units.retain(|u| u.owner != a());
            board.planet_units.values_mut().for_each(|units| units.retain(|u| u.owner != a()));
        }
        put(&mut state, &centre, "saar_spacedock", &a(), 1);
        if tech {
            super::give(&mut state, "a", "ffac2");
            crate::technology::apply_unit_upgrades(&mut state, content(), SOURCES, &a());
        }
        state.phase = Phase::Action;
        state.active = Some(a());
        let table = opening_then(&[TACTICAL_ACTION_ID, centre.as_str(), "done_moving"], &[]);
        let mut game = game_of(state, hub.galaxy, table);
        for _ in 0..40 {
            if let Some(choice) = game.legal_options()
                && choice.prompt.starts_with("produce in ")
            {
                return choice.prompt.rsplit(" (").next().unwrap().trim_end_matches(" left)").to_owned();
            }
            assert_eq!(game.step().error, None);
        }
        panic!("no production prompt");
    };
    assert_eq!(left(true), "7");
    assert_eq!(left(false), "5", "Floating Factory I produces 5");
}

#[test]
fn ffac2_5_a_blockaded_floating_factory_is_destroyed() {
    let mut state = seated_all(&[("a", "saar"), ("b", "sol")]);
    let hub = open_hub();
    let target = SystemId::new(hub.outer[0].clone());
    let origin = SystemId::new(hub.centre.clone());
    for system in state.board.keys().cloned().collect::<Vec<_>>() {
        let board = state.system_mut(&system);
        board.units.clear();
        board.planet_units.values_mut().for_each(|units| units.retain(|u| u.owner != a()));
    }
    put(&mut state, &target, "saar_spacedock", &a(), 1);
    put(&mut state, &origin, "cruiser", &b(), 1);
    state.phase = Phase::Action;
    state.active = Some(b());
    let table = opening_then(
        &[TACTICAL_ACTION_ID, target.as_str()],
        &["move|", "done_loading", "done_moving"],
    );
    let mut game = game_of(state, hub.galaxy, table);
    drive(&mut game, 60, tactical_done);
    assert_eq!(count(&game.state, &target, "cruiser", &b()), 1, "Sol's ship arrived");
    assert_eq!(count(&game.state, &target, "saar_spacedock", &a()), 0, "the blockaded dock is destroyed");
}

#[test]
fn cm_1_chaos_mapping_bars_other_players_from_activating_an_asteroid_field_with_your_ships() {
    let field = crate::fixtures::a_system_where("asteroid field");
    let offered = |tech: bool, who: &PlayerId| {
        let mut state = seated_all(&[("a", "saar"), ("b", "sol")]);
        let hub = crate::fixtures::hub_with_outer(&field);
        put(&mut state, &SystemId::new(field.as_str()), "destroyer", &a(), 1);
        if tech {
            super::give(&mut state, "a", "cm");
        }
        state.phase = Phase::Action;
        state.active = Some(who.clone());
        let mut game = game_of(state, hub.galaxy, preferring(&[TACTICAL_ACTION_ID, "decline"]));
        let choice = until_prompt(&mut game, "activate a system");
        choice.ids().contains(&field.as_str())
    };
    assert!(offered(true, &a()), "the holder may activate its own field");
    assert!(!offered(true, &b()), "another player may not");
    assert!(offered(false, &b()), "without the technology the field is open");
}

#[test]
fn cm_2_chaos_mapping_produces_a_unit_at_a_space_dock_at_the_start_of_the_turn() {
    let destroyers = |tech: bool| {
        let mut state = seated_all(&[("a", "saar"), ("b", "sol")]);
        let hub = open_hub();
        let dock = SystemId::new(hub.outer[0].clone());
        put(&mut state, &dock, "saar_spacedock", &a(), 1);
        if tech {
            super::give(&mut state, "a", "cm");
        }
        state.player_mut(&a()).unwrap().trade_goods = 10;
        state.phase = Phase::Action;
        state.active = Some(a());
        // With the technology the turn opens with a production offer; without it, nothing.
        let queue: Vec<String> = if tech {
            [dock.as_str(), "build|destroyer|1", "trade_good"].iter().map(|a| (*a).to_owned()).collect()
        } else {
            Vec::new()
        };
        let table = Table::with_default(Box::new(Opening {
            queue: queue.into_iter().collect(),
            then: Prefer(vec!["pass".to_owned()]),
            builds: None,
            order: None,
        }));
        let mut game = game_of(state, hub.galaxy, table);
        drive(&mut game, 12, |g| g.events.iter().any(|e| e == "PLAYER_PASSED"));
        count(&game.state, &dock, "destroyer", &a())
    };
    assert_eq!(destroyers(true), 1);
    assert_eq!(destroyers(false), 0);
}

// -- Sol -----------------------------------------------------------------------------------------

#[test]
fn so2_2_spec_ops_ii_survivors_go_on_the_card_and_return_home_at_the_start_of_the_next_turn() {
    let (mut state, hub, centre, planet) =
        production_board(&[("a", "sol"), ("b", "hacan")], "spacedock");
    super::give(&mut state, "a", "so2");
    // Sol's four Spec Ops II hold the planet; Hacan bombards it with six dreadnoughts.
    state.system_mut(&centre).planet_units.clear();
    put_on_planet(&mut state, &centre, &planet, "sol_infantry2", &a(), 4);
    for system in state.board.keys().cloned().collect::<Vec<_>>() {
        if system != centre {
            state.system_mut(&system).units.clear();
        }
    }
    let origin = SystemId::new(hub.outer[0].clone());
    put(&mut state, &origin, "dreadnought", &b(), 6);
    let (_, home_planet_id) = home_planet(&state, &a());
    let home = home_of(&state, &a());
    let before_home = count_on(&state, &home, &home_planet_id, "sol_infantry2", &a());
    state.active = Some(b());
    let table = opening_then(
        &[TACTICAL_ACTION_ID, centre.as_str()],
        &["move|", "done_loading", "done_moving", "pass"],
    );
    let mut game = game_of(state, hub.galaxy, table);
    drive(&mut game, 150, |g| g.events.iter().any(|e| e.starts_with("SPEC_OPS_RETURNED")) || g.state.round >= 2);
    let faces: Vec<u32> = game
        .rolls()
        .iter()
        .filter(|roll| roll.reason == "spec ops II")
        .flat_map(|roll| roll.faces.clone())
        .collect();
    let survivors = faces.iter().filter(|face| **face >= 5).count();
    let destroyed = 4 - count_on(&game.state, &centre, &planet, "sol_infantry2", &a());
    assert!(destroyed > 0, "the bombardment killed some");
    assert_eq!(faces.len(), destroyed, "one die for each Spec Ops II destroyed");
    assert!(
        survivors == 0 || game.events.iter().any(|e| e == &format!("SPEC_OPS_RETURNED:a:{survivors}")),
        "survivors {survivors}: {:?}",
        game.events
    );
    // Back in the home system, on a planet Sol controls.
    let after_home = count_on(&game.state, &home, &home_planet_id, "sol_infantry2", &a());
    assert_eq!(after_home, before_home + survivors);
}

#[test]
fn versatile_1_sol_gains_one_more_command_token_in_the_status_phase() {
    let mut state = seated_all(&[("a", "sol"), ("b", "hacan")]);
    state.phase = Phase::Action;
    state.active = Some(a());
    let total = |state: &GameState, who: &PlayerId| state.player(who).unwrap().total_tokens();
    let (a_before, b_before) = (total(&state, &a()), total(&state, &b()));
    let mut game = Game::with_table(state, content(), preferring(&["pass"])).with_sources(SOURCES);
    drive(&mut game, 200, |g| g.state.round >= 2);
    let (a_gain, b_gain) = (
        total(&game.state, &a()) - a_before,
        total(&game.state, &b()) - b_before,
    );
    assert_eq!(b_gain, 2, "everyone gains two command tokens in the status phase");
    assert_eq!(a_gain, 3, "Sol's Versatile adds one");
}

/// Sol (a) defends a planet with two Spec Ops; Hacan (b) lands six infantry from two carriers.
/// `abilities` are the window answers Sol gives; anything else it declines. Runs to the end of the
/// tactical action.
fn sol_defends(commander: LeaderStatus, agent: LeaderStatus, abilities: &[&str]) -> Game<'static> {
    let (mut state, hub, centre, planet) =
        production_board(&[("a", "sol"), ("b", "hacan")], "spacedock");
    state.system_mut(&centre).planet_units.clear();
    put_on_planet(&mut state, &centre, &planet, "sol_infantry", &a(), 2);
    set_leader(&mut state, &a(), "solcommander", commander);
    set_leader(&mut state, &a(), "solagent", agent);
    for system in state.board.keys().cloned().collect::<Vec<_>>() {
        if system != centre {
            state.system_mut(&system).units.clear();
        }
    }
    let origin = SystemId::new(hub.outer[0].clone());
    put(&mut state, &origin, "carrier", &b(), 2);
    put(&mut state, &origin, "infantry", &b(), 6);
    state.active = Some(b());
    let mut prefixes = vec!["move|", "load|", "done_loading", "done_moving", "commit|", "done_committing", "fight"];
    prefixes.extend_from_slice(abilities);
    prefixes.push("decline");
    let table = opening_then(&[TACTICAL_ACTION_ID, centre.as_str()], &prefixes);
    let mut game = game_of(state, hub.galaxy, table);
    drive(&mut game, 120, tactical_done);
    game
}

#[test]
fn solcommander_2_claire_gibson_adds_an_infantry_when_a_ground_combat_starts_on_a_sol_planet() {
    let commander = "leader:sol:solcommander";
    let dice = |status, abilities: &[&str]| {
        let game = sol_defends(status, LeaderStatus::Exhausted, abilities);
        first_roll_len(&game, "a", "ground combat")
    };
    assert_eq!(dice(LeaderStatus::Unlocked, &[commander]), 3, "unlocked: one more infantry rolls");
    assert_eq!(dice(LeaderStatus::Unlocked, &[]), 2, "declined");
    assert_eq!(dice(LeaderStatus::Locked, &[commander]), 2, "locked: nothing is offered");
}

#[test]
fn solagent_1_the_chosen_ground_force_rolls_one_additional_die_in_a_ground_combat_round() {
    let agent = "leader:sol:solagent";
    let game = sol_defends(LeaderStatus::Locked, LeaderStatus::Readied, &[agent, "ground|", "pick", "force"]);
    assert_eq!(first_roll_len(&game, "a", "ground combat"), 3, "two Spec Ops, one of them rolling twice");
    assert_eq!(leader_status(&game.state, &a(), "solagent"), Some(LeaderStatus::Exhausted));
    let declined = sol_defends(LeaderStatus::Locked, LeaderStatus::Readied, &[]);
    assert_eq!(first_roll_len(&declined, "a", "ground combat"), 2);
    assert_eq!(leader_status(&declined.state, &a(), "solagent"), Some(LeaderStatus::Readied));
}

#[test]
fn solbt_1_bellum_gloriosum_a_produced_ship_brings_ground_forces_and_fighters_beyond_the_production_limit() {
    // The dock has PRODUCTION 3. A carrier and two fighters use all of it; the infantry only fit
    // because the breakthrough lets them ride on the carrier's capacity outside the limit. They are
    // still paid for.
    let run = |breakthrough: bool| {
        let (mut state, hub, centre, _planet) =
            production_board(&[("a", "sol"), ("b", "hacan")], "spacedock");
        if breakthrough {
            state.player_mut(&a()).unwrap().breakthrough =
                Some(ti4_model::id::BreakthroughId::new("solbt"));
        }
        state.player_mut(&a()).unwrap().trade_goods = 20;
        let table = Table::with_default(Box::new(Opening {
            queue: [TACTICAL_ACTION_ID, centre.as_str(), "done_moving"].iter().map(|a| (*a).to_owned()).collect(),
            then: Prefer(Vec::new()),
            builds: None,
            order: Some(BuildOrder(
                ["build|sol_carrier|1", "build|fighter|2", "build|sol_infantry|2"].iter().map(|a| (*a).to_owned()).collect(),
            )),
        }));
        let mut game = game_of(state, hub.galaxy, table);
        drive(&mut game, 80, tactical_done);
        let board = game.state.system_state(&centre);
        let infantry = board
            .units
            .iter()
            .chain(board.planet_units.values().flatten())
            .filter(|u| u.owner == a() && u.type_id.as_str().contains("infantry"))
            .count();
        (infantry, 20 - game.state.player(&a()).unwrap().trade_goods)
    };
    assert_eq!(run(true), (2, 5), "carrier 3, two fighters 1, two infantry 1: all five paid for");
    assert_eq!(run(false), (0, 4), "without it the PRODUCTION limit ends the build at the fighters");
}

#[test]
fn ac2_1_advanced_carrier_ii_moves_two_systems_and_carries_eight() {
    // Eight infantry wait in the ring system with one carrier; the target is two systems away.
    let arrived = |tech: bool| {
        let mut state = seated_all(&[("a", "sol"), ("b", "hacan")]);
        let hub = open_hub();
        let origin = SystemId::new(hub.outer[0].clone());
        let target = SystemId::new(hub.across(&hub.outer[0]));
        for system in state.board.keys().cloned().collect::<Vec<_>>() {
            state.system_mut(&system).units.retain(|u| u.owner != a());
        }
        put(&mut state, &origin, "sol_carrier", &a(), 1);
        put(&mut state, &origin, "sol_infantry", &a(), 8);
        if tech {
            super::give(&mut state, "a", "ac2");
            crate::technology::apply_unit_upgrades(&mut state, content(), SOURCES, &a());
        }
        state.phase = Phase::Action;
        state.active = Some(a());
        let table = opening_then(
            &[TACTICAL_ACTION_ID, target.as_str()],
            &["move|", "load|", "done_loading", "done_moving", "done_committing"],
        );
        let mut game = game_of(state, hub.galaxy, table);
        drive(&mut game, 120, tactical_done);
        let board = game.state.system_state(&target);
        let ground = board
            .units
            .iter()
            .chain(board.planet_units.values().flatten())
            .filter(|u| u.owner == a() && u.type_id.as_str().contains("infantry"))
            .count();
        (board.units.iter().filter(|u| u.owner == a() && u.type_id.as_str().contains("carrier")).count(), ground)
    };
    assert_eq!(arrived(true), (1, 8), "Advanced Carrier II: move 2, capacity 8");
    assert_eq!(arrived(false), (0, 0), "Advanced Carrier I has move 1 and cannot make two systems");
}

#[test]
fn ac2_2_advanced_carrier_ii_can_sustain_damage_in_a_space_combat() {
    let survives = |tech: bool| {
        let mut state = seated_all(&[("a", "sol"), ("b", "hacan")]);
        let system = open_system(0);
        put(&mut state, &system, "sol_carrier", &a(), 1);
        put(&mut state, &system, "cruiser", &b(), 1);
        if tech {
            super::give(&mut state, "a", "ac2");
            crate::technology::apply_unit_upgrades(&mut state, content(), SOURCES, &a());
        }
        // One round: the carrier (hits on 9) and the cruiser both hit.
        let (state, _) = space_combat(state, &system, &[10, 10, 10, 10, 10, 10], &["sustain"]);
        state.system_state(&system).units.iter().filter(|u| u.owner == a()).count()
    };
    assert_eq!(survives(true), 1, "the carrier sustains the hit");
    assert_eq!(survives(false), 0, "Advanced Carrier I is destroyed");
}

#[test]
fn ms_1_military_support_at_the_start_of_sols_turn_costs_a_token_plants_two_infantry_and_goes_home() {
    let run = |answer: &str| {
        let mut state = seated_all(&[("a", "sol"), ("b", "jolnar")]);
        let (system, planet) = crate::fixtures::a_placed_planet();
        state.system_mut(&system).set_control(planet.clone(), b());
        crate::promissory::take(&mut state, content(), &b(), "ms:sol");
        state.phase = Phase::Action;
        state.active = Some(a());
        let tokens = state.player(&a()).unwrap().strategic_tokens;
        let first = format!("{system}|{planet}");
        let first = if answer == "place" { first.as_str() } else { "decline" };
        let table = Table::with_default(Box::new(Opening {
            queue: [first].iter().map(|a| (*a).to_owned()).collect(),
            then: Prefer(vec!["pass".to_owned()]),
            builds: None,
            order: None,
        }));
        let mut game = Game::with_table(state, content(), table).with_sources(SOURCES);
        drive(&mut game, 10, |g| g.events.iter().any(|e| e == "MILITARY_SUPPORT_USED") || g.events.iter().any(|e| e == "PLAYER_PASSED"));
        (
            count_on(&game.state, &system, &planet, "infantry", &b()),
            tokens - game.state.player(&a()).unwrap().strategic_tokens,
            crate::promissory::holder_of(&game.state, "ms", &a()),
        )
    };
    assert_eq!(run("place"), (2, 1, None), "two infantry, one token from Sol's pool, the note home");
    assert_eq!(run("decline"), (0, 1, None), "declining the infantry still takes the token and returns the card");
}

// -- Unit abilities ------------------------------------------------------------------------------

/// What a ship does in the game, measured by playing it: its dice and hit number in a space
/// combat, whether it can sustain, whether it reaches an adjacent and a two-away system in a
/// tactical action, and how many ground forces the loading step lets it carry.
#[derive(Debug, PartialEq, Eq)]
struct Played {
    dice: usize,
    hits_on: Option<u32>,
    sustains: bool,
    adjacent: bool,
    two_away: bool,
    capacity: usize,
}

fn play_ship(faction: &str, unit: &str, tech: Option<&str>) -> Played {
    let ready = |state: &mut GameState| {
        if let Some(tech) = tech {
            super::give(state, "a", tech);
        }
    };
    // Combat: the unit alone against one Sol cruiser. Everything rolls 10: the unit's own roll is
    // read, and the cruiser's hit shows whether it sustains.
    let mut state = seated_all(&[("a", faction), ("b", "sol")]);
    ready(&mut state);
    let system = open_system(0);
    put(&mut state, &system, unit, &a(), 1);
    put(&mut state, &system, "cruiser", &b(), 1);
    let (state, dice) = space_combat(state, &system, &[10; 40], &["sustain"]);
    let roll = dice
        .history()
        .iter()
        .find(|roll| roll.reason == "space combat" && roll.by.as_deref() == Some("a"))
        .expect("the unit rolled");
    let (dice_rolled, hits_on) = (roll.faces.len(), roll.hits_on);
    let sustains = state.system_state(&system).units.iter().any(|u| u.owner == a());
    // Movement and loading.
    let moves = |across: bool| {
        let mut state = seated_all(&[("a", faction), ("b", "sol")]);
        ready(&mut state);
        let hub = open_hub();
        let origin = SystemId::new(hub.outer[0].clone());
        let target = SystemId::new(if across { hub.across(&hub.outer[0]) } else { hub.centre.clone() });
        for system in state.board.keys().cloned().collect::<Vec<_>>() {
            state.system_mut(&system).units.retain(|u| u.owner != a());
        }
        put(&mut state, &origin, unit, &a(), 1);
        put(&mut state, &origin, "infantry", &a(), 12);
        state.phase = Phase::Action;
        state.active = Some(a());
        let table = opening_then(
            &[TACTICAL_ACTION_ID, target.as_str()],
            &["move|", "load|", "done_loading", "done_moving", "done_committing", "decline"],
        );
        let mut game = game_of(state, hub.galaxy, table);
        let mut capacity = 0;
        for _ in 0..120 {
            if let Some(choice) = game.legal_options()
                && choice.prompt.starts_with("load ")
                && capacity == 0
            {
                capacity = choice
                    .prompt
                    .rsplit('(')
                    .next()
                    .and_then(|rest| rest.split(' ').next())
                    .and_then(|n| n.parse::<usize>().ok())
                    .unwrap_or(0);
            }
            assert_eq!(game.step().error, None, "{:?}", game.events);
            if tactical_done(&game) {
                break;
            }
        }
        let arrived = game.state.system_state(&target).units.iter().any(|u| u.owner == a() && u.type_id.as_str() == unit);
        (arrived, capacity)
    };
    let (adjacent, capacity) = moves(false);
    let (two_away, _) = moves(true);
    Played { dice: dice_rolled, hits_on, sustains, adjacent, two_away, capacity }
}

#[test]
fn arborec_flagship_2_duha_menaimon_plays_as_printed() {
    let played = play_ship("arborec", "arborec_flagship", None);
    assert_eq!(
        played,
        Played { dice: 2, hits_on: Some(7), sustains: true, adjacent: true, two_away: false, capacity: 5 }
    );
}

#[test]
fn letnev_flagship_3_arc_secundus_plays_as_printed() {
    let played = play_ship("letnev", "letnev_flagship", None);
    assert_eq!(
        played,
        Played { dice: 2, hits_on: Some(5), sustains: true, adjacent: true, two_away: false, capacity: 3 }
    );
}

#[test]
fn muaat_flagship_2_the_inferno_plays_as_printed() {
    let played = play_ship("muaat", "muaat_flagship", None);
    assert_eq!(
        played,
        Played { dice: 2, hits_on: Some(5), sustains: true, adjacent: true, two_away: false, capacity: 3 }
    );
}

#[test]
fn hacan_flagship_2_wrath_of_kenara_plays_as_printed() {
    let played = play_ship("hacan", "hacan_flagship", None);
    assert_eq!(
        played,
        Played { dice: 2, hits_on: Some(7), sustains: true, adjacent: true, two_away: false, capacity: 3 }
    );
}

#[test]
fn saar_flagship_2_son_of_ragh_plays_as_printed() {
    let played = play_ship("saar", "saar_flagship", None);
    assert_eq!(
        played,
        Played { dice: 2, hits_on: Some(5), sustains: true, adjacent: true, two_away: false, capacity: 3 }
    );
}

#[test]
fn sol_flagship_2_genesis_plays_as_printed() {
    let played = play_ship("sol", "sol_flagship", None);
    assert_eq!(
        played,
        Played { dice: 2, hits_on: Some(5), sustains: true, adjacent: true, two_away: false, capacity: 12 }
    );
}

#[test]
fn muaat_warsun_2_prototype_war_sun_i_plays_as_printed() {
    let played = play_ship("muaat", "muaat_warsun", None);
    assert_eq!(
        played,
        Played { dice: 3, hits_on: Some(3), sustains: true, adjacent: true, two_away: false, capacity: 6 }
    );
}

#[test]
fn muaat_warsun2_1_prototype_war_sun_ii_plays_as_printed() {
    let played = play_ship("muaat", "muaat_warsun2", Some("pws2"));
    assert_eq!(
        played,
        Played { dice: 3, hits_on: Some(3), sustains: true, adjacent: true, two_away: true, capacity: 6 }
    );
}

#[test]
fn sol_carrier_1_advanced_carrier_i_plays_as_printed() {
    let played = play_ship("sol", "sol_carrier", None);
    assert_eq!(
        played,
        Played { dice: 1, hits_on: Some(9), sustains: false, adjacent: true, two_away: false, capacity: 6 }
    );
}

#[test]
fn sol_carrier2_1_advanced_carrier_ii_plays_as_printed() {
    let played = play_ship("sol", "sol_carrier2", Some("ac2"));
    assert_eq!(
        played,
        Played { dice: 1, hits_on: Some(9), sustains: true, adjacent: true, two_away: true, capacity: 8 }
    );
}

/// Letnev (a) holds a planet with two infantry (and ten trade goods); Hacan (b) lands `landing`
/// infantry from one carrier. Letnev answers the windows with `abilities`, and declines the rest.
fn letnev_defends(landing: usize, abilities: &[&str]) -> (Game<'static>, SystemId, PlanetId) {
    let (mut state, hub, centre, planet) =
        production_board(&[("a", "letnev"), ("b", "hacan")], "spacedock");
    state.system_mut(&centre).planet_units.clear();
    put_on_planet(&mut state, &centre, &planet, "infantry", &a(), 2);
    for system in state.board.keys().cloned().collect::<Vec<_>>() {
        if system != centre {
            state.system_mut(&system).units.clear();
        }
    }
    let origin = SystemId::new(hub.outer[0].clone());
    put(&mut state, &origin, "carrier", &b(), 1);
    put(&mut state, &origin, "infantry", &b(), landing);
    state.active = Some(b());
    let mut prefixes = vec!["move|", "load|", "done_loading", "done_moving", "commit|", "done_committing", "fight"];
    prefixes.extend_from_slice(abilities);
    prefixes.extend(["trade_good", "decline"]);
    let table = opening_then(&[TACTICAL_ACTION_ID, centre.as_str()], &prefixes);
    let mut game = game_of(state, hub.galaxy, table);
    drive(&mut game, 120, tactical_done);
    (game, centre, planet)
}

#[test]
fn letnev_mech_1_dunlain_reaper_replaces_an_infantry_for_two_resources_at_the_start_of_a_ground_round() {
    let (deployed, centre, planet) = letnev_defends(1, &["deploy"]);
    let (declined, _, _) = letnev_defends(1, &[]);
    let spent = |game: &Game<'_>| 10 - game.state.player(&a()).unwrap().trade_goods
        + i32::try_from(game.state.exhausted_planets.len()).unwrap();
    assert_eq!(spent(&deployed), 2, "two resources");
    assert_eq!(spent(&declined), 0);
    let mechs = |game: &Game<'_>| count_on(&game.state, &centre, &planet, "letnev_mech", &a());
    assert_eq!(mechs(&deployed), 1, "the mech stands where an infantry stood");
    assert_eq!(mechs(&declined), 0);
    let infantry = |game: &Game<'_>| count_on(&game.state, &centre, &planet, "infantry", &a());
    assert_eq!(infantry(&declined) - infantry(&deployed), 1, "one infantry fewer: it was replaced");
}

#[test]
fn sol_mech_1_zs_thunderbolt_deploys_for_three_resources_after_orbital_drop() {
    let run = |deploy: bool| {
        let (mut state, hub, centre, planet) =
            production_board(&[("a", "sol"), ("b", "hacan")], "spacedock");
        state.system_mut(&centre).planet_units.clear();
        let answers = [
            "faction|orbital_drop".to_owned(),
            planet.to_string(),
            if deploy { "deploy|sol_mech|1".to_owned() } else { "decline".to_owned() },
        ];
        let table = Table::with_default(Box::new(Opening {
            queue: answers.into_iter().collect(),
            then: Prefer(vec!["trade_good".to_owned(), "pass".to_owned()]),
            builds: None,
            order: None,
        }));
        let mut game = game_of(state, hub.galaxy, table);
        drive(&mut game, 12, |g| g.events.iter().any(|e| e == "PLAYER_PASSED") || g.events.iter().any(|e| e == "TURN_PASSED"));
        let spent = 10 - game.state.player(&a()).unwrap().trade_goods
            + i32::try_from(game.state.exhausted_planets.len()).unwrap();
        (
            count_on(&game.state, &centre, &planet, "sol_mech", &a()),
            count_on(&game.state, &centre, &planet, "sol_infantry", &a()),
            spent,
        )
    };
    assert_eq!(run(true), (1, 2, 3), "two infantry from the drop, a mech for three resources");
    assert_eq!(run(false), (0, 2, 0));
}

#[test]
fn letnev_flagship_1_arc_secundus_removes_planetary_shield_and_bombards_five_three_times() {
    let (left, roll) = bombard_with("letnev", "letnev_flagship", None);
    assert_eq!(roll, Some((3, Some(5))), "BOMBARDMENT 5 (x3) although a PDS shields the planet");
    assert!(left < 3, "{left} Sol infantry left");
    let (left, roll) = bombard_with("letnev", "dreadnought", None);
    assert_eq!(roll, None, "a dreadnought alone is stopped by the shield");
    assert_eq!(left, 3);
}

#[test]
fn letnev_flagship_2_arc_secundus_repairs_itself_at_the_start_of_each_space_combat_round() {
    // Both ships start damaged and are hit once in round one: the flagship repairs first and
    // sustains again; the dreadnought, still damaged, is destroyed.
    let survivor = |unit: &str| {
        let mut state = seated_all(&[("a", "letnev"), ("b", "sol")]);
        let system = open_system(0);
        state
            .system_mut(&system)
            .units
            .push(ti4_model::units::Unit::new(ti4_model::id::UnitTypeId::new(unit), a()).sustained());
        put(&mut state, &system, "cruiser", &b(), 1);
        let (state, _) = space_combat(state, &system, &[1, 10, 10, 10, 10, 10, 10, 10], &["sustain"]);
        state.system_state(&system).units.iter().any(|u| u.owner == a())
    };
    assert!(survivor("letnev_flagship"), "repaired at the start of the round, then sustained");
    assert!(!survivor("dreadnought"), "an ordinary damaged dreadnought dies to the same hit");
}

#[test]
fn hacan_flagship_1_wrath_of_kenara_spends_a_trade_good_to_add_one_to_a_roll_in_a_space_combat() {
    let run = |answer: &str| {
        let mut state = seated_all(&[("a", "hacan"), ("b", "sol")]);
        let system = open_system(0);
        put(&mut state, &system, "hacan_flagship", &a(), 1);
        put(&mut state, &system, "cruiser", &b(), 1);
        state.player_mut(&a()).unwrap().trade_goods = 3;
        // The flagship hits on 7: two 6s are two near misses. The cruiser misses. Later rounds hit.
        let mut faces = vec![6, 6, 1];
        faces.extend([10; 20]);
        let (state, dice) = space_combat(state, &system, &faces, &[answer]);
        let rounds = dice.history().iter().filter(|r| r.reason == "space combat" && r.by.as_deref() == Some("a")).count();
        (state.player(&a()).unwrap().trade_goods, rounds)
    };
    assert_eq!(run("kenara|1"), (2, 1), "one trade good turns a 6 into a hit: the cruiser dies in round one");
    assert_eq!(run("decline"), (3, 2), "without it round one misses and a second round is fought");
}

#[test]
fn saar_flagship_1_son_of_ragh_fires_anti_fighter_barrage_six_times_four() {
    let mut state = seated_all(&[("a", "saar"), ("b", "sol")]);
    let system = open_system(0);
    put(&mut state, &system, "saar_flagship", &a(), 1);
    put(&mut state, &system, "fighter", &b(), 2);
    let (state, dice) = space_combat(state, &system, &[6, 6, 6, 6, 10, 10, 10, 10], &[]);
    let barrage = dice
        .history()
        .iter()
        .find(|roll| roll.reason == "anti-fighter barrage" && roll.by.as_deref() == Some("a"))
        .expect("the flagship fired a barrage");
    assert_eq!((barrage.faces.len(), barrage.hits_on), (4, Some(6)));
    assert_eq!(count(&state, &system, "fighter", &b()), 0, "both fighters were shot down");
}

#[test]
fn genesis_1_sol_flagship_places_an_infantry_in_its_space_area_at_the_end_of_the_status_phase() {
    let mut state = seated_all(&[("a", "sol"), ("b", "hacan")]);
    let system = open_system(0);
    put(&mut state, &system, "sol_flagship", &a(), 1);
    state.phase = Phase::Action;
    state.active = Some(a());
    let mut game = Game::with_table(state, content(), preferring(&["pass"])).with_sources(SOURCES);
    let infantry = |game: &Game<'_>| {
        game.state.system_state(&system).units.iter().filter(|u| u.owner == a() && u.type_id.as_str().contains("infantry")).count()
    };
    assert_eq!(infantry(&game), 0);
    drive(&mut game, 200, |g| g.state.round >= 2);
    assert_eq!(infantry(&game), 1, "one Spec Ops beside the flagship");
}

#[test]
#[ignore = "MISSING/BUG: hacan_mech (Pride of Kenara) the transaction menu never offers a planet card, so its planet cannot be traded and its units cannot move with it (registered_mech_abilities lists it, nothing implements it)"]
fn hacan_mech_1_pride_of_kenara_makes_its_planet_card_tradable_and_moves_the_units() {
    let mut game = trading_game("hacan", "sol", true, &[]);
    let (home, planet) = home_planet(&game.state, &a());
    put_on_planet(&mut game.state, &home, &planet, "hacan_mech", &a(), 1);
    let (cap, seen) = crate::choice::Capturing::new(Box::new(Scripted::new(["component|trade|sol"])));
    game.install_table(Table::with_default(Box::new(cap)));
    for _ in 0..4 {
        let _ = game.step();
    }
    let offered: Vec<String> = seen
        .borrow()
        .iter()
        .filter(|choice| choice.prompt.starts_with("transaction with"))
        .flat_map(|choice| choice.ids().into_iter().map(str::to_owned).collect::<Vec<_>>())
        .collect();
    assert!(!offered.is_empty(), "the transaction menu was shown");
    assert!(
        offered.iter().any(|id| id.contains(planet.as_str())),
        "a deal that gives the planet {planet} is on offer: {offered:?}"
    );
}
