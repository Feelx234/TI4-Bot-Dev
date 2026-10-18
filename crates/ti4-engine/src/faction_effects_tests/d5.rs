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
    assert_eq!(
        crate::supply::captured_by(&game.state, &a()),
        [(b(), ti4_model::id::UnitTypeId::new("cruiser"))],
        "the 3 captured one cruiser"
    );
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
