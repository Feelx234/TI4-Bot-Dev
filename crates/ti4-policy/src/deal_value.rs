//! Deal values: the trade arena's value sheet, as facts a policy reads on diplomacy options.
//!
//! `plans/TRADE_ARENA_VALUE_SHEET_2026-09-22.md`, with the operator's values. Every item has a value
//! to the seat that receives it and a cost to the seat that gives it, in trade goods, judged from the
//! position. A seat's score is its own net gain minus [`ALPHA`] times its partner's:
//!
//! ```text
//! own_i   = sum V(items i receives) - sum C(items i gives)
//! score_i = own_i - ALPHA * own_j
//! ```
//!
//! Like the battle predictor, this is a fixed component the policy reads rather than something it
//! learns: [`deal_facts`] puts the value of what each diplomacy option would leave on the table onto
//! that option, and PPO learns how much to trust it. Unlike combat, the sheet is exact and cheap, so
//! no network stands in for it.
//!
//! Everything read is public: turn, custodians, factions and relic fragments (faceup, 35.9), and
//! the terms of the deal itself.

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;
use ti4_content::ContentStore;
use ti4_engine::choice::{Choice, Observed};
use ti4_engine::diplomacy::builder::Draft;
use ti4_model::{DealTerm, GameState, PlayerId, TransferAsset};

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

/// The fact names, in the `diplomacy:` family. A bundle whose vocabulary does not place them never
/// sees them (see [`FACT_NAMES`] and the bot).
pub const FACT_OWN: &str = "diplomacy:deal-value-own";
pub const FACT_PARTNER: &str = "diplomacy:deal-value-partner";
pub const FACT_SCORE: &str = "diplomacy:deal-value-score";
pub const FACT_NAMES: [&str; 3] = [FACT_OWN, FACT_PARTNER, FACT_SCORE];

/// The public facts the sheet reads.
#[derive(Debug, Clone)]
pub struct Position<'a> {
    pub content: &'a ContentStore,
    pub round: u32,
    pub custodians_removed: bool,
    /// Each seat's faction name, which is what note ids carry.
    pub factions: BTreeMap<PlayerId, String>,
    /// Faceup relic fragments by trait key (`CULTURAL`, …, `FRONTIER`).
    pub fragments: BTreeMap<PlayerId, BTreeMap<String, i32>>,
}

impl<'a> Position<'a> {
    #[must_use]
    pub fn of_state(state: &GameState, content: &'a ContentStore) -> Self {
        Self {
            content,
            round: state.round,
            custodians_removed: state.custodians_removed,
            factions: state
                .players
                .iter()
                .map(|seat| (seat.id.clone(), seat.faction.as_str().to_owned()))
                .collect(),
            fragments: state
                .players
                .iter()
                .map(|seat| (seat.id.clone(), seat.relic_fragments.clone()))
                .collect(),
        }
    }

    #[must_use]
    pub fn of_view(observed: &Observed<'a>) -> Self {
        let mut factions = BTreeMap::new();
        let mut fragments = BTreeMap::new();
        for player in observed.players() {
            if let Some(seat) = observed.seat(player) {
                factions.insert(player.clone(), seat.faction.as_str().to_owned());
                fragments.insert(player.clone(), seat.relic_fragments.clone());
            }
        }
        Self {
            content: observed.content(),
            round: observed.round(),
            custodians_removed: observed.custodians_removed(),
            factions,
            fragments,
        }
    }

    fn seat_of(&self, faction: &str) -> Option<&PlayerId> {
        self.factions
            .iter()
            .find(|(_, name)| name.as_str() == faction)
            .map(|(seat, _)| seat)
    }

    fn rounds_left(&self) -> u32 {
        LAST_ROUND.saturating_sub(self.round)
    }
}

/// One side's view of a deal.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct SideValue {
    /// Sum of what this seat receives, valued to it.
    pub received: f64,
    /// Sum of what this seat gives, costed to it.
    pub given: f64,
}

impl SideValue {
    #[must_use]
    pub fn net(&self) -> f64 {
        self.received - self.given
    }
}

/// Both seats' values for one deal, in trade goods.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct DealScore {
    pub proposer: SideValue,
    pub recipient: SideValue,
    pub proposer_score: f64,
    pub recipient_score: f64,
}

/// Value a deal: `proposer_terms` go from the proposer to the recipient, `recipient_terms` back.
#[must_use]
pub fn score(
    position: &Position<'_>,
    proposer: &PlayerId,
    recipient: &PlayerId,
    proposer_terms: &[DealTerm],
    recipient_terms: &[DealTerm],
) -> DealScore {
    let (proposer_gives, recipient_receives) = side(position, proposer_terms, proposer, recipient);
    let (recipient_gives, proposer_receives) = side(position, recipient_terms, recipient, proposer);
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
    position: &Position<'_>,
    terms: &[DealTerm],
    giver: &PlayerId,
    receiver: &PlayerId,
) -> (f64, f64) {
    let held = |player: &PlayerId| position.fragments.get(player).cloned().unwrap_or_default();
    let mut fragments = Fragments {
        giver: held(giver),
        receiver: held(receiver),
    };
    let (mut cost, mut value) = (0.0, 0.0);
    for term in terms {
        let (v, c) = item(position, giver, term, &mut fragments);
        value += v;
        cost += c;
    }
    (cost, value)
}

