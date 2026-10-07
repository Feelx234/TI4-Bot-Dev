//! Deals built item by item (`plans/TRADE_REWORK_2026-09-22.md`).
//!
//! A contact no longer offers a menu of pre-combined bundles. The side building a revision adds
//! what it gives, one item at a time, then what it asks for, then reviews and proposes. Physical
//! components are a transaction (LRR 94) and are offered only when the pair may transact; promises
//! are not a transaction and are always offered.

use serde::{Deserialize, Serialize};
use ti4_content::ContentStore;
use ti4_model::{DealTerm, GameState, PlayerId, SystemId, TransferAsset};

use crate::choice::ChoiceOption;

pub const ITEM_KIND: &str = "diplomacy_item";
pub const AMOUNT_KIND: &str = "diplomacy_amount";
pub const REVIEW_KIND: &str = "diplomacy_review";
pub const DONE_ID: &str = "diplomacy|done";
pub const PROPOSE_ID: &str = "diplomacy|propose";
pub const EDIT_ID: &str = "diplomacy|edit";
pub const CANCEL_ID: &str = "diplomacy|cancel";

/// Largest amount offered for one counted item in one choice.
const MAX_AMOUNT: u8 = 10;
/// Largest amount a promise to pay later may name.
const MAX_LATER: u8 = 5;
/// Item operations (add or remove) one revision may take. Without a bound, adding and removing
/// can cycle forever: a self-play game stalled at the engine's step cap doing exactly that.
pub const MAX_BUILD_STEPS: u8 = 8;

/// What the contact knew when it opened, which the window cannot recompute without the map.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContactScope {
    /// The pair may exchange physical components now: neighbours, Guild Ships, Trade Convoys, or
    /// the agenda phase, and they have not already transacted this turn.
    pub physical: bool,
    /// Seats either side could promise to attack.
    pub attack_targets: Vec<PlayerId>,
    /// Systems worth promising not to activate.
    pub systems: Vec<SystemId>,
    /// The agenda under negotiation and its outcomes, when the contact is held before a vote.
    pub agenda: Option<(String, Vec<String>)>,
}

/// A component that comes in amounts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Counted {
    TradeGoods,
    Commodities,
    Cultural,
    Hazardous,
    Industrial,
    Unknown,
}

impl Counted {
    const ALL: [Self; 6] = [
        Self::TradeGoods,
        Self::Commodities,
        Self::Cultural,
        Self::Hazardous,
        Self::Industrial,
        Self::Unknown,
    ];

    const fn id(self) -> &'static str {
        match self {
            Self::TradeGoods => "tg",
            Self::Commodities => "commodities",
            Self::Cultural => "cultural",
            Self::Hazardous => "hazardous",
            Self::Industrial => "industrial",
            Self::Unknown => "unknown",
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::TradeGoods => "trade goods",
            Self::Commodities => "commodities",
            Self::Cultural => "cultural relic fragments",
            Self::Hazardous => "hazardous relic fragments",
            Self::Industrial => "industrial relic fragments",
            Self::Unknown => "unknown relic fragments",
        }
    }

    fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.id() == id)
    }

    const fn asset(self, amount: u8) -> TransferAsset {
        match self {
            Self::TradeGoods => TransferAsset::TradeGoods(amount),
            Self::Commodities => TransferAsset::Commodities(amount),
            Self::Cultural => TransferAsset::CulturalFragments(amount),
            Self::Hazardous => TransferAsset::HazardousFragments(amount),
            Self::Industrial => TransferAsset::IndustrialFragments(amount),
            Self::Unknown => TransferAsset::UnknownFragments(amount),
        }
    }

    const fn of(asset: &TransferAsset) -> Option<Self> {
        match asset {
            TransferAsset::TradeGoods(_) => Some(Self::TradeGoods),
            TransferAsset::Commodities(_) => Some(Self::Commodities),
            TransferAsset::CulturalFragments(_) => Some(Self::Cultural),
            TransferAsset::HazardousFragments(_) => Some(Self::Hazardous),
            TransferAsset::IndustrialFragments(_) => Some(Self::Industrial),
            TransferAsset::UnknownFragments(_) => Some(Self::Unknown),
            _ => None,
        }
    }

    /// How many of these `player` holds right now.
    fn held(self, state: &GameState, player: &PlayerId) -> u8 {
        let Some(seat) = state.player(player) else {
            return 0;
        };
        let raw = match self {
            Self::TradeGoods => seat.trade_goods,
            Self::Commodities => seat.commodities,
            fragment => seat
                .relic_fragments
                .get(&crate::transactions::fragment_key(fragment.id()))
                .copied()
                .unwrap_or(0),
        };
        u8::try_from(raw.clamp(0, i32::from(u8::MAX))).unwrap_or(0)
    }
}

