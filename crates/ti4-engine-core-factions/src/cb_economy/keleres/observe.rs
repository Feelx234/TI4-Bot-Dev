//! keleres: tier O: observers and bookkeeping (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CB). Owned by CB only.
//!
//! | string call | tier | target |
//! |---|---|---|
//! | `close_agent_window` (4 call sites in `production`) | O | `direct.keleres_close_agent_window`, fired by `bridge_legacy` on `Obs::ProductionWindowClosed` |
//! | `asn_mark` | O | `direct.keleres_asn_mark` |
//! | `reconcile` (Custodia Vigilia, a board edit at every driver step) | O, BLOCKED | needs the planet-card placement of `planets::place` in the core |

use ti4_engine_core::hooks::HookTable;
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::Cx;
use ti4_engine_core::keys::{Atom, MarkKey, MarkVal};

/// Fill the slots of this tier.
pub fn register(t: &mut HookTable) {
    t.direct.keleres_close_agent_window.push(close_agent_window);
    t.direct.keleres_asn_mark.push(asn_mark);
}

/// End `payer`'s agent window: their commodities are not trade goods any more.
pub fn close_agent_window(cx: &mut Cx<'_>, payer: Seat) {
    cx.state.marks_mut().tail.edit().remove(&MarkKey::KeleresAgentGoods(payer));
}

/// Agency Supply Network was used: record it until the action ends.
pub fn asn_mark(cx: &mut Cx<'_>, who: Seat) {
    cx.state.marks_mut().tail.edit().insert(MarkKey::KeleresAsnUsed(who), MarkVal::Atom(Atom::Num(1)));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cb_economy::testkit::table;
    use ti4_engine_core::generated::Faction;
    use ti4_engine_core::keys::Word;

    #[test]
    fn the_window_closes_and_asn_is_marked_used() {
        let (mut s, k) = table(&[Faction::TheCouncilKeleresArgent, Faction::TheFederationOfSol]);
        s.marks_mut().tail.edit().insert(MarkKey::KeleresAgentGoods(Seat(1)), MarkVal::Atom(Atom::Word(Word::Probe)));
        let mut ask = ti4_engine_core::kernel::FirstOption;
        close_agent_window(&mut Cx::new(&mut s, &k, &mut ask), Seat(1));
        assert!(s.marks().tail.get(&MarkKey::KeleresAgentGoods(Seat(1))).is_none());
        asn_mark(&mut Cx::new(&mut s, &k, &mut ask), Seat(0));
        assert_eq!(s.marks().tail.get(&MarkKey::KeleresAsnUsed(Seat(0))), Some(&MarkVal::Atom(Atom::Num(1))));
    }
}
