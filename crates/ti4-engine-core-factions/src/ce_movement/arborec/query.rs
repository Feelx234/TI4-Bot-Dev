//! arborec: tier Q: pure hooks (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CE). Owned by CE only.
//!
//! | string slot | string function | tier | target |
//! |---|---|---|---|
//! | `economy.cannot_produce` | `cannot_produce` | Q | seam `cannot_produce` (an `Allow` rule) |
//! | `component_actions`, `perform_component`, `commander_unlocked`, `leader_action`, `use_leader`, `timing_abilities` | | W | B7 (leaders), Mitosis and the other inline asks in `offer.rs` |

use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::generated::{Ability, Faction, UnitBaseType};
use ti4_engine_core::hooks::HookTable;
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::seam::{ProduceSite, Q, Source, Verdict};
use ti4_engine_core::state::CoreState;

use crate::common::player;

const SOURCE: Source = Source::Faction(Faction::TheArborec);

/// Fill the slots of this tier.
pub fn register(t: &mut HookTable) {
    t.general.commander_unlocked.push(commander_unlocked);
    t.general.leader_action.push(leader_action);
    t.seam.cannot_produce.push(SOURCE, rule);
}

fn rule(q: Q<'_>, site: &ProduceSite) -> Verdict {
    if cannot_produce(q.s, site.who, site.unit, site.producer) { Verdict::Allow } else { Verdict::Abstain }
}

/// Mitosis: "Your space docks cannot produce infantry." (`faction_abilities::has(.., "mitosis")`: the printed ability
/// of the seat's faction.)
pub fn cannot_produce(s: &CoreState, who: Seat, unit: UnitBaseType, producer: UnitBaseType) -> bool {
    unit == UnitBaseType::Infantry
        && producer == UnitBaseType::Spacedock
        && player(s, who).is_some_and(|p| p.faction.data().abilities.contains(&Ability::Mitosis))
}

// -- leader hooks (package F2) ----------------------------------------

/// Dirzuga Rophal, unlock: "Have 12 ground forces on planets you control." (`arborec::commander_unlocked`)
pub fn commander_unlocked(s: &CoreState, _: &Kernel, who: Seat, leader: ti4_engine_core::generated::Leader) -> Option<bool> {
    use ti4_engine_core::query::{board, fleet};
    (leader == ti4_engine_core::generated::Leader::DirzugaRophal).then(|| {
        board::controlled_planets(s, who)
            .as_slice()
            .iter()
            .map(|(_, planet)| board::planet_units(s, *planet).iter().filter(|p| p.owner() == who && fleet::is_ground_force(p.kind())).count())
            .sum::<usize>()
            >= 12
    })
}

/// Letani Ospha (some ship has an owner who could swap it) and Letani Miasmiala (some system with a ground force could build something):
/// whether the action can resolve (`arborec::leader_action`).
pub fn leader_action(s: &CoreState, k: &Kernel, who: Seat, leader: ti4_engine_core::generated::Leader) -> Option<bool> {
    use ti4_engine_core::generated::Leader;
    match leader {
        Leader::LetaniOspha => Some(ospha_has_target(s, k)),
        Leader::LetaniMiasmiala => Some(hero_systems_exist(s, k, who)),
        _ => None,
    }
}

/// The base types a ship may be replaced with (`arborec::ship_bases`).
const SHIP_BASES: [UnitBaseType; 6] =
    [UnitBaseType::Carrier, UnitBaseType::Cruiser, UnitBaseType::Destroyer, UnitBaseType::Dreadnought, UnitBaseType::Flagship, UnitBaseType::Warsun];

/// `!arborec::ship_targets(..).is_empty()`: a non-fighter ship (anyone's) whose owner has, in the reinforcements, a ship of another base
/// type that costs at most 2 more (the form the owner places: its upgrade, its faction's unit, the plain one).
fn ospha_has_target(s: &CoreState, k: &Kernel) -> bool {
    use ti4_engine_core::query::{fleet, tech};
    let scope = crate::common::scope(k);
    let roster = crate::cd_registry::yin::timing::roster(scope);
    let plain = |u: ti4_engine_core::generated::Unit| scope.unit(u).filter(|d| d.ship && d.base_type != UnitBaseType::Fighter);
    s.board().systems().any(|(_, cell)| {
        cell.units.as_slice().iter().any(|piece| {
            let Some(current) = plain(piece.kind()) else { return false };
            let ceiling = u16::from(current.cost_halves.unwrap_or(0)) + 4;
            let owner = piece.owner();
            SHIP_BASES.iter().filter(|base| **base != current.base_type).any(|base| {
                tech::unit_form(s, scope, &roster, owner, *base).is_some_and(|form| {
                    plain(form).is_some_and(|d| u16::from(d.cost_halves.unwrap_or(0)) <= ceiling) && fleet::remaining_all(s, scope, owner, form) > 0
                })
            })
        })
    })
}

/// `!arborec::hero_systems(..).is_empty()`: a system with one of the seat's ground forces in which a production by the hero (any
/// number of units) would ask something.
fn hero_systems_exist(s: &CoreState, k: &Kernel, who: Seat) -> bool {
    let roster = crate::cd_registry::yin::timing::roster(crate::common::scope(k));
    ti4_engine_core::rules::leader_production::ArborecHero::ground_force_systems(s, who)
        .into_iter()
        .any(|system| ti4_engine_core::rules::production::probe::would_ask(s, k, &roster, who, system, HERO_LIMIT, None))
}

/// `arborec::HERO_LIMIT`: "any number of units".
const HERO_LIMIT: i64 = 1000;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ce_movement::testkit::table;

    #[test]
    fn mitosis_bars_infantry_from_the_docks_of_the_arborec_alone() {
        let (s, _) = table(&[Faction::TheArborec, Faction::TheFederationOfSol]);
        assert!(cannot_produce(&s, Seat(0), UnitBaseType::Infantry, UnitBaseType::Spacedock));
        assert!(!cannot_produce(&s, Seat(0), UnitBaseType::Fighter, UnitBaseType::Spacedock));
        assert!(!cannot_produce(&s, Seat(0), UnitBaseType::Infantry, UnitBaseType::Pds));
        assert!(!cannot_produce(&s, Seat(1), UnitBaseType::Infantry, UnitBaseType::Spacedock));
    }

    #[test]
    fn registering_fills_the_predicate() {
        let mut t = HookTable::new();
        register(&mut t);
        assert_eq!(t.seam.cannot_produce.len(), 1);
    }
}
