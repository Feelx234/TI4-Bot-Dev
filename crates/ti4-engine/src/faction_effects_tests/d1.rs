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

/// The scripted `opening` answers first (they must be offered), then the `prefer`red ones.
struct Opening {
    queue: std::collections::VecDeque<String>,
    then: Prefer,
    builds: Option<Builds>,
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
