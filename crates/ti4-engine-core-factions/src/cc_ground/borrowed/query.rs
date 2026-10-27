//! borrowed: tier Q: pure hooks, stacks, predicates and gathers (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CC).
//! Registered as `Id::Borrowed`: the shared helpers of the borrowed commanders and round agents, with one provider each.
//!
//! | slot | string function | tier | target |
//! |---|---|---|---|
//! | `general.unit_roll_modifier` | `borrowed_commanders::unit_roll_modifier` | Q | `general.unit_roll_modifier` (Obsidian commander, the Fracture; a sum) |
//! | `borrowed_commanders_b_firmament_planets` | `borrowed_commanders_b::firmament_planets` | Q | direct (concat) |
//! | `borrowed_round_agents_has_owner_space_round_agent` | `borrowed_round_agents::has_owner_space_round_agent` | Q | direct |
//! | `borrowed_round_agents_has_owner_round_agent` | `borrowed_round_agents::has_owner_round_agent` | Q | direct |
//! | `borrowed_round_agents_extra_die_for` | `borrowed_round_agents::extra_die_for` | blocked | the round bonus is a JSON blob mark (`MarkKey::YssarilSsruuRoundAgent` is a BLOB) and `live_target` needs `invasion::is_ground_force_here` (B2): the record needs F5's typed schema |
//! | `borrowed_round_agents_has_round_copy` | `borrowed_round_agents::has_round_copy` | blocked | needs `hooks_cards::borrowable_agents` (package CD, Yssaril) |
//! | `borrowed_round_agents_abilities` | `borrowed_round_agents::abilities` | W | `offer.rs` (B1/B7) |
//! | `borrowed_commanders::timing_abilities`, `borrowed_commanders_b::timing_abilities` | | W | `offer.rs` (B7) |

use ti4_engine_core::generated::{Faction, Leader, Planet, PromissoryNote};
use ti4_engine_core::hooks::direct::Planets;
use ti4_engine_core::hooks::{CombatUnit, HookTable};
use ti4_engine_core::kernel::seam::{Delta, Layer, Q, RollKind, RollSite, Source};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::keys::MarkKey;
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::query::board;
use ti4_engine_core::state::{CoreState, LeaderStatus, MetaFlags};
use ti4_engine_core::generated::TileBack;

use crate::common::{is_faction, mark, player, scope};

/// `ti4_model::state::RULES_FACTION_EFFECTS`.
const RULES_FACTION_EFFECTS: u32 = 16;
/// `ti4_model::state::RULES_FACTION_BUGFIXES`.
const RULES_FACTION_BUGFIXES: u32 = 17;

/// Fill the slots of this tier.
pub fn register(t: &mut HookTable) {
    t.seam.hit_shift.push_add(Source::Faction(Faction::TheObsidian), Layer::Faction, commander_ground_shift);
    t.general.unit_roll_modifier.push(unit_roll_modifier);
    let d = &mut t.direct;
    d.borrowed_commanders_b_firmament_planets.push(firmament_planets);
    d.borrowed_round_agents_has_owner_space_round_agent.push(has_owner_space_round_agent);
    d.borrowed_round_agents_has_owner_round_agent.push(has_owner_round_agent);
}

/// `promissory::has_commander_ability`: a granted mark, the commander unlocked, the Mahact's Imperia, or a faceup
/// Alliance from a seated owner whose commander is unlocked.
pub fn has_commander_ability(s: &CoreState, k: &Kernel, who: Seat, commander: Leader) -> bool {
    let unlocked = |seat: Seat| player(s, seat).is_some_and(|p| p.leaders.status.get(&commander) == Some(&LeaderStatus::Unlocked));
    if mark(s, MarkKey::CommanderAbility(who, commander)).is_some() || unlocked(who) {
        return true;
    }
    if k.hooks.direct.mahact_imperia_grants(s, who, commander) {
        return true;
    }
    s.cards().promissory.iter().any(|(key, holder)| {
        *holder == who
            && key.note == PromissoryNote::Alliance
            && s.cards().promissory_faceup.contains_key(key)
            && s.seats()
                .seating_order()
                .iter()
                .copied()
                .find(|seat| is_faction(s, *seat, key.faction))
                .is_some_and(|owner| owner != who && unlocked(owner))
    })
}

/// The same +1 on a ground roll: the space rolls ride the legacy bridge (`general.unit_roll_modifier`), the ground
/// ones have no bridge, so the shift is a native provider.
fn commander_ground_shift(q: Q<'_>, site: &RollSite, _: i64) -> Delta {
    if site.kind != RollKind::Ground {
        return Delta::None;
    }
    match unit_roll_modifier(q.s, q.k, site.unit) {
        0 => Delta::None,
        n => Delta::Add(n),
    }
}

/// Obsidian commander: +1 to each of the unit's combat rolls in the Fracture, while it is in play.
fn unit_roll_modifier(s: &CoreState, k: &Kernel, unit: CombatUnit) -> i64 {
    let Some(system) = unit.system else { return 0 };
    if s.meta().flags.has(MetaFlags::FRACTURE_IN_PLAY)
        && scope(k).system(system).is_some_and(|d| d.tile_back == Some(TileBack::Fracture))
        && has_commander_ability(s, k, unit.owner, Leader::ArozHollow)
    {
        1
    } else {
        0
    }
}

