//! crimson: tier O: observers and bookkeeping (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CA).
//!
//! | slot | string function | tier | status |
//! |---|---|---|---|
//! | `crimson_note_deployed` | `crimson::note_deployed` | O | ported |
//! | `crimson_enforce_sundered` | `crimson::enforce_sundered` | O | open: moves units and asks (Sundered), needs the B2/B3 stage calls |
//! | `crimson_place_sorrow_breach` | `crimson::place_sorrow_breach` | O | open: writes `breach_tokens` and the tile's explore state (Sorrow); no typed writer in the core yet |
//! | `crimson_cards_put_on_card` | `crimson_cards::put_on_card` | O | open: the card list is a mark of unit atoms in the string's push order; needs the B3 production stage |

use ti4_engine_core::hooks::HookTable;
use ti4_engine_core::ids::Seat;
use ti4_engine_core::keys::{Atom, DecimalTextOrd, MarkKey, MarkVal};
use ti4_engine_core::kernel::Cx;

/// Fill the slots of this tier.
pub fn register(t: &mut HookTable) {
    t.direct.crimson_note_deployed.push(note_deployed);
}

/// Record that the Revenant was deployed in this activation; the marks of earlier activations are dropped with it.
fn note_deployed(cx: &mut Cx<'_>, invader: Seat) {
    let stale: Vec<MarkKey> =
        cx.state.marks.tail.iter().filter_map(|(key, _)| matches!(key, MarkKey::CrimsonDeployed(..)).then_some(*key)).collect();
    let seq = cx.state.meta().activation_seq;
    let tail = cx.state.marks.tail.edit();
    for key in stale {
        tail.remove(&key);
    }
    tail.insert(MarkKey::CrimsonDeployed(invader, DecimalTextOrd(seq)), MarkVal::Atom(Atom::Num(1)));
}
