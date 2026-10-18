//! Depth tests, group d5: Titans of Ul, Vuil'raith Cabal, Keleres (all variants) and Ral Nel (see
//! mod.rs and docs/FACTION_EFFECT_COVERAGE.md). Each test drives the engine's own flow (a `Game`
//! with a scripted table, a whole space combat or invasion with loaded dice) and asserts what the
//! card promises, plus the opposite case where cheap.
//!
//! Test names: `<card id>_<effect no.>_<what>`; unit tests `unit_<unit id>_<n>_<what>`.

use ti4_content::galaxy::Galaxy;
use ti4_model::content_types::DEFAULT;
use ti4_model::id::{PlanetId, PlayerId, SystemId};
use ti4_model::state::{GameState, LeaderStatus};

use super::d2::{
    Seen, action_game, first_roll, invade, leader_status, on_planet, run_status_phase,
    score_objectives, set_leader, ships, space_fight, unlocks_in_the_game, was_offered,
    was_offered_like,
};
use super::{content, seated_all};
use crate::choice::Table;
use crate::dice::Dice;
use crate::fixtures::{Hub, plain_hub, put, put_on_planet};
use crate::game::{Game, TACTICAL_ACTION_ID};

fn a() -> PlayerId {
    PlayerId::new("a")
}
fn b() -> PlayerId {
    PlayerId::new("b")
}
fn sys(id: &str) -> SystemId {
    SystemId::new(id)
}

// -- shared drivers -------------------------------------------------------------------------------

/// Step `game` until it has logged `event` (true) or `max` steps went by (false); every step must
/// be legal.
fn drive(game: &mut Game<'_>, event: &str, max: usize) -> bool {
    for _ in 0..max {
        let step = game.step();
        assert_eq!(step.error, None, "log: {:?}", game.events);
        if game.events.iter().any(|logged| logged == event) {
            return true;
        }
    }
    false
}

/// Step `game` exactly `count` times; every step must be legal.
fn steps(game: &mut Game<'_>, count: usize) {
    for _ in 0..count {
        assert_eq!(game.step().error, None, "log: {:?}", game.events);
    }
}

/// Did the game log this event?
fn logged(game: &Game<'_>, event: &str) -> bool {
    game.events.iter().any(|logged| logged == event)
}

/// Every question the table was asked, as `option ids` per question, for failure messages.
fn asked(seen: &Seen) -> Vec<String> {
    seen.borrow()
        .iter()
        .map(|question| format!("{}: {}", question.prompt, question.options.join(",")))
        .collect()
}

/// Every ring system of `hub` emptied of units and control, so the map is a blank slate.
fn blank_hub(state: &mut GameState, hub: &Hub) {
    for id in std::iter::once(&hub.centre).chain(&hub.outer) {
        let system = state.system_mut(&sys(id));
        system.units.clear();
        system.planet_units.clear();
        system.planet_control.clear();
    }
}

/// A game with `dice` loaded (then 10s: every later die hits) and the action phase to `active`.
fn loaded(
    state: GameState,
    active: &str,
    galaxy: Option<Galaxy>,
    rules: &[&str],
    faces: &[u32],
) -> (Game<'static>, Seen) {
    let (mut game, seen) = action_game(state, active, galaxy, rules);
    game.load_dice(Dice::from_faces(super::d2::padded_hits(faces)));
    (game, seen)
}

/// `active` takes a tactical action into `target`, then `rules` answer what follows; steps until
/// the action completes.
fn tactical(
    state: GameState,
    active: &str,
    galaxy: &Galaxy,
    target: &str,
    rules: &[&str],
    faces: &[u32],
) -> (Game<'static>, Seen) {
    let mut all = vec![TACTICAL_ACTION_ID, target];
    all.extend_from_slice(rules);
    let (mut game, seen) = loaded(state, active, Some(galaxy.clone()), &all, faces);
    for _ in 0..60 {
        assert_eq!(game.step().error, None, "log: {:?}", game.events);
        if logged(&game, "TACTICAL_ACTION_COMPLETE") {
            break;
        }
    }
    (game, seen)
}

/// Every agent of these seats readies in the engine's own status phase.
fn agent_readies_at_status(faction: &str, agent: &str) {
    let mut state = seated_all(&[("a", faction), ("b", "sol")]);
    set_leader(&mut state, &a(), agent, LeaderStatus::Exhausted);
    run_status_phase(&mut state);
    assert_eq!(
        leader_status(&state, &a(), agent),
        Some(LeaderStatus::Readied),
        "{agent}"
    );
}

/// Wipe every unit of `who` off the board.
fn strip_units(state: &mut GameState, who: &PlayerId) {
    for board in state.board.values_mut() {
        board.units.retain(|unit| unit.owner != *who);
        for units in board.planet_units.values_mut() {
            units.retain(|unit| unit.owner != *who);
        }
    }
}

// -- Titans of Ul ---------------------------------------------------------------------------------

fn titans_game() -> GameState {
    seated_all(&[("a", "titans"), ("b", "sol")])
}

#[test]
fn titansagent_1_tellurian_cancels_a_hit_in_a_real_space_combat_and_readies_at_status() {
    // Sol attacks the Titans cruiser with one cruiser; every die hits. Tellurian cancels the hit
    // on the Titans cruiser (so it survives and Sol's dies); declined, both cruisers die.
    let tellurian = "leader:titans:titansagent:HITS_TO_ASSIGN:when";
    let hub = plain_hub();
    let (target, from) = (hub.outer[0].clone(), hub.outer[1].clone());
    for (use_agent, survives) in [(true, true), (false, false)] {
        let mut state = titans_game();
        blank_hub(&mut state, &hub);
        put(&mut state, &sys(&target), "titans_cruiser", &a(), 1);
        put(&mut state, &sys(&from), "cruiser", &b(), 1);
        let move_it = format!("move|{from}|0");
        let rules: Vec<&str> = if use_agent {
            vec![&move_it, "done_moving", tellurian]
        } else {
            vec![&move_it, "done_moving"]
        };
        let (mut game, seen) = tactical(state, "b", &hub.galaxy, &target, &rules, &[10; 12]);
        assert_eq!(
            was_offered(&seen, tellurian),
            true,
            "offered either way: {:?}",
            asked(&seen)
        );
        assert_eq!(
            ships(&game.state, &sys(&target), &a(), "titans_cruiser"),
            usize::from(survives),
            "{:?}",
            asked(&seen)
        );
        assert_eq!(
            ships(&game.state, &sys(&target), &b(), "cruiser"),
            0,
            "the Titans hit landed"
        );
        assert_eq!(
            leader_status(&game.state, &a(), "titansagent"),
            Some(if use_agent {
                LeaderStatus::Exhausted
            } else {
                LeaderStatus::Readied
            })
        );
        if use_agent {
            run_status_phase(&mut game.state);
            assert_eq!(
                leader_status(&game.state, &a(), "titansagent"),
                Some(LeaderStatus::Readied)
            );
        }
    }
}

#[test]
fn titanscommander_1_unlocks_in_the_game_with_five_structures_on_the_board() {
    let (system, planet) = crate::fixtures::a_placed_planet();
    let mut state = titans_game();
    strip_units(&mut state, &a());
    put_on_planet(&mut state, &system, &planet, "pds", &a(), 4);
    assert!(
        !unlocks_in_the_game(&state, &a(), "titanscommander"),
        "four"
    );
    // The other player's structures do not count.
    put_on_planet(&mut state, &system, &planet, "pds", &b(), 3);
    assert!(
        !unlocks_in_the_game(&state, &a(), "titanscommander"),
        "four of yours"
    );
    put_on_planet(&mut state, &system, &planet, "pds", &a(), 1);
    assert!(unlocks_in_the_game(&state, &a(), "titanscommander"), "five");
}

#[test]
fn titanscommander_2_tungstantus_gains_a_trade_good_when_production_is_used_in_a_tactical_action() {
    let window = "leader:titans:titanscommander:PRODUCTION_USED:when";
    for (status, gains) in [(LeaderStatus::Unlocked, 1), (LeaderStatus::Locked, 0)] {
        let mut state = titans_game();
        set_leader(&mut state, &a(), "titanscommander", status);
        let goods = state.player(&a()).unwrap().trade_goods;
        let home = state.player(&a()).unwrap().home_system.clone().unwrap();
        let (mut game, seen) = action_game(
            state,
            "a",
            Some(plain_hub().galaxy),
            &[TACTICAL_ACTION_ID, home.as_str(), "done_moving", window],
        );
        assert!(
            drive(&mut game, "PRODUCTION_USED", 30),
            "{:?}",
            asked(&seen)
        );
        steps(&mut game, 3);
        assert_eq!(was_offered(&seen, window), gains == 1, "{:?}", asked(&seen));
        assert_eq!(
            game.state.player(&a()).unwrap().trade_goods - goods,
            gains,
            "{:?}",
            asked(&seen)
        );
    }
}

