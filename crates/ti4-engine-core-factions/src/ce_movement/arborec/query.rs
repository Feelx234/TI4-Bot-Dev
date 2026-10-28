//! arborec: tier Q: pure hooks (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CE). Owned by CE only.
//!
//! | string slot | string function | tier | target |
//! |---|---|---|---|
//! | `economy.cannot_produce` | `cannot_produce` | Q | seam `cannot_produce` (an `Allow` rule) |
//! | `component_actions`, `perform_component`, `commander_unlocked`, `leader_action`, `use_leader`, `timing_abilities` | | W | B7 (leaders), Mitosis and the other inline asks in `offer.rs` |

use ti4_engine_core::generated::{Ability, Faction, UnitBaseType};
use ti4_engine_core::hooks::HookTable;
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::seam::{ProduceSite, Q, Source, Verdict};
use ti4_engine_core::state::CoreState;

use crate::common::player;

const SOURCE: Source = Source::Faction(Faction::TheArborec);

/// Fill the slots of this tier.
pub fn register(t: &mut HookTable) {
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
