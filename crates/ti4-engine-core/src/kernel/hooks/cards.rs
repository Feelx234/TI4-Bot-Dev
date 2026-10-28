//! Hidden-hand, transaction, vote and borrowed-agent hooks (`factions/hooks_cards.rs`, 7 slots).

use super::{RevealKind, hook_group};
use crate::generated::Leader;
use crate::ids::Seat;
use crate::kernel::{Cx, Kernel};
use crate::state::CoreState;

hook_group! {
    /// `hooks_cards::CardHooks`.
    CardHooks {
        /// `viewer` may look at `owner`'s hidden cards of `kind`. String: `choice` (views), `action_cards`.
        any may_view_hand(s: &CoreState, k: &Kernel, viewer: Seat, owner: Seat, kind: RevealKind) -> bool;
        /// `who` may transact with `partner` although they are not neighbours. String: `transactions`.
        any transaction_reach(s: &CoreState, k: &Kernel, who: Seat, partner: Seat) -> bool;
        /// The per-turn transaction limit does not apply. String: `transactions`.
        any transaction_limit_exempt(s: &CoreState, k: &Kernel, who: Seat, partner: Seat) -> bool;
        /// `who` votes first. String: `vote`.
        any votes_first(s: &CoreState, who: Seat) -> bool;
        /// Extra votes that need the content tables (summed). String: `vote`.
        sum vote_bonus_with_content(s: &CoreState, k: &Kernel, who: Seat) -> i64;
        /// A borrowed agent was used (with the timing context). String: `leaders`.
        each borrowed_agent_used(cx: &mut Cx<'_>, who: Seat, leader: Leader) -> ();
        /// A borrowed agent was used (state only). String: `leaders`.
        each borrowed_agent_used_state(cx: &mut Cx<'_>, who: Seat, leader: Leader) -> ();
    }
}