#[test]
fn titanshero_1_unlocks_in_the_game_on_the_third_scored_objective() {
    let mut state = titans_game();
    score_objectives(&mut state, &a(), 2);
    assert!(!unlocks_in_the_game(&state, &a(), "titanshero"));
    score_objectives(&mut state, &a(), 3);
    assert!(unlocks_in_the_game(&state, &a(), "titanshero"));
}

#[test]
fn titanshero_2_geoform_readies_elysium_adds_three_and_three_and_is_purged() {
    use crate::production::Spend;
    let elysium = PlanetId::new("elysium");
    let mut state = titans_game();
    state.exhausted_planets.insert(elysium.clone());
    set_leader(&mut state, &a(), "titanshero", LeaderStatus::Unlocked);
    let value = |state: &GameState, kind| {
        crate::production::planet_value_now(state, content(), DEFAULT, &elysium, kind)
    };
    let before = (
        value(&state, Spend::Resources),
        value(&state, Spend::Influence),
    );
    let (mut game, seen) = action_game(state, "a", None, &["component|leader|titanshero"]);
    steps(&mut game, 2);
    assert!(
        was_offered(&seen, "component|leader|titanshero"),
        "{:?}",
        asked(&seen)
    );
    assert!(!game.state.exhausted_planets.contains(&elysium), "readied");
    assert_eq!(value(&game.state, Spend::Resources), before.0 + 3);
    assert_eq!(value(&game.state, Spend::Influence), before.1 + 3);
    assert_eq!(
        leader_status(&game.state, &a(), "titanshero"),
        Some(LeaderStatus::Purged)
    );
}

#[test]
fn titanshero_2_geoform_planet_fires_space_cannon_five_three_dice_at_ships_that_activate_it() {
    // Sol activates the Titans home system, whose defences are stripped bar Elysium; with the
    // Geoform card on Elysium the planet fires three dice at the arriving ship, without it none.
    let hub = crate::fixtures::hub_with_outer("55");
    let cannon_dice = |geoform: bool| {
        let mut state = titans_game();
        blank_hub(&mut state, &hub);
        state
            .system_mut(&sys("55"))
            .set_control(PlanetId::new("elysium"), a());
        if geoform {
            state
                .planet_attachments
                .entry(PlanetId::new("elysium"))
                .or_default()
                .push("titanshero".to_owned());
        }
        put(&mut state, &sys(&hub.centre), "cruiser", &b(), 1);
        let move_it = format!("move|{}|0", hub.centre);
        let (game, seen) = tactical(
            state,
            "b",
            &hub.galaxy,
            "55",
            &[&move_it, "done_moving"],
            &[1; 40],
        );
        let rolled: Vec<_> = game
            .rolls()
            .iter()
            .filter(|roll| roll.by.as_deref() == Some("a"))
            .collect();
        assert!(
            rolled.iter().all(|roll| roll.hits_on.is_some()),
            "{:?}",
            asked(&seen)
        );
        rolled.iter().map(|roll| roll.faces.len()).sum::<usize>()
    };
    assert_eq!(cannon_dice(false), 0);
    assert_eq!(cannon_dice(true), 3, "SPACE CANNON 5(x3)");
}

/// A planet that is nobody's home planet and not Mecatol Rex, with its system.
fn non_home_planet() -> (SystemId, PlanetId) {
    let catalogue = ti4_content::galaxy::all_planets(content(), DEFAULT);
    let planet = crate::fixtures::non_home_planets(400)
        .into_iter()
        .filter(|p| !crate::seating::is_mecatol_planet(p))
        .find(|p| {
            catalogue
                .get(p.as_str())
                .and_then(ti4_content::Planet::system_id)
                .is_some()
        })
        .map(PlanetId::new)
        .expect("a planet");
    let system = SystemId::new(catalogue[planet.as_str()].system_id().unwrap());
    (system, planet)
}

/// Sol holds Terraform and a planet; the Titans seat is `a`.
fn terraform_game() -> (GameState, SystemId, PlanetId) {
    let (system, planet) = non_home_planet();
    let mut state = titans_game();
    state.system_mut(&system).set_control(planet.clone(), b());
    let note = crate::promissory::note_id("terraform", "titans");
    crate::promissory::take(&mut state, content(), &b(), &note);
    state.promissory_faceup.remove(&note);
    (state, system, planet)
}

#[test]
fn terraform_1_the_holder_attaches_it_as_an_action_to_a_planet_it_controls() {
    let (state, _, planet) = terraform_game();
    let note = crate::promissory::note_id("terraform", "titans");
    let action = format!("faction|titans|terraform|{planet}");
    let (mut game, seen) = action_game(state, "b", None, &[&action]);
    steps(&mut game, 2);
    assert!(was_offered(&seen, &action), "{:?}", asked(&seen));
    assert_eq!(
        game.state.planet_attachments.get(&planet).map(Vec::len),
        Some(1),
        "attached"
    );
    assert!(game.state.promissory_faceup.contains(&note), "in play");
    assert_eq!(game.state.promissory_notes.get(&note), Some(&b()));
}

#[test]
fn terraform_1_is_not_offered_for_a_home_planet_mecatol_rex_or_to_the_titans() {
    let (mut state, _, legal) = terraform_game();
    let home = state.player(&b()).unwrap().home_planets[0].clone();
    let mecatol = crate::seating::mecatol_for(content(), DEFAULT);
    let mr = if mecatol == crate::seating::MECATOL_TE {
        "mrte"
    } else {
        "mr"
    };
    state
        .system_mut(&sys(mecatol))
        .set_control(PlanetId::new(mr), b());
    let (_, seen) = {
        let (mut game, seen) = action_game(state, "b", None, &[]);
        steps(&mut game, 1);
        (game, seen)
    };
    assert!(
        was_offered_like(&seen, &format!("terraform|{legal}")),
        "a legal planet is offered: {:?}",
        asked(&seen)
    );
    assert!(!was_offered_like(&seen, &format!("terraform|{home}")));
    assert!(!was_offered_like(&seen, &format!("terraform|{mr}")));
    let (state, _, planet) = terraform_game();
    let (mut game, seen) = action_game(state, "a", None, &[]);
    steps(&mut game, 1);
    assert!(
        !was_offered_like(&seen, &format!("terraform|{planet}")),
        "the Titans do not hold their own note"
    );
}

#[test]
fn terraform_2_gives_the_planet_one_more_resource_and_influence_and_all_three_traits() {
    use crate::production::Spend;
    let (state, _, planet) = terraform_game();
    let value = |state: &GameState, kind| {
        crate::production::planet_value_now(state, content(), DEFAULT, &planet, kind)
    };
    let influence =
        |state: &GameState| crate::production::available(state, content(), DEFAULT, &b(), Spend::Influence);
    let before = (
        value(&state, Spend::Resources),
        value(&state, Spend::Influence),
        influence(&state),
    );
    let action = format!("faction|titans|terraform|{planet}");
    let (mut game, _) = action_game(state, "b", None, &[&action]);
    steps(&mut game, 2);
    assert_eq!(value(&game.state, Spend::Resources), before.0 + 1);
    assert_eq!(value(&game.state, Spend::Influence), before.1 + 1);
    assert_eq!(
        influence(&game.state),
        before.2 + 1,
        "the holder can spend the extra influence"
    );
    let traits = crate::planets::traits_now(&game.state, content(), DEFAULT, &planet);
    for kind in ["CULTURAL", "HAZARDOUS", "INDUSTRIAL"] {
        assert!(traits.iter().any(|t| t == kind), "{kind} in {traits:?}");
    }
}

fn pile(aliases: &[&str]) -> Vec<ti4_model::id::ActionCardId> {
    aliases
        .iter()
        .map(|alias| ti4_model::id::ActionCardId::new(*alias))
        .collect()
}

fn hand_size(state: &GameState, who: &PlayerId) -> usize {
    state.player(who).expect("seat").action_cards.len()
}

#[test]
fn titansbt_1_slumberstate_adds_a_status_phase_draw_for_each_coexistence_partner() {
    // The real status phase: with the card and a coexistence both Titans and partner draw two
    // action cards, not one; with the card alone, or the coexistence alone, one.
    let hands = |breakthrough: bool, coexisting: bool| {
        let (system, planet) = non_home_planet();
        let mut state = titans_game();
        for who in [a(), b()] {
            state.player_mut(&who).unwrap().action_cards.clear();
        }
        state.action_card_deck = pile(&[
            "bribery", "rally", "plague", "spy", "uprising", "scuttle", "seize", "decoy",
        ]);
        put_on_planet(&mut state, &system, &planet, "infantry", &a(), 1);
        put_on_planet(&mut state, &system, &planet, "infantry", &b(), 1);
        state.system_mut(&system).set_control(planet.clone(), b());
        if coexisting {
            crate::coexistence::begin(&mut state, &system, &planet, &a(), None).unwrap();
        }
        if breakthrough {
            state.player_mut(&a()).unwrap().breakthrough =
                Some(ti4_model::id::BreakthroughId::new("titansbt"));
        }
        let game = super::d2::play_status_phase(state);
        (hand_size(&game.state, &a()), hand_size(&game.state, &b()))
    };
    let (base_a, base_b) = hands(false, false);
    assert_eq!(
        hands(true, true),
        (base_a + 1, base_b + 1),
        "one additional card each"
    );
    assert_eq!(hands(false, true), (base_a, base_b), "no breakthrough");
    assert_eq!(hands(true, false), (base_a, base_b), "not coexisting");
}

