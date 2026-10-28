//! naalu: tier O: observers (`Obs`) and bookkeeping (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CD).
//!
//! | slot | string function | tier | status |
//! |---|---|---|---|
//! | `strategy.strategy_phase_ended` | `naalu::strategy_phase_ended` (Telepathic) | O | ported |
//! | `strategy.secondary_waived` | `naalu::secondary_waived` (Mindsieve) | O | open: it hands the note over with `promissory::take`, whose "goes faceup on receipt" rule reads the note texts; needs the typed `promissory::take` (the cards area owner) |

use ti4_engine_core::generated::Faction;
use ti4_engine_core::hooks::HookTable;
use ti4_engine_core::keys::MarkKey;
use ti4_engine_core::kernel::Cx;

use crate::common::{mark, num};

/// Fill the slots of this tier.
pub fn register(t: &mut HookTable) {
    t.strategy.strategy_phase_ended.push(strategy_phase_ended);
}

/// Whether Gift of Prescience was played this game round (Telepathic is then off).
fn gift_played_this_round(cx: &Cx<'_>) -> bool {
    mark(cx.state, MarkKey::NaaluGiftRound()).and_then(num) == Some(cx.state.meta().round)
}

/// Telepathic: "Place the Naalu '0' token on your strategy card; you are first in initiative order." Mandatory, so decided
/// without a question. The token is the Naalu player's only while Gift of Prescience has not been played this round.
pub fn strategy_phase_ended(cx: &mut Cx<'_>) {
    if gift_played_this_round(cx) {
        return;
    }
    let naalu: Vec<_> = cx.state.players().iter().filter(|p| p.faction == Faction::TheNaaluCollective).map(|p| p.seat).collect();
    for seat in naalu {
        cx.state.strategy_mut().initiative_overrides.insert(seat, 0);
    }
}
