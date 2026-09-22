//! Trade arena scoring: what one settled deal is worth to each side.
//!
//! `plans/TRADE_ARENA_VALUE_SHEET_2026-09-22.md`, with the operator's values. Every item has a value
//! to the seat that receives it and a cost to the seat that gives it, in trade goods, judged from the
//! position the negotiation started in. A seat's score is its own net gain minus [`ALPHA`] times its
//! partner's:
//!
//! ```text
//! own_i   = sum V(items i receives) - sum C(items i gives)
//! score_i = own_i - ALPHA * own_j
//! ```
//!
//! Below 1 a deal can be good for both sides; at 1 every deal would be exactly zero-sum.

use serde::Serialize;
use ti4_content::ContentStore;
use ti4_model::{DealRevision, DealTerm, GameState, PlayerId, TransferAsset};

/// How much of the partner's net gain counts against a seat (operator, 2026-09-22: 0.75 for now).
pub const ALPHA: f64 = 0.75;
/// Trade goods per victory point: the public objective that spends 5 trade goods.
pub const TRADE_GOODS_PER_VP: f64 = 5.0;
/// Support for the Throne: below a full point because it can be lost back and has a drawback.
pub const SUPPORT_VALUE: f64 = 4.5;
/// Action cards: a flat base, the card's own worth is not modelled.
pub const ACTION_CARD_VALUE: f64 = 1.0;
/// A commodity's cost to its owner: only the chance to trade it elsewhere.
pub const COMMODITY_COST: f64 = 0.25;
/// A promise (or anything else that settles in the future) is worth nothing to receive and costs a
/// little to give, so the policy starts biased against them (operator).
pub const PROMISE_VALUE: f64 = 0.0;
pub const PROMISE_COST: f64 = 0.1;
/// Rounds a full game is played for; later rounds leave less time for a note to pay off.
pub const LAST_ROUND: u32 = 4;

/// One side's view of a settled deal.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct SideValue {
    /// Sum of what this seat received, valued to it.
    pub received: f64,
    /// Sum of what this seat gave, costed to it.
    pub given: f64,
}

impl SideValue {
    #[must_use]
    pub fn net(&self) -> f64 {
        self.received - self.given
    }
}

/// Both seats' scores for one deal, in trade goods.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct DealScore {
    pub proposer: SideValue,
    pub recipient: SideValue,
    pub proposer_score: f64,
    pub recipient_score: f64,
}

/// Score an accepted revision against the position the negotiation opened in.
#[must_use]
pub fn score(
    state: &GameState,
    content: &ContentStore,
    proposer: &PlayerId,
    recipient: &PlayerId,
    revision: &DealRevision,
) -> DealScore {
    // The proposer's terms go from the proposer to the recipient, and the other way round.
    let (proposer_gives, recipient_receives) = side(
        state,
        content,
        &revision.proposer_terms,
        proposer,
        recipient,
    );
    let (recipient_gives, proposer_receives) = side(
        state,
        content,
        &revision.recipient_terms,
        recipient,
        proposer,
    );
    combine(
        SideValue {
            received: proposer_receives,
            given: proposer_gives,
        },
        SideValue {
            received: recipient_receives,
            given: recipient_gives,
        },
    )
}

/// Both sides' scores from what each received and gave.
#[must_use]
pub fn combine(proposer: SideValue, recipient: SideValue) -> DealScore {
    let proposer_score = proposer.net() - ALPHA * recipient.net();
    let recipient_score = recipient.net() - ALPHA * proposer.net();
    DealScore {
        proposer,
        recipient,
        proposer_score,
        recipient_score,
    }
}

/// One direction of a deal: (what the giver gives up, what the receiver gains).
fn side(
    state: &GameState,
    content: &ContentStore,
    terms: &[DealTerm],
    giver: &PlayerId,
    receiver: &PlayerId,
) -> (f64, f64) {
    let mut ledger = Fragments::of(state, giver, receiver);
    let (mut cost, mut value) = (0.0, 0.0);
    for term in terms {
        let (v, c) = item(state, content, giver, term, &mut ledger);
        value += v;
        cost += c;
    }
    (cost, value)
}

/// Fragment holdings on both sides as items move, so a second fragment is valued after the first.
struct Fragments {
    giver: std::collections::BTreeMap<String, i32>,
    receiver: std::collections::BTreeMap<String, i32>,
}

impl Fragments {
    fn of(state: &GameState, giver: &PlayerId, receiver: &PlayerId) -> Self {
        let held = |player: &PlayerId| {
            state
                .player(player)
                .map(|seat| seat.relic_fragments.clone())
                .unwrap_or_default()
        };
        Self {
            giver: held(giver),
            receiver: held(receiver),
        }
    }
}

/// How far toward a set of three a holder is for this fragment kind: its own kind plus frontier
/// (unknown) fragments, which count toward any; for a frontier fragment, the best kind held.
fn toward_set(held: &std::collections::BTreeMap<String, i32>, key: &str) -> i32 {
    let frontier = held.get("FRONTIER").copied().unwrap_or(0);
    if key == "FRONTIER" {
        ["CULTURAL", "HAZARDOUS", "INDUSTRIAL"]
            .iter()
            .map(|kind| held.get(*kind).copied().unwrap_or(0))
            .max()
            .unwrap_or(0)
            + frontier
    } else {
        held.get(key).copied().unwrap_or(0) + frontier
    }
}

/// A fragment's worth to a holder that already has `toward` toward a set: a relic is valued at one
/// point (5 TG) paid over three fragments, the one that completes a set counting most.
fn fragment_worth(toward: i32) -> f64 {
    match toward.rem_euclid(3) {
        2 => 2.5,
        1 => 1.5,
        _ => 1.0,
    }
}

