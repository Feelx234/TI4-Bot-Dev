//! What cards, factions and laws add to a combat roll, in one place.
//!
//! The combat and invasion windows used to sum these terms inline (faction ability, module hook,
//! Mahact and Nekro unit text, Assail, Fighter Prototype, nebula defence, Last Bastion's extra
//! die). Each function here is the expression that was at the window's call site, moved without
//! change, so a window asks one question per effect and knows nothing about who answers it.

use ti4_content::ContentStore;
use ti4_model::content_types::SourceSet;
use ti4_model::id::{PlanetId, PlayerId, SystemId};
use ti4_model::state::GameState;
use ti4_model::units::Unit;

/// The shift to the *roll* of one space-combat ship of `player` (everything but the morale
/// boost, which the caller counts). The threshold moves the other way: `threshold - shift`.
pub(crate) fn space_roll_shift(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
    unit: &Unit,
    is_fighter: bool,
) -> i64 {
    // A faction shift applies to the *roll*, so it moves the threshold the other way: Sardakk's
    // Unrelenting adds one to each die, which is the same as needing one less.
    let faction = crate::faction_abilities::combat_modifier(state, content, player, "space");
    // Fighter Prototype: "+2 to the result of each of your fighters' combat rolls". One entry
    // per copy, so two cards played in the same round give four.
    let fighter_bonus = if is_fighter {
        crate::combat::fighter_bonus_now(state, player)
    } else {
        0
    };
    // 59.5: "If a space combat occurs in a nebula, the defender applies +1 to each combat roll of
    // their ships during that combat." The defender is whoever is not the active player -- an
    // activation is what starts a space combat, so the attacker is always the active seat.
    //
    // Applied to the threshold rather than the die, like the faction shift above: +1 to the roll is
    // the same as needing one less. Only ships, and the caller is reached only from
    // `fleet_groups`, which iterates `ships_of` -- so ground combat in a nebula is untouched, which
    // is what the rule says.
    let nebula_defender = state
        .active_system
        .as_ref()
        .filter(|_| state.active.as_ref() != Some(player))
        .and_then(|system| ti4_content::galaxy::system(content, system.as_str(), sources))
        .is_some_and(|system| system.is_nebula());

    let module = crate::factions::unit_roll_modifier(
        state,
        content,
        sources,
        &crate::factions::CombatUnit {
            player,
            system: state.active_system.as_ref(),
            planet: None,
            unit_type: unit.type_id.as_str(),
            context: "space",
        },
    );
    // Mahact Arvicon Rex: "+2 to the results of this unit's combat rolls" against an opponent whose
    // command token is not in the owner's fleet pool. A roll bonus, so it lowers the threshold.
    let mahact_flagship = crate::factions::mahact_units::flagship_roll_bonus(
        state,
        content,
        sources,
        player,
        unit.type_id.as_str(),
    );

    // Nekro Mordred: +2 to its rolls against an opponent with an "X" or "Y" assimilator token.
    let nekro_mech = crate::factions::nekro_units::mech_roll_bonus(
        state,
        content,
        sources,
        player,
        unit.type_id.as_str(),
        state.active_system.as_ref(),
        None,
    );

    // Assail (Obsidian plot): +1 to each of the owner's combat rolls against a puppeted player.
    let assail = crate::factions::firmament_plots::assail_space(
        state,
        content,
        sources,
        player,
        state.active_system.as_ref(),
    );

    // Wave C (package CA): the Egeiro and Mordred terms next to the seam, when the dual is on.
    #[cfg(feature = "typed-core")]
    crate::core_factions::ca::dual_roll_shift(state, content, sources, player, unit.type_id.as_str(), state.active_system.as_ref(), None);
    // Wave C (package CC): Arvicon Rex, Assail and the Obsidian commander.
    #[cfg(feature = "typed-core")]
    crate::core_factions::cc::dual_roll_shift(state, content, sources, player, unit.type_id.as_str(), state.active_system.as_ref(), None);
    faction
        + fighter_bonus
        + i64::from(nebula_defender)
        + module
        + mahact_flagship
        + nekro_mech
        + assail
}

/// The shift to the roll of one ground force of `player` on `planet`.
pub(crate) fn ground_roll_shift(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
    system: &SystemId,
    planet: &PlanetId,
    unit_type: &str,
) -> i64 {
    let mut faction = crate::faction_abilities::combat_modifier(state, content, player, "ground");

    // Jol-Nar's Shield Paling mech: "Your infantry on this planet are not affected by your Fragile
    // faction ability." Only infantry, and only on the planet the mech is standing on -- which is
    // why this is decided per planet rather than per seat.
    if faction < 0
        && unit_type.contains("infantry")
        && crate::invasion::shield_paling(state, player, system, planet)
    {
        faction = 0;
    }
    let module = crate::factions::unit_roll_modifier(
        state,
        content,
        sources,
        &crate::factions::CombatUnit {
            player,
            system: Some(system),
            planet: Some(planet),
            unit_type,
            context: "ground",
        },
    );
    // Nekro Mordred: +2 to its rolls against an opponent with an "X" or "Y" assimilator token.
    let nekro_mech = crate::factions::nekro_units::mech_roll_bonus(
        state,
        content,
        sources,
        player,
        unit_type,
        Some(system),
        Some(planet),
    );
    // Assail (Obsidian plot): +1 to each of the owner's combat rolls against a puppeted player.
    let assail = crate::factions::firmament_plots::assail_ground(state, player, system, planet);
    #[cfg(feature = "typed-core")]
    crate::core_factions::ca::dual_roll_shift(state, content, sources, player, unit_type, Some(system), Some(planet));
    #[cfg(feature = "typed-core")]
    crate::core_factions::cc::dual_roll_shift(state, content, sources, player, unit_type, Some(system), Some(planet));
    faction + module + nekro_mech + assail
}

/// The value an anti-fighter barrage of `player` in `system` rolls on, given the printed
/// `value`: Fighter Prototype's bonus for fighters, then Assail, never below 1.
pub(crate) fn barrage_value(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
    system: &SystemId,
    is_fighter: bool,
    value: i64,
) -> i64 {
    // Fighter Prototype names "each of your fighters' combat rolls": the barrage is a
    // fighters' combat roll, so it gets the same +2 per copy as the fleet rolls do.
    let value = if is_fighter {
        value - crate::combat::fighter_bonus_now(state, player)
    } else {
        value
    };
    // Assail: +1 to the owner's unit ability rolls against a puppeted player.
    (value
        - crate::factions::firmament_plots::assail_space(
            state,
            content,
            sources,
            player,
            Some(system),
        ))
    .max(1)
}

/// The extra dice a galvanized unit rolls (Last Bastion): one.
pub(crate) fn galvanized_dice(unit: &Unit) -> usize {
    crate::factions::bastion::extra_die(unit)
}
