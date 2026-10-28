//! Package-local readers of the CE factions (the string engine's `promissory::has_commander_ability` and friends).

use ti4_engine_core::generated::{Faction, Leader, PromissoryNote};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::keys::{Atom, MarkKey, MarkVal};
use ti4_engine_core::state::{CoreState, LeaderStatus};

use crate::common::{is_faction, mark, player};

/// Whether `who`'s `leader` has the status `status`.
pub fn leader_is(s: &CoreState, who: Seat, leader: Leader, status: LeaderStatus) -> bool {
    player(s, who).is_some_and(|p| p.leaders.status.get(&leader) == Some(&status))
}

/// `promissory::has_commander_ability(player, commander)`: a granted mark, the commander unlocked, the Mahact's
/// Imperia (another player's unlocked commander while their token is in the fleet pool), or a faceup Alliance from a
/// seated owner whose commander is unlocked.
pub fn has_commander_ability(s: &CoreState, who: Seat, commander: Leader) -> bool {
    let unlocked = |seat: Seat| leader_is(s, seat, commander, LeaderStatus::Unlocked);
    if mark(s, MarkKey::CommanderAbility(who, commander)).is_some() || unlocked(who) {
        return true;
    }
    if is_faction(s, who, Faction::TheMahactGeneSorcerers)
        && let Some(MarkVal::List(pool)) = mark(s, MarkKey::MahactFleet(who))
    {
        let owned = pool.iter().any(|v| matches!(v, MarkVal::Atom(Atom::Seat(owner)) if *owner != who && unlocked(*owner)));
        if owned {
            return true;
        }
    }
    s.cards().promissory.iter().any(|(key, holder)| {
        *holder == who
            && key.note == PromissoryNote::Alliance
            && s.cards().promissory_faceup.contains_key(key)
            && s.seats()
                .seating_order()
                .iter()
                .copied()
                .find(|seat| is_faction(s, *seat, key.faction))
                .is_some_and(|owner| owner != who && unlocked(owner))
    })
}
