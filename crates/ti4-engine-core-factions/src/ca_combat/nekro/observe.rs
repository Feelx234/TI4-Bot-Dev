//! nekro: tier O: observers and bookkeeping (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CA).
//!
//! | slot | string function | tier | status |
//! |---|---|---|---|
//! | `nekro_units_alastor_decrement` | `nekro_units::alastor_decrement` | O | ported |
//! | `nekro_units_alastor_clear` | `nekro_units::alastor_clear` | O | ported |

use ti4_engine_core::generated::{Planet, System, Unit};
use ti4_engine_core::hooks::HookTable;
use ti4_engine_core::ids::Seat;
use ti4_engine_core::keys::{Atom, DecimalTextOrd, MarkKey, MarkVal};
use ti4_engine_core::kernel::Cx;

use crate::common::num;

/// Fill the slots of this tier.
pub fn register(t: &mut HookTable) {
    t.direct.nekro_units_alastor_decrement.push(alastor_decrement);
    t.direct.nekro_units_alastor_clear.push(alastor_clear);
}

/// One chosen participant of `unit` on `planet` is gone: the choice shrinks by one.
fn alastor_decrement(cx: &mut Cx<'_>, system: System, owner: Seat, planet: Planet, unit: Unit) {
    let key = MarkKey::NekroAlastor(DecimalTextOrd(cx.state.meta().activation_seq), system, owner, planet, unit);
    let tail = cx.state.marks.tail.edit();
    let Some(count) = tail.get(&key).and_then(num) else {
        return;
    };
    if count <= 1 {
        tail.remove(&key);
    } else {
        tail.insert(key, MarkVal::Atom(Atom::Num(count - 1)));
    }
}

/// End the participation in `system` (of one player, or of everyone), whatever activation made it.
fn alastor_clear(cx: &mut Cx<'_>, system: System, who: Option<Seat>) {
    let stale: Vec<MarkKey> = cx
        .state
        .marks
        .tail
        .iter()
        .filter_map(|(key, _)| {
            matches!(key, MarkKey::NekroAlastor(_, s, o, _, _) if *s == system && who.is_none_or(|w| w == *o)).then_some(*key)
        })
        .collect();
    if stale.is_empty() {
        return;
    }
    let tail = cx.state.marks.tail.edit();
    for key in stale {
        tail.remove(&key);
    }
}