#[test]
fn titansbt_1_slumberstate_lets_another_player_allow_a_sleeper_token_on_their_planet() {
    let (system, planet) = non_home_planet();
    let planet_id = planet.to_string();
    for (breakthrough, placed) in [(true, true), (false, false)] {
        let mut state = titans_game();
        state.system_mut(&system).set_control(planet.clone(), b());
        if breakthrough {
            state.player_mut(&a()).unwrap().breakthrough =
                Some(ti4_model::id::BreakthroughId::new("titansbt"));
        }
        // The Titans' turn begins: they name the planet, its controller allows it.
        let window = "breakthrough:titans:titansbt_sleeper:TURN_BEGAN:after";
        let (mut game, seen) = action_game(state, "a", None, &[window, &planet_id, "allow"]);
        steps(&mut game, 2);
        assert_eq!(
            crate::factions::titans::has_sleeper(&game.state, &planet),
            placed,
            "{:?}",
            asked(&seen)
        );
        assert_eq!(was_offered(&seen, &planet_id), breakthrough);
    }
}

// -- Vuil'raith Cabal -----------------------------------------------------------------------------

fn cabal_game() -> GameState {
    seated_all(&[("a", "cabal"), ("b", "sol")])
}

/// Give the Technology strategy card to `who`.
fn with_card(state: &mut GameState, who: &PlayerId, card: &str) {
    state.player_mut(who).expect("seat").strategy_cards =
        vec![ti4_model::id::StrategyCardId::new(card)];
}

fn owns_tech(state: &GameState, who: &PlayerId, tech: &str) -> bool {
    state
        .player(who)
        .expect("seat")
        .technologies
        .contains(&ti4_model::id::TechnologyId::new(tech))
}

#[test]
fn riftmeld_1_a_captured_cruiser_pays_for_cruiser_ii_in_a_technology_research() {
    let cruiser = ti4_model::id::UnitTypeId::new("cruiser");
    let research = |captured: bool| {
        let mut state = cabal_game();
        state.player_mut(&a()).unwrap().technologies.clear();
        with_card(&mut state, &a(), "pok7technology");
        if captured {
            assert!(crate::supply::capture_from_reinforcements(
                &mut state,
                content(),
                DEFAULT,
                &a(),
                &b(),
                &cruiser
            ));
        }
        let held_by_b = crate::supply::remaining(&state, content(), DEFAULT, &b(), &cruiser);
        let (mut game, seen) = action_game(
            state,
            "a",
            None,
            &["strategic", "cr2", "waiver|0", "~captured|"],
        );
        steps(&mut game, 8);
        (game, seen, held_by_b)
    };
    let (game, seen, held_by_b) = research(true);
    assert!(owns_tech(&game.state, &a(), "cr2"), "{:?}", asked(&seen));
    assert!(
        crate::supply::captured_by(&game.state, &a()).is_empty(),
        "the captured cruiser was returned"
    );
    assert_eq!(
        crate::supply::remaining(&game.state, content(), DEFAULT, &b(), &cruiser),
        held_by_b + 1,
        "to its owner's reinforcements"
    );
    let (game, seen, _) = research(false);
    assert!(!owns_tech(&game.state, &a(), "cr2"), "no capture, no waiver");
    assert!(!was_offered(&seen, "cr2"), "{:?}", asked(&seen));
}

/// The Cabal's only dock is in `dock_system` on a hub whose first outer system it is; `b` has a
/// cruiser next door.
fn vortex_game() -> (GameState, Hub, SystemId) {
    let (dock_system, dock_planet) = crate::fixtures::a_placed_planet();
    let hub = crate::fixtures::hub_with_centre(dock_system.as_str());
    let mut state = cabal_game();
    blank_hub(&mut state, &hub);
    for board in state.board.values_mut() {
        board
            .units
            .retain(|u| !(u.owner == a() && u.type_id.as_str().contains("spacedock")));
        for units in board.planet_units.values_mut() {
            units.retain(|u| !(u.owner == a() && u.type_id.as_str().contains("spacedock")));
        }
    }
    put_on_planet(
        &mut state,
        &dock_system,
        &dock_planet,
        "cabal_spacedock",
        &a(),
        1,
    );
    put(&mut state, &sys(&hub.outer[0]), "cruiser", &b(), 1);
    (state, hub, dock_system)
}

#[test]
fn vtx_1_the_vortex_action_captures_a_unit_from_another_players_reinforcements() {
    let cruiser = ti4_model::id::UnitTypeId::new("cruiser");
    for (card, captures) in [(true, true), (false, false)] {
        let (mut state, hub, _) = vortex_game();
        if card {
            super::give(&mut state, "a", "vtx");
        }
        let before = crate::supply::remaining(&state, content(), DEFAULT, &b(), &cruiser);
        let (mut game, seen) = action_game(
            state,
            "a",
            Some(hub.galaxy.clone()),
            &["faction|cabal|vortex", "cruiser"],
        );
        steps(&mut game, 3);
        assert_eq!(
            was_offered(&seen, "faction|cabal|vortex"),
            card,
            "{:?}",
            asked(&seen)
        );
        assert_eq!(
            crate::supply::captured_by(&game.state, &a()).len(),
            usize::from(captures),
            "{:?}",
            asked(&seen)
        );
        if captures {
            assert_eq!(
                crate::supply::remaining(&game.state, content(), DEFAULT, &b(), &cruiser),
                before - 1,
                "taken from the reinforcements, not the board"
            );
            assert_eq!(ships(&game.state, &sys(&hub.outer[0]), &b(), "cruiser"), 1);
            assert!(
                game.state
                    .player(&a())
                    .unwrap()
                    .exhausted_technologies
                    .contains(&ti4_model::id::TechnologyId::new("vtx")),
                "the card exhausts"
            );
        }
    }
}

#[test]
fn cabalagent_1_stillness_of_stars_converts_commodities_and_captures_a_unit_when_trade_replenishes() {
    // Sol plays Trade and replenishes; the Cabal agent exhausts, Sol's commodities become trade
    // goods and a unit is captured from Sol's reinforcements.
    let window = "leader:cabal:cabalagent:COMMODITIES_REPLENISHED:after";
    for (agent, used) in [(LeaderStatus::Readied, true), (LeaderStatus::Exhausted, false)] {
        let mut state = cabal_game();
        with_card(&mut state, &b(), "pok5trade");
        set_leader(&mut state, &a(), "cabalagent", agent);
        state.player_mut(&b()).unwrap().commodities = 0;
        let goods = state.player(&b()).unwrap().trade_goods;
        let (mut game, seen) = action_game(
            state,
            "b",
            None,
            &["strategic", window, "~dreadnought"],
        );
        steps(&mut game, 8);
        assert_eq!(was_offered(&seen, window), used, "{:?}", asked(&seen));
        let limit = crate::strategy_cards::commodity_limit(&game.state, content(), &b());
        if used {
            assert_eq!(
                crate::supply::captured_by(&game.state, &a()).len(),
                1,
                "{:?}",
                asked(&seen)
            );
            assert_eq!(game.state.player(&b()).unwrap().commodities, 0, "converted");
            assert_eq!(
                game.state.player(&b()).unwrap().trade_goods,
                goods + 3 + limit,
                "the Trade card's 3 plus the commodities one for one"
            );
            assert_eq!(
                leader_status(&game.state, &a(), "cabalagent"),
                Some(LeaderStatus::Exhausted)
            );
            run_status_phase(&mut game.state);
            assert_eq!(
                leader_status(&game.state, &a(), "cabalagent"),
                Some(LeaderStatus::Readied)
            );
        } else {
            assert!(crate::supply::captured_by(&game.state, &a()).is_empty());
            assert_eq!(
                game.state.player(&b()).unwrap().commodities,
                limit,
                "an exhausted agent converts nothing"
            );
        }
    }
}

/// Printed gravity rifts in the corpus, in system-id order.
fn printed_rifts() -> Vec<SystemId> {
    ti4_content::galaxy::all_systems(content(), DEFAULT)
        .iter()
        .filter(|(_, system)| system.is_gravity_rift())
        .map(|(id, _)| SystemId::new(*id))
        .collect()
}

