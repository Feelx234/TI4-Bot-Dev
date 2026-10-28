//! empyrean: tier O: observers and bookkeeping (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CE). Owned by CE only.
//!
//! | string call | tier | target |
//! |---|---|---|
//! | `deal_notes` (`seating::deploy_seat`) | O | `direct.empyrean_deal_notes` |

use ti4_engine_core::generated::Faction;
use ti4_engine_core::hooks::HookTable;
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::Cx;
use ti4_engine_core::state::NoteKey;

use crate::common::is_faction;

/// Fill the slots of this tier.
pub fn register(t: &mut HookTable) {
    t.direct.empyrean_deal_notes.push(deal_notes);
}

/// Dark Whispers: "During setup, take the additional Empyrean faction promissory note; you have 2 faction promissory
/// notes." A note already in the table (an earlier deal, or one lent out) is left where it is.
pub fn deal_notes(cx: &mut Cx<'_>, who: Seat) {
    if !is_faction(cx.state, who, Faction::TheEmpyrean) {
        return;
    }
    for note in Faction::TheEmpyrean.data().promissory_notes {
        let key = NoteKey { note: *note, faction: Faction::TheEmpyrean };
        if cx.state.cards().promissory.get(&key).is_none() {
            cx.state.cards_mut().promissory.edit().insert(key, who);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ce_movement::testkit::table;
    use ti4_engine_core::kernel::FirstOption;

    #[test]
    fn dealing_gives_the_empyrean_its_notes_and_leaves_a_lent_one() {
        let (mut s, k) = table(&[Faction::TheEmpyrean, Faction::TheFederationOfSol]);
        let notes = Faction::TheEmpyrean.data().promissory_notes;
        assert!(notes.len() >= 2, "the faction has its pair");
        let lent = NoteKey { note: notes[0], faction: Faction::TheEmpyrean };
        s.cards_mut().promissory.edit().insert(lent, Seat(1));
        let mut ask = FirstOption;
        deal_notes(&mut Cx::new(&mut s, &k, &mut ask), Seat(0));
        assert_eq!(s.cards().promissory.get(&lent), Some(&Seat(1)), "a lent note stays");
        let dealt = NoteKey { note: notes[1], faction: Faction::TheEmpyrean };
        assert_eq!(s.cards().promissory.get(&dealt), Some(&Seat(0)));
        let before = s.cards().promissory.clone();
        deal_notes(&mut Cx::new(&mut s, &k, &mut ask), Seat(1));
        assert_eq!(*s.cards().promissory, *before, "another faction deals nothing");
    }
}
