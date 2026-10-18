//! Technology effect tests, group g2: faction technologies `lw2` .. `mi` (see mod.rs).

use ti4_content::units::UnitType;
use ti4_model::content_types::POK;
use ti4_model::id::SystemId;

use std::collections::BTreeSet;

use ti4_content::galaxy::Galaxy;
use ti4_model::id::{PlanetId, TechnologyId};
use ti4_model::state::{GameState, RerollEntry, RerollSet};

use super::{content, give, owns, pid, seated};
use crate::fixtures::put;

/// An ordinary system holding no units yet.
fn empty_system(state: &ti4_model::state::GameState) -> SystemId {
    let id = crate::fixtures::plain_systems(60)
        .into_iter()
        .find(|id| state.board.get(&SystemId::new(id.as_str())).is_none_or(|s| s.units.is_empty() && s.planet_units.values().all(Vec::is_empty)))
        .expect("an empty system");
    SystemId::new(id)
}

/// Seat `a` of `faction` with one of the faction's own `base` units on the board; assert that
/// without `tech` the unit stays the printed one and that with it the card replaces it. Returns
/// the upgraded unit type.
fn upgrade(faction: &str, tech: &str, base: &str) -> UnitType<'static> {
    let mut state = seated(&[("a", faction)]);
    let system = empty_system(&state);
    let printed = ti4_content::units::catalogue(content(), POK)
        .values()
        .find(|u| u.base_type() == base && u.faction() == Some(faction) && u.required_technology().is_none())
        .expect("the faction has its own printed unit")
        .id()
        .to_owned();
    put(&mut state, &system, &printed, &pid("a"), 2);
    assert!(!owns(&state, "a", tech), "{faction} does not start with {tech}");
    crate::technology::apply_unit_upgrades(&mut state, content(), POK, &pid("a"));
    assert!(
        state.system_state(&system).units.iter().all(|u| u.type_id.as_str() == printed),
        "without {tech} the printed unit {printed} stays: {:?}", state.system_state(&system).units.iter().map(|u| u.type_id.to_string()).collect::<Vec<_>>()
    );
    give(&mut state, "a", tech);
    crate::technology::apply_unit_upgrades(&mut state, content(), POK, &pid("a"));
    let now: Vec<String> = state
        .system_state(&system)
        .units
        .iter()
        .map(|u| u.type_id.to_string())
        .collect();
    assert_eq!(now.len(), 2);
    assert!(now.iter().all(|id| *id != printed), "{tech} replaces {printed}: {now:?}");
    let upgraded = now[0].clone();
    ti4_content::units::unit_type(content(), &upgraded, POK).expect("the upgraded unit exists")
}

/// Cost, combat value and dice.
fn assert_combat(unit: &UnitType<'_>, cost: f64, hits_on: i64, dice: i64) {
    assert!((unit.cost() - cost).abs() < 1e-9, "cost {} != {cost}", unit.cost());
    assert_eq!((unit.combat_hits_on(), unit.combat_dice()), (Some(hits_on), dice));
}

/// Move and capacity.
fn assert_mobility(unit: &UnitType<'_>, move_value: i64, capacity: i64) {
    assert_eq!((unit.move_value(), unit.capacity()), (move_value, capacity));
}

#[test]
fn lw2_1_letani_warrior_ii_stats() {
    let unit = upgrade("arborec", "lw2", "infantry");
    assert_combat(&unit, 0.5, 7, 1);
    assert!(unit.produces_two(), "Cost 1(x2)");
}

#[test]
fn swa2_1_strike_wing_alpha_ii_stats() {
    let unit = upgrade("argent", "swa2", "destroyer");
    assert_combat(&unit, 1.0, 7, 1);
    assert_mobility(&unit, 2, 1);
}

#[test]
fn swa2_2_strike_wing_alpha_ii_anti_fighter_barrage() {
    let unit = upgrade("argent", "swa2", "destroyer");
    assert_eq!((unit.afb_hits_on(), unit.afb_dice()), (Some(6), 3));
}

#[test]
fn pws2_1_prototype_war_sun_ii_stats() {
    let unit = upgrade("muaat", "pws2", "warsun");
    assert_combat(&unit, 10.0, 3, 3);
    assert_mobility(&unit, 3, 6);
}

#[test]
fn pws2_2_prototype_war_sun_ii_sustain_and_bombardment() {
    let unit = upgrade("muaat", "pws2", "warsun");
    assert!(unit.sustain_damage());
    assert_eq!((unit.bombard_hits_on(), unit.bombard_dice()), (Some(3), 3));
}

#[test]
fn ac2_1_advanced_carrier_ii_stats() {
    let unit = upgrade("sol", "ac2", "carrier");
    assert_combat(&unit, 3.0, 9, 1);
    assert_mobility(&unit, 2, 8);
}

#[test]
fn ac2_2_advanced_carrier_ii_sustain_damage() {
    assert!(upgrade("sol", "ac2", "carrier").sustain_damage());
}