#[test]
fn cabalcommander_1_unlocks_in_the_game_with_units_in_three_gravity_rifts() {
    let rifts = printed_rifts();
    let mut state = cabal_game();
    strip_units(&mut state, &a());
    put(&mut state, &rifts[0], "cruiser", &a(), 2);
    put(&mut state, &rifts[1], "fighter", &a(), 1);
    assert!(
        !unlocks_in_the_game(&state, &a(), "cabalcommander"),
        "two rifts"
    );
    // The third rift is a Dimensional Tear: the dock is the unit in it.
    let (system, planet) = crate::fixtures::a_placed_planet();
    assert!(!rifts.contains(&system));
    put_on_planet(&mut state, &system, &planet, "cabal_spacedock", &a(), 1);
    assert!(unlocks_in_the_game(&state, &a(), "cabalcommander"), "three");
    // Other players' units in rifts do not count.
    let mut state = cabal_game();
    strip_units(&mut state, &a());
    for rift in &rifts {
        put(&mut state, rift, "cruiser", &b(), 1);
    }
    assert!(!unlocks_in_the_game(&state, &a(), "cabalcommander"));
}

#[test]
fn cabalhero_1_unlocks_in_the_game_on_the_third_scored_objective() {
    let mut state = cabal_game();
    score_objectives(&mut state, &a(), 2);
    assert!(!unlocks_in_the_game(&state, &a(), "cabalhero"));
    score_objectives(&mut state, &a(), 3);
    assert!(unlocks_in_the_game(&state, &a(), "cabalhero"));
}

/// A hub whose first outer system holds the Cabal's only dock (a Dimensional Tear I), nobody
/// else's fleet in space anywhere.
fn tear_hub() -> (GameState, Hub, SystemId) {
    let (dock_system, dock_planet) = crate::fixtures::a_placed_planet();
    let hub = crate::fixtures::hub_with_outer(dock_system.as_str());
    let mut state = cabal_game();
    strip_units(&mut state, &a());
    for board in state.board.values_mut() {
        board.units.clear();
    }
    put_on_planet(
        &mut state,
        &dock_system,
        &dock_planet,
        "cabal_spacedock",
        &a(),
        1,
    );
    (state, hub, dock_system)
}

#[test]
fn cabalhero_2_it_which_is_not_captures_on_a_one_to_three_and_is_purged() {
    // One die per non-fighter ship of the other player in or next to a tear: the 3 captures, the 4
    // does not.
    let (mut state, hub, tear) = tear_hub();
    set_leader(&mut state, &a(), "cabalhero", LeaderStatus::Unlocked);
    put(&mut state, &tear, "cruiser", &b(), 2);
    put(&mut state, &tear, "fighter", &b(), 1);
    let (mut game, seen) = loaded(
        state,
        "a",
        Some(hub.galaxy.clone()),
        &["component|leader|cabalhero"],
        &[3, 4],
    );
    steps(&mut game, 2);
    assert!(
        was_offered(&seen, "component|leader|cabalhero"),
        "{:?}",
        asked(&seen)
    );
    let hero_rolls: Vec<_> = game
        .rolls()
        .iter()
        .filter(|roll| roll.reason == "cabalhero")
        .collect();
    assert_eq!(
        hero_rolls.iter().map(|r| r.faces.len()).sum::<usize>(),
        2,
        "one die per cruiser, none for the fighter: {hero_rolls:?}"
    );
    // The captured cruiser's fighter no longer fits any hold, so it is captured with it.
    let taken: Vec<&str> = crate::supply::captured_by(&game.state, &a())
        .iter()
        .map(|(_, unit)| unit.as_str())
        .collect();
    assert_eq!(taken, ["cruiser", "fighter"], "the 3 captured one cruiser");
    assert_eq!(ships(&game.state, &tear, &b(), "cruiser"), 1, "the 4 missed");
    assert_eq!(
        leader_status(&game.state, &a(), "cabalhero"),
        Some(LeaderStatus::Purged)
    );
}

#[test]
fn crucible_1_the_holder_plays_it_after_activating_so_its_ship_leaves_a_rift_without_a_roll() {
    // Sol's cruiser starts in the Cabal's Dimensional Tear (a gravity rift) and leaves it. Every
    // die is a 1: the rift would destroy it. With the Crucible played there is no roll and it
    // arrives; declined, it dies. The note goes home to the Cabal when played.
    let window = "promissory:sol:crucible:SYSTEM_ACTIVATED:after";
    for (play, arrives) in [(true, true), (false, false)] {
        let (mut state, hub, tear) = tear_hub();
        state
            .promissory_notes
            .insert("crucible:cabal".to_owned(), b());
        put(&mut state, &tear, "cruiser", &b(), 1);
        let move_it = format!("move|{tear}|0");
        let rules: Vec<&str> = if play {
            vec![window, &move_it, "done_moving"]
        } else {
            vec!["decline", &move_it, "done_moving"]
        };
        let (game, seen) = tactical(state, "b", &hub.galaxy, &hub.centre, &rules, &[1; 20]);
        assert!(was_offered(&seen, window), "{:?}", asked(&seen));
        assert_eq!(
            ships(&game.state, &sys(&hub.centre), &b(), "cruiser"),
            usize::from(arrives),
            "{:?}",
            asked(&seen)
        );
        let rift_rolls = game
            .rolls()
            .iter()
            .filter(|roll| roll.by.as_deref() == Some("b"))
            .count();
        assert_eq!(rift_rolls > 0, !play, "rolls: {:?}", game.rolls());
        assert_eq!(
            game.state.promissory_notes.get("crucible:cabal"),
            Some(&if play { a() } else { b() }),
            "played, the note returns to the Vuil'raith player"
        );
    }
}

#[test]
fn crucible_2_only_the_activating_holder_may_play_it_never_the_cabal() {
    let window = "promissory:sol:crucible:SYSTEM_ACTIVATED:after";
    let (mut state, hub, tear) = tear_hub();
    state
        .promissory_notes
        .insert("crucible:cabal".to_owned(), b());
    put(&mut state, &sys(&hub.centre), "cruiser", &a(), 1);
    // The Cabal activates: neither it nor the holder is offered the note.
    let (game, seen) = tactical(state, "a", &hub.galaxy, tear.as_str(), &["done_moving"], &[1; 20]);
    assert!(!was_offered(&seen, window), "{:?}", asked(&seen));
    assert_eq!(
        game.state.promissory_notes.get("crucible:cabal"),
        Some(&b()),
        "the note stays with its holder"
    );
}

#[test]
fn cabalbt_1_the_fracture_enters_play_without_a_roll_and_ingress_tokens_move() {
    let mut state = cabal_game();
    state.phase = ti4_model::state::Phase::Action;
    state.active = Some(a());
    let before = state.ingress_tokens.clone();
    assert!(!state.fracture_in_play);
    // The game records what each seat held at its first look; an empty baseline makes the card
    // below a gain it announces at the next step.
    state
        .faction_marks
        .insert("private:#seen:breakthrough:a".to_owned(), String::new());
    state.player_mut(&a()).unwrap().breakthrough =
        Some(ti4_model::id::BreakthroughId::new("cabalbt"));
    let (mut game, seen) = super::d2::eager_game(state, "a", Some(plain_hub().galaxy), &[]);
    steps(&mut game, 2);
    assert!(
        game.state.fracture_in_play,
        "no roll was needed: {:?} {:?}",
        asked(&seen),
        game.events
    );
    let moved = game
        .state
        .ingress_tokens
        .iter()
        .filter(|system| !before.contains(system))
        .count();
    assert!(
        (1..=2).contains(&moved),
        "up to two ingress tokens moved: {moved}"
    );
}

// -- Keleres --------------------------------------------------------------------------------------

fn keleres_game(variant: &str) -> GameState {
    seated_all(&[("a", variant), ("b", "sol"), ("c", "hacan")])
}

/// Action cards of one kind (component action or not), in corpus order.
fn action_cards_where(component: bool, how_many: usize) -> Vec<ti4_model::id::ActionCardId> {
    content()
        .from_sources(ti4_model::content_types::ContentType::ActionCards, DEFAULT)
        .filter_map(|record| record.text("alias").or_else(|| record.text("id")))
        .map(ti4_model::id::ActionCardId::new)
        .filter(|card| crate::action_cards::is_component_action(content(), card) == component)
        .take(how_many)
        .collect()
}

#[test]
fn the_tribuni_1_each_variant_takes_its_base_factions_home_system_and_its_matching_hero() {
    for (variant, base, hero) in [
        ("keleresm", "mentak", "keleresheroharka"),
        ("keleresx", "xxcha", "keleresheroodlynn"),
        ("keleresa", "argent", "keleresherokuuasi"),
    ] {
        let keleres = seated_all(&[("a", variant), ("b", "sol")]);
        let alone = seated_all(&[("a", base), ("b", "sol")]);
        let seat = keleres.player(&a()).unwrap();
        // The content models the Tribuni as a faction record per variant with its own home tile
        // (not the base faction's), so the seat only needs a home system of its own.
        assert!(seat.home_system.is_some(), "{variant} has a home system");
        assert_ne!(
            seat.home_system,
            alone.player(&a()).unwrap().home_system,
            "{variant}: the variant's own tile, not {base}'s"
        );
        let heroes: Vec<&str> = seat
            .leaders
            .keys()
            .map(|leader| leader.as_str())
            .filter(|leader| leader.starts_with("keleresHero") || leader.starts_with("keleresh"))
            .collect();
        assert_eq!(heroes, [hero], "{variant} has only the matching hero");
    }
}