fn item(
    state: &GameState,
    content: &ContentStore,
    giver: &PlayerId,
    term: &DealTerm,
    fragments: &mut Fragments,
) -> (f64, f64) {
    let DealTerm::ImmediateTransfer(asset) = term else {
        return (PROMISE_VALUE, PROMISE_COST);
    };
    match asset {
        TransferAsset::TradeGoods(n) => (f64::from(*n), f64::from(*n)),
        TransferAsset::Commodities(n) => (f64::from(*n), COMMODITY_COST * f64::from(*n)),
        TransferAsset::CulturalFragments(n) => fragment_move(fragments, "CULTURAL", *n),
        TransferAsset::HazardousFragments(n) => fragment_move(fragments, "HAZARDOUS", *n),
        TransferAsset::IndustrialFragments(n) => fragment_move(fragments, "INDUSTRIAL", *n),
        TransferAsset::UnknownFragments(n) => fragment_move(fragments, "FRONTIER", *n),
        TransferAsset::ActionCard(_) => (ACTION_CARD_VALUE, ACTION_CARD_VALUE),
        TransferAsset::PromissoryNote(note) => note_move(state, content, giver, note),
        // Not modelled in v1: priced like a promise.
        TransferAsset::SecretObjective(_) => (PROMISE_VALUE, PROMISE_COST),
    }
}

fn fragment_move(fragments: &mut Fragments, key: &str, count: u8) -> (f64, f64) {
    let (mut value, mut cost) = (0.0, 0.0);
    for _ in 0..count {
        let giver_left = toward_set(&fragments.giver, key) - 1;
        cost += fragment_worth(giver_left.max(0));
        *fragments.giver.entry(key.to_owned()).or_insert(0) -= 1;
        value += fragment_worth(toward_set(&fragments.receiver, key));
        *fragments.receiver.entry(key.to_owned()).or_insert(0) += 1;
    }
    (value, cost)
}

fn rounds_left(state: &GameState) -> u32 {
    LAST_ROUND.saturating_sub(state.round)
}

/// Whether an agenda can still be revealed before the game ends.
fn agenda_chance(state: &GameState) -> f64 {
    if state.custodians_removed {
        1.0
    } else if rounds_left(state) >= 1 {
        0.1
    } else {
        0.0
    }
}

fn commodity_value(content: &ContentStore, faction: &str) -> f64 {
    ti4_content::factions::get(content, faction).map_or(0.0, |f| f64::from(f.commodities()))
}

/// A note's worth to `holder`, and what its owner gives up by parting with it.
#[expect(
    clippy::match_same_arms,
    reason = "one row per note, as the value sheet lists them"
)]
fn note_worth(state: &GameState, content: &ContentStore, note: &str) -> (f64, f64) {
    let owner = ti4_engine::promissory::owner_of(note).unwrap_or_default();
    let later = rounds_left(state) >= 1;
    match ti4_engine::promissory::alias_of(note) {
        _ if note.starts_with(ti4_engine::promissory::SUPPORT_PREFIX) => (SUPPORT_VALUE, 0.0),
        "ta" => {
            let chance = if later { 0.6 } else { 0.3 };
            let goods = commodity_value(content, &owner) * chance;
            (goods, COMMODITY_COST * goods)
        }
        // Adjacency is not judged in v1: the midpoint of the sheet's 1 and 0.25.
        "cf" => (0.5, 0.0),
        "ps" | "favor" => (agenda_chance(state), 0.0),
        "an" => (2.0, 0.0),
        "war_funding" => (1.0, 0.0),
        "convoys" => (1.0, 0.0),
        "ms" => (2.0, 0.5),
        "ce" => (1.5, 0.5),
        "ra" => (if later { 2.0 } else { 0.5 }, 0.0),
        _ => (0.5, 0.0),
    }
}

fn note_move(
    state: &GameState,
    content: &ContentStore,
    giver: &PlayerId,
    note: &str,
) -> (f64, f64) {
    let (worth, owner_cost) = note_worth(state, content, note);
    let owner = ti4_engine::promissory::owner_of(note)
        .and_then(|name| ti4_engine::promissory::seat_of(state, &name));
    // The owner gives up what the sheet charges; anyone passing on a note they hold loses its
    // worth to them.
    let cost = if owner.as_ref() == Some(giver) {
        owner_cost
    } else {
        worth
    };
    (worth, cost)
}

/// Trade goods to the victory-point scale the PPO returns use.
#[must_use]
pub fn as_return(score_in_trade_goods: f64) -> f64 {
    score_in_trade_goods / TRADE_GOODS_PER_VP
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fragment_that_completes_a_set_counts_most() {
        assert!((fragment_worth(2) - 2.5).abs() < 1e-12);
        assert!((fragment_worth(1) - 1.5).abs() < 1e-12);
        assert!((fragment_worth(0) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn below_one_alpha_commodities_for_goods_are_good_for_both() {
        // Three commodities (3 to the receiver, 0.75 to their owner) for two trade goods.
        let score = combine(
            SideValue {
                received: 3.0,
                given: 2.0,
            },
            SideValue {
                received: 2.0,
                given: 0.75,
            },
        );
        assert!(score.proposer_score > 0.0, "{score:?}");
        assert!(score.recipient_score > 0.0, "{score:?}");
    }

    #[test]
    fn giving_support_away_for_nothing_costs_the_giver() {
        let score = combine(
            SideValue {
                received: 0.0,
                given: 0.0,
            },
            SideValue {
                received: SUPPORT_VALUE,
                given: 0.0,
            },
        );
        assert!((score.proposer_score + ALPHA * SUPPORT_VALUE).abs() < 1e-12);
    }
}
