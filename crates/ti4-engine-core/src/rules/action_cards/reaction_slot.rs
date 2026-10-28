//! The standing reaction-card slot (`reactions::slot`, `reactions::playable_now` of the string engine): one
//! registered ability per seat and printed window, optional, unlimited and repeatable, eligible while the seat
//! holds an action card that could be played into the window.
//!
//! Ported: the eligibility (hand, reaction modes, the printed window and its guard, 22.3 `reaction_can_matter`,
//! the faction and law bars) and the list of cards the slot would offer ([`playable`]), which the adapter reads
//! to render the option ("Play X" / "Choose an action card…"). Not ported: playing the card (the spec is
//! `eligibility_only`; a run in which the string played one is not followed).
//!
//! The windows: every non-`Action` printed timing maps to an event and a relation (`kernel::reaction::window_of`);
//! the distinct pairs, sorted, are the slots `SLOT_BASE + index`. The string sorts the same pairs by event text;
//! the order between windows is not observable (a bucket holds one slot per owner).

use crate::choice::reaction::{AbilityKind, Relation};
use crate::containers::SmallList;
use crate::generated::{ActionCard, Faction, Timing};
use crate::ids::Seat;
use crate::kernel::ability::{AbilityTable, CondCx, Slot, Spec};
use crate::kernel::reaction::window_of;
use crate::kernel::Kernel;
use crate::state::{CoreState, EventKind, EventRec, Key, ReactionMode};
use crate::tables::Scope;

use super::{cost_is_payable, reaction_can_matter, Window, RULES_ACTION_CARD_COSTS};

/// The first slot number of the reaction-card slots.
pub const SLOT_BASE: u16 = 100;

pub use crate::kernel::ability::Unknown;

/// The distinct `(event, relation)` windows of the printed timings, sorted.
#[must_use]
pub fn windows() -> Vec<(EventKind, Relation)> {
    let mut all: Vec<(EventKind, Relation)> = Timing::ALL.iter().filter_map(|t| window_of(*t)).map(|w| (w.event, w.relation)).collect();
    all.sort();
    all.dedup();
    all
}

/// The window of a slot number.
#[must_use]
pub fn window_of_slot(slot: Slot) -> Option<(EventKind, Relation)> {
    windows().get(usize::from(slot.0.checked_sub(SLOT_BASE)?)).copied()
}

/// The cards in the hand (and on the salvage yard, while Garbozia is controlled) that could be played into the
/// window of `event` now, in hand order (`playable_now`).
///
/// # Errors
/// [`Unknown`] when a rule the answer reads is a documented bail of the core.
pub fn playable(s: &CoreState, k: &Kernel, barred: bool, who: Seat, event: &EventRec, relation: Relation) -> Result<SmallList<ActionCard, 12>, Unknown> {
    let mut out = SmallList::new();
    let Some(p) = s.players().by_seat(who) else { return Ok(out) };
    if p.faction == Faction::TheYssarilTribes {
        // Transparasteel Plating: the string hook is the faction module's.
        return Err(Unknown::YssarilTransparasteel);
    }
    if barred || k.hooks.economy.action_cards_forbidden(s, who) {
        return Ok(out);
    }
    let scope = Scope(u32::MAX);
    let salvaged = crate::query::explore::salvaged(s, who);
    for card in p.cards.hand.as_slice().iter().chain(salvaged) {
        if p.cards.reaction_modes.get(card) == Some(&ReactionMode::Never) {
            continue;
        }
        let Some(spec) = window_of(card.data().timing) else { continue };
        if spec.event != event.kind || spec.relation != relation || !spec.guard.holds(event, who, s, k) {
            continue;
        }
        if !can_matter(s, scope, who, *card, event)? {
            continue;
        }
        out.push(*card);
    }
    Ok(out)
}

/// `action_cards::reaction_can_matter`: the printed cost first (rules version 23), then the per-card requirement
/// for a window that names a system.
fn can_matter(s: &CoreState, scope: Scope, who: Seat, card: ActionCard, event: &EventRec) -> Result<bool, Unknown> {
    if s.meta().rules_at_least(RULES_ACTION_CARD_COSTS) {
        if card == ActionCard::Bribery {
            return Err(Unknown::BriberyGoods);
        }
        if !cost_is_payable(s, who, card, 0).ok_or(Unknown::CardCost)? {
            return Ok(false);
        }
    }
    let Some(system) = event.payload.system(Key::System) else { return Ok(true) };
    let actor = event.payload.seat(Key::Player);
    reaction_can_matter(s, scope, who, card, Window { system, actor }).ok_or(Unknown::HarnessOrInfiltrate)
}

fn condition(c: &CondCx<'_>) -> bool {
    match playable(c.state, c.kernel, c.barred, c.def.owner, c.event, c.def.relation) {
        Ok(cards) => !cards.is_empty(),
        Err(why) => c.unknown(why),
    }
}

/// Register the slot of every window.
pub fn register(t: &mut AbilityTable) {
    for (i, (event, relation)) in windows().into_iter().enumerate() {
        let slot = Slot(SLOT_BASE + u16::try_from(i).unwrap_or(0));
        t.add(
            Spec::eligibility_only(slot, AbilityKind::Reaction, event, relation, condition)
                .optional()
                .frequency(crate::choice::reaction::Frequency::Unlimited)
                .repeatable(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_printed_window_has_one_slot_and_the_slot_numbers_are_dense() {
        let w = windows();
        assert!(w.len() > 20 && w.len() < 100, "{}", w.len());
        let mut t = AbilityTable::default();
        register(&mut t);
        assert_eq!(t.len(), w.len());
        for (i, _) in w.iter().enumerate() {
            assert_eq!(window_of_slot(Slot(SLOT_BASE + u16::try_from(i).unwrap())), Some(w[i]));
        }
        assert!(w.contains(&(EventKind::HitsToAssign, Relation::When)));
    }

    #[test]
    fn a_condition_the_core_cannot_answer_is_unknown_not_false() {
        use crate::state::{Payload, Player, Players};
        let mut s = CoreState::all_loaded();
        s.players = Players::from_vec(vec![Player::new(Seat(0), Faction::TheYssarilTribes), Player::new(Seat(1), Faction::TheVuilraithCabal)]);
        s.players_mut().by_seat_mut(Seat(0)).unwrap().cards.hand.push(ActionCard::ShieldsHolding1);
        let k = Kernel::plain();
        let e = EventRec { id: 1, kind: EventKind::HitsToAssign, payload: Payload::new().with(Key::Player, crate::state::Val::Seat(Seat(0))), cancelled: false };
        assert_eq!(playable(&s, &k, false, Seat(0), &e, Relation::When), Err(Unknown::YssarilTransparasteel));
        // Another faction with the card in hand: Shields Holding listens to the hits before they are assigned.
        s.players_mut().by_seat_mut(Seat(1)).unwrap().cards.hand.push(ActionCard::ShieldsHolding1);
        let e1 = EventRec { id: 1, kind: EventKind::HitsToAssign, payload: Payload::new().with(Key::Player, crate::state::Val::Seat(Seat(1))), cancelled: false };
        assert_eq!(playable(&s, &k, false, Seat(1), &e1, Relation::When).unwrap().as_slice(), &[ActionCard::ShieldsHolding1]);
        assert!(playable(&s, &k, true, Seat(1), &e1, Relation::When).unwrap().is_empty(), "Political Secret bars the play");
        assert!(playable(&s, &k, false, Seat(1), &e1, Relation::After).unwrap().is_empty(), "the other relation is another window");
    }
}