/// Fragment holdings on both sides as items move, so a second fragment is valued after the first.
struct Fragments {
    giver: BTreeMap<String, i32>,
    receiver: BTreeMap<String, i32>,
}

/// How far toward a set of three a holder is for this fragment kind: its own kind plus frontier
/// (unknown) fragments, which count toward any; for a frontier fragment, the best kind held.
fn toward_set(held: &BTreeMap<String, i32>, key: &str) -> i32 {
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
    position: &Position<'_>,
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
        TransferAsset::PromissoryNote(note) => note_move(position, giver, note),
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

/// Whether an agenda can still be revealed before the game ends.
fn agenda_chance(position: &Position<'_>) -> f64 {
    if position.custodians_removed {
        1.0
    } else if position.rounds_left() >= 1 {
        0.1
    } else {
        0.0
    }
}

/// A note's worth to its holder, and what its owner gives up by parting with it.
#[expect(
    clippy::match_same_arms,
    reason = "one row per note, as the value sheet lists them"
)]
fn note_worth(position: &Position<'_>, note: &str) -> (f64, f64) {
    let owner = ti4_engine::promissory::owner_of(note).unwrap_or_default();
    let later = position.rounds_left() >= 1;
    match ti4_engine::promissory::alias_of(note) {
        _ if note.starts_with(ti4_engine::promissory::SUPPORT_PREFIX) => (SUPPORT_VALUE, 0.0),
        "ta" => {
            let chance = if later { 0.6 } else { 0.3 };
            let goods = ti4_content::factions::get(position.content, &owner)
                .map_or(0.0, |faction| f64::from(faction.commodities()))
                * chance;
            (goods, COMMODITY_COST * goods)
        }
        // Adjacency is not judged in v1: the midpoint of the sheet's 1 and 0.25.
        "cf" => (0.5, 0.0),
        "ps" | "favor" => (agenda_chance(position), 0.0),
        "an" => (2.0, 0.0),
        "war_funding" => (1.0, 0.0),
        "convoys" => (1.0, 0.0),
        "ms" => (2.0, 0.5),
        "ce" => (1.5, 0.5),
        "ra" => (if later { 2.0 } else { 0.5 }, 0.0),
        _ => (0.5, 0.0),
    }
}

fn note_move(position: &Position<'_>, giver: &PlayerId, note: &str) -> (f64, f64) {
    let (worth, owner_cost) = note_worth(position, note);
    let owner = ti4_engine::promissory::owner_of(note);
    let owned = owner.as_deref().and_then(|name| position.seat_of(name)) == Some(giver);
    // The owner gives up what the sheet charges; anyone passing on a note they hold loses its
    // worth to them.
    (worth, if owned { owner_cost } else { worth })
}

/// Trade goods to the victory-point scale returns and facts use.
#[must_use]
pub fn in_points(trade_goods: f64) -> f64 {
    trade_goods / TRADE_GOODS_PER_VP
}

/// Subtypes whose options carry deal facts.
const BUILDING: [&str; 4] = [
    "diplomacy_offer_item",
    "diplomacy_ask_item",
    "diplomacy_amount",
    "diplomacy_review",
];

