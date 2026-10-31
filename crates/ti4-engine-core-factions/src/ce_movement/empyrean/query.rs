//! empyrean: tier Q: pure hooks, stacks, predicates and gathers (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CE).
//! Owned by CE only. Empty until the package ports this tier.

use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::state::CoreState;
use ti4_engine_core::hooks::HookTable;

/// Fill the slots of this tier.
pub fn register(t: &mut HookTable) {
    t.general.commander_unlocked.push(commander_unlocked);
    t.general.leader_action.push(leader_action);
}

// -- leader hooks (package F2) ----------------------------------------

/// Xuange, unlock: "Be neighbors with all other players." Without a map there is no adjacency (`empyrean::commander_unlocked`).
pub fn commander_unlocked(s: &CoreState, _: &Kernel, who: Seat, leader: ti4_engine_core::generated::Leader) -> Option<bool> {
    use ti4_engine_core::query::trade::{present_together, presence};
    (leader == ti4_engine_core::generated::Leader::Xuange).then(|| {
        let Some(adjacency) = s.map().adjacency() else { return false };
        let mine = presence(s, who);
        let mut others = s.players().iter().map(|p| p.seat).filter(|seat| *seat != who).peekable();
        others.peek().is_some() && others.all(|other| present_together(adjacency, &mine, &presence(s, other)))
    })
}

/// Conservator Procyon can always be used by its owner: placing and exploring may have nothing to do, the card still resolves.
pub fn leader_action(s: &CoreState, _: &Kernel, who: Seat, leader: ti4_engine_core::generated::Leader) -> Option<bool> {
    (leader == ti4_engine_core::generated::Leader::ConservatorProcyon).then(|| crate::common::is_faction(s, who, ti4_engine_core::generated::Faction::TheEmpyrean))
}

