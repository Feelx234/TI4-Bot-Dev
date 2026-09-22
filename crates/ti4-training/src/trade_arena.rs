//! Trade arena scoring: what one settled deal is worth to each side.
//!
//! The value sheet itself lives in `ti4_policy::deal_value`, where the policy reads it as facts on
//! diplomacy options; this is the arena's view of it over a whole game state.

use ti4_content::ContentStore;
use ti4_model::{DealRevision, GameState, PlayerId};

pub use ti4_policy::deal_value::{
    ALPHA, DealScore, SUPPORT_VALUE, SideValue, TRADE_GOODS_PER_VP, combine,
};

/// Score an accepted revision against the position the negotiation opened in.
#[must_use]
pub fn score(
    state: &GameState,
    content: &ContentStore,
    proposer: &PlayerId,
    recipient: &PlayerId,
    revision: &DealRevision,
) -> DealScore {
    ti4_policy::deal_value::score(
        &ti4_policy::deal_value::Position::of_state(state, content),
        proposer,
        recipient,
        &revision.proposer_terms,
        &revision.recipient_terms,
    )
}

/// Trade goods to the victory-point scale the PPO returns use.
#[must_use]
pub fn as_return(score_in_trade_goods: f64) -> f64 {
    ti4_policy::deal_value::in_points(score_in_trade_goods)
}
