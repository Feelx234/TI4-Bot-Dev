//! Technology effect tests, group g1: generic (non-faction) technologies (see mod.rs).

use super::{content, give, pid, seated};
use crate::choice::{FirstOption, Scripted, Table};
use crate::fixtures::{game, hub_with_centre, plain_hub, put, put_on_planet};
use crate::game::{Game, END_TURN_ID, TACTICAL_ACTION_ID};
use ti4_model::content_types::POK;
use ti4_model::id::{PlanetId, SystemId, TechnologyId};
use ti4_model::state::{GameState, Phase};

/// A two-seat game in the action phase with a seven-system map; seat `a` is active.
fn tactical_fixture() -> (GameState, ti4_content::galaxy::Galaxy, Vec<SystemId>) {
    let mut state = game(&["a", "b"]);
    let ids = crate::fixtures::plain_systems(7);
    let refs: Vec<&str> = ids.iter().map(String::as_str).collect();
    let galaxy = ti4_content::galaxy::Galaxy::build(content(), &refs, POK, 1).unwrap();
    state.phase = Phase::Action;
    state.active = Some(pid("a"));
    (state, galaxy, ids.into_iter().map(SystemId::new).collect())
}

/// Movement rules for `mover` towards `target` on `hub`, with the mover's technology effects.
fn rules_for<'a>(
    hub: &'a crate::fixtures::Hub,
    state: &GameState,
    mover: &str,
    target: &str,
) -> crate::movement::MovementRules<'a> {
    let board = crate::movement::Board::for_player(state, content(), POK, &pid(mover));
    let mut rules = crate::movement::MovementRules::with_laws(
        &hub.galaxy,
        content(),
        POK,
        target,
        board,
        Some(state),
    );
    crate::action_cards::apply_movement_effects(&mut rules, state, &pid(mover));
    rules
}

// ---- Propulsion (blue) ----------------------------------------------------------------------

#[test]
fn amd_1_ships_move_into_and_through_asteroid_fields() {
    let asteroid = crate::fixtures::a_system_where("asteroid field");
    let hub = hub_with_centre(&asteroid);
    let mut state = game(&["a", "b"]);
    let near = hub.outer[0].clone();
    let far = hub.across(&near);
    assert!(!rules_for(&hub, &state, "a", &far).can_enter(&asteroid));
    give(&mut state, "a", "amd");
    let rules = rules_for(&hub, &state, "a", &far);
    assert!(rules.can_enter(&asteroid));
    assert!(rules.can_reach(&near, 2), "through the field to the far side");
    assert!(!rules_for(&hub, &state, "b", &far).can_enter(&asteroid), "only the owner");
}

#[test]
#[ignore = "MISSING: amd effect 2 (-1 to other players' SPACE CANNON rolls against the owner's units): no technology check in combat.rs space_cannon_offense"]
fn amd_2_space_cannon_against_the_owner_gets_minus_one() {
    // A PDS hits on 6: a rolled 6 hits the target, but with the -1 it is a 5 and misses.
    let hits_with = |owner: bool| {
        let (mut state, _galaxy, ids) = tactical_fixture();
        let system = ids[0].clone();
        put(&mut state, &system, "cruiser", &pid("a"), 1);
        put_on_planet(&mut state, &system, &ti4_model::id::PlanetId::new("p"), "pds", &pid("b"), 1);
        if owner {
            give(&mut state, "a", "amd");
        }
        let mut dice = crate::dice::Dice::from_faces([6u32]);
        let mut rng = crate::rng::GameRng::new(1);
        let fired = crate::combat::space_cannon_offense(&mut state, content(), POK, &mut dice, &mut rng, &system, &pid("a"), None);
        fired.iter().map(|(_, hits, _)| *hits).sum::<usize>()
    };
    assert_eq!(hits_with(false), 1, "control: a 6 hits a PDS target");
    assert_eq!(hits_with(true), 0, "Antimass Deflectors: the 6 becomes a 5");
}

