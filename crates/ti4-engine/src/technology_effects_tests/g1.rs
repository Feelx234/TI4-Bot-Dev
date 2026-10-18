//! Technology effect tests, group g1: generic (non-faction) technologies (see mod.rs).

use super::{content, give, pid, seated};
use crate::choice::{FirstOption, Scripted, Table, Window as _};
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
fn x89c4_2_the_bombarded_planet_is_exhausted() {
    let (_, planet) = crate::fixtures::a_placed_planet();
    let (_, with) = bombard_kills(Some("x89c4"), "dreadnought", &[10]);
    assert!(with.exhausted_planets.contains(&planet));
    let (_, without) = bombard_kills(None, "dreadnought", &[10]);
    assert!(!without.exhausted_planets.contains(&planet), "control");
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

// ---- Warfare (red) --------------------------------------------------------------------------

/// A two-player game and the Mecatol system, for space combat.
fn arena() -> (GameState, SystemId) {
    (game(&["a", "b"]), SystemId::new("18"))
}

#[test]
fn md_base_1_superseded_printing_is_not_in_play() {
    assert_superseded("md_base");
}

#[test]
fn md_c1_1_superseded_printing_is_not_in_play() {
    assert_superseded("md_c1");
}

/// A galaxy of a real planet's system plus six plain ones, seat `a` active in the action phase.
fn planet_fixture() -> (GameState, ti4_content::galaxy::Galaxy, Vec<SystemId>, PlanetId) {
    let (planet_system, planet) = crate::fixtures::a_placed_planet();
    let mut ids = vec![planet_system.to_string()];
    ids.extend(crate::fixtures::plain_systems(8).into_iter().filter(|id| *id != planet_system.as_str()).take(6));
    let refs: Vec<&str> = ids.iter().map(String::as_str).collect();
    let galaxy = ti4_content::galaxy::Galaxy::build(content(), &refs, POK, 1).unwrap();
    let mut state = game(&["a", "b"]);
    state.phase = Phase::Action;
    state.active = Some(pid("a"));
    (state, galaxy, ids.into_iter().map(SystemId::new).collect(), planet)
}

#[test]
#[ignore = "MISSING: md (Magen Defense Grid ΩΩ) effect 1 (activation places an infantry with each of the owner's structures): no implementation of 'md' anywhere in crates/ti4-engine/src (only research tests)"]
fn md_1_activating_a_system_with_the_owners_structures_places_infantry() {
    let (mut state, galaxy, ids, planet) = planet_fixture();
    state.system_mut(&ids[0]).set_control(planet.clone(), pid("b"));
    put_on_planet(&mut state, &ids[0], &planet, "pds", &pid("b"), 1);
    give(&mut state, "b", "md");
    let table = Table::with_default(Box::new(Scripted::new([TACTICAL_ACTION_ID.to_owned(), ids[0].to_string(), "done_moving".to_owned()])));
    let mut game = Game::with_table(state, content(), table).with_galaxy(galaxy);
    for _ in 0..40 {
        assert_eq!(game.step().error, None);
        if game.events.iter().any(|e| e == "TACTICAL_ACTION_COMPLETE") {
            break;
        }
    }
    let infantry = game.state.system_state(&ids[0]).on_planet_of(&planet, &pid("b")).iter().filter(|u| u.type_id.as_str() == "infantry").count();
    assert_eq!(infantry, 1, "one infantry beside the PDS");
}

#[test]
#[ignore = "MISSING: md (Magen Defense Grid ΩΩ) effect 2 (hit at the start of ground combat on a planet with the owner's structures): no implementation of 'md' in crates/ti4-engine/src/invasion.rs"]
fn md_2_ground_combat_starts_with_a_free_hit_on_the_attacker() {
    let fight = |owner: bool| {
        let (mut state, _g, ids, planet) = planet_fixture();
        state.system_mut(&ids[0]).set_control(planet.clone(), pid("b"));
        put_on_planet(&mut state, &ids[0], &planet, "pds", &pid("b"), 1);
        put_on_planet(&mut state, &ids[0], &planet, "infantry", &pid("b"), 1);
        put_on_planet(&mut state, &ids[0], &planet, "infantry", &pid("a"), 1);
        if owner {
            give(&mut state, "b", "md");
        }
        // Nobody ever hits, so only the card can kill anything.
        let mut dice = crate::dice::Dice::from_faces(vec![1u32; 64]);
        let mut rng = crate::rng::GameRng::new(1);
        let mut table = Table::with_default(Box::new(FirstOption));
        crate::invasion::ground_combat(&mut state, content(), POK, &mut table, &mut dice, &mut rng, &ids[0], &planet, &pid("a")).unwrap();
        state.system_state(&ids[0]).on_planet_of(&planet, &pid("a")).len()
    };
    assert_eq!(fight(false), 1, "control: both sides miss forever");
    assert_eq!(fight(true), 0, "the grid's hit kills the lone attacker");
}

#[test]
fn da_1_repairs_a_unit_that_did_not_sustain_this_round() {
    // The defender's cruiser hits once (round 1) and the attacker assigns it to the cruiser; the
    // dreadnought that was already damaged is repaired by Duranium Armor and so survives the
    // second round's hit by sustaining again. Without the card the damaged dreadnought dies.
    let survivors = |owner: bool| {
        let (mut state, system) = arena();
        put(&mut state, &system, "cruiser", &pid("a"), 1);
        put(&mut state, &system, "dreadnought", &pid("a"), 1);
        let dread = state.system_state(&system).units.iter().find(|u| u.type_id.as_str() == "dreadnought").cloned().unwrap();
        state.system_mut(&system).replace_unit(&dread, dread.sustained());
        put(&mut state, &system, "cruiser", &pid("b"), 1);
        if owner {
            give(&mut state, "a", "da");
        }
        // Defender's single die: hit (10), hit (10), then misses; attacker never hits.
        let mut faces = vec![1u32, 1, 10, 1, 10];
        faces.extend(vec![1u32; 4000]);
        let mut dice = crate::dice::Dice::from_faces(faces);
        let mut rng = crate::rng::GameRng::new(1);
        let mut table = Table::with_default(Box::new(FirstOption));
        let _ = crate::combat::resolve(&mut state, content(), POK, &mut table, &mut dice, &mut rng, &system);
        state.system_state(&system).units_of(&pid("a")).iter().filter(|u| u.type_id.as_str() == "dreadnought").count()
    };
    assert_eq!((survivors(false), survivors(true)), (0, 1));
}

#[test]
fn asc_1_opponent_destroys_a_non_fighter_when_three_non_fighters_attack() {
    let defender_ships = |owner: bool, cruisers: usize| {
        let (mut state, system) = arena();
        put(&mut state, &system, "cruiser", &pid("a"), cruisers);
        put(&mut state, &system, "destroyer", &pid("b"), 1);
        if owner {
            give(&mut state, "a", "asc");
        }
        let mut table = Table::with_default(Box::new(FirstOption));
        let mut dice = crate::dice::Dice::new();
        let mut rng = crate::rng::GameRng::new(3);
        crate::combat::resolve(&mut state, content(), POK, &mut table, &mut dice, &mut rng, &system).unwrap();
        (crate::combat::ships_of(&state, content(), POK, &pid("b"), &system).len(), dice.rolled("space combat").len())
    };
    assert_eq!(defender_ships(true, 3), (0, 0), "destroyed before any die");
    assert_ne!(defender_ships(false, 3).1, 0, "control: without the card the dice roll");
    assert_ne!(defender_ships(true, 2).1, 0, "two ships are not enough");
}

#[test]
#[ignore = "MISSING: aida effect 1 (ignore 1 prerequisite when researching a unit upgrade): no 'aida' check in technology.rs prerequisites_met/research; only effect 2 (production discount) exists"]
fn aida_1_ignores_one_prerequisite_when_researching_a_unit_upgrade() {
    // Cruiser II needs green, yellow and red; own two of them and the card stands in for the third.
    let a = pid("a");
    let cr2 = TechnologyId::new("cr2");
    let mut state = game(&["a"]);
    for t in ["nm", "st"] {
        give(&mut state, "a", t); // biotic (nm) and cybernetic (st)
    }
    assert!(!crate::technology::prerequisites_met(&state, content(), POK, &a, &cr2), "control: red missing");
    give(&mut state, "a", "aida");
    assert!(crate::technology::research(&mut state, content(), POK, &a, &cr2), "aida waives the missing prerequisite");
    assert!(state.player(&a).unwrap().exhausted_technologies.contains(&TechnologyId::new("aida")));
}

/// The cost of a carrier in a fresh production window, after `production_used` has set the discount.
fn carrier_cost_after_production(techs: &[&str], answers: &[&str]) -> (i64, GameState) {
    let (mut state, system, planet) = {
        let mut state = game(&["a"]);
        let (system, planet) = crate::fixtures::a_placed_planet();
        state.system_mut(&system).set_control(planet.clone(), pid("a"));
        put_on_planet(&mut state, &system, &planet, "spacedock", &pid("a"), 1);
        (state, system, planet)
    };
    let _ = planet;
    for t in techs {
        give(&mut state, "a", t);
    }
    let mut table = scripted(answers);
    crate::technology::production_used(&mut state, content(), POK, None, &mut table, &pid("a")).unwrap();
    let mut window = crate::production::ProductionWindow::new(&state, content(), POK, &pid("a"), &system);
    window.refresh(&state, content(), POK);
    let choice = window.pending_choice(&state, content(), POK).expect("a production choice");
    let carrier = choice.options.iter().find(|o| o.id.starts_with("build|carrier|")).expect("carrier");
    (carrier.payload.get("cost").and_then(serde_json::Value::as_i64).unwrap(), state)
}

#[test]
fn aida_2_exhaust_to_reduce_production_cost_by_owned_unit_upgrades() {
    assert_eq!(carrier_cost_after_production(&[], &[]).0, 3, "control");
    let (cost, state) = carrier_cost_after_production(&["aida", "cr2", "dn2"], &["exhaust"]);
    assert_eq!(cost, 1, "3 minus two owned unit upgrades");
    assert!(state.player(&pid("a")).unwrap().exhausted_technologies.contains(&TechnologyId::new("aida")));
}

#[test]
#[ignore = "MISSING: sar effect 1 (place a mech after PRODUCTION): Self-Assembly Routines has no engine effect on main; fixed by branch fb-warfare-mech-production (rules version 13)"]
fn sar_1_production_lets_the_owner_place_a_mech_on_a_planet_in_that_system() {
    let mut state = seated(&[("a", "arborec"), ("b", "letnev")]);
    let a = pid("a");
    let home = state.player(&a).unwrap().home_system.clone().unwrap();
    give(&mut state, "a", "sar");
    state.player_mut(&a).unwrap().trade_goods = 10;
    let mechs = |s: &GameState| {
        let types = ti4_content::units::catalogue(content(), POK);
        s.system_state(&home).planet_units.values().flatten().filter(|u| u.owner == a && types.get(u.type_id.as_str()).is_some_and(|k| k.base_type() == "mech")).count()
    };
    let before = mechs(&state);
    // Agree to every offer that mentions the card, otherwise take the first non-decline option.
    struct Eager;
    impl crate::choice::Decider for Eager {
        fn choose(&mut self, choice: &crate::choice::Choice) -> Result<crate::choice::ChoiceOption, crate::choice::IllegalChoice> {
            let pick = choice.options.iter().find(|o| o.id.contains("sar")).or_else(|| choice.options.iter().find(|o| !o.is_decline())).or(choice.options.first());
            Ok(pick.expect("an option").clone())
        }
    }
    let mut table = Table::with_default(Box::new(Eager));
    crate::production::produce_one(&mut state, content(), POK, None, &mut table, &a, &home).unwrap();
    assert_eq!(mechs(&state), before + 1);
}

#[test]
#[ignore = "MISSING: sar effect 2 (gain 1 trade good when a mech is destroyed): no engine effect on main; fixed by branch fb-warfare-mech-production (rules version 13)"]
fn sar_2_a_destroyed_mech_pays_one_trade_good() {
    let tg_after = |owner: bool| {
        let mut state = seated(&[("a", "arborec"), ("b", "letnev")]);
        let a = pid("a");
        if owner {
            give(&mut state, "a", "sar");
        }
        let home = state.player(&a).unwrap().home_system.clone().unwrap();
        let before = state.player(&a).unwrap().trade_goods;
        let mut resolver = crate::fixtures::armed_resolver(&state);
        let mut dice = crate::dice::Dice::new();
        let mut rng = crate::rng::GameRng::new(0);
        let mut sequence = crate::event::EventSequence::new();
        let mut table = Table::with_default(Box::new(FirstOption));
        let mut context = crate::timing::TimingContext { state: &mut state, content: content(), sources: POK, table: &mut table, dice: &mut dice, rng: &mut rng, event_sequence: &mut sequence, galaxy: None };
        let payload = [("player", "a"), ("unit", "arborec_mech"), ("system", home.as_str()), ("planet", "nestphar")].iter().map(|(k, v)| ((*k).to_owned(), serde_json::Value::from(*v))).collect();
        let event = context.event_sequence.next("GROUND_FORCE_DESTROYED", payload).expect("an event id");
        resolver.emit_with_context(&mut context, event, |_, _| {}).expect("resolves");
        state.player(&a).unwrap().trade_goods - before
    };
    assert_eq!(tg_after(true), 1);
    assert_eq!(tg_after(false), 0);
}
