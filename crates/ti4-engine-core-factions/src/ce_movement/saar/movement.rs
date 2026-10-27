//! The Clan of Saar: Chaos Mapping and the agent's move bonus.

use ti4_engine_core::generated::{System, Technology};
use ti4_engine_core::hooks::{HookTable, MoveSite};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::keys::MarkKey;
use ti4_engine_core::state::CoreState;

use crate::common::{has_tech_text, mark, num, rec, scope, system, unit};

/// `ti4_model::state::RULES_MENDOSA_SHIP`.
const RULES_MENDOSA_SHIP: u32 = 2;

pub fn register(t: &mut HookTable) {
    t.movement.cannot_activate.push(chaos_mapping_blocks_activation);
    t.movement.move_bonus.push(agent_move_bonus);
}

/// "Other players cannot activate asteroid fields that contain 1 or more of your ships."
fn chaos_mapping_blocks_activation(s: &CoreState, k: &Kernel, activator: Seat, target: System) -> bool {
    let scope = scope(k);
    if !scope.system(target).is_some_and(|d| d.anomalies.asteroid_field) {
        return false;
    }
    s.board().sys(target).units.as_slice().iter().any(|p| {
        p.owner() != activator
            && has_tech_text(s, p.owner(), Technology::ChaosMapping)
            && scope.unit(p.kind()).is_some_and(|d| d.ship)
    })
}

fn agent_move_bonus(s: &CoreState, site: MoveSite) -> i32 {
    let Some(fields) = mark(s, MarkKey::SaarAgentBoost(site.player)).and_then(rec) else {
        return 0;
    };
    let activation = s.meta().activation_seq;
    if s.meta().rules_version >= RULES_MENDOSA_SHIP {
        // The boosted kind's first ship, until one of that kind has left the origin.
        let [seq, origin, kind, damaged, count, bonus] = fields else {
            return 0;
        };
        let (Some(index), Some(count)) = (site.index, num(count)) else {
            return 0;
        };
        let (Some(kind), Some(damaged)) = (unit(kind), num(damaged)) else {
            return 0;
        };
        let damaged = damaged == 1;
        let mut first = None;
        let mut matching = 0usize;
        for (i, p) in s.board().sys(site.origin).units.as_slice().iter().filter(|p| p.owner() == site.player).enumerate() {
            if p.kind() == kind && p.sustained() == damaged {
                first.get_or_insert(i);
                matching += 1;
            }
        }
        return if num(seq) == Some(activation)
            && system(origin) == Some(site.origin)
            && matching == count as usize
            && first == Some(usize::from(index))
        {
            num(bonus).map_or(0, |b| b as i32)
        } else {
            0
        };
    }
    let [seq, origin, index, bonus] = fields else {
        return 0;
    };
    if num(seq) == Some(activation)
        && system(origin) == Some(site.origin)
        && site.index.is_some_and(|i| num(index) == Some(u32::from(i)))
    {
        num(bonus).map_or(0, |b| b as i32)
    } else {
        0
    }
}
