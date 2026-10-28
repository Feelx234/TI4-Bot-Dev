//! yin: tier W: offers, replacements and anything that asks (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CD).
//!
//! | slot | string function | tier | status |
//! |---|---|---|---|
//! | `strategy.research_waiver_paid` | `yin::research_waiver_paid` | W | ported: a performing hook, no ask (the choice of the payment is the research window's) |
//! | `combat.produced_hits` | `yin::produced_hits` | W | open: asks (`Ask` family of the combat window, B1) |
//! | `leader_action`, `use_leader`, `use_leader_timed`, `timing_abilities` | | W | open: B7 |

use ti4_engine_core::generated::{Faction, Technology, UnitBaseType};
use ti4_engine_core::hooks::{HookTable, Waiver};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::Cx;

use super::query::{SPACE, infantry_payments};

/// Fill the slots of this tier.
pub fn register(t: &mut HookTable) {
    t.strategy.research_waiver_paid.push(research_waiver_paid);
}

/// Pay the player-selected infantry. The payment target is recomputed and revalidated before any mutation, so a stale
/// choice fails without granting the technology.
pub fn research_waiver_paid(cx: &mut Cx<'_>, who: Seat, _tech: Technology, payment: Waiver) -> bool {
    if payment.module != Faction::TheYinBrotherhood {
        return false;
    }
    let Some((_, system, planet)) = infantry_payments(cx.state, who).iter().copied().find(|(code, _, _)| *code == payment.code) else {
        return false;
    };
    debug_assert_eq!(planet.is_none(), payment.code & SPACE != 0);
    let board = cx.state.board_mut();
    let units = match planet {
        Some(planet) => &mut board.planet_mut(planet).units,
        None => &mut board.sys_mut(system).units,
    };
    if let Some(index) = units.as_slice().iter().position(|p| p.owner() == who && p.kind().data().base_type == UnitBaseType::Infantry) {
        units.remove(index);
        return true;
    }
    false
}
