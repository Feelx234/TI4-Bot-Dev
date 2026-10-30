//! What a revision of a structured negotiation can still take (`diplomacy::builder::{item_options,
//! amount_options}`) and the terms an item makes. Package B4d.
//!
//! The draft (the terms so far) and the scope of the contact (what it knew when it opened) are values of the
//! window; the position is read from the typed state. The facts of the string engine a window needs (who may
//! deal with whom, who trades action cards, whose commander is unlocked, the four agents a seat can promise)
//! come in [`DipFacts`].

use crate::choice::diplomacy::{Counted, Item, Pledge};
use crate::containers::SmallList;
use crate::generated::{ActionCard, Agenda, ExploreType, Leader, PromissoryNote, StrategyCardKind, System};
use crate::ids::Seat;
use crate::keys::Outcome;
use crate::query::promissory as qn;
use crate::query::trade_deals::{self as deals_q, DealFacts};
use crate::state::{CoreState, DealTerm, LeaderStatus, NoteKey, TransferAsset, MAX_TERMS_PER_SIDE};

/// `builder::MAX_AMOUNT`: the largest amount offered for one counted item.
pub const MAX_AMOUNT: u8 = 10;
/// `builder::MAX_LATER`: the largest amount a promise to pay later may name.
pub const MAX_LATER: u8 = 5;
/// `builder::MAX_BUILD_STEPS`.
pub const MAX_BUILD_STEPS: u8 = 8;

/// The string engine's answers a window needs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DipFacts {
    /// The transaction facts (reach, Arbiters, commanders) of the two chairs.
    pub deal: DealFacts,
    /// The agents whose use can be promised (`builder::FAVOUR_AGENTS`), as leaders.
    pub agents: [Leader; 4],
}

/// `builder::ContactScope`.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Scope {
    pub physical: bool,
    pub attack_targets: SmallList<Seat, 8>,
    pub systems: SmallList<System, 8>,
    pub agenda: Option<(Agenda, SmallList<Outcome, 8>)>,
}

/// `builder::Draft`: the revision being built.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Draft {
    pub builder: Seat,
    pub other: Seat,
    pub give: SmallList<DealTerm, 4>,
    pub take: SmallList<DealTerm, 4>,
    pub asking: bool,
    pub reviewing: bool,
    /// A counted item waiting for its amount: the kind and whether it is paid later.
    pub pending: Option<(Counted, bool)>,
    pub steps: u8,
}

impl Draft {
    #[must_use]
    pub fn new(builder: Seat, other: Seat) -> Self {
        Self { builder, other, give: SmallList::new(), take: SmallList::new(), asking: false, reviewing: false, pending: None, steps: 0 }
    }

    /// The side being edited: who hands it over, who receives it, and its terms.
    #[must_use]
    pub fn side(&self) -> (Seat, Seat, &[DealTerm]) {
        if self.asking { (self.other, self.builder, self.take.as_slice()) } else { (self.builder, self.other, self.give.as_slice()) }
    }

    pub fn side_mut(&mut self) -> &mut SmallList<DealTerm, 4> {
        if self.asking { &mut self.take } else { &mut self.give }
    }

    #[must_use]
    pub const fn can_edit(&self) -> bool {
        self.steps < MAX_BUILD_STEPS
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.give.is_empty() && self.take.is_empty()
    }
}

/// `Counted::asset`.
#[must_use]
pub const fn asset_of(kind: Counted, amount: u8) -> TransferAsset {
    match kind {
        Counted::TradeGoods => TransferAsset::TradeGoods(amount),
        Counted::Commodities => TransferAsset::Commodities(amount),
        Counted::Cultural => TransferAsset::CulturalFragments(amount),
        Counted::Hazardous => TransferAsset::HazardousFragments(amount),
        Counted::Industrial => TransferAsset::IndustrialFragments(amount),
        Counted::Unknown => TransferAsset::UnknownFragments(amount),
    }
}