#[test]
fn council_patronage_1_replenishes_commodities_then_gains_a_trade_good_when_the_strategy_phase_begins() {
    // The real status phase hands over to the strategy phase, whose start fires the ability.
    let after_status = |faction: &str| {
        let mut state = seated_all(&[("a", faction), ("b", "sol")]);
        state.player_mut(&a()).unwrap().commodities = 0;
        let goods = state.player(&a()).unwrap().trade_goods;
        let mut game = super::d2::play_status_phase(state);
        steps(&mut game, 3);
        let seat = game.state.player(&a()).unwrap();
        (seat.commodities, seat.trade_goods - goods)
    };
    let plain = after_status("sol");
    for variant in crate::factions::keleres::VARIANTS {
        let (commodities, goods) = after_status(variant);
        let limit = i32::try_from(
            crate::strategy_cards::commodity_limit(
                &seated_all(&[("a", variant), ("b", "sol")]),
                content(),
                &a(),
            ),
        )
        .unwrap();
        assert_eq!(commodities, limit, "{variant}: replenished");
        assert_eq!(goods, plain.1 + 1, "{variant}: and one trade good");
    }
}

#[test]
fn laws_order_1_spending_a_trade_good_blanks_every_law_until_that_turn_ends() {
    // Political Censure bars Sol from action cards. At the start of Sol's turn the Keleres spend a
    // trade good: the law is blank, so Sol's component action card is offered; declined, it is not.
    let window = "ability:keleresx:laws_order:TURN_BEGAN:after";
    let component = pile(&["mining_initiative"]);
    for (spend, offered) in [(true, true), (false, false)] {
        let mut state = keleres_game("keleresx");
        state.laws.insert("censure".to_owned(), "b".to_owned());
        state.player_mut(&a()).unwrap().trade_goods = 1;
        state.player_mut(&b()).unwrap().action_cards = component.clone();
        let rules: Vec<&str> = if spend { vec![window] } else { vec!["decline"] };
        let (mut game, seen) = action_game(state, "b", None, &rules);
        steps(&mut game, 1);
        assert_eq!(
            was_offered(&seen, "action_card|0"),
            offered,
            "{:?}",
            asked(&seen)
        );
        assert_eq!(
            game.state.player(&a()).unwrap().trade_goods,
            i32::from(!spend),
            "paid with the trade good"
        );
        // Sol passed, ending the turn: the law is back.
        assert!(crate::laws::active(&game.state, "censure"), "until the turn ends");
    }
}

/// A Mecatol hub with Keleres' carrier in one outer system and Sol's across the ring, everything
/// else cleared: the two are neighbours only through I.I.H.Q. Modernization.
fn mecatol_neighbours() -> (GameState, Hub) {
    let hub = crate::fixtures::hub_with_centre("18");
    let mut state = seated_all(&[("a", "keleresm"), ("b", "sol")]);
    for board in state.board.values_mut() {
        board.units.clear();
        board.planet_units.clear();
        board.planet_control.clear();
    }
    put(&mut state, &sys(&hub.outer[0]), "carrier", &a(), 1);
    put(&mut state, &sys(&hub.outer[3]), "carrier", &b(), 1);
    (state, hub)
}

#[test]
fn iihq_1_makes_keleres_neighbours_with_everyone_near_mecatol_in_a_real_transaction_offer() {
    for (card, neighbours) in [(true, true), (false, false)] {
        let (mut state, hub) = mecatol_neighbours();
        if card {
            super::give(&mut state, "a", "iihq");
        }
        let (mut game, seen) = action_game(state, "a", Some(hub.galaxy.clone()), &[]);
        steps(&mut game, 1);
        assert_eq!(
            was_offered(&seen, "component|trade|sol"),
            neighbours,
            "{:?}",
            asked(&seen)
        );
    }
}

#[test]
fn iihq_2_the_card_gains_custodia_vigilia_whose_planet_card_cannot_be_lost() {
    let (mut state, _) = mecatol_neighbours();
    super::give(&mut state, "a", "iihq");
    let home = state.player(&a()).unwrap().home_system.clone().unwrap();
    let (mut game, _) = action_game(state, "a", None, &[]);
    steps(&mut game, 1);
    assert!(crate::factions::keleres::holds_custodia(&game.state, &a()));
    // Another player takes control of it: the next step gives it back.
    game.state
        .system_mut(&home)
        .set_control(PlanetId::new("custodiavigilia"), b());
    steps(&mut game, 1);
    assert!(crate::factions::keleres::holds_custodia(&game.state, &a()));
}

#[test]
fn iihq_2_custodia_vigilia_gives_mecatol_rex_space_cannon_five_and_production_three() {
    // The Keleres control Mecatol Rex and Custodia Vigilia: a ship activating Mecatol is shot at by
    // one die, and the Keleres activating it may produce there with no dock (3 units).
    let mecatol = sys("18");
    let mr = PlanetId::new("mr");
    let galaxy = crate::fixtures::hub_with_centre("18").galaxy;
    let from = crate::fixtures::hub_with_centre("18").outer[2].clone();
    for (card, dice, production) in [(true, 1, true), (false, 0, false)] {
        let (mut state, _) = mecatol_neighbours();
        state.player_mut(&b()).unwrap().technologies.clear();
        if card {
            super::give(&mut state, "a", "iihq");
        }
        state.system_mut(&mecatol).set_control(mr.clone(), a());
        // Sol activates Mecatol: its cruiser takes the space cannon.
        put(&mut state, &sys(&from), "cruiser", &b(), 1);
        let move_from = format!("move|{from}|0");
        let (game, seen) = tactical(
            state.clone(),
            "b",
            &galaxy,
            "18",
            &[&move_from, "done_moving"],
            &[1; 40],
        );
        let cannon: usize = game
            .rolls()
            .iter()
            .filter(|roll| roll.by.as_deref() == Some("a"))
            .map(|roll| roll.faces.len())
            .sum();
        assert_eq!(cannon, dice, "{:?}", asked(&seen));
        // The Keleres activate Mecatol: a production window with a budget of 3 opens.
        let (_, seen) = tactical(
            state,
            "a",
            &galaxy,
            "18",
            &["done_moving"],
            &[1; 40],
        );
        assert_eq!(
            was_offered_like(&seen, "build|"),
            production,
            "{:?}",
            asked(&seen)
        );
    }
}

#[test]
fn kelerescommander_1_unlocks_by_spending_a_trade_good_after_a_component_action_card() {
    let window = "leader:keleresm:kelerescommander_unlock:ACTION_CARD_PLAYED:after";
    // Trade goods after the card (Mining Initiative pays some itself) with the unlock declined.
    let play = |goods: i32, rules: &[&str]| {
        let mut state = keleres_game("keleresm");
        state.player_mut(&a()).unwrap().trade_goods = goods;
        state.player_mut(&a()).unwrap().action_cards = pile(&["mining_initiative"]);
        let (mut game, seen) = action_game(state, "a", None, rules);
        steps(&mut game, 3);
        (game, seen)
    };
    let (declined, _) = play(1, &["action_card|0", "decline"]);
    let kept = declined.state.player(&a()).unwrap().trade_goods;
    assert_eq!(
        leader_status(&declined.state, &a(), "kelerescommander"),
        Some(LeaderStatus::Locked)
    );
    let (game, seen) = play(1, &["action_card|0", window]);
    assert_eq!(
        leader_status(&game.state, &a(), "kelerescommander"),
        Some(LeaderStatus::Unlocked),
        "{:?}",
        asked(&seen)
    );
    assert_eq!(
        game.state.player(&a()).unwrap().trade_goods,
        kept - 1,
        "one trade good spent"
    );
    // Without a trade good to spend the unlock is not offered.
    let (game, seen) = play(0, &["action_card|0", window]);
    assert!(!was_offered(&seen, window), "{:?}", asked(&seen));
    assert_eq!(
        leader_status(&game.state, &a(), "kelerescommander"),
        Some(LeaderStatus::Locked)
    );
}

