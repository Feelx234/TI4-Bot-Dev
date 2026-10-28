//! argent: tier O: observers and bookkeeping (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CB). Owned by CB only.
//!
//! | string call | tier | target |
//! |---|---|---|
//! | `clear_agent_production_destinations` (8 call sites in `production`) | O | `direct.argent_clear_agent_production_destinations`, which `adapters::bridge_legacy` fires on `Obs::ProductionWindowClosed` |

use ti4_engine_core::generated::System;
use ti4_engine_core::hooks::HookTable;
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::Cx;
use ti4_engine_core::keys::MarkKey;

/// Fill the slots of this tier.
pub fn register(t: &mut HookTable) {
    t.direct.argent_clear_agent_production_destinations.push(clear_agent_production_destinations);
}

/// The agent's grant is a one-build mark, removed when the production window closes.
pub fn clear_agent_production_destinations(cx: &mut Cx<'_>, who: Seat, system: System) {
    cx.state.marks_mut().tail.edit().remove(&MarkKey::ArgentProduction(who, system));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cb_economy::testkit::table;
    use ti4_engine_core::generated::Faction;
    use ti4_engine_core::keys::{Atom, MarkVal};

    #[test]
    fn closing_the_window_removes_that_systems_grant_only() {
        let (mut s, k) = table(&[Faction::TheArgentFlight, Faction::TheFederationOfSol]);
        let (a, b) = (ti4_engine_core::ids::system_at(30), ti4_engine_core::ids::system_at(31));
        for sys in [a, b] {
            s.marks_mut().tail.edit().insert(MarkKey::ArgentProduction(Seat(0), sys), MarkVal::List(vec![MarkVal::Atom(Atom::Num(0))]));
        }
        let mut ask = ti4_engine_core::kernel::FirstOption;
        clear_agent_production_destinations(&mut Cx::new(&mut s, &k, &mut ask), Seat(0), a);
        assert!(s.marks().tail.get(&MarkKey::ArgentProduction(Seat(0), a)).is_none());
        assert!(s.marks().tail.get(&MarkKey::ArgentProduction(Seat(0), b)).is_some());
    }
}
