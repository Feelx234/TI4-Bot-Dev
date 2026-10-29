//! `fx_strategy`: the writes of the strategy area that more than one rule area needs (package B8c-2): the waivers of a
//! secondary (a note handed over, an agent exhausted), the writes a research ends in, and the unit forms an upgrade
//! changes. Nothing here asks a question; nothing calls a rule area (the layer rule: `rules::*` may not import each
//! other, so the writes of `rules::trade` and `rules::leaders` that the strategy windows need are repeated here).

use crate::generated::Leader;
use crate::ids::Seat;
use crate::query::promissory as qn;
use crate::state::{CoreState, LeaderStatus, NoteKey};

/// `promissory::take`: `holder` gets the note, faceup if that is where the card lives (69.3). The same write as
/// `rules::trade::take_note`.
pub fn take_note(core: &mut CoreState, holder: Seat, key: NoteKey) {
    core.cards_mut().promissory.edit().insert(key, holder);
    if qn::is_play_area(key.note, key.faction)
        && !qn::is_action_placed(core, key.note)
        && key.note != crate::generated::PromissoryNote::Antivirus
    {
        core.cards_mut().promissory_faceup.edit().insert(key, ());
    }
}

/// `leaders::exhaust` without the Nomad's watch: a readied leader of `who` becomes exhausted; `false` when it was not
/// readied (or the seat does not hold it).
pub fn exhaust_leader(core: &mut CoreState, who: Seat, leader: Leader) -> bool {
    let Some(p) = core.players_mut().by_seat_mut(who) else { return false };
    match p.leaders.status.get_mut(&leader) {
        Some(status) if *status == LeaderStatus::Readied => {
            *status = LeaderStatus::Exhausted;
            true
        }
        _ => false,
    }
}
