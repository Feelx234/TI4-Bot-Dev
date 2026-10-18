//! Technology effect tests, group g3 (faction techs; see mod.rs and docs/TECH_EFFECT_COVERAGE.md).
//!
//! Faction technologies are mostly timing abilities armed per seat, so the tests drive them the
//! way the game does: an event through [`crate::fixtures::armed_resolver`] with a scripted table,
//! or the component-action entry points.

use std::collections::BTreeMap;

use ti4_content::units::catalogue;
use ti4_model::content_types::{DEFAULT, POK};
use ti4_model::id::{PlanetId, PlayerId, SystemId};
use ti4_model::state::GameState;

use super::{content, give, owns, pid, seated};
use crate::choice::{Scripted, Table};
use crate::fixtures::{put, put_on_planet};

fn a() -> PlayerId {
    pid("a")
}
fn b() -> PlayerId {
    pid("b")
}

fn scripted(answers: &[&str]) -> Table {
    Table::with_default(Box::new(Scripted::new(
        answers.iter().map(|answer| (*answer).to_owned()),
    )))
}

/// Emit one typed event through a resolver armed exactly as the game arms it.
fn emit_with(
    state: &mut GameState,
    table: &mut Table,
    faces: &[u32],
    event_type: &str,
    pairs: &[(&str, &str)],
) {
    let mut resolver = crate::fixtures::armed_resolver(state);
    let mut dice = crate::dice::Dice::from_faces(faces.iter().copied());
    let mut rng = crate::rng::GameRng::new(0);
    let mut sequence = crate::event::EventSequence::new();
    let mut context = crate::timing::TimingContext {
        state,
        content: content(),
        sources: POK,
        table,
        dice: &mut dice,
        rng: &mut rng,
        event_sequence: &mut sequence,
        galaxy: None,
    };
    let payload: BTreeMap<String, serde_json::Value> = pairs
        .iter()
        .map(|(key, value)| ((*key).to_owned(), serde_json::Value::from(*value)))
        .collect();
    let event = context
        .event_sequence
        .next(event_type, payload)
        .expect("an event id");
    resolver
        .emit_with_context(&mut context, event, |_, _| {})
        .expect("the window resolves");
}

fn emit(state: &mut GameState, answers: &[&str], event_type: &str, pairs: &[(&str, &str)]) {
    emit_with(state, &mut scripted(answers), &[], event_type, pairs);
}

fn on_planet(state: &GameState, system: &SystemId, planet: &str, who: &PlayerId, kind: &str) -> usize {
    state
        .system_state(system)
        .on_planet_of(&PlanetId::new(planet), who)
        .into_iter()
        .filter(|unit| unit.type_id.as_str() == kind)
        .count()
}

fn in_space(state: &GameState, system: &SystemId, who: &PlayerId, kind: &str) -> usize {
    state
        .system_state(system)
        .units
        .iter()
        .filter(|unit| &unit.owner == who && unit.type_id.as_str() == kind)
        .count()
}

/// Strip a seat of its units and planets so only what a test adds exists.
fn bare(state: &mut GameState, who: &PlayerId) {
    state.board.values_mut().for_each(|board| {
        board.planet_control.retain(|_, owner| owner != who);
        board
            .planet_units
            .values_mut()
            .for_each(|units| units.retain(|unit| &unit.owner != who));
        board.units.retain(|unit| &unit.owner != who);
    });
}

/// The unit card `id` as the catalogue prints it.
fn stats(id: &str) -> ti4_content::units::UnitType<'static> {
    *catalogue(content(), DEFAULT)
        .get(id)
        .unwrap_or_else(|| panic!("unit {id} in the catalogue"))
}

// -- Bioplasmosis ---------------------------------------------------------------------------------

#[test]
fn bio_1_moves_infantry_between_planets_at_the_end_of_the_status_phase() {
    const BIO: &str = "technology:arborec:bio:STATUS_PHASE_ENDED:after";
    let (system, _) = crate::fixtures::a_placed_planet();
    let run = |with_tech: bool| {
        let mut state = seated(&[("a", "arborec"), ("b", "sol")]);
        bare(&mut state, &a());
        if with_tech {
            give(&mut state, "a", "bio");
        }
        for (planet, n) in [("src", 2), ("dst", 0)] {
            state.system_mut(&system).set_control(PlanetId::new(planet), a());
            put_on_planet(&mut state, &system, &PlanetId::new(planet), "arborec_infantry", &a(), n);
        }
        emit(&mut state, &[BIO, "move|0", "move|0"], "STATUS_PHASE_ENDED", &[]);
        (
            on_planet(&state, &system, "src", &a(), "arborec_infantry"),
            on_planet(&state, &system, "dst", &a(), "arborec_infantry"),
        )
    };
    assert_eq!(run(true), (0, 2), "both infantry were moved");
    assert_eq!(run(false), (2, 0), "without the card nothing is offered");
}