/// A counted item waiting for its amount.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pending {
    pub kind: Counted,
    /// Paid later (a promise) rather than handed over now.
    pub later: bool,
}

/// The revision being built: what the builder gives, what it asks for, and where it is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Draft {
    pub builder: PlayerId,
    pub other: PlayerId,
    pub give: Vec<DealTerm>,
    pub take: Vec<DealTerm>,
    /// Adding to the ask rather than to the offer.
    pub asking: bool,
    pub reviewing: bool,
    pub pending: Option<Pending>,
    /// Item operations taken so far on this revision; see [`MAX_BUILD_STEPS`].
    #[serde(default)]
    pub steps: u8,
}

impl Draft {
    #[must_use]
    pub const fn new(builder: PlayerId, other: PlayerId) -> Self {
        Self {
            builder,
            other,
            give: Vec::new(),
            take: Vec::new(),
            asking: false,
            reviewing: false,
            pending: None,
            steps: 0,
        }
    }

    /// The side being edited: who hands it over, who receives it, and its terms.
    fn side(&self) -> (&PlayerId, &PlayerId, &Vec<DealTerm>) {
        if self.asking {
            (&self.other, &self.builder, &self.take)
        } else {
            (&self.builder, &self.other, &self.give)
        }
    }

    fn side_mut(&mut self) -> &mut Vec<DealTerm> {
        if self.asking {
            &mut self.take
        } else {
            &mut self.give
        }
    }

    /// Whether this revision may still add or remove items.
    #[must_use]
    pub const fn can_edit(&self) -> bool {
        self.steps < MAX_BUILD_STEPS
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.give.is_empty() && self.take.is_empty()
    }
}

/// One line of a side: "3 trade goods now", "do not attack seat2 through round 2".
#[must_use]
pub fn describe(term: &DealTerm) -> String {
    let asset = |asset: &TransferAsset| match asset {
        TransferAsset::PromissoryNote(note) => format!("promissory note {note}"),
        TransferAsset::ActionCard(card) => format!("action card {card}"),
        TransferAsset::SecretObjective(secret) => format!("secret objective {secret}"),
        other => {
            let label = Counted::of(other).map_or("items", Counted::label);
            let amount = match other {
                TransferAsset::TradeGoods(n)
                | TransferAsset::Commodities(n)
                | TransferAsset::CulturalFragments(n)
                | TransferAsset::HazardousFragments(n)
                | TransferAsset::IndustrialFragments(n)
                | TransferAsset::UnknownFragments(n) => *n,
                _ => 0,
            };
            format!("{amount} {label}")
        }
    };
    match term {
        DealTerm::ImmediateTransfer(what) => format!("{} now", asset(what)),
        DealTerm::FuturePayment {
            asset: what,
            deadline_round,
        } => format!("{} by the end of round {deadline_round}", asset(what)),
        DealTerm::DoNotActivate {
            system,
            deadline_round,
        } => format!("do not activate {system} through round {deadline_round}"),
        DealTerm::DoNotAttack {
            player,
            deadline_round,
        } => format!("do not attack {player} through round {deadline_round}"),
        DealTerm::Vote {
            agenda, outcome, ..
        } => format!("vote {outcome} on {agenda}"),
        DealTerm::Attack {
            player,
            deadline_round,
        } => format!("attack {player} by the end of round {deadline_round}"),
        DealTerm::ReplenishFor {
            beneficiary,
            deadline_round,
        } => format!(
            "replenish {beneficiary}'s commodities with Trade by the end of round {deadline_round}"
        ),
        DealTerm::UseLeaderFor {
            leader,
            beneficiary,
            deadline_round,
        } => format!("use {leader} for {beneficiary} by the end of round {deadline_round}"),
    }
}

/// Whether a side already has a term of this shape, so the catalogue does not offer it twice.
fn has(terms: &[DealTerm], wanted: impl Fn(&DealTerm) -> bool) -> bool {
    terms.iter().any(wanted)
}

/// Agents whose use can be promised to another seat: each reports the seat it helped, which is how
/// the promise is judged kept. Sol's and Letnev's agents are not here: nothing offers them yet, even
/// to their owner (they act "at the start of a combat round", which has no leader window).
const FAVOUR_AGENTS: [&str; 4] = ["hacanagent", "jolnaragent", "l1z1xagent", "xxchaagent"];

