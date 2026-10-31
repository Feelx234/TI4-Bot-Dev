//! The names of the LOUD gaps of the live path (`ti4_engine_core_factions::silent::gaps::GAPS`): row `i` is the string ability `GAP_IDS[i]` (its id with the
//! owner's faction left out as `*`). The registry of a native game registers the row for every seat, so the id a row is shown with is the string's with
//! the owner filled in.

use ti4_engine_core::kernel::ability::Slot;

/// The string ability each row stands for, in the order of the rows.
pub(crate) const GAP_IDS: &[&str] = &[
    "ability:*:foresight:MOVEMENT_FINISHED:after",
    "ability:*:propagation:PROPAGATION_RESEARCH:after",
    "ability:*:reclamation:ACTION_COMPLETED:after",
    "ability:*:reclamation_clear:TURN_BEGAN:after",
    "ability:*:reclamation_mark:PLANET_CONTROL_GAINED:after",
    "leader:*:argentcommander:UNIT_ABILITY_ROLLED:when",
    "leader:*:cabalagent:COMMODITIES_REPLENISHED:after",
    "leader:*:empyreanagent:MOVEMENT_FINISHED:after",
    "leader:*:empyreancommander:SHIP_MOVED:after",
    "leader:*:ghostcommander:MOVEMENT_FINISHED:after",
    "leader:*:ghostcommander:SHIP_MOVED:after",
    "leader:*:mentakagent:PILLAGE_USED:after",
    "leader:*:mentakcommander:SPACE_COMBAT_WON:after",
    "leader:*:muaatcommander:STRATEGY_TOKEN_SPENT:after",
    "leader:*:muaatcommander_unlock:UNITS_PRODUCED:after",
    "leader:*:muaathero:MOVEMENT_FINISHED:after",
    "leader:*:muaathero_moved:SHIP_MOVED:after",
    "leader:*:naaluagent-te:COMMAND_TOKEN_PLACED:after",
    "leader:*:naaluhero:STATUS_PHASE_ENDED:after",
    "leader:*:naazagent:TURN_PASSED:after",
    "leader:*:naazcommander:PLANET_CONTROL_GAINED:after",
    "leader:*:nekrocommander:TECHNOLOGY_GAINED:after",
    "leader:*:sardakkagent_strategic:STRATEGIC_ACTION_BEGAN:after",
    "leader:*:sardakkagent_turn:TURN_BEGAN:after",
    "leader:*:sardakkhero_turn:TURN_BEGAN:after",
    "leader:*:solcommander:GROUND_COMBAT_STARTED:after",
    "leader:*:titanscommander:PRODUCTION_USED:when",
    "leader:*:winnuagent:PRODUCTION_USED:after",
    "leader:*:yssarilcommander_clear:ACTION_COMPLETED:after",
    "promissory:*:ambuscade:UNIT_ABILITY_ROLLED:when",
    "promissory:*:iff:TURN_BEGAN:after",
    "promissory:*:ragh:UNITS_COMMITTED:after",
    "promissory:*:scepter:STRATEGY_PHASE_BEGAN:after",
    "promissory:*:spynet:TURN_BEGAN:after",
    "promissory:*:stymie:SHIP_MOVED:after",
    "promissory:*:tekklar:GROUND_COMBAT_STARTED:after",
    "promissory:*:tekklar_end:GROUND_COMBAT_ENDED:after",
    "promissory:*:tekklar_turn:TURN_BEGAN:after",
    "technology:*:asn_clear:ACTION_COMPLETED:after",
    "technology:*:bio:STATUS_PHASE_ENDED:after",
    "technology:*:exo2:SPACE_COMBAT_ROUND_ENDED:after",
    "technology:*:htp:PRODUCTION_USED:after",
    "technology:*:md_hit:GROUND_COMBAT_STARTED:after",
    "technology:*:nekroc4y:SHIP_DESTROYED:after",
    "technology:*:swa2:UNIT_ABILITY_ROLLED:after",
    "technology:*:vw:MOVEMENT_FINISHED:after",
    "technology:*:yso:UNITS_PRODUCED:after",
    "unit:*:naalu_mech_te:RELIC_GAINED:after",
    "unit:*:nekro_flagship:SPACE_COMBAT_STARTED:after",
    "unit:*:winnu_mech:ACTION_COMPLETED:after",
    "unit:*:saar_mech:PLANET_CONTROL_GAINED:after",
    "leader:*:a:yssarilagent:mentakagent:PILLAGE_USED:after",
    "leader:*:a:yssarilagent:naazagent:TURN_PASSED:after",
    "leader:*:a:yssarilagent:winnuagent:PRODUCTION_USED:after",
    "leader:*:arboreccommander:SYSTEM_ACTIVATED:after",
    "unit:*:arborec_flagship:SYSTEM_ACTIVATED:after",
    "leader:*:yssarilcommander:SYSTEM_ACTIVATED:after",
    "leader:*:c:l1z1xagent:a:SYSTEM_ACTIVATED:after",
    "leader:*:nomadagentmercer:TACTICAL_ACTION_ENDED:after",
];

/// The row a slot belongs to, if it is a gap's.
pub(crate) fn index_of(slot: Slot) -> Option<usize> {
    let index = usize::from(slot.0.checked_sub(ti4_engine_core_factions::silent::gaps::FIRST_SLOT)?);
    (index < GAP_IDS.len()).then_some(index)
}

/// The id row `index` is registered under for the faction called `owner`.
pub(crate) fn display_id(index: usize, owner: &str) -> String {
    let mut parts: Vec<&str> = GAP_IDS[index].split(':').collect();
    parts[1] = owner;
    parts.join(":")
}
