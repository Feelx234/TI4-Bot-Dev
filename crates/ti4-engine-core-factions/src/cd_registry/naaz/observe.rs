//! naaz: tier O: observers (`Obs`) and bookkeeping (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CD).
//!
//! | slot | string function | tier | status |
//! |---|---|---|---|
//! | `economy.explored` | `naaz::explored` (Pre-Fab Arcologies) | O | ported |
//! | `general.space_combat_round_started`, `ground.ground_combat_round_started` | `naaz::*_combat_round_started` | O + W | open: each repairs the Maximum (bookkeeping) and then offers Supercharge (an ask); the two halves are one program point, so they port together with the combat window's `Obs::CombatRoundStarted` / `GroundRoundStarted` and the `Offering` stage (B1) |

use ti4_engine_core::generated::{Planet, Technology};
use ti4_engine_core::hooks::HookTable;
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::Cx;
use ti4_engine_core::state::PlanetFlags;

use crate::common::has_tech_text;

/// Fill the slots of this tier.
pub fn register(t: &mut HookTable) {
    t.economy.explored.push(explored);
}

/// Pre-Fab Arcologies: "After you explore a planet, ready that planet."
pub fn explored(cx: &mut Cx<'_>, who: Seat, planet: Planet) {
    if has_tech_text(cx.state, who, Technology::PreFabArcologies) {
        cx.state.board_mut().planet_mut(planet).flags.set(PlanetFlags::EXHAUSTED, false);
    }
}