/// Every item the side being edited could still add, as options.
#[must_use]
#[expect(
    clippy::too_many_lines,
    reason = "one block per kind of item, in the order the player reads them"
)]
pub fn item_options(
    state: &GameState,
    content: &ContentStore,
    scope: &ContactScope,
    draft: &Draft,
) -> Vec<ChoiceOption> {
    let (giver, receiver, terms) = draft.side();
    let verb = if draft.asking { "ask for" } else { "give" };
    let round = state.round;
    let mut out = Vec::new();
    if !draft.can_edit() {
        return out;
    }
    let full = terms.len() >= ti4_model::diplomacy::MAX_TERMS_PER_SIDE;

    if !full && scope.physical {
        for kind in Counted::ALL {
            let held = kind.held(state, giver);
            let already = has(
                terms,
                |term| matches!(term, DealTerm::ImmediateTransfer(asset) if Counted::of(asset) == Some(kind)),
            );
            if held > 0 && !already {
                out.push(ChoiceOption::labelled(
                    format!("diplomacy|now|{}", kind.id()),
                    ITEM_KIND,
                    format!("{verb} {} now ({held} held)", kind.label()),
                ));
            }
        }
        // 94: one promissory note per player per transaction.
        let note_on_side = has(terms, |term| {
            matches!(
                term,
                DealTerm::ImmediateTransfer(TransferAsset::PromissoryNote(_))
            )
        });
        if !note_on_side {
            let mut notes = crate::promissory::available_notes(state, content, giver);
            if let Some(support) = crate::promissory::available_support(state, giver) {
                notes.push(support);
            }
            // Hubris: nobody may give the Mahact their Alliance.
            notes.retain(|note| crate::promissory::may_receive(state, receiver, note));
            notes.sort();
            notes.dedup();
            for note in notes {
                out.push(ChoiceOption::labelled(
                    format!("diplomacy|note|{note}"),
                    ITEM_KIND,
                    format!("{verb} promissory note {note}"),
                ));
            }
        }
        // Action cards change hands only with Hacan at the table (Arbiters), and only the
        // builder's own: the partner's hand is not the builder's to see.
        // One action card per side: a transaction carries at most one card of each kind.
        let card_on_side = has(terms, |term| {
            matches!(
                term,
                DealTerm::ImmediateTransfer(TransferAsset::ActionCard(_))
            )
        });
        if !draft.asking
            && !card_on_side
            && (crate::faction_abilities::trades_action_cards(state, content, giver)
                || crate::faction_abilities::trades_action_cards(state, content, receiver))
            && let Some(seat) = state.player(giver)
        {
            let mut cards = seat.action_cards.clone();
            cards.sort();
            cards.dedup();
            for card in cards {
                let already = has(
                    terms,
                    |term| matches!(term, DealTerm::ImmediateTransfer(TransferAsset::ActionCard(held)) if *held == card),
                );
                if !already {
                    out.push(ChoiceOption::labelled(
                        format!("diplomacy|card|{card}"),
                        ITEM_KIND,
                        format!("give action card {card}"),
                    ));
                }
            }
        }
    }

    if !full {
        for kind in [Counted::TradeGoods, Counted::Commodities] {
            let already = has(
                terms,
                |term| matches!(term, DealTerm::FuturePayment { asset, .. } if Counted::of(asset) == Some(kind)),
            );
            if !already {
                out.push(ChoiceOption::labelled(
                    format!("diplomacy|later|{}", kind.id()),
                    ITEM_KIND,
                    format!(
                        "{verb} {} by the end of round {}",
                        kind.label(),
                        round.saturating_add(1)
                    ),
                ));
            }
        }
        for deadline in [round, round.saturating_add(1)] {
            let term = DealTerm::DoNotAttack {
                player: receiver.clone(),
                deadline_round: deadline,
            };
            push_promise(&mut out, terms, &term, draft.asking);
        }
        for system in &scope.systems {
            for deadline in [round, round.saturating_add(1)] {
                let term = DealTerm::DoNotActivate {
                    system: system.clone(),
                    deadline_round: deadline,
                };
                push_promise(&mut out, terms, &term, draft.asking);
            }
        }
        for target in scope
            .attack_targets
            .iter()
            .filter(|target| *target != giver && *target != receiver)
        {
            for deadline in [round, round.saturating_add(1)] {
                let term = DealTerm::Attack {
                    player: target.clone(),
                    deadline_round: deadline,
                };
                push_promise(&mut out, terms, &term, draft.asking);
            }
        }
        // A seat barred from this vote (a prediction, Political Secret) has no votes to promise.
        if let Some((agenda, outcomes)) = &scope.agenda
            && !state.agenda_predictions.contains_key(giver)
        {
            for outcome in outcomes {
                let term = DealTerm::Vote {
                    agenda: agenda.clone(),
                    outcome: outcome.clone(),
                    deadline_round: round,
                };
                push_promise(&mut out, terms, &term, draft.asking);
            }
        }
        let holds_unplayed_trade = state.player(giver).is_some_and(|seat| {
            seat.strategy_cards.iter().any(|card| {
                !seat.exhausted_strategy_cards.contains(card)
                    && crate::strategy_cards::card_name(content, card.as_str())
                        .is_some_and(|name| name == "Trade")
            })
        });
        if holds_unplayed_trade {
            let term = DealTerm::ReplenishFor {
                beneficiary: receiver.clone(),
                deadline_round: round.saturating_add(1),
            };
            push_promise(&mut out, terms, &term, draft.asking);
        }
        if let Some(seat) = state.player(giver) {
            for (leader, status) in &seat.leaders {
                if FAVOUR_AGENTS.contains(&leader.as_str())
                    && *status == ti4_model::state::LeaderStatus::Readied
                {
                    let term = DealTerm::UseLeaderFor {
                        leader: leader.to_string(),
                        beneficiary: receiver.clone(),
                        deadline_round: round,
                    };
                    push_promise(&mut out, terms, &term, draft.asking);
                }
            }
        }
    }

    for (index, term) in terms.iter().enumerate() {
        out.push(ChoiceOption::labelled(
            format!("diplomacy|remove|{index}"),
            ITEM_KIND,
            format!("remove: {}", describe(term)),
        ));
    }
    out
}