#[test]
fn kelerescommander_2_suffi_an_lets_the_owner_perform_an_additional_action_after_a_component_action() {
    let window = "leader:keleresm:kelerescommander:ACTION_COMPLETED:after";
    // (commander, accept) -> the Keleres seat's "action phase" questions in a row.
    for (status, accept, extra) in [
        (LeaderStatus::Unlocked, true, true),
        (LeaderStatus::Unlocked, false, false),
        (LeaderStatus::Locked, true, false),
    ] {
        let mut state = keleres_game("keleresm");
        set_leader(&mut state, &a(), "kelerescommander", status);
        state.player_mut(&a()).unwrap().action_cards = pile(&["mining_initiative"]);
        let rules: Vec<&str> = if accept {
            vec!["action_card|0", window]
        } else {
            vec!["action_card|0", "decline"]
        };
        let (mut game, seen) = action_game(state, "a", None, &rules);
        steps(&mut game, 5);
        let actions_by_a = seen
            .borrow()
            .iter()
            .filter(|question| question.prompt.starts_with("action phase") && question.player == "a")
            .count();
        assert_eq!(
            actions_by_a >= 2 && actions_by_a > 1 && {
                // The second question must come before any question of Sol's.
                let log = seen.borrow();
                let first_sol = log
                    .iter()
                    .position(|q| q.prompt.starts_with("action phase") && q.player == "b");
                let second_a = log
                    .iter()
                    .enumerate()
                    .filter(|(_, q)| q.prompt.starts_with("action phase") && q.player == "a")
                    .nth(1)
                    .map(|(i, _)| i);
                match (first_sol, second_a) {
                    (Some(sol), Some(again)) => again < sol,
                    (None, Some(_)) => true,
                    _ => false,
                }
            },
            extra,
            "{:?}",
            asked(&seen)
        );
        assert_eq!(was_offered(&seen, window), status == LeaderStatus::Unlocked);
    }
}

#[test]
fn keleresheroharka_1_unlocks_in_the_game_on_the_third_scored_objective() {
    let mut state = keleres_game("keleresm");
    score_objectives(&mut state, &a(), 2);
    assert!(!unlocks_in_the_game(&state, &a(), "keleresheroharka"));
    score_objectives(&mut state, &a(), 3);
    assert!(unlocks_in_the_game(&state, &a(), "keleresheroharka"));
}

#[test]
fn keleresheroodlynn_1_unlocks_in_the_game_on_the_third_scored_objective() {
    let mut state = keleres_game("keleresx");
    score_objectives(&mut state, &a(), 2);
    assert!(!unlocks_in_the_game(&state, &a(), "keleresheroodlynn"));
    score_objectives(&mut state, &a(), 3);
    assert!(unlocks_in_the_game(&state, &a(), "keleresheroodlynn"));
}

#[test]
fn keleresherokuuasi_1_unlocks_in_the_game_on_the_third_scored_objective() {
    let mut state = keleres_game("keleresa");
    score_objectives(&mut state, &a(), 2);
    assert!(!unlocks_in_the_game(&state, &a(), "keleresherokuuasi"));
    score_objectives(&mut state, &a(), 3);
    assert!(unlocks_in_the_game(&state, &a(), "keleresherokuuasi"));
}

#[test]
fn keleresheroharka_2_draws_the_first_three_component_action_cards_shuffles_the_rest_and_is_purged() {
    let plain = action_cards_where(false, 4);
    let action = action_cards_where(true, 4);
    let deck = vec![
        plain[0].clone(),
        action[0].clone(),
        plain[1].clone(),
        action[1].clone(),
        plain[2].clone(),
        action[2].clone(),
        action[3].clone(),
        plain[3].clone(),
    ];
    let mut state = keleres_game("keleresm");
    set_leader(&mut state, &a(), "keleresheroharka", LeaderStatus::Unlocked);
    state.player_mut(&a()).unwrap().action_cards.clear();
    state.action_card_deck = deck.clone();
    let (mut game, seen) = action_game(state, "a", None, &["component|leader|keleresheroharka"]);
    steps(&mut game, 2);
    assert!(
        was_offered(&seen, "component|leader|keleresheroharka"),
        "{:?}",
        asked(&seen)
    );
    assert_eq!(
        game.state.player(&a()).unwrap().action_cards,
        [action[0].clone(), action[1].clone(), action[2].clone()],
        "the first three component action cards"
    );
    let mut left = game.state.action_card_deck.clone();
    left.sort();
    let mut expected: Vec<_> = deck
        .into_iter()
        .filter(|card| !game.state.player(&a()).unwrap().action_cards.contains(card))
        .collect();
    expected.sort();
    assert_eq!(left, expected, "everything else is the deck again");
    assert_eq!(
        leader_status(&game.state, &a(), "keleresheroharka"),
        Some(LeaderStatus::Purged)
    );
}

#[test]
fn keleresherokuuasi_2_places_the_flagship_and_two_escorts_at_the_start_of_a_combat_round_and_is_purged() {
    let window = "leader:keleresa:keleresherokuuasi:COMBAT_ROUND_STARTED:after";
    let (system, planet) = crate::fixtures::a_placed_planet();
    let hub = crate::fixtures::hub_with_outer(system.as_str());
    for (unlocked, placed) in [(true, true), (false, false)] {
        let mut state = keleres_game("keleresa");
        blank_hub(&mut state, &hub);
        state.system_mut(&system).set_control(planet.clone(), a());
        put(&mut state, &system, "cruiser", &a(), 1);
        put(&mut state, &sys(&hub.centre), "cruiser", &b(), 1);
        set_leader(
            &mut state,
            &a(),
            "keleresherokuuasi",
            if unlocked {
                LeaderStatus::Unlocked
            } else {
                LeaderStatus::Locked
            },
        );
        let move_it = format!("move|{}|0", hub.centre);
        let (game, seen) = tactical(
            state,
            "b",
            &hub.galaxy,
            system.as_str(),
            &[&move_it, "done_moving", window, "~kuuasi|"],
            &[1; 60],
        );
        assert_eq!(was_offered(&seen, window), placed, "{:?}", asked(&seen));
        if placed {
            assert_eq!(
                leader_status(&game.state, &a(), "keleresherokuuasi"),
                Some(LeaderStatus::Purged)
            );
            assert!(
                game.state
                    .system_state(&system)
                    .units
                    .iter()
                    .any(|u| u.owner == a() && u.type_id.as_str() == "keleres_flagship")
                    || game.state.board.values().any(|board| board
                        .units
                        .iter()
                        .any(|u| u.owner == a() && u.type_id.as_str() == "keleres_flagship")),
                "the flagship came: {:?}",
                asked(&seen)
            );
            assert!(
                game.rolls()
                    .iter()
                    .any(|roll| roll.by.as_deref() == Some("a") && roll.faces.len() >= 2),
                "the placed ships fought: {:?}",
                game.rolls()
            );
        } else {
            assert_eq!(ships(&game.state, &system, &a(), "keleres_flagship"), 0);
        }
    }
}

/// An agenda phase of `state` (the Keleres seat `a`, Sol `b`, deck `sanctions` first), each seat
/// answering from its own rules (see `Pick::eager`: questions no rule wants take the first option
/// that is not a decline, so the seat votes for "for" with its planets).
fn agenda_game(
    mut state: GameState,
    a_rules: &[&str],
    b_rules: &[&str],
) -> (Game<'static>, Seen, Seen) {
    state.phase = ti4_model::state::Phase::Agenda;
    state.custodians_removed = true;
    for seat in state.players.clone() {
        let planets: Vec<PlanetId> = state
            .controlled_planets(&seat.id)
            .into_iter()
            .map(|(_, planet)| planet.clone())
            .collect();
        for planet in planets {
            state.exhausted_planets.remove(&planet);
        }
    }
    state.agenda_deck = vec!["sanctions".to_owned(), "mutiny".to_owned()];
    let (pick_a, seen_a) = super::d2::Pick::eager(a_rules);
    let (pick_b, seen_b) = super::d2::Pick::eager(b_rules);
    let mut table = Table::default();
    table.seat(a(), Box::new(pick_a));
    table.seat(b(), Box::new(pick_b));
    let mut game = Game::with_table(state, content(), table).with_sources(DEFAULT);
    for _ in 0..200 {
        if game.state.phase != ti4_model::state::Phase::Agenda {
            break;
        }
        assert_eq!(game.step().error, None, "log: {:?}", game.events);
    }
    (game, seen_a, seen_b)
}

fn tokens_held(state: &GameState, who: &PlayerId) -> i32 {
    let seat = state.player(who).unwrap();
    seat.tactic_tokens + seat.fleet_tokens + seat.strategic_tokens
}

#[test]
fn keleresheroodlynn_2_casts_six_more_votes_predicts_pays_per_dissenter_and_is_purged() {
    let window = "leader:keleresx:keleresheroodlynn:AGENDA_REVEALED:after";
    let setup = || {
        let mut state = keleres_game("keleresx");
        state.players.retain(|seat| seat.id != PlayerId::new("c"));
        state.seating_order.retain(|id| *id != PlayerId::new("c"));
        set_leader(&mut state, &a(), "keleresheroodlynn", LeaderStatus::Unlocked);
        state
    };
    // Sol votes "against"; the Keleres predict and vote "for" with six extra votes.
    let (game, seen_a, _) = agenda_game(setup(), &[window, "for", "for"], &["against"]);
    let events = &game.events;
    assert!(
        events.iter().any(|e| e == "AGENDA_RESOLVED:sanctions:for"),
        "six more votes carry the outcome: {events:?} {:?}",
        asked(&seen_a)
    );
    assert_eq!(
        leader_status(&game.state, &a(), "keleresheroodlynn"),
        Some(LeaderStatus::Purged)
    );
    // One trade good and one command token for the one player who voted otherwise: compare with
    // the same agenda phase without the hero (Council Patronage's good is in both).
    let plain = {
        let mut state = setup();
        set_leader(&mut state, &a(), "keleresheroodlynn", LeaderStatus::Locked);
        let (game, ..) = agenda_game(state, &["for"], &["against"]);
        (
            game.state.player(&a()).unwrap().trade_goods,
            tokens_held(&game.state, &a()),
        )
    };
    assert_eq!(game.state.player(&a()).unwrap().trade_goods, plain.0 + 1);
    assert_eq!(tokens_held(&game.state, &a()), plain.1 + 1);
}

