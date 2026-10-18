//! Technology effect tests, group g2: faction technologies `lw2` .. `mi` (see mod.rs).

use ti4_content::units::UnitType;
use ti4_model::content_types::POK;
use ti4_model::id::{SystemId, UnitTypeId};

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

#[allow(dead_code, reason = "used by the ability tests below")]
fn _unit_id(id: &str) -> UnitTypeId {
    UnitTypeId::new(id)
}
