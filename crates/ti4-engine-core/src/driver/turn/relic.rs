//! The relic component actions of the action menu (`relics::perform`): purging fragments for a relic. Using a relic with an action, and the
//! Enigmatic Device cards, are the counted gap [`crate::kernel::turn_codes::MenuGap::Unrouted`] until their windows are native.

use super::component::{conclude, Verdict};
use super::inline::Flow;
use crate::generated::ExploreType;
use crate::ids::Seat;
use crate::kernel::stage::Sx;

/// "Purge 3 relic fragments of a trait to gain a relic" (`relics::perform`, the `purge|` option): three fragments (frontier ones make up a
/// shortfall) for the top relic. The Shard of the Throne scores at once and its record says so. A purge that cannot be paid (too few fragments, an
/// empty relic deck) fails and is not offered again this turn.
pub fn purge(sx: &mut Sx<'_>, who: Seat, key: u32, kind: ExploreType) -> Flow {
    let gained = crate::rules::explore::purge_for_relic_scored(sx.state, who, kind);
    conclude(sx, who, key, if gained.is_some() { Verdict::Resolved } else { Verdict::Failed { withhold: true } }, false)
}