#[test]
fn gd_1_adds_one_to_one_ships_move_once_per_activation() {
    let mut state = game(&["a"]);
    let a = pid("a");
    state.activation_seq = 3;
    assert!(!crate::technology::gravity_drive_available(&state, &a), "needs the card");
    give(&mut state, "a", "gd");
    assert!(crate::technology::gravity_drive_available(&state, &a));
    let types = ti4_content::units::catalogue(content(), POK);
    let cruiser = types.get("cruiser").unwrap();
    let origin = SystemId::new("19");
    let plain = crate::tactical::effective_move_value_for_ship(&state, cruiser, &a, &origin, None, false, false);
    let boosted = crate::tactical::effective_move_value_for_ship(&state, cruiser, &a, &origin, None, true, false);
    assert_eq!(boosted, plain + 1);
    assert!(crate::technology::use_gravity_drive(&mut state, &a));
    assert!(!crate::technology::gravity_drive_available(&state, &a), "once per tactical action");
    state.activation_seq = 4;
    assert!(crate::technology::gravity_drive_available(&state, &a), "renews with the next activation");
}

#[test]
fn gd_1_a_ship_one_short_is_offered_with_the_boost_only_for_the_owner() {
    // A carrier (move 1) two systems from the target is out of reach until Gravity Drive.
    let hub = plain_hub();
    let mut state = game(&["a", "b"]);
    let a = pid("a");
    let from = SystemId::new(hub.outer[0].clone());
    let target = SystemId::new(
        hub.outer
            .iter()
            .find(|id| hub.galaxy.distance(id, &hub.outer[0]) == Some(2))
            .cloned()
            .unwrap(),
    );
    put(&mut state, &from, "carrier", &a, 1);
    let movable = |state: &GameState| crate::tactical::movable_into(state, content(), POK, &hub.galaxy, &a, &target);
    assert!(movable(&state).is_empty(), "out of reach without the drive");
    give(&mut state, "a", "gd");
    let found = movable(&state);
    assert_eq!(found.len(), 1);
    assert!(found[0].gravity_drive, "reachable only with the +1");
}

#[test]
fn fl_1_two_actions_in_one_turn() {
    let run = |owner: bool| {
        let (mut state, galaxy, ids) = tactical_fixture();
        if owner {
            give(&mut state, "a", "fl");
        }
        let script = [
            TACTICAL_ACTION_ID.to_owned(),
            ids[0].to_string(),
            "done_moving".to_owned(),
        ];
        let table = Table::with_default(Box::new(Scripted::new(script)));
        let mut game = Game::with_table(state, content(), table).with_galaxy(galaxy);
        let mut second_offered = false;
        for _ in 0..30 {
            if let Some(choice) = game.legal_options()
                && choice.options.first().is_some_and(|o| o.id == END_TURN_ID)
            {
                second_offered = choice.ids().contains(&TACTICAL_ACTION_ID);
                break;
            }
            assert_eq!(game.step().error, None);
        }
        second_offered
    };
    assert!(run(true), "Fleet Logistics offers a second action");
    assert!(!run(false), "without it the turn ends after one");
}

#[test]
fn lwd_1_ships_move_through_systems_with_other_players_ships() {
    let hub = plain_hub();
    let mut state = game(&["a", "b"]);
    let near = hub.outer[0].clone();
    let far = hub.across(&near);
    put(&mut state, &SystemId::new(hub.centre.clone()), "cruiser", &pid("b"), 1);
    assert!(!rules_for(&hub, &state, "a", &far).can_reach(&near, 2));
    give(&mut state, "a", "lwd");
    assert!(rules_for(&hub, &state, "a", &far).can_reach(&near, 2));
}

#[test]
fn det_1_a_tactical_action_in_a_frontier_system_explores_it() {
    let run = |owner: bool| {
        let (mut state, galaxy, ids) = tactical_fixture();
        if owner {
            give(&mut state, "a", "det");
        }
        put(&mut state, &ids[0], "carrier", &pid("a"), 1);
        let table = Table::with_default(Box::new(Scripted::new([
            TACTICAL_ACTION_ID.to_owned(),
            ids[0].to_string(),
            "done_moving".to_owned(),
        ])));
        let mut game = Game::with_table(state, content(), table).with_galaxy(galaxy);
        game.state.frontier_tokens.insert(ids[0].clone());
        for _ in 0..40 {
            assert_eq!(game.step().error, None);
            if game.events.iter().any(|e| e == "TACTICAL_ACTION_COMPLETE") {
                break;
            }
        }
        game.events.iter().any(|e| e == "FRONTIER_EXPLORED")
    };
    assert!(run(true));
    assert!(!run(false));
}