#[test]
fn riders_1_a_keleres_rider_predicts_gives_up_the_vote_and_pays_when_right() {
    // Riders of the three variants share one implementation; each variant's own note is played
    // in a real agenda phase by Sol, who may not vote on it.
    let window = "note:sol:keleres_rider:AGENDA_REVEALED:after";
    for (variant, alias) in [
        ("keleresm", "riderm"),
        ("keleresx", "riderx"),
        ("keleresa", "ridera"),
    ] {
        for (prediction, pays) in [("for", true), ("against", false)] {
            let mut state = seated_all(&[("a", variant), ("b", "sol")]);
            let note = crate::promissory::note_id(alias, variant);
            crate::promissory::take(&mut state, content(), &b(), &note);
            state.player_mut(&b()).unwrap().action_cards.clear();
            state.action_card_deck = pile(&["bribery", "rally", "plague"]);
            let (goods, hand) = (state.player(&b()).unwrap().trade_goods, 0);
            let (game, seen_a, seen_b) = agenda_game(state, &["for"], &[window, prediction]);
            assert!(
                game.events.iter().any(|e| e == "AGENDA_RESOLVED:sanctions:for"),
                "{variant}: {:?}",
                asked(&seen_a)
            );
            assert_eq!(
                game.state.promissory_notes.get(&note),
                Some(&a()),
                "{variant}: returned to the Keleres either way"
            );
            // Two agendas are voted on: Sol votes on the second only (the first is the rider's).
            let votes_by_sol = seen_b
                .borrow()
                .iter()
                .filter(|question| question.prompt.contains("vote for which outcome"))
                .count();
            assert_eq!(
                votes_by_sol,
                1,
                "{variant}: the rider cannot vote on that agenda: {:?}",
                asked(&seen_b)
            );
            let seat = game.state.player(&b()).unwrap();
            assert_eq!(
                seat.trade_goods - goods,
                if pays { 2 } else { 0 },
                "{variant} {prediction}: 2 trade goods only when right"
            );
            assert_eq!(
                game.state.action_card_deck.len(),
                3 - usize::from(pays),
                "{variant}: one action card drawn only when right"
            );
            let _ = (seat, hand);
        }
    }
}

// -- Ral Nel --------------------------------------------------------------------------------------

fn ralnel_game() -> GameState {
    seated_all(&[("a", "ralnel"), ("b", "sol")])
}

/// The first planet of `system`.
fn planet_of(system: &str) -> PlanetId {
    PlanetId::new(
        ti4_content::galaxy::planets_in(content(), system, DEFAULT)
            .first()
            .expect("a planet")
            .id(),
    )
}

#[test]
fn survivalinstinct_1_moves_up_to_two_ships_in_from_adjacent_systems_when_another_player_activates() {
    let window = "ability:ralnel:survivalinstinct:SYSTEM_ACTIVATED:after";
    let hub = plain_hub();
    let centre = sys(&hub.centre);
    let (first, second, third) = (&hub.outer[0], &hub.outer[1], &hub.outer[2]);
    for (decline, token) in [(false, false), (true, false), (false, true)] {
        let mut state = ralnel_game();
        blank_hub(&mut state, &hub);
        put(&mut state, &centre, "cruiser", &a(), 1);
        for system in [first, second, third] {
            put(&mut state, &sys(system), "cruiser", &a(), 1);
        }
        if token {
            state.system_mut(&sys(first)).place_token(a());
        }
        let move_first = format!("{first}|0");
        let move_second = format!("{second}|0");
        let rules: Vec<&str> = if decline {
            vec!["decline"]
        } else {
            vec![window, &move_first, &move_second, "done_moving"]
        };
        let (game, seen) = tactical(state, "b", &hub.galaxy, &hub.centre, &rules, &[1; 20]);
        let here = ships(&game.state, &centre, &a(), "cruiser");
        let moved = |system: &String| ships(&game.state, &sys(system), &a(), "cruiser") == 0;
        match (decline, token) {
            (false, false) => {
                assert_eq!(here, 3, "two came in: {:?}", asked(&seen));
                assert!(moved(first) && moved(second), "from the first two");
                assert!(!moved(third), "at most two ships");
            }
            (true, _) => assert_eq!(here, 1, "declined: {:?}", asked(&seen)),
            (false, true) => {
                assert!(!moved(first), "a system with your command token is not taken from");
                assert!(!was_offered(&seen, &move_first), "{:?}", asked(&seen));
            }
        }
    }
}

fn nano_game() -> GameState {
    let mut state = ralnel_game();
    super::give(&mut state, "a", "nanomachines");
    state
}

/// Whether Nanomachines is ready: its component actions are on offer.
fn nanomachines_ready(state: &GameState) -> bool {
    crate::factions::ralnel_cards::component_actions(state, content(), &a())
        .iter()
        .any(|option| option.id.starts_with("faction|ralnel|nano_"))
}

#[test]
fn nanomachines_1_places_a_pds_on_a_planet_you_control_exhausts_and_readies_at_status() {
    let mut state = nano_game();
    let planet = planet_of("18");
    state.system_mut(&sys("18")).set_control(planet.clone(), a());
    let (mut game, seen) = action_game(
        state,
        "a",
        None,
        &["faction|ralnel|nano_pds", planet.as_str()],
    );
    steps(&mut game, 2);
    assert!(
        was_offered(&seen, "faction|ralnel|nano_pds"),
        "{:?}",
        asked(&seen)
    );
    assert_eq!(on_planet(&game.state, &sys("18"), &planet, &a(), "pds"), 1);
    assert!(
        !nanomachines_ready(&game.state),
        "one exhaustion covers all three actions"
    );
    let after = super::d2::play_status_phase(game.state.clone());
    assert!(nanomachines_ready(&after.state), "readied by the status phase");
}

#[test]
fn nanomachines_2_repairs_all_of_your_damaged_units_and_none_of_anyone_elses() {
    let mut state = nano_game();
    put(&mut state, &sys("18"), "cruiser", &a(), 1);
    put(&mut state, &sys("18"), "cruiser", &b(), 1);
    for unit in &mut state.system_mut(&sys("18")).units {
        unit.sustained_damage = true;
    }
    let (mut game, seen) = action_game(state, "a", None, &["faction|ralnel|nano_repair"]);
    steps(&mut game, 2);
    assert!(
        was_offered(&seen, "faction|ralnel|nano_repair"),
        "{:?}",
        asked(&seen)
    );
    let damaged = |who: &PlayerId| {
        game.state
            .board
            .values()
            .flat_map(|board| board.units.iter().chain(board.planet_units.values().flatten()))
            .filter(|unit| unit.owner == *who && unit.sustained_damage)
            .count()
    };
    assert_eq!(damaged(&a()), 0, "all of yours");
    assert!(damaged(&b()) > 0, "not the other player's");
    assert!(!nanomachines_ready(&game.state), "exhausted");
}

#[test]
fn nanomachines_3_discards_an_action_card_to_draw_one() {
    let mut state = nano_game();
    state.player_mut(&a()).unwrap().action_cards = pile(&["sabotage"]);
    let deck = state.action_card_deck.len();
    let discarded = state.discarded_action_cards.len();
    let (mut game, seen) = action_game(state, "a", None, &["faction|ralnel|nano_draw", "sabotage"]);
    steps(&mut game, 3);
    assert!(
        was_offered(&seen, "faction|ralnel|nano_draw"),
        "{:?}",
        asked(&seen)
    );
    assert_eq!(game.state.discarded_action_cards.len(), discarded + 1);
    assert_eq!(game.state.action_card_deck.len(), deck - 1, "one drawn");
    assert_eq!(game.state.player(&a()).unwrap().action_cards.len(), 1);
    assert!(!nanomachines_ready(&game.state), "exhausted");
}

#[test]
fn ralnelagent_1_kan_kip_rel_draws_two_action_cards_gives_one_away_and_readies_at_status() {
    let mut state = ralnel_game();
    state.player_mut(&a()).unwrap().action_cards.clear();
    state.player_mut(&b()).unwrap().action_cards.clear();
    let (mut game, seen) = action_game(
        state,
        "a",
        None,
        &["component|leader|ralnelagent", "~", "b"],
    );
    steps(&mut game, 4);
    assert!(
        was_offered(&seen, "component|leader|ralnelagent"),
        "{:?}",
        asked(&seen)
    );
    assert_eq!(hand_size(&game.state, &a()), 1, "one kept: {:?}", asked(&seen));
    assert_eq!(hand_size(&game.state, &b()), 1, "one given to the chosen player");
    assert_eq!(
        leader_status(&game.state, &a(), "ralnelagent"),
        Some(LeaderStatus::Exhausted)
    );
    run_status_phase(&mut game.state);
    assert_eq!(
        leader_status(&game.state, &a(), "ralnelagent"),
        Some(LeaderStatus::Readied)
    );
}