/// `Counted::of`.
#[must_use]
pub const fn counted_of(asset: &TransferAsset) -> Option<Counted> {
    match asset {
        TransferAsset::TradeGoods(_) => Some(Counted::TradeGoods),
        TransferAsset::Commodities(_) => Some(Counted::Commodities),
        TransferAsset::CulturalFragments(_) => Some(Counted::Cultural),
        TransferAsset::HazardousFragments(_) => Some(Counted::Hazardous),
        TransferAsset::IndustrialFragments(_) => Some(Counted::Industrial),
        TransferAsset::UnknownFragments(_) => Some(Counted::Unknown),
        _ => None,
    }
}

/// `Counted::held`: how many of these `seat` holds right now (an unknown fragment is a frontier one).
#[must_use]
pub fn held(core: &CoreState, kind: Counted, seat: Seat) -> u8 {
    let Some(p) = core.players().by_seat(seat) else { return 0 };
    let raw = match kind {
        Counted::TradeGoods => i32::from(p.econ.trade_goods),
        Counted::Commodities => i32::from(p.econ.commodities),
        Counted::Cultural => fragments(core, seat, ExploreType::Cultural),
        Counted::Hazardous => fragments(core, seat, ExploreType::Hazardous),
        Counted::Industrial => fragments(core, seat, ExploreType::Industrial),
        Counted::Unknown => fragments(core, seat, ExploreType::Frontier),
    };
    u8::try_from(raw.clamp(0, i32::from(u8::MAX))).unwrap_or(0)
}

fn fragments(core: &CoreState, seat: Seat, kind: ExploreType) -> i32 {
    core.players().by_seat(seat).and_then(|p| p.cards.fragments.get(kind)).map_or(0, i32::from)
}

/// The term a pledge makes in `round` between `giver` and `receiver`.
#[must_use]
pub fn term_of(pledge: Pledge, round: u32, receiver: Seat) -> DealTerm {
    let at = |late: bool| if late { round.saturating_add(1) } else { round };
    match pledge {
        Pledge::NoAttack { late } => DealTerm::DoNotAttack { player: receiver, deadline_round: at(late) },
        Pledge::NoActivate { system, late } => DealTerm::DoNotActivate { system, deadline_round: at(late) },
        Pledge::Attack { who, late } => DealTerm::Attack { player: who, deadline_round: at(late) },
        Pledge::Vote { agenda, outcome } => DealTerm::Vote { agenda, outcome, deadline_round: round },
        Pledge::Replenish => DealTerm::ReplenishFor { beneficiary: receiver, deadline_round: round.saturating_add(1) },
        Pledge::Leader { leader } => DealTerm::UseLeaderFor { leader, beneficiary: receiver, deadline_round: round },
    }
}

/// `holds_unplayed_trade`: a Trade card the seat has not exhausted.
fn holds_unplayed_trade(core: &CoreState, seat: Seat) -> bool {
    core.players().by_seat(seat).is_some_and(|p| p.strat.as_slice().iter().any(|c| !p.strat_exhausted.contains(*c) && c.data().kind == StrategyCardKind::Trade))
}

fn has(terms: &[DealTerm], wanted: impl Fn(&DealTerm) -> bool) -> bool {
    terms.iter().any(wanted)
}