// -- Aerie Hololattice ----------------------------------------------------------------------------

fn hololattice(with_tech: bool) -> (GameState, SystemId, PlanetId) {
    let mut state = seated(&[("a", "argent"), ("b", "sol")]);
    let (system, planet) = crate::fixtures::a_placed_planet();
    if with_tech {
        give(&mut state, "a", "ah");
    }
    put_on_planet(&mut state, &system, &planet, "pds", &a(), 1);
    (state, system, planet)
}

#[test]
fn ah_1_other_players_cannot_move_ships_through_systems_with_your_structures() {
    let blocks = |with_tech: bool, mover: &PlayerId| {
        let (state, system, _) = hololattice(with_tech);
        crate::factions::hooks_movement::blocks_passage(&state, content(), POK, mover, &system)
    };
    assert!(blocks(true, &b()));
    assert!(!blocks(true, &a()), "the owner passes");
    assert!(!blocks(false, &b()), "no card, no wall");
}

#[test]
fn ah_2_a_planet_with_your_structure_gains_production_one() {
    let production = |with_tech: bool, who: &PlayerId| {
        let (state, system, planet) = hololattice(with_tech);
        crate::factions::hooks_economy::extra_production_planet(
            &state, content(), POK, who, &system, &planet,
        )
    };
    assert_eq!(production(true, &a()), 1);
    assert_eq!(production(true, &b()), 0);
    assert_eq!(production(false, &a()), 0);
}

// -- Non-Euclidean Shielding ----------------------------------------------------------------------

#[test]
fn nes_1_one_sustain_cancels_two_hits() {
    let run = |with_tech: bool| {
        let mut state = seated(&[("a", "letnev"), ("b", "sol")]);
        let system = SystemId::new("19");
        put(&mut state, &system, "dreadnought", &a(), 1);
        put(&mut state, &system, "cruiser", &a(), 1);
        if with_tech {
            give(&mut state, "a", "nes");
        }
        let mut table = Table::with_default(Box::new(crate::choice::FirstOption));
        crate::combat::absorb_hits(&mut state, content(), POK, &mut table, &a(), &system, &b(), 2)
            .expect("hits resolve");
        (
            state.system_state(&system).units_of(&a()).len(),
            state.system_state(&system).units.iter().filter(|u| u.sustained_damage).count(),
        )
    };
    assert_eq!(run(true), (2, 1), "the dreadnought alone absorbs both hits");
    assert_eq!(run(false).0, 1, "without the card the second hit costs a ship");
}

// -- Floating Factory II --------------------------------------------------------------------------

#[test]
fn ffac2_1_has_move_two_and_capacity_five_and_replaces_the_first_factory() {
    let (old, new) = (stats("saar_spacedock"), stats("saar_spacedock2"));
    assert_eq!((old.move_value(), old.capacity()), (1, 4));
    assert_eq!((new.move_value(), new.capacity()), (2, 5));
    assert_eq!(new.upgrades_from(), Some("saar_spacedock"));
    assert_eq!(new.required_technology(), Some("ffac2"));
    let mut state = seated(&[("a", "saar"), ("b", "sol")]);
    let system = SystemId::new("19");
    put(&mut state, &system, "saar_spacedock", &a(), 1);
    give(&mut state, "a", "ffac2");
    crate::technology::apply_unit_upgrades(&mut state, content(), DEFAULT, &a());
    assert_eq!(in_space(&state, &system, &a(), "saar_spacedock2"), 1);
    assert_eq!(in_space(&state, &system, &a(), "saar_spacedock"), 0);
}

#[test]
fn ffac2_2_production_is_seven() {
    assert_eq!(stats("saar_spacedock2").production(0), 7);
    assert_eq!(stats("saar_spacedock").production(0), 5);
}

