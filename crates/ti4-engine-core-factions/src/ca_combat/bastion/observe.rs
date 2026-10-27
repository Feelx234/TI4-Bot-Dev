//! bastion: tier O: observers and bookkeeping (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CA).
//!
//! | slot | string function | tier | status |
//! |---|---|---|---|
//! | `bastion_note_ship_lost` | `bastion::note_ship_lost` | O | ported |
//! | `bastion_take_ship_lost` | `bastion::take_ship_lost` | O | ported |
//! | `bastion_clear_retreated` | `bastion::clear_retreated` | O | ported |
//! | `bastion_note_retreat` | `bastion::note_retreat` | O | open: the kept list is sorted by the id TEXT of the unit type
//! (`carrier2:0` before `carrier:0`), and the core has no text order of `Unit`; asked of C0 |

use ti4_engine_core::generated::{System, Unit};
use ti4_engine_core::hooks::HookTable;
use ti4_engine_core::ids::Seat;
use ti4_engine_core::keys::{Atom, MarkKey, MarkVal};
use ti4_engine_core::kernel::Cx;
use ti4_engine_core::state::Piece;

use crate::common::num;

/// Fill the slots of this tier.
pub fn register(t: &mut HookTable) {
    let d = &mut t.direct;
    d.bastion_note_ship_lost.push(note_ship_lost);
    d.bastion_take_ship_lost.push(take_ship_lost);
    d.bastion_clear_retreated.push(clear_retreated);
}

/// A galvanized ship of the piece's owner was just destroyed in `system`: for the announcement that follows.
fn note_ship_lost(cx: &mut Cx<'_>, system: System, piece: Piece) {
    if !piece.galvanized() {
        return;
    }
    let key = MarkKey::BastionLost(system, piece.owner(), piece.kind());
    let tail = cx.state.marks.tail.edit();
    let count = tail.get(&key).and_then(num).unwrap_or(0);
    tail.insert(key, MarkVal::Atom(Atom::Num(count + 1)));
}

/// Take one record of [`note_ship_lost`]: `true` if the ship was galvanized.
fn take_ship_lost(cx: &mut Cx<'_>, system: System, owner: Seat, unit: Unit) -> bool {
    let key = MarkKey::BastionLost(system, owner, unit);
    let tail = cx.state.marks.tail.edit();
    let Some(count) = tail.get(&key).and_then(num) else {
        return false;
    };
    if count <= 1 {
        tail.remove(&key);
    } else {
        tail.insert(key, MarkVal::Atom(Atom::Num(count - 1)));
    }
    true
}

/// Forget the retreats from the combat in `system`.
fn clear_retreated(cx: &mut Cx<'_>, system: System) {
    let stale: Vec<MarkKey> = cx
        .state
        .marks
        .tail
        .iter()
        .filter_map(|(key, _)| matches!(key, MarkKey::BastionRetreated(s, _) if *s == system).then_some(*key))
        .collect();
    if stale.is_empty() {
        return;
    }
    let tail = cx.state.marks.tail.edit();
    for key in stale {
        tail.remove(&key);
    }
}