#[test]
fn det_2_ships_retreat_into_adjacent_systems_without_enemies_or_own_presence() {
    let hub = plain_hub();
    let state = {
        let mut s = game(&["a", "b"]);
        put(&mut s, &SystemId::new(hub.centre.clone()), "cruiser", &pid("a"), 1);
        s
    };
    let centre = SystemId::new(hub.centre.clone());
    let empty = SystemId::new(hub.outer[0].clone());
    let retreats = |state: &GameState| {
        crate::combat::eligible_retreats(state, content(), POK, &hub.galaxy, &pid("a"), &centre)
    };
    assert!(!retreats(&state).contains(&empty), "78.7c needs your units or a planet there");
    let det = crate::combat::det_retreat_destinations(&state, content(), POK, &hub.galaxy, &pid("a"), &centre);
    assert!(det.contains(&empty), "Dark Energy Tap's looser set includes it");
    let mut enemy = state.clone();
    put(&mut enemy, &empty, "cruiser", &pid("b"), 1);
    let det = crate::combat::det_retreat_destinations(&enemy, content(), POK, &hub.galaxy, &pid("a"), &centre);
    assert!(!det.contains(&empty), "other players' units still close it");
}

#[test]
fn sr_1_action_produces_one_ship_at_a_space_dock() {
    let mut state = seated(&[("a", "arborec"), ("b", "letnev")]);
    let a = pid("a");
    let home = state.player(&a).unwrap().home_system.clone().unwrap();
    state.player_mut(&a).unwrap().trade_goods = 10;
    let ships = |state: &GameState| state.system_state(&home).units_of(&a).len();
    let before = ships(&state);
    assert!(
        crate::technology::component_actions(&state, content(), POK, &a).is_empty(),
        "no card, no action"
    );
    give(&mut state, "a", "sr");
    let actions = crate::technology::component_actions(&state, content(), POK, &a);
    assert_eq!(actions.len(), 1);
    let mut table = Table::with_default(Box::new(FirstOption));
    let done = crate::technology::perform_component(&mut state, content(), POK, None, &mut table, &a, &actions[0]).unwrap();
    assert!(done);
    assert_eq!(ships(&state), before + 1);
    assert!(state.player(&a).unwrap().exhausted_technologies.contains(&TechnologyId::new("sr")));
    assert!(crate::technology::component_actions(&state, content(), POK, &a).is_empty(), "exhausted");
}

// ---- Biotic (green) -------------------------------------------------------------------------

/// A placed planet carrying a technology specialty of `colour` (lower case, as the corpus has it).
fn specialty_planet(colour: &str) -> (SystemId, PlanetId) {
    ti4_content::galaxy::all_planets(content(), POK)
        .iter()
        .find(|(_, planet)| {
            planet.system_id().is_some()
                && !planet.is_placed_during_play()
                && planet.tech_specialties().iter().any(|s| s.eq_ignore_ascii_case(colour))
        })
        .map(|(id, planet)| (SystemId::new(planet.system_id().unwrap()), PlanetId::new(*id)))
        .expect("the corpus has such a specialty planet")
}

/// Seat `a` controls `planet` in `system`.
fn control(state: &mut GameState, system: &SystemId, planet: &PlanetId) {
    state.system_mut(system).set_control(planet.clone(), pid("a"));
}

fn scripted(answers: &[&str]) -> Table {
    Table::with_default(Box::new(Scripted::new(answers.iter().map(|a| (*a).to_owned()))))
}

#[test]
fn nm_1_draws_two_action_cards_in_the_status_phase() {
    let draws = |owner: bool| {
        let mut state = game(&["a", "b"]);
        state.phase = Phase::Status;
        if owner {
            give(&mut state, "a", "nm");
        }
        let report = crate::status::resolve_status_phase(&mut state).unwrap();
        report.action_cards_drawn.iter().find(|(p, _)| p == &pid("a")).map(|(_, n)| *n)
    };
    assert_eq!(draws(true), Some(2));
    assert_eq!(draws(false), Some(1), "control: one card without it");
}