#[test]
fn ffac2_3_is_placed_in_the_space_area() {
    assert!(stats("saar_spacedock2").is_space_only_structure());
}

#[test]
fn ffac2_4_moves_like_a_ship() {
    let mut state = seated(&[("a", "saar"), ("b", "sol")]);
    give(&mut state, "a", "ffac2");
    let hub = crate::fixtures::plain_hub();
    let origin = SystemId::new(hub.outer[0].clone());
    let active = SystemId::new(hub.centre.clone());
    put(&mut state, &origin, "saar_spacedock2", &a(), 1);
    let moves = crate::tactical::movable_into(&state, content(), DEFAULT, &hub.galaxy, &a(), &active);
    assert!(stats("saar_spacedock2").moves_as_ship());
    assert!(moves.iter().any(|m| m.origin == origin && m.unit.type_id.as_str() == "saar_spacedock2"));
}

#[test]
fn ffac2_5_is_destroyed_when_blockaded() {
    let mut state = seated(&[("a", "saar"), ("b", "sol")]);
    let system = SystemId::new("19");
    put(&mut state, &system, "saar_spacedock2", &a(), 1);
    put(&mut state, &system, "cruiser", &b(), 1);
    let gone = crate::production::destroy_blockaded_mobile_docks(&mut state, content(), DEFAULT, &system);
    assert_eq!(gone.len(), 1);
    assert_eq!(in_space(&state, &system, &a(), "saar_spacedock2"), 0);
}

// -- Magmus Reactor -------------------------------------------------------------------------------

#[test]
fn mr_1_ships_can_move_into_supernovas() {
    let reach = |with_tech: bool| {
        let mut state = seated(&[("a", "muaat"), ("b", "sol")]);
        if with_tech {
            give(&mut state, "a", "mr");
        }
        let supernova = crate::fixtures::a_system_where("supernova");
        let hub = crate::fixtures::hub_with_centre(&supernova);
        let origin = SystemId::new(hub.outer[0].clone());
        put(&mut state, &origin, "cruiser", &a(), 1);
        crate::tactical::activate(&mut state, &a(), &SystemId::new(hub.centre.clone())).unwrap();
        crate::tactical::movable(&state, content(), POK, &hub.galaxy, &a()).len()
    };
    assert!(reach(true) > 0);
    assert_eq!(reach(false), 0);
}

#[test]
fn mr_2_a_supernova_with_your_units_gains_production_five() {
    let production = |with_tech: bool, units: bool| {
        let mut state = seated(&[("a", "muaat"), ("b", "sol")]);
        if with_tech {
            give(&mut state, "a", "mr");
        }
        let supernova = SystemId::new(crate::fixtures::a_system_where("supernova"));
        if units {
            put(&mut state, &supernova, "cruiser", &a(), 1);
        }
        crate::factions::hooks_economy::extra_production(&state, content(), POK, &a(), &supernova)
    };
    assert_eq!(production(true, true), 5);
    assert_eq!(production(true, false), 0, "needs a unit of yours there");
    assert_eq!(production(false, true), 0);
}

// -- Production Biomes ----------------------------------------------------------------------------

#[test]
fn pm_1_exhaust_and_a_token_for_four_trade_goods_and_two_to_another_player() {
    let mut state = seated(&[("a", "hacan"), ("b", "sol")]);
    let offered = |state: &GameState| {
        crate::faction_abilities::component_actions(state, content(), &a())
            .into_iter()
            .find(|option| option.id == "faction|production_biomes")
    };
    assert!(offered(&state).is_none(), "not without the card");
    give(&mut state, "a", "pm");
    state.player_mut(&a()).unwrap().strategic_tokens = 1;
    state.player_mut(&a()).unwrap().trade_goods = 0;
    state.player_mut(&b()).unwrap().trade_goods = 0;
    let option = offered(&state).expect("offered with the card and a token");
    let mut table = scripted(&["b"]);
    let done = crate::fixtures::with_context(&mut state, POK, None, &mut table, |context| {
        crate::faction_abilities::perform_component(context, &a(), &option)
    });
    assert!(done);
    let seat = state.player(&a()).unwrap();
    assert_eq!((seat.trade_goods, seat.strategic_tokens), (4, 0));
    assert_eq!(state.player(&b()).unwrap().trade_goods, 2);
    assert!(offered(&state).is_none(), "exhausted");
    assert!(owns(&state, "a", "pm"));
}
