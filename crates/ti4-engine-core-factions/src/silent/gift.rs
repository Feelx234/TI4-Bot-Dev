//! naalu: Gift of Prescience, the promissory note of the Naalu (`factions::naalu::{gift, gift_return}`).
//!
//! "At the end of the strategy phase: place this card faceup in your play area and place the Naalu '0' token on your strategy card; you are first
//! in initiative order. The Naalu player cannot use their Telepathic faction ability during this game round. Return this card to the Naalu player at
//! the end of the status phase."
//!
//! The string registers both halves for every seat (the holder is whoever has the note): the play is optional (`promissory:*:gift:STRATEGY_PHASE_ENDED:after`),
//! the return is mandatory (`promissory:*:gift_return:STATUS_PHASE_ENDED:after`). Telepathic (`observe::strategy_phase_ended`) reads the same round mark.

use ti4_engine_core::choice::reaction::{AbilityKind, Relation};
use ti4_engine_core::generated::{Faction, PromissoryNote};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::ability::{AbilityTable, CondCx, Eff, Slot, Spec};
use ti4_engine_core::kernel::fx;
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::keys::{Atom, MarkKey, MarkVal};
use ti4_engine_core::state::{CoreState, EventKind, NoteKey};

use crate::common::{mark, num};

/// Gift of Prescience, played (`promissory:*:gift:STRATEGY_PHASE_ENDED:after`).
pub const GIFT: Slot = Slot(40_001);
/// Gift of Prescience, returned (`promissory:*:gift_return:STATUS_PHASE_ENDED:after`).
pub const GIFT_RETURN: Slot = Slot(40_002);

/// The note.
const NOTE: NoteKey = NoteKey { note: PromissoryNote::GiftOfPrescience, faction: Faction::TheNaaluCollective };

fn is_naalu(s: &CoreState, who: Seat) -> bool {
    s.players().by_seat(who).is_some_and(|p| p.faction == Faction::TheNaaluCollective)
}

/// `gift_played_this_round`: the Naalu mark names this game round.
fn played_this_round(s: &CoreState) -> bool {
    mark(s, MarkKey::NaaluGiftRound()).and_then(num) == Some(s.meta().round)
}

/// `gift_holder`: the seat that holds the Naalu's note, when it is another faction's and a Naalu is seated.
fn holder(s: &CoreState) -> Option<Seat> {
    let holder = *s.cards().promissory.get(&NOTE)?;
    if is_naalu(s, holder) || !s.players().iter().any(|p| p.faction == Faction::TheNaaluCollective) {
        return None;
    }
    Some(holder)
}

fn ready(s: &CoreState, who: Seat) -> bool {
    !played_this_round(s) && holder(s) == Some(who)
}

fn play_condition(c: &CondCx<'_>) -> bool {
    ready(c.state, c.def.owner)
}

fn play(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    if !ready(eff.state(), me) {
        return Ok(());
    }
    let round = eff.state().meta().round;
    let s = eff.state();
    s.strategy_mut().initiative_overrides.insert(me, 0);
    s.cards_mut().promissory_faceup.edit().insert(NOTE, ());
    s.marks_mut().tail.edit().insert(MarkKey::NaaluGiftRound(), MarkVal::Atom(Atom::Num(round)));
    Ok(())
}

/// `gift_return`'s `ready`: the round mark is set and a seat of another faction holds the note.
fn return_ready(s: &CoreState, who: Seat) -> bool {
    mark(s, MarkKey::NaaluGiftRound()).is_some() && s.cards().promissory.get(&NOTE) == Some(&who) && !is_naalu(s, who)
}

fn return_condition(c: &CondCx<'_>) -> bool {
    return_ready(c.state, c.def.owner)
}

fn give_back(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    if return_ready(eff.state(), me) {
        fx::give_back_note(eff.state(), NOTE);
        eff.state().marks_mut().tail.edit().remove(&MarkKey::NaaluGiftRound());
    }
    Ok(())
}

pub fn register_abilities(t: &mut AbilityTable) {
    t.add(Spec::new(GIFT, AbilityKind::Promissory, EventKind::StrategyPhaseEnded, Relation::After, play).optional().when(play_condition));
    t.add(Spec::new(GIFT_RETURN, AbilityKind::Promissory, EventKind::StatusPhaseEnded, Relation::After, give_back).when(return_condition));
}
