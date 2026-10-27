//! `TradeState`: transactions this round/turn, negotiations, promises, diplomacy.

mod diplomacy;

pub use diplomacy::*;

use crate::containers::{Cow, SeatSet, SortedMap};
use crate::ids::Seat;

use super::Code;

/// A legacy promise text (`PromiseKey.2`, deserialize-only); adapter-interned.
pub type Promise = Code;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TradeState {
    /// `GameState.transactions_this_round`: (initiator, partner) pairs, duplicates kept.
    pub this_round: Cow<Vec<(Seat, Seat)>>,
    pub negotiations_this_action: SortedMap<Seat, u8, 8>,
    pub negotiations_this_round: SortedMap<Seat, u8, 8>,
    pub transactions_this_turn: SortedMap<Seat, SeatSet, 8>,
    pub traded_goods: SortedMap<Seat, i32, 8>,
    pub traded_goods_for_promissory: SortedMap<Seat, i32, 8>,
    /// `GameState.promises`: (promiser, partner, promise) -> kept.
    pub promises: Cow<SortedMap<(Seat, Seat, Promise), Option<bool>, 4>>,
    pub support_holders: SortedMap<Seat, Seat, 8>,
    pub diplomacy: Cow<Diplomacy>,
}