/// Every planet of a system where `who` has a ship, for a Firmament commander's holder (secret objectives only); a
/// planet may appear twice only across systems, never within the list.
fn firmament_planets(s: &CoreState, k: &Kernel, who: Seat) -> Planets {
    let mut planets = Planets::default();
    if !has_commander_ability(s, k, who, Leader::CaptainAroz) {
        return planets;
    }
    let scope = scope(k);
    for (system, cell) in s.board().systems() {
        if !cell.units.as_slice().iter().any(|u| u.owner() == who && scope.unit(u.kind()).is_some_and(|d| d.ship)) {
            continue;
        }
        for planet in board::planets_in(s, scope, system).iter() {
            let planet: Planet = *planet;
            if !planets.as_slice().contains(&planet) {
                planets.push(planet);
            }
        }
    }
    planets
}

/// Whether any seat holds a readied Letnev agent of its own (rules version 17).
fn has_owner_space_round_agent(s: &CoreState) -> bool {
    s.meta().rules_at_least(RULES_FACTION_BUGFIXES) && holds_readied(s, Faction::TheBaronyOfLetnev, Leader::ViscountUnlenn)
}

/// Whether any seat holds a readied Sol agent of its own (rules version 16).
fn has_owner_round_agent(s: &CoreState) -> bool {
    s.meta().rules_at_least(RULES_FACTION_EFFECTS) && holds_readied(s, Faction::TheFederationOfSol, Leader::EvelynDeLouis)
}

fn holds_readied(s: &CoreState, faction: Faction, agent: Leader) -> bool {
    s.players().iter().any(|p| p.faction == faction && p.leaders.status.get(&agent) == Some(&LeaderStatus::Readied))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cc_ground::testkit::table;
    use ti4_engine_core::generated::{System, Unit};
    use ti4_engine_core::state::Piece;

    const SOL: Faction = Faction::TheFederationOfSol;
    const LETNEV: Faction = Faction::TheBaronyOfLetnev;

    #[test]
    fn the_owner_round_agents_follow_the_rules_version_and_the_ready_agent() {
        let (mut s, _) = table(&[SOL, LETNEV]);
        s.players_mut().edit(0).leaders.status.insert(Leader::EvelynDeLouis, LeaderStatus::Readied);
        s.players_mut().edit(1).leaders.status.insert(Leader::ViscountUnlenn, LeaderStatus::Readied);
        assert!(!has_owner_round_agent(&s) && !has_owner_space_round_agent(&s), "version 0");
        s.meta_mut().rules_version = 16;
        assert!(has_owner_round_agent(&s) && !has_owner_space_round_agent(&s));
        s.meta_mut().rules_version = 17;
        assert!(has_owner_space_round_agent(&s));
        s.players_mut().edit(1).leaders.status.insert(Leader::ViscountUnlenn, LeaderStatus::Exhausted);
        assert!(!has_owner_space_round_agent(&s));
    }

    #[test]
    fn the_firmament_commander_counts_the_planets_of_systems_with_a_ship() {
        let (mut s, _) = table(&[SOL, LETNEV]);
        let k = Kernel { sources: ti4_engine_core::kernel::Sources(u8::MAX), ..Kernel::plain() };
        let system = System::ALL.iter().copied().find(|sys| !sys.data().planets.is_empty()).expect("a system with planets");
        s.board_mut().sys_mut(system).units.push(Piece::new(Unit::CruiserI, Seat(0)));
        assert!(firmament_planets(&s, &k, Seat(0)).is_empty(), "no commander");
        s.players_mut().edit(0).leaders.status.insert(Leader::CaptainAroz, LeaderStatus::Unlocked);
        assert!(!firmament_planets(&s, &k, Seat(0)).is_empty());
        assert!(firmament_planets(&s, &k, Seat(1)).is_empty());
    }

    #[test]
    fn the_obsidian_commander_shifts_rolls_in_the_fracture_only() {
        let (mut s, _) = table(&[SOL, LETNEV]);
        let k = Kernel { sources: ti4_engine_core::kernel::Sources(u8::MAX), ..Kernel::plain() };
        s.players_mut().edit(0).leaders.status.insert(Leader::ArozHollow, LeaderStatus::Unlocked);
        let fracture = System::ALL.iter().copied().find(|sys| sys.data().tile_back == Some(TileBack::Fracture)).expect("a fracture tile");
        let plain = System::ALL.iter().copied().find(|sys| sys.data().tile_back != Some(TileBack::Fracture)).expect("another tile");
        let unit = |system| CombatUnit { owner: Seat(0), system: Some(system), planet: None, unit: Unit::CruiserI, ctx: ti4_engine_core::hooks::CombatCtx::Space };
        assert_eq!(unit_roll_modifier(&s, &k, unit(fracture)), 0, "not in play");
        s.meta_mut().flags.set(MetaFlags::FRACTURE_IN_PLAY, true);
        assert_eq!(unit_roll_modifier(&s, &k, unit(fracture)), 1);
        assert_eq!(unit_roll_modifier(&s, &k, unit(plain)), 0);
    }
}