#[test]
fn dxa_1_the_winner_of_a_ground_combat_places_an_infantry() {
    let mut state = game(&["a"]);
    let winner = pid("a");
    let (system, planet) = crate::fixtures::a_placed_planet();
    assert!(!crate::faction_techs::dacxive_animators(&mut state, content(), POK, &winner, &system, &planet));
    give(&mut state, "a", "dxa");
    assert!(crate::faction_techs::dacxive_animators(&mut state, content(), POK, &winner, &system, &planet));
    assert_eq!(state.system_state(&system).on_planet_of(&planet, &winner).len(), 1);
}

#[test]
fn hm_1_three_command_tokens_in_the_status_phase() {
    let mut state = game(&["a"]);
    let a = pid("a");
    let tokens = |state: &GameState| crate::faction_abilities::status_tokens(state, content(), &a, 2);
    assert_eq!(tokens(&state), 2);
    give(&mut state, "a", "hm");
    assert_eq!(tokens(&state), 3);
}

#[test]
#[ignore = "MISSING: pa effect 1 (technology specialties usable without exhausting, even when exhausted): technology.rs specialties() skips exhausted planets and exhaust_specialties_for_research exhausts them; no pa check"]
fn pa_1_specialties_are_usable_without_exhausting_and_when_exhausted() {
    // Gravity Drive needs one blue; a controlled propulsion specialty stands in for it.
    let (system, planet) = specialty_planet("propulsion");
    let gd = TechnologyId::new("gd");
    let a = pid("a");
    let mut state = game(&["a"]);
    control(&mut state, &system, &planet);
    state.exhausted_planets.insert(planet.clone());
    assert!(!crate::technology::prerequisites_met(&state, content(), POK, &a, &gd), "control: exhausted, no pa");
    give(&mut state, "a", "pa");
    assert!(crate::technology::prerequisites_met(&state, content(), POK, &a, &gd), "pa: an exhausted specialty still counts");
    state.ready_planet(&planet);
    assert!(crate::technology::research(&mut state, content(), POK, &a, &gd));
    assert!(!state.exhausted_planets.contains(&planet), "pa: using the specialty does not exhaust the planet");
}

#[test]
fn pa_2_exhaust_a_specialty_planet_for_a_trade_good() {
    let (system, planet) = specialty_planet("biotic");
    let a = pid("a");
    let run = |owner: bool| {
        let mut state = game(&["a"]);
        control(&mut state, &system, &planet);
        if owner {
            give(&mut state, "a", "pa");
        }
        let before = state.player(&a).unwrap().trade_goods;
        let mut table = scripted(&[planet.as_str()]);
        crate::technology::start_turn(&mut state, content(), POK, None, &mut table, &a).unwrap();
        (state.player(&a).unwrap().trade_goods - before, state.exhausted_planets.contains(&planet))
    };
    assert_eq!(run(true), (1, true));
    assert_eq!(run(false), (0, false));
}

#[test]
fn bs_1_ready_an_exhausted_specialty_planet_or_another_technology() {
    let (system, planet) = specialty_planet("biotic");
    let a = pid("a");
    // The planet branch.
    let mut state = game(&["a"]);
    control(&mut state, &system, &planet);
    state.exhausted_planets.insert(planet.clone());
    give(&mut state, "a", "bs");
    let mut table = scripted(&[&format!("ready|planet|{planet}")]);
    crate::technology::end_turn(&mut state, content(), POK, None, &mut table, &a).unwrap();
    assert!(!state.exhausted_planets.contains(&planet));
    assert!(state.player(&a).unwrap().exhausted_technologies.contains(&TechnologyId::new("bs")), "Bio-Stims exhausts itself");
    // The technology branch.
    let mut state = game(&["a"]);
    give(&mut state, "a", "bs");
    give(&mut state, "a", "sr");
    state.player_mut(&a).unwrap().exhausted_technologies.insert(TechnologyId::new("sr"));
    let mut table = scripted(&["ready|technology|sr"]);
    crate::technology::end_turn(&mut state, content(), POK, None, &mut table, &a).unwrap();
    assert!(!state.player(&a).unwrap().exhausted_technologies.contains(&TechnologyId::new("sr")));
    // Control: without the card nothing is offered.
    let mut state = game(&["a"]);
    state.player_mut(&a).unwrap().exhausted_technologies.insert(TechnologyId::new("sr"));
    crate::technology::end_turn(&mut state, content(), POK, None, &mut scripted(&["ready|technology|sr"]), &a).unwrap();
    assert!(state.player(&a).unwrap().exhausted_technologies.contains(&TechnologyId::new("sr")));
}