#[test]
fn sdn2_1_super_dreadnought_ii_stats() {
    let unit = upgrade("l1z1x", "sdn2", "dreadnought");
    assert_combat(&unit, 4.0, 4, 1);
    assert_mobility(&unit, 2, 2);
}

#[test]
fn sdn2_2_super_dreadnought_ii_sustain_and_bombardment() {
    let unit = upgrade("l1z1x", "sdn2", "dreadnought");
    assert!(unit.sustain_damage());
    assert_eq!((unit.bombard_hits_on(), unit.bombard_dice()), (Some(4), 1));
}

#[test]
fn cl2_1_crimson_legionnaire_ii_stats() {
    let unit = upgrade("mahact", "cl2", "infantry");
    assert_combat(&unit, 0.5, 7, 1);
    assert!(unit.produces_two());
}

#[test]
fn hcf2_1_hybrid_crystal_fighter_ii_stats() {
    let unit = upgrade("naalu", "hcf2", "fighter");
    assert_combat(&unit, 0.5, 7, 1);
    assert_eq!(unit.move_value(), 2);
}

#[test]
fn m2_1_memoria_ii_stats() {
    let unit = upgrade("nomad", "m2", "flagship");
    assert_combat(&unit, 8.0, 5, 2);
    assert_mobility(&unit, 2, 6);
}

#[test]
fn m2_2_memoria_ii_sustain_and_anti_fighter_barrage() {
    let unit = upgrade("nomad", "m2", "flagship");
    assert!(unit.sustain_damage());
    assert_eq!((unit.afb_hits_on(), unit.afb_dice()), (Some(5), 3));
}

#[test]
fn exo2_1_exotrireme_ii_stats() {
    let unit = upgrade("sardakk", "exo2", "dreadnought");
    assert_combat(&unit, 4.0, 5, 1);
    assert_mobility(&unit, 2, 1);
}

#[test]
fn exo2_2_exotrireme_ii_sustain_and_bombardment() {
    let unit = upgrade("sardakk", "exo2", "dreadnought");
    assert!(unit.sustain_damage());
    assert_eq!((unit.bombard_hits_on(), unit.bombard_dice()), (Some(4), 2));
}

#[test]
fn se2_1_saturn_engine_ii_stats() {
    let unit = upgrade("titans", "se2", "cruiser");
    assert_combat(&unit, 2.0, 6, 1);
    assert_mobility(&unit, 3, 2);
}

#[test]
fn se2_2_saturn_engine_ii_sustain_damage() {
    assert!(upgrade("titans", "se2", "cruiser").sustain_damage());
}

#[test]
fn dt2_1_dimensional_tear_ii_production_seven() {
    let unit = upgrade("cabal", "dt2", "spacedock");
    assert_eq!(unit.production(2), 7);
}


// -- shared helpers ---------------------------------------------------------------------------

/// Emit one typed event through a resolver armed exactly as the game arms it, answering prompts
/// from `answers` (then declining) and rolling `faces` first. Returns the dice rolled.
fn emit(
    state: &mut GameState,
    answers: &[&str],
    faces: &[u32],
    galaxy: Option<&Galaxy>,
    event_type: &str,
    pairs: &[(&str, &str)],
) -> usize {
    let mut resolver = crate::fixtures::armed_resolver(state);
    let mut table = crate::choice::Table::with_default(Box::new(
        crate::choice::Scripted::with_fallback(
            answers.iter().map(|a| (*a).to_owned()),
            Box::new(crate::choice::AlwaysDecline),
        ),
    ));
    let mut dice = crate::dice::Dice::from_faces(faces.iter().copied());
    let mut rng = crate::rng::GameRng::new(0);
    let mut sequence = crate::event::EventSequence::new();
    let mut context = crate::timing::TimingContext {
        state,
        content: content(),
        sources: POK,
        table: &mut table,
        dice: &mut dice,
        rng: &mut rng,
        event_sequence: &mut sequence,
        galaxy,
    };
    let payload = pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), serde_json::Value::from(*v)))
        .collect();
    let event = context.event_sequence.next(event_type, payload).expect("an event id");
    resolver
        .emit_with_context(&mut context, event, |_, _| {})
        .expect("the window resolves");
    context.dice.history().len()
}

/// How many of `player`'s `kind` units stand in the space area of `system`.
fn count(state: &GameState, system: &SystemId, player: &str, kind: &str) -> usize {
    state
        .system_state(system)
        .units
        .iter()
        .filter(|u| u.owner == pid(player) && u.type_id.as_str() == kind)
        .count()
}

/// How many of `player`'s `kind` units stand on `planet` in `system`.
fn count_on(state: &GameState, system: &SystemId, planet: &PlanetId, player: &str, kind: &str) -> usize {
    state
        .system_state(system)
        .on_planet_of(planet, &pid(player))
        .iter()
        .filter(|u| u.type_id.as_str() == kind)
        .count()
}

/// The seat's home system and its first controlled planet there.
fn home_planet(state: &GameState, player: &str) -> (SystemId, PlanetId) {
    let home = state.player(&pid(player)).and_then(|s| s.home_system.clone()).expect("home");
    let planet = state
        .controlled_planets(&pid(player))
        .into_iter()
        .find(|(system, _)| **system == home)
        .map(|(_, planet)| planet.clone())
        .expect("a home planet");
    (home, planet)
}