/// Each option's deal facts, one list per option; empty for anything that is not a negotiation.
///
/// Facts are from the deciding seat's side, in victory points: its own net gain, its partner's,
/// and its score, for the deal the option would leave on the table. Walking away, declining and
/// signals leave no deal and carry no facts, which reads as zero.
#[must_use]
pub fn deal_facts(observed: &Observed<'_>, choice: &Choice) -> Vec<Vec<(&'static str, f64)>> {
    let empty = || vec![Vec::new(); choice.options.len()];
    let Some(subtype) = choice.context.as_ref().map(|c| c.subtype.as_str()) else {
        return empty();
    };
    let position = Position::of_view(observed);
    if BUILDING.contains(&subtype) {
        return choice
            .options
            .iter()
            .map(|option| {
                let Some(draft) = option
                    .payload
                    .get("draft")
                    .and_then(|value| serde_json::from_value::<Draft>(value.clone()).ok())
                else {
                    return Vec::new();
                };
                preview(&draft, &option.id, position.round)
                    .map(|after| {
                        facts(
                            &position,
                            &after.builder,
                            &after.other,
                            &after.give,
                            &after.take,
                        )
                    })
                    .unwrap_or_default()
            })
            .collect();
    }
    if subtype == "diplomacy_response" {
        return choice
            .options
            .iter()
            .map(|option| {
                if option.id != ti4_engine::diplomacy::window::ACCEPT_ID {
                    return Vec::new();
                }
                let Some(revision) = option.payload.get("bundle").and_then(|b| b.get("revision"))
                else {
                    return Vec::new();
                };
                let terms = |side: &str| -> Vec<DealTerm> {
                    revision
                        .get(side)
                        .cloned()
                        .and_then(|value| serde_json::from_value(value).ok())
                        .unwrap_or_default()
                };
                let (proposer_terms, recipient_terms) =
                    (terms("proposer_terms"), terms("recipient_terms"));
                let actor = &choice.player;
                let Some(other) = option
                    .payload
                    .get("bundle")
                    .and_then(|b| b.get("revision"))
                    .and_then(|r| r.get("author"))
                    .and_then(Value::as_str)
                    .map(PlayerId::new)
                else {
                    return Vec::new();
                };
                // The actor answers the other side's revision: what the actor gives is the side
                // that is the actor's own.
                let actor_is_proposer = option
                    .payload
                    .get("actor_is_proposer")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let (give, take) = if actor_is_proposer {
                    (proposer_terms, recipient_terms)
                } else {
                    (recipient_terms, proposer_terms)
                };
                facts(&position, actor, &other, &give, &take)
            })
            .collect();
    }
    empty()
}

/// The draft an option would leave, or `None` for an option that ends the build with no deal.
fn preview(draft: &Draft, id: &str, round: u32) -> Option<Draft> {
    use ti4_engine::diplomacy::builder::{CANCEL_ID, DONE_ID, EDIT_ID, PROPOSE_ID};
    if id == CANCEL_ID || !id.starts_with("diplomacy|") || id.starts_with("diplomacy|signal|") {
        return None;
    }
    let mut after = draft.clone();
    if id == DONE_ID || id == EDIT_ID || id == PROPOSE_ID {
        return Some(after);
    }
    if !ti4_engine::diplomacy::builder::apply_item_at(round, &mut after, id) {
        return None;
    }
    // A counted item is only picked here; its amount comes next. Preview one of it.
    if after.pending.is_some() {
        ti4_engine::diplomacy::builder::apply_item_at(round, &mut after, "diplomacy|amount|1");
    }
    Some(after)
}

/// The deciding seat's facts for a deal where it gives `give` and receives `take`.
fn facts(
    position: &Position<'_>,
    actor: &PlayerId,
    other: &PlayerId,
    give: &[DealTerm],
    take: &[DealTerm],
) -> Vec<(&'static str, f64)> {
    let valued = score(position, actor, other, give, take);
    vec![
        (FACT_OWN, in_points(valued.proposer.net())),
        (FACT_PARTNER, in_points(valued.recipient.net())),
        (FACT_SCORE, in_points(valued.proposer_score)),
    ]
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
            SideValue::default(),
            SideValue {
                received: SUPPORT_VALUE,
                given: 0.0,
            },
        );
        assert!((score.proposer_score + ALPHA * SUPPORT_VALUE).abs() < 1e-12);
    }

    fn position(content: &ContentStore) -> Position<'_> {
        Position {
            content,
            round: 2,
            custodians_removed: false,
            factions: [("a", "hacan"), ("b", "sol")]
                .into_iter()
                .map(|(seat, faction)| (PlayerId::new(seat), faction.to_owned()))
                .collect(),
            fragments: BTreeMap::new(),
        }
    }

    #[test]
    fn each_builder_option_is_valued_by_the_draft_it_leaves() {
        let content = ContentStore::embedded();
        let position = position(content);
        let draft = Draft::new(PlayerId::new("a"), PlayerId::new("b"));
        // Offering one's own Support: nothing for the builder, 4.5 for the partner.
        let after = preview(&draft, "diplomacy|note|support:hacan", position.round).expect("item");
        let f = facts(
            &position,
            &after.builder,
            &after.other,
            &after.give,
            &after.take,
        );
        assert_eq!(f[0], (FACT_OWN, 0.0));
        assert!((f[1].1 - in_points(SUPPORT_VALUE)).abs() < 1e-12, "{f:?}");
        assert!(
            (f[2].1 + ALPHA * in_points(SUPPORT_VALUE)).abs() < 1e-12,
            "{f:?}"
        );
        // Picking trade goods previews one of them.
        let after = preview(&draft, "diplomacy|now|tg", position.round).expect("item");
        assert_eq!(after.give.len(), 1);
        // Walking away leaves no deal.
        assert!(
            preview(
                &draft,
                ti4_engine::diplomacy::builder::CANCEL_ID,
                position.round
            )
            .is_none()
        );
    }
}