/// A superseded printing is not in the active deck, so nobody can own it: nothing to implement.
fn assert_superseded(alias: &str) {
    let active = crate::technology::active_aliases(content(), POK);
    assert!(!active.contains(&TechnologyId::new(alias)), "{alias} is a superseded printing");
    let mut state = game(&["a"]);
    for colour_tech in ["x89c4", "md"] {
        assert!(active.contains(&TechnologyId::new(colour_tech)));
    }
    let offered = crate::technology::researchable(&state, content(), POK, &pid("a"));
    assert!(!offered.contains(&TechnologyId::new(alias)));
    let _ = &mut state;
}

#[test]
fn x89_base_1_superseded_printing_is_not_in_play() {
    assert_superseded("x89_base");
}

#[test]
fn x89_1_superseded_printing_is_not_in_play() {
    assert_superseded("x89");
}

/// A defended planet and an invader bombarding it from space: `n_dice_faces` all hit.
fn bombard_kills(owner_tech: Option<&str>, ship: &str, faces: &[u32]) -> (usize, GameState) {
    let mut state = game(&["a", "b"]);
    let (system, planet) = crate::fixtures::a_placed_planet();
    put_on_planet(&mut state, &system, &planet, "infantry", &pid("b"), 4);
    put(&mut state, &system, ship, &pid("a"), 1);
    if let Some(tech) = owner_tech {
        give(&mut state, "a", tech);
    }
    let mut dice = crate::dice::Dice::from_faces(faces.iter().copied());
    let mut rng = crate::rng::GameRng::new(1);
    let mut table = Table::new();
    let killed = crate::invasion::bombardment(&mut state, content(), POK, &mut dice, &mut rng, &mut table, &system, &pid("a")).unwrap();
    (killed, state)
}

#[test]
fn x89c4_1_doubles_the_hits_of_bombardment_and_ground_combat() {
    // One dreadnought die, one hit: two infantry die with the card, one without.
    assert_eq!(bombard_kills(None, "dreadnought", &[10]).0, 1);
    assert_eq!(bombard_kills(Some("x89c4"), "dreadnought", &[10]).0, 2);
}

#[test]
#[ignore = "MISSING: x89c4 effect 2 (exhaust each planet you use BOMBARDMENT against): bombardment never exhausts the target planet (invasion.rs bombardment)"]
fn x89c4_2_the_bombarded_planet_is_exhausted() {
    let (_, state) = bombard_kills(Some("x89c4"), "dreadnought", &[10]);
    let (_, planet) = crate::fixtures::a_placed_planet();
    assert!(state.exhausted_planets.contains(&planet));
}

#[test]
fn ps_1_one_unit_rolls_an_extra_die_when_bombarding_or_firing_space_cannon() {
    // Bombardment: a single dreadnought rolls 1 die, 2 with Plasma Scoring.
    assert_eq!(bombard_kills(None, "dreadnought", &[10, 10]).0, 1);
    assert_eq!(bombard_kills(Some("ps"), "dreadnought", &[10, 10]).0, 2);
    // Space cannon: a PDS rolls 1 die, 2 with Plasma Scoring.
    let dice_rolled = |owner: bool| {
        let (mut state, _g, ids) = tactical_fixture();
        put(&mut state, &ids[0], "cruiser", &pid("b"), 1);
        put_on_planet(&mut state, &ids[0], &PlanetId::new("p"), "pds", &pid("a"), 1);
        if owner {
            give(&mut state, "a", "ps");
        }
        let mut dice = crate::dice::Dice::new();
        let mut rng = crate::rng::GameRng::new(3);
        crate::combat::space_cannon_offense(&mut state, content(), POK, &mut dice, &mut rng, &ids[0], &pid("b"), None);
        dice.history().iter().map(|roll| roll.faces.len()).sum::<usize>()
    };
    assert_eq!(dice_rolled(false), 1);
    assert_eq!(dice_rolled(true), 2);
}