/// Stage `faces` as `player`'s just-rolled barrage dice of `unit` in `system`.
fn stage_barrage(state: &mut GameState, player: &str, system: &SystemId, unit: &str, hits_on: u32, faces: &[u32]) {
    state.reroll_staging.insert(
        pid(player),
        RerollSet {
            kind: "anti_fighter_barrage".to_owned(),
            system: system.clone(),
            rolls: vec![RerollEntry {
                unit: unit.to_owned(),
                planet: None,
                hits_on: Some(hits_on),
                faces: faces.to_vec(),
                rerolled: BTreeSet::new(),
                deltas: std::collections::BTreeMap::new(),
                unit_types: std::iter::once((unit.to_owned(), 1)).collect(),
            }],
        },
    );
}

fn learn(state: &mut GameState, player: &str, tech: &str) {
    state.player_mut(&pid(player)).unwrap().technologies.insert(TechnologyId::new(tech));
}

// -- Arborec: Letani Warrior II -----------------------------------------------------------------

#[test]
fn lw2_2_letani_warrior_ii_has_production_two() {
    let mut state = seated(&[("a", "arborec")]);
    let system = empty_system(&state);
    let planet = PlanetId::new("lw2-test-planet");
    state.system_mut(&system).set_control(planet.clone(), pid("a"));
    crate::fixtures::put_on_planet(&mut state, &system, &planet, "arborec_infantry", &pid("a"), 1);
    let before = crate::production::capacity(&state, content(), POK, &pid("a"), &system);
    assert_eq!(before, 1, "the printed Letani Warrior has PRODUCTION 1");
    state.system_mut(&system).planet_units.get_mut(&planet).unwrap().clear();
    crate::fixtures::put_on_planet(&mut state, &system, &planet, "arborec_infantry2", &pid("a"), 1);
    assert_eq!(crate::production::capacity(&state, content(), POK, &pid("a"), &system), 2);
}

#[test]
fn lw2_3_letani_warrior_ii_returns_home_after_a_six() {
    let (mut state, home, planet) = {
        let state = seated(&[("a", "arborec"), ("b", "sol")]);
        let (home, planet) = home_planet(&state, "a");
        (state, home, planet)
    };
    learn(&mut state, "a", "lw2");
    let pairs = [("player", "a"), ("unit", "arborec_infantry2"), ("system", home.as_str()), ("planet", planet.as_str())];
    let before = count_on(&state, &home, &planet, "a", "arborec_infantry2");
    assert_eq!(emit(&mut state, &[], &[6], None, "GROUND_FORCE_DESTROYED", &pairs), 1);
    emit(&mut state, &[], &[], None, "TURN_BEGAN", &[("player", "a")]);
    assert_eq!(count_on(&state, &home, &planet, "a", "arborec_infantry2"), before + 1);
    // A 5 does not save it, and a seat without the card does not roll at all.
    let mut miss = seated(&[("a", "arborec"), ("b", "sol")]);
    learn(&mut miss, "a", "lw2");
    emit(&mut miss, &[], &[5], None, "GROUND_FORCE_DESTROYED", &pairs);
    emit(&mut miss, &[], &[], None, "TURN_BEGAN", &[("player", "a")]);
    assert_eq!(count_on(&miss, &home, &planet, "a", "arborec_infantry2"), 0);
    let mut bare = seated(&[("a", "arborec"), ("b", "sol")]);
    assert_eq!(emit(&mut bare, &[], &[6], None, "GROUND_FORCE_DESTROYED", &pairs), 0);
}

// -- Argent: Strike Wing Alpha II ---------------------------------------------------------------

#[test]
fn swa2_3_strike_wing_alpha_ii_barrage_nines_destroy_infantry() {
    let build = |tech: bool| {
        let mut state = seated(&[("a", "argent"), ("b", "sol")]);
        let system = empty_system(&state);
        if tech {
            learn(&mut state, "a", "swa2");
        }
        put(&mut state, &system, "argent_destroyer2", &pid("a"), 1);
        put(&mut state, &system, "cruiser", &pid("b"), 1);
        put(&mut state, &system, "infantry", &pid("b"), 3);
        stage_barrage(&mut state, "a", &system, "argent_destroyer2", 6, &[9, 10, 4]);
        (state, system)
    };
    let (mut state, system) = build(true);
    let pairs = [("player", "a"), ("system", system.as_str()), ("kind", "anti_fighter_barrage")];
    emit(&mut state, &[], &[], None, "UNIT_ABILITY_ROLLED", &pairs);
    assert_eq!(count(&state, &system, "b", "infantry"), 1, "the 9 and the 10 each kill one");
    assert_eq!(count(&state, &system, "b", "cruiser"), 1);
    let (mut state, system) = build(false);
    emit(&mut state, &[], &[], None, "UNIT_ABILITY_ROLLED", &pairs);
    assert_eq!(count(&state, &system, "b", "infantry"), 3, "no card, no kill");
}