fn push_promise(out: &mut Vec<ChoiceOption>, terms: &[DealTerm], term: &DealTerm, asking: bool) {
    if terms.contains(term) {
        return;
    }
    let id = format!(
        "diplomacy|promise|{}",
        serde_json::to_string(term).expect("a deal term serializes")
    );
    let label = format!(
        "{} {}",
        if asking { "ask:" } else { "promise:" },
        describe(term)
    );
    out.push(ChoiceOption::labelled(id, ITEM_KIND, label).with(
        "term",
        serde_json::to_value(term).expect("a deal term serializes"),
    ));
}

/// The amounts a pending counted item may take.
#[must_use]
pub fn amount_options(state: &GameState, draft: &Draft) -> Vec<ChoiceOption> {
    let Some(pending) = draft.pending else {
        return Vec::new();
    };
    let (giver, _, _) = draft.side();
    let most = if pending.later {
        MAX_LATER
    } else {
        pending.kind.held(state, giver).min(MAX_AMOUNT)
    };
    (1..=most)
        .map(|amount| {
            ChoiceOption::labelled(
                format!("diplomacy|amount|{amount}"),
                AMOUNT_KIND,
                format!("{amount} {}", pending.kind.label()),
            )
            .with("amount", i64::from(amount))
        })
        .collect()
}

/// Apply one item choice to the draft. Returns `false` for an id that is not an item.
pub fn apply_item(state: &GameState, draft: &mut Draft, id: &str) -> bool {
    apply_item_at(state.round, draft, id)
}

/// [`apply_item`] given only the round, which is all an item needs from the position: a policy
/// previewing what each option would leave the draft as has no game state to pass.
pub fn apply_item_at(round: u32, draft: &mut Draft, id: &str) -> bool {
    let applied = apply_item_inner(round, draft, id);
    // Choosing an amount finishes the add that picked the item; it is not another step.
    if applied && !id.starts_with("diplomacy|amount|") {
        draft.steps = draft.steps.saturating_add(1);
    }
    applied
}

fn apply_item_inner(round: u32, draft: &mut Draft, id: &str) -> bool {
    let Some(rest) = id.strip_prefix("diplomacy|") else {
        return false;
    };
    let (head, tail) = rest.split_once('|').unwrap_or((rest, ""));
    match head {
        "now" | "later" => {
            let Some(kind) = Counted::from_id(tail) else {
                return false;
            };
            draft.pending = Some(Pending {
                kind,
                later: head == "later",
            });
            true
        }
        "amount" => {
            let (Some(pending), Ok(amount)) = (draft.pending.take(), tail.parse::<u8>()) else {
                return false;
            };
            let asset = pending.kind.asset(amount);
            let term = if pending.later {
                DealTerm::FuturePayment {
                    asset,
                    deadline_round: round.saturating_add(1),
                }
            } else {
                DealTerm::ImmediateTransfer(asset)
            };
            draft.side_mut().push(term);
            true
        }
        "note" => {
            draft
                .side_mut()
                .push(DealTerm::ImmediateTransfer(TransferAsset::PromissoryNote(
                    tail.to_owned(),
                )));
            true
        }
        "card" => {
            draft
                .side_mut()
                .push(DealTerm::ImmediateTransfer(TransferAsset::ActionCard(
                    ti4_model::ActionCardId::new(tail),
                )));
            true
        }
        "promise" => match serde_json::from_str::<DealTerm>(tail) {
            Ok(term) => {
                draft.side_mut().push(term);
                true
            }
            Err(_) => false,
        },
        "remove" => match tail.parse::<usize>() {
            Ok(index) if index < draft.side().2.len() => {
                draft.side_mut().remove(index);
                true
            }
            _ => false,
        },
        _ => false,
    }
}
