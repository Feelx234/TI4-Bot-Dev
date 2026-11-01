//! The relic component actions of the action menu (`relics::perform`): purging fragments for a relic. Using a relic with an action, and the
//! Enigmatic Device cards, are the counted gap [`crate::kernel::turn_codes::MenuGap::Unrouted`] until their windows are native.

use super::component::{conclude, Verdict};
use super::inline::{gap_flow, run as run_window, Flow};
use crate::generated::{ExploreType, Relic};
use crate::ids::Seat;
use crate::kernel::stage::Sx;
use crate::kernel::turn_codes::{MenuGap, TurnGap};

/// "Purge 3 relic fragments of a trait to gain a relic" (`relics::perform`, the `purge|` option): three fragments (frontier ones make up a
/// shortfall) for the top relic. The Shard of the Throne scores at once and its record says so. A purge that cannot be paid (too few fragments, an
/// empty relic deck) fails and is not offered again this turn.
pub fn purge(sx: &mut Sx<'_>, who: Seat, key: u32, kind: ExploreType) -> Flow {
    let gained = crate::rules::explore::purge_for_relic_scored(sx.state, who, kind);
    conclude(sx, who, key, if gained.is_some() { Verdict::Resolved } else { Verdict::Failed { withhold: true } }, false)
}

/// Whether using the relic has a native body ([`use_relic`] never ends it in the counted gap).
#[must_use]
pub const fn routed(relic: Relic) -> bool {
    matches!(relic, Relic::DynamisCore | Relic::BookOfLatvinia | Relic::TheCodex)
}

/// Using a relic with its printed ACTION (`relics::use_relic`, then the purge): the Dynamis Core, the Book of Latvinia and the Codex are native; the
/// rest (the Enigmatic Device and the Maw of Worlds research, the Stellar Converter, the Titan Prototype, the Silver Flame's die) are the counted gap
/// [`MenuGap::Unrouted`].
pub fn use_relic(sx: &mut Sx<'_>, who: Seat, key: u32, relic: Relic) -> Flow {
    match relic {
        Relic::DynamisCore => {
            // "Gain trade goods equal to your commodity value, +2": the faction's printed number, not what the seat holds.
            let value = sx.state.players().by_seat(who).map_or(0, |p| i32::from(p.faction.data().commodities)) + 2;
            crate::kernel::fx::gain_trade_goods_staged(sx.state, who, value, sx.table.codes.turn.relic);
        }
        Relic::BookOfLatvinia => {
            crate::rules::explore::book_of_latvinia(sx.state, who, crate::query::reach::scope_of(sx.kernel));
        }
        Relic::TheCodex => {
            let mut window = crate::rules::relic_windows::Codex::new(who);
            if run_window(sx, &mut window) == Flow::Halt {
                return Flow::Halt;
            }
        }
        // mutants:skip (the key default): every option on the menu has a key
        other => return gap_flow(sx, TurnGap::Menu(MenuGap::Unrouted(7, crate::choice::turn::Opt::UseRelic(other).key().unwrap_or(0)))),
    }
    // A relic used is purged; the three bodies above cannot fail (the menu offers a relic only when it can resolve).
    crate::rules::explore::purge_relic(sx.state, who, relic);
    conclude(sx, who, key, Verdict::Resolved, false)
}