/// `builder::item_options`: every item the side being edited could still add, in the order the player reads them.
/// Each is followed by nothing here: the removals come last as [`removals`].
#[must_use]
pub fn items(core: &CoreState, f: &DipFacts, scope: &Scope, d: &Draft) -> Vec<Item> {
    let (giver, receiver, terms) = d.side();
    let round = core.meta().round;
    let mut out = Vec::new();
    if !d.can_edit() {
        return out;
    }
    let full = terms.len() >= MAX_TERMS_PER_SIDE;
    if !full && scope.physical {
        for kind in Counted::ALL {
            let already = has(terms, |t| matches!(t, DealTerm::ImmediateTransfer(a) if counted_of(a) == Some(kind)));
            if held(core, kind, giver) > 0 && !already {
                out.push(Item::Now(kind));
            }
        }
        // 94: one promissory note per player per transaction.
        if !has(terms, |t| matches!(t, DealTerm::ImmediateTransfer(TransferAsset::PromissoryNote(_)))) {
            let mut notes = deals_q::available_notes(core, &f.deal, giver);
            if let Some(faction) = deals_q::available_support(core, giver) {
                notes.push(NoteKey { note: PromissoryNote::SupportForTheThrone, faction });
            }
            // Hubris: nobody may give the Mahact their Alliance.
            notes.retain(|k| qn::may_receive(core, receiver, k.note));
            notes.sort_by(|a, b| (f.deal.note_order)(*a, *b));
            notes.dedup();
            out.extend(notes.into_iter().map(Item::Note));
        }
        // Action cards change hands only with Hacan at the table (Arbiters), and only the builder's own.
        let card_on_side = has(terms, |t| matches!(t, DealTerm::ImmediateTransfer(TransferAsset::ActionCard(_))));
        if !d.asking
            && !card_on_side
            && (f.deal.arbiter(giver) || f.deal.arbiter(receiver))
            && let Some(p) = core.players().by_seat(giver)
        {
            let mut cards: Vec<ActionCard> = p.cards.hand.as_slice().to_vec();
            cards.sort_by(|a, b| (f.deal.card_order)(*a, *b));
            cards.dedup();
            for card in cards {
                if !has(terms, |t| matches!(t, DealTerm::ImmediateTransfer(TransferAsset::ActionCard(h)) if *h == card)) {
                    out.push(Item::Card(card));
                }
            }
        }
    }
    if !full {
        for kind in [Counted::TradeGoods, Counted::Commodities] {
            if !has(terms, |t| matches!(t, DealTerm::FuturePayment { asset, .. } if counted_of(asset) == Some(kind))) {
                out.push(Item::Later(kind));
            }
        }
        let mut push = |pledge: Pledge| {
            if !terms.contains(&term_of(pledge, round, receiver)) {
                out.push(Item::Pledge(pledge));
            }
        };
        for late in [false, true] {
            push(Pledge::NoAttack { late });
        }
        for system in scope.systems.as_slice() {
            for late in [false, true] {
                push(Pledge::NoActivate { system: *system, late });
            }
        }
        for who in scope.attack_targets.as_slice().iter().filter(|t| **t != giver && **t != receiver) {
            for late in [false, true] {
                push(Pledge::Attack { who: *who, late });
            }
        }
        // A seat barred from this vote (a prediction, Political Secret) has no votes to promise.
        if let Some((agenda, outcomes)) = &scope.agenda
            && !core.agenda().predictions.contains_key(&giver)
        {
            for outcome in outcomes.as_slice() {
                push(Pledge::Vote { agenda: *agenda, outcome: *outcome });
            }
        }
        if holds_unplayed_trade(core, giver) {
            push(Pledge::Replenish);
        }
        if let Some(p) = core.players().by_seat(giver) {
            for (leader, status) in p.leaders.status.iter() {
                if f.agents.contains(leader) && *status == LeaderStatus::Readied {
                    push(Pledge::Leader { leader: *leader });
                }
            }
        }
    }
    out
}

/// The removals the side offers last: the index of each term.
#[must_use]
pub fn removals(d: &Draft) -> Vec<u8> {
    if !d.can_edit() {
        return Vec::new();
    }
    (0..d.side().2.len()).map(|i| u8::try_from(i).unwrap_or(u8::MAX)).collect()
}

/// `builder::amount_options`: the amounts a pending counted item may take.
#[must_use]
pub fn amounts(core: &CoreState, d: &Draft) -> Vec<u8> {
    let Some((kind, later)) = d.pending else { return Vec::new() };
    let (giver, _, _) = d.side();
    let most = if later { MAX_LATER } else { held(core, kind, giver).min(MAX_AMOUNT) };
    (1..=most).collect()
}