#[test]
fn ralnelcommander_1_unlocks_for_the_last_player_to_pass_in_the_real_action_phase() {
    for (last, unlocks) in [("a", true), ("b", false)] {
        let mut state = ralnel_game();
        state.player_mut(&sys_player(last, "other")).unwrap().passed = true;
        let (mut game, _) = action_game(state, last, None, &["pass"]);
        steps(&mut game, 2);
        assert_eq!(
            leader_status(&game.state, &a(), "ralnelcommander"),
            Some(if unlocks {
                LeaderStatus::Unlocked
            } else {
                LeaderStatus::Locked
            }),
            "{last} passes last"
        );
    }
}

/// The seat that is not `one` of the two-player game (`a`, `b`).
fn sys_player(one: &str, _: &str) -> PlayerId {
    if one == "a" { b() } else { a() }
}

#[test]
fn ralnelhero_1_unlocks_in_the_game_on_the_third_scored_objective() {
    let mut state = ralnel_game();
    score_objectives(&mut state, &a(), 2);
    assert!(!unlocks_in_the_game(&state, &a(), "ralnelhero"));
    score_objectives(&mut state, &a(), 3);
    assert!(unlocks_in_the_game(&state, &a(), "ralnelhero"));
}

#[test]
fn ralnelhero_2_director_nel_takes_back_the_last_pass_for_two_command_tokens_and_a_card() {
    let mut state = ralnel_game();
    state.phase = ti4_model::state::Phase::Action;
    state.player_mut(&b()).unwrap().passed = true;
    set_leader(&mut state, &a(), "ralnelhero", LeaderStatus::Unlocked);
    state.player_mut(&a()).unwrap().action_cards.clear();
    let tokens = state.player(&a()).unwrap().tactic_tokens;
    let window = "leader:ralnel:ralnelhero:PLAYER_PASSED:after";
    let (mut game, seen) = action_game(state, "a", None, &["pass", window, "tactic_tokens", "tactic_tokens"]);
    steps(&mut game, 2);
    let seat = game.state.player(&a()).unwrap();
    assert!(!seat.passed, "the pass was taken back: {:?}", asked(&seen));
    assert_eq!(seat.tactic_tokens, tokens + 2, "two command tokens");
    assert_eq!(seat.action_cards.len(), 1, "one action card");
    assert_eq!(
        seat.leaders.get(&ti4_model::id::LeaderId::new("ralnelhero")),
        Some(&LeaderStatus::Purged)
    );
    assert_eq!(game.state.phase, ti4_model::state::Phase::Action, "the phase went on");
}

#[test]
fn ralnelcommander_2_watchful_ojz_retreats_up_to_two_ships_when_a_retreat_is_declared() {
    let window = "leader:ralnel:ralnelcommander:RETREAT_DECLARED:when";
    let hub = plain_hub();
    let centre = sys(&hub.centre);
    let safe = &hub.outer[0];
    for (status, retreated) in [(LeaderStatus::Unlocked, true), (LeaderStatus::Locked, false)] {
        let mut state = ralnel_game();
        blank_hub(&mut state, &hub);
        set_leader(&mut state, &a(), "ralnelcommander", status);
        put(&mut state, &centre, "cruiser", &a(), 3);
        put(&mut state, &sys(&hub.outer[1]), "cruiser", &b(), 1);
        // An ordinary retreat needs a system of the retreater's: a unit makes one.
        put(&mut state, &sys(safe), "destroyer", &a(), 1);
        let move_it = format!("move|{}|0", hub.outer[1]);
        let to = format!("to|{safe}");
        let (game, seen) = tactical(
            state,
            "b",
            &hub.galaxy,
            &hub.centre,
            &[&move_it, "done_moving", "stay", "retreat", window, &to, "ship|0|cruiser", "ship|0|cruiser"],
            &[1; 8],
        );
        assert_eq!(was_offered(&seen, window), retreated, "{:?}", asked(&seen));
        assert!(
            ships(&game.state, &sys(safe), &a(), "cruiser") >= 2,
            "at least the two cruisers that retreated"
        );
    }
}

#[test]
fn nanolink_1_the_holder_moves_its_structures_in_from_adjacent_systems_and_the_note_goes_home() {
    let window = "promissory:sol:nanolink:SYSTEM_ACTIVATED:after";
    let hub = plain_hub();
    let centre = sys(&hub.centre);
    let mine = planet_of(&hub.centre);
    let theirs = planet_of(&hub.outer[0]);
    for (play, moves) in [(true, true), (false, false)] {
        let mut state = ralnel_game();
        blank_hub(&mut state, &hub);
        state.system_mut(&centre).set_control(mine.clone(), b());
        put_on_planet(&mut state, &sys(&hub.outer[0]), &theirs, "pds", &b(), 1);
        state
            .promissory_notes
            .insert("nanolink:ralnel".to_owned(), b());
        let rules: Vec<&str> = if play {
            vec![window, mine.as_str(), "done_moving"]
        } else {
            vec!["decline", "done_moving"]
        };
        let (game, seen) = tactical(state, "b", &hub.galaxy, &hub.centre, &rules, &[1; 10]);
        assert!(was_offered(&seen, window), "{:?}", asked(&seen));
        assert_eq!(
            on_planet(&game.state, &centre, &mine, &b(), "pds"),
            usize::from(moves),
            "{:?}",
            asked(&seen)
        );
        assert_eq!(
            on_planet(&game.state, &sys(&hub.outer[0]), &theirs, &b(), "pds"),
            usize::from(!moves),
            "left its old system only when played"
        );
        assert_eq!(
            game.state.promissory_notes.get("nanolink:ralnel"),
            Some(&if play { a() } else { b() }),
            "returned to Ral Nel when played"
        );
    }
}

#[test]
fn nanolink_2_is_dealt_to_ral_nel_and_offered_in_a_transaction_to_be_lent() {
    let mut state = ralnel_game();
    crate::promissory::deal(&mut state, content(), DEFAULT);
    assert_eq!(
        state.promissory_notes.get("nanolink:ralnel"),
        Some(&a()),
        "dealt to its owner"
    );
    // Neighbours: ships in adjacent systems of a map.
    let hub = plain_hub();
    blank_hub(&mut state, &hub);
    put(&mut state, &sys(&hub.outer[0]), "carrier", &a(), 1);
    put(&mut state, &sys(&hub.outer[1]), "carrier", &b(), 1);
    let (mut game, seen) = action_game(
        state,
        "a",
        Some(hub.galaxy.clone()),
        &["component|trade|sol"],
    );
    steps(&mut game, 2);
    assert!(
        was_offered_like(&seen, "nanolink"),
        "the note can be lent: {:?}",
        asked(&seen)
    );
}

#[test]
fn ralnelbt_1_data_skimmer_keeps_other_players_discards_and_the_holder_takes_one_when_passing() {
    // Sol plays a component action card, which would be discarded: it goes on the Data Skimmer
    // instead. The Ral Nel passes and takes it.
    let mut state = ralnel_game();
    state.phase = ti4_model::state::Phase::Action;
    state.player_mut(&a()).unwrap().breakthrough =
        Some(ti4_model::id::BreakthroughId::new("ralnelbt"));
    state.player_mut(&a()).unwrap().action_cards.clear();
    state.player_mut(&b()).unwrap().action_cards = pile(&["mining_initiative"]);
    let card = ti4_model::id::ActionCardId::new("mining_initiative");
    let (mut game, seen) = action_game(state, "b", None, &["action_card|0"]);
    steps(&mut game, 6);
    assert!(
        game.state.player(&a()).unwrap().action_cards.contains(&card)
            || game.state.player(&a()).unwrap().passed,
        "{:?}",
        asked(&seen)
    );
    assert!(
        !game.state.discarded_action_cards.contains(&card),
        "never reached the discard pile"
    );
    assert_eq!(
        game.state.player(&a()).unwrap().action_cards,
        [card],
        "the holder took it when passing: {:?}",
        asked(&seen)
    );
}

#[test]
fn keleresbt_1_i_i_h_q_modernization_gives_neighbours_and_custodia_vigilia_like_the_technology() {
    let (mut state, hub) = mecatol_neighbours();
    state.player_mut(&a()).unwrap().breakthrough =
        Some(ti4_model::id::BreakthroughId::new("keleresbt"));
    let (mut game, seen) = action_game(state, "a", Some(hub.galaxy.clone()), &[]);
    steps(&mut game, 1);
    assert!(
        was_offered(&seen, "component|trade|sol"),
        "neighbours with the player near Mecatol: {:?}",
        asked(&seen)
    );
    assert!(crate::factions::keleres::holds_custodia(&game.state, &a()));
}
