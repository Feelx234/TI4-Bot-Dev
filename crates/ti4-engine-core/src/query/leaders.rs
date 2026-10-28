//! The commander ability and the leader status, as pure reads of the state (package B7, moved down from
//! `rules::leaders` so that the other rule areas can ask it without importing an area: Gila the Silvertongue
//! and Elder Qanoj in the vote, Rear Admiral Farran in combat). `rules::leaders` re-exports these.

use crate::generated::{Leader, PromissoryNote};
use crate::ids::Seat;
use crate::keys::MarkKey;
use crate::state::{CoreState, LeaderStatus};

/// `leaders::status`.
#[must_use]
pub fn status(core: &CoreState, who: Seat, leader: Leader) -> Option<LeaderStatus> {
    core.players().by_seat(who)?.leaders.status.get(&leader).copied()
}

/// `promissory::has_commander_ability` over the state alone: Mahact's Imperia is read off the fleet pool
/// ([`imperia_from_pool`]). The one copy the faction packages call; a caller with the
/// `mahact_imperia_grants` hook at hand passes it to [`has_commander_ability_with`] instead.
#[must_use]
pub fn has_commander_ability_in(core: &CoreState, who: Seat, commander: Leader) -> bool {
    has_commander_ability_with(core, who, commander, || imperia_from_pool(core, who, commander))
}

/// The routes of `promissory::has_commander_ability` that read the state; `imperia` answers the
/// Mahact route (asked last-but-one, only when the own routes failed).
#[must_use]
pub fn has_commander_ability_with(core: &CoreState, who: Seat, commander: Leader, imperia: impl FnOnce() -> bool) -> bool {
    if core.marks().tail.contains_key(&MarkKey::CommanderAbility(who, commander))
        || status(core, who, commander) == Some(LeaderStatus::Unlocked)
    {
        return true;
    }
    if imperia() {
        return true;
    }
    let cards = core.cards();
    cards.promissory.iter().any(|(key, holder)| {
        *holder == who
            && key.note == PromissoryNote::Alliance
            && cards.promissory_faceup.contains_key(key)
            && core
                .players()
                .iter()
                .find(|p| p.faction == key.faction)
                .is_some_and(|owner| owner.seat != who && status(core, owner.seat, commander) == Some(LeaderStatus::Unlocked))
    })
}

/// `mahact::imperia_grants` on the state: the Mahact seat has another player's unlocked `commander` while
/// that player's command token is in its fleet pool.
#[must_use]
pub fn imperia_from_pool(core: &CoreState, who: Seat, commander: Leader) -> bool {
    use crate::keys::{Atom, MarkVal};
    core.players().by_seat(who).is_some_and(|p| p.faction == crate::generated::Faction::TheMahactGeneSorcerers)
        && matches!(
            core.marks().tail.get(&MarkKey::MahactFleet(who)),
            Some(MarkVal::List(pool)) if pool.iter().any(|v| matches!(v, MarkVal::Atom(Atom::Seat(owner))
                if *owner != who && status(core, *owner, commander) == Some(LeaderStatus::Unlocked)))
        )
}

