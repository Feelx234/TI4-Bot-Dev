//! Package CD, the registry line (docs/ENGINE_CORE_FACTIONS_PLAN.md). Owns every file under this directory;
//! nobody else edits it. One sub-directory per faction, one file per tier.

use crate::{Id, Modules};

pub mod naalu;
pub mod naaz;
pub mod sardakk;
pub mod titans;
pub mod yin;
pub mod yssaril;

/// The modules this package provides, with all their tiers.
pub const MODULES: Modules = &[
    (Id::Naalu, naalu::register),
    (Id::Naaz, naaz::register),
    (Id::Sardakk, sardakk::register),
    (Id::Titans, titans::register),
    (Id::Yin, yin::register),
    (Id::Yssaril, yssaril::register),
];

/// The movement slots only (package A2's hooks), for `movement_table`.
pub const MOVEMENT: Modules = &[
    (Id::Yssaril, yssaril::register_movement),
];

/// `promissory::has_commander_ability(player, commander)`: a granted mark, the commander unlocked, the Mahact's Imperia
/// (another player's unlocked commander while their token is in the fleet pool), or a faceup Alliance from a seated owner
/// whose commander is unlocked.
pub(crate) fn has_commander_ability(
    s: &ti4_engine_core::state::CoreState,
    who: ti4_engine_core::ids::Seat,
    commander: ti4_engine_core::generated::Leader,
) -> bool {
    use ti4_engine_core::generated::{Faction, PromissoryNote};
    use ti4_engine_core::keys::{Atom, MarkKey, MarkVal};
    use ti4_engine_core::state::LeaderStatus;

    use crate::common::{is_faction, mark, player};
    let unlocked = |seat| player(s, seat).is_some_and(|p| p.leaders.status.get(&commander) == Some(&LeaderStatus::Unlocked));
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
