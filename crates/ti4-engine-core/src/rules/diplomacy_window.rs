//! The structured negotiation window (`diplomacy::window::DiplomacyWindow`, rules v1) as a [`Window`]: the
//! proposer builds a revision item by item (what they give, then what they ask for), reviews it and proposes; the
//! recipient accepts, declines or builds a counter the same way, at most twice. Package B4d.
//!
//! * Stages: [`DStage::Building`] (a draft is open: items, an amount, the review), [`DStage::Responding`] (the
//!   revision on the table is answered), [`DStage::Done`].
//! * The deal book (`rules::diplomacy_book`) records every step: the offer, the counters, the decline, the accept.
//!   An accepted deal's immediate terms move through `rules::trade::exchange` first; a deal that cannot settle (its
//!   transfers no longer legal) is void, like a decline.
//! * What stays with the string engine: the staged `TRADE_GOODS_GAINED` events of the two sides and Dark Pact's
//!   gains (as in `rules::trade_window`), the `TRANSACTION_RESOLVED` / Lie in Wait window, the candidate bundles
//!   (`CandidateBundle` features are the bots' payload) and every window with a planet-card leg.

use crate::choice::diplomacy::{Ask, Item, Opt, Step, Subtype};
use crate::choice::generic::Rule;
use crate::choice::{Choice, Ctx, Opt as COpt, Opts, Origin, Pick, Subtype as CSub, Target};
use crate::containers::SmallList;
use crate::generated::{ExploreType, PromissoryNote};
use crate::ids::Seat;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::query::diplomacy_items::{self as items_q, Draft, DipFacts, Scope, asset_of};
use crate::query::trade::{self as q, NoteFacts, NoteMove, Terms};
use crate::query::trade_deals::{self as deals_q, Held, Side};
use crate::state::{CoreState, DealId, DealRevision, DealTerm, NoteKey, SignalStatement, TransferAsset};

use super::diplomacy_book as book;
use super::trade::{exchange, record_resolved};

/// `window::EDIT_COST`: the steps one "edit" spends (at most two edits per revision).
pub const EDIT_COST: u8 = items_q::MAX_BUILD_STEPS / 2;
/// `diplomacy::MAX_COUNTERS`.
pub const MAX_COUNTERS: u8 = book::MAX_COUNTERS;

/// The suspension points of [`DiplomacyWindow`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DStage {
    Building,
    Responding,
    Done,
}

/// An open contact between two seats.
#[derive(Debug, Clone, PartialEq)]
pub struct DiplomacyWindow {
    pub proposer: Seat,
    pub recipient: Seat,
    pub stage: DStage,
    pub scope: Scope,
    /// The signals the proposer may send instead of an offer.
    pub signals: SmallList<SignalStatement, 4>,
    pub draft: Option<Draft>,
    /// The revision on the table.
    pub current: Option<DealRevision>,
    pub deal_id: Option<DealId>,
    /// Counters made so far.
    pub responses: u8,
    /// A counter was started and abandoned: not a counter made, but the choice to counter is spent.
    pub counter_abandoned: bool,
    facts: DipFacts,
    /// The window ends after one answer (the adapter asks the string's one step at a time).
    single: bool,
    /// Whether an answer has been applied (with `single`, the window then asks nothing).
    stepped: bool,
}

impl DiplomacyWindow {
    /// A contact at the position the string window is in.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn at(
        proposer: Seat,
        recipient: Seat,
        stage: DStage,
        scope: Scope,
        signals: SmallList<SignalStatement, 4>,
        draft: Option<Draft>,
        current: Option<DealRevision>,
        deal_id: Option<DealId>,
        responses: u8,
        counter_abandoned: bool,
        facts: DipFacts,
    ) -> Self {
        Self { proposer, recipient, stage, scope, signals, draft, current, deal_id, responses, counter_abandoned, facts, single: false, stepped: false }
    }

    /// Stop after the next answer.
    #[must_use]
    pub fn single(mut self) -> Self {
        self.single = true;
        self
    }

    /// Whether an answer has been applied to this window.
    #[must_use]
    pub const fn stepped(&self) -> bool {
        self.stepped
    }

    fn actor(&self) -> Seat {
        if self.responses.is_multiple_of(2) { self.recipient } else { self.proposer }
    }

    /// The draft's terms as a revision stored against the original proposer and recipient.
    fn revision(&self, round: u32, d: &Draft) -> Result<DealRevision, book::BookError> {
        let (proposer_terms, recipient_terms) = if d.builder == self.proposer { (d.give.clone(), d.take.clone()) } else { (d.take.clone(), d.give.clone()) };
        // Revision 0 is the opening offer; each counter is the next number.
        let number = if self.deal_id.is_some() { self.responses.saturating_add(1) } else { 0 };
        book::new_revision(number, d.builder, proposer_terms.as_slice(), recipient_terms.as_slice(), round)
    }

    fn ctx(subtype: Subtype, other: Seat) -> Ctx {
        Ctx {
            origin: Origin::Rule(Rule::R94),
            subtype: CSub::Diplomacy(subtype),
            optional: true,
            target: Some(Target::Seat(other)),
            outstanding: SmallList::new(),
            space_battle: false,
            invasion_seq: None,
        }
    }

    fn opts(list: impl IntoIterator<Item = Opt>) -> Opts {
        list.into_iter().map(COpt::Diplomacy).collect()
    }

    /// The question of a draft: an amount, the review, or the items.
    fn building(&self, s: &CoreState) -> Option<Choice> {
        let d = self.draft.as_ref()?;
        let (builder, other) = (d.builder, d.other);
        if let Some((kind, later)) = d.pending {
            let opts = Self::opts(items_q::amounts(s, d).into_iter().map(Opt::Amount));
            return Some(Choice { actor: builder, ask: crate::choice::Ask::Diplomacy(Ask::Amount { builder, other, kind, later }), ctx: Self::ctx(Subtype::Amount, other), opts });
        }
        if d.reviewing {
            let mut list = Vec::new();
            if !d.is_empty() {
                list.push(Opt::Propose);
            }
            if d.can_edit() {
                list.push(Opt::Edit);
            }
            list.push(Opt::Cancel);
            let ask = Ask::Review { builder, other, back: self.current.is_some() };
            return Some(Choice { actor: builder, ask: crate::choice::Ask::Diplomacy(ask), ctx: Self::ctx(Subtype::Review, other), opts: Self::opts(list) });
        }
        let mut list: Vec<Opt> = items_q::items(s, &self.facts, &self.scope, d).into_iter().map(Opt::Item).collect();
        list.extend(items_q::removals(d).into_iter().map(Opt::Remove));
        list.push(Opt::Done);
        // The opening screen of a fresh contact also sends a signal or walks away.
        if self.current.is_none() && d.is_empty() && !d.asking {
            list.extend((0..self.signals.len()).map(|i| Opt::Signal(u8::try_from(i).unwrap_or(u8::MAX))));
            list.push(Opt::Cancel);
        }
        let (step, subtype) = if d.asking { (Step::Take, Subtype::AskItem) } else { (Step::Give, Subtype::OfferItem) };
        Some(Choice { actor: builder, ask: crate::choice::Ask::Diplomacy(Ask::Build { builder, other, step }), ctx: Self::ctx(subtype, other), opts: Self::opts(list) })
    }

    /// `apply_item`: choosing an item, an amount or a removal. `false` for a pick that is none of them.
    fn apply_item(round: u32, d: &mut Draft, opt: Opt) -> bool {
        let applied = match opt {
            Opt::Item(Item::Now(kind)) => {
                d.pending = Some((kind, false));
                true
            }
            Opt::Item(Item::Later(kind)) => {
                d.pending = Some((kind, true));
                true
            }
            Opt::Amount(amount) => {
                let Some((kind, later)) = d.pending.take() else { return false };
                let asset = asset_of(kind, amount);
                let term = if later { DealTerm::FuturePayment { asset, deadline_round: round.saturating_add(1) } } else { DealTerm::ImmediateTransfer(asset) };
                d.side_mut().push(term);
                true
            }
            Opt::Item(Item::Note(key)) => {
                d.side_mut().push(DealTerm::ImmediateTransfer(TransferAsset::PromissoryNote(key)));
                true
            }
            Opt::Item(Item::Card(card)) => {
                d.side_mut().push(DealTerm::ImmediateTransfer(TransferAsset::ActionCard(card)));
                true
            }
            Opt::Item(Item::Pledge(p)) => {
                let receiver = d.side().1;
                d.side_mut().push(items_q::term_of(p, round, receiver));
                true
            }
            Opt::Remove(i) if usize::from(i) < d.side().2.len() => {
                d.side_mut().remove(usize::from(i));
                true
            }
            _ => false,
        };
        // Choosing an amount finishes the add that picked the item; it is not another step.
        if applied && !matches!(opt, Opt::Amount(_)) {
            d.steps = d.steps.saturating_add(1);
        }
        applied
    }

    /// `transfers::immediate_terms`: the immediate legs of one side as the transaction's terms, `None` when a side
    /// carries two cards of one kind. The note leg carries the facts `exchange` needs.
    fn terms(&self, s: &CoreState, list: &[DealTerm], giver: Seat, receiver: Seat) -> Option<Terms> {
        let mut out = Terms::default();
        let (mut note, mut card, mut secret) = (false, false, false);
        for term in list {
            let DealTerm::ImmediateTransfer(asset) = term else { continue };
            match *asset {
                TransferAsset::TradeGoods(n) => out.trade_goods += i32::from(n),
                TransferAsset::Commodities(n) => out.commodities += i32::from(n),
                TransferAsset::CulturalFragments(n) => out.fragments[ExploreType::Cultural as usize] += i32::from(n),
                TransferAsset::HazardousFragments(n) => out.fragments[ExploreType::Hazardous as usize] += i32::from(n),
                TransferAsset::IndustrialFragments(n) => out.fragments[ExploreType::Industrial as usize] += i32::from(n),
                TransferAsset::UnknownFragments(n) => out.fragments[ExploreType::Frontier as usize] += i32::from(n),
                TransferAsset::PromissoryNote(key) if !note => {
                    note = true;
                    out.note = Some(self.note_facts(s, key, giver, receiver));
                }
                TransferAsset::ActionCard(c) if !card => {
                    card = true;
                    out.action_card = Some(c);
                }
                TransferAsset::SecretObjective(x) if !secret => {
                    secret = true;
                    out.secret = Some(x);
                }
                _ => return None,
            }
        }
        Some(out)
    }

    fn note_facts(&self, s: &CoreState, key: NoteKey, giver: Seat, receiver: Seat) -> NoteFacts {
        if key.note == PromissoryNote::SupportForTheThrone {
            NoteFacts { payable: deals_q::available_support(s, giver) == Some(key.faction), receivable: true, mv: Some(NoteMove::Support(key.faction)) }
        } else {
            NoteFacts {
                payable: deals_q::available_notes(s, &self.facts.deal, giver).contains(&key),
                receivable: crate::query::promissory::may_receive(s, receiver, key.note),
                mv: Some(NoteMove::Held(key)),
            }
        }
    }

    /// The price side of a list of terms (`Terms::worth_to_receiver`).
    fn side(list: &[DealTerm]) -> Side {
        let mut side = Side::default();
        for term in list {
            let DealTerm::ImmediateTransfer(asset) = term else { continue };
            match *asset {
                TransferAsset::TradeGoods(n) => side.goods += i32::from(n),
                TransferAsset::Commodities(n) => side.commodities += i32::from(n),
                TransferAsset::CulturalFragments(n)
                | TransferAsset::HazardousFragments(n)
                | TransferAsset::IndustrialFragments(n)
                | TransferAsset::UnknownFragments(n) => side.fragments += i32::from(n),
                TransferAsset::PromissoryNote(key) => {
                    side.note.get_or_insert(if key.note == PromissoryNote::SupportForTheThrone { Held::Support } else { Held::Note(key) });
                }
                TransferAsset::ActionCard(_) => side.card = true,
                TransferAsset::SecretObjective(_) => side.secret = true,
            }
        }
        side
    }

    /// `apply_acceptance`: the immediate legs move, then the book records the acceptance. `Err` when the deal
    /// cannot settle (nothing has changed then).
    fn accept(&self, s: &mut CoreState, actor: Seat) -> Result<(), ()> {
        let id = self.deal_id.ok_or(())?;
        let deal = s.trade().diplomacy.active_deals.iter().find(|d| d.id == id).cloned().ok_or(())?;
        let revision = deal.revisions.last().ok_or(())?.clone();
        let given = self.terms(s, revision.proposer_terms.as_slice(), deal.proposer, deal.recipient).ok_or(())?;
        let received = self.terms(s, revision.recipient_terms.as_slice(), deal.recipient, deal.proposer).ok_or(())?;
        // Priced before anything moves, exactly as the transaction window prices a deal.
        let fair = (Self::side(revision.proposer_terms.as_slice()).worth() - Self::side(revision.recipient_terms.as_slice()).worth()).abs() <= 0.5;
        if !given.is_empty() || !received.is_empty() {
            let offer = q::Offer { proposer: deal.proposer, partner: deal.recipient, given, received };
            let reach = self.facts.deal.may_from(deal.proposer);
            let arbiters = |x: Seat| self.facts.deal.arbiter(x);
            if q::why_illegal_with(s, || reach, || s.transient().misc.flags.has(crate::state::TransientFlags::BLACK_MARKET), arbiters, &offer).is_some() {
                return Err(());
            }
            exchange(s, &offer).ok_or(())?;
            record_resolved(s, deal.proposer, deal.recipient);
        }
        book::accept(s, id, actor, fair).map_err(|_| ())
    }

    fn end(&mut self) {
        self.stage = DStage::Done;
        self.draft = None;
    }
}

impl Window for DiplomacyWindow {
    type Stage = DStage;

    fn stage(&self) -> DStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        match self.ask_of(sx.state, sx.kernel) {
            Some(Choice { ask, .. }) => Next::Continue(ask),
            None => Next::Done,
        }
    }

    fn ask_of(&self, s: &CoreState, _: &Kernel) -> Option<Choice> {
        if self.single && self.stepped {
            return None;
        }
        match self.stage {
            DStage::Done => None,
            DStage::Building => self.building(s),
            DStage::Responding => {
                let actor = self.actor();
                let other = if self.responses.is_multiple_of(2) { self.proposer } else { self.recipient };
                let may_counter = self.responses < MAX_COUNTERS && self.current.is_some() && !self.counter_abandoned;
                let mut list = vec![Opt::Accept, Opt::Decline];
                if may_counter {
                    list.push(Opt::Counter);
                }
                Some(Choice {
                    actor,
                    ask: crate::choice::Ask::Diplomacy(Ask::Respond { actor, other, may_counter }),
                    ctx: Self::ctx(Subtype::Response, other),
                    opts: Self::opts(list),
                })
            }
        }
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        let Pick::Opt(COpt::Diplomacy(opt)) = pick else { return Err(Illegal) };
        let round = sx.state.meta().round;
        self.stepped = true;
        match self.stage {
            DStage::Done => Err(Illegal),
            DStage::Building => {
                let mut d = self.draft.clone().ok_or(Illegal)?;
                match opt {
                    Opt::Signal(i) => {
                        let statement = *self.signals.as_slice().get(usize::from(i)).ok_or(Illegal)?;
                        // A vote assurance is about this agenda, so it is judged before the round ends.
                        let expires = if matches!(statement, SignalStatement::WillVote { .. }) { round } else { round.saturating_add(1) };
                        book::emit_signal(sx.state, self.proposer, self.recipient, statement, expires, false).map_err(|_| Illegal)?;
                        self.end();
                        return Ok(());
                    }
                    Opt::Done if !d.asking => d.asking = true,
                    Opt::Done => d.reviewing = true,
                    Opt::Edit => {
                        d.reviewing = false;
                        d.asking = false;
                        // Going back to edit spends the budget too.
                        d.steps = d.steps.saturating_add(EDIT_COST);
                    }
                    Opt::Cancel => {
                        self.draft = None;
                        if self.current.is_some() {
                            self.counter_abandoned = true;
                        }
                        // Walking away from a counter leaves the offer on the table to answer.
                        self.stage = if self.current.is_some() { DStage::Responding } else { DStage::Done };
                        return Ok(());
                    }
                    Opt::Propose => {
                        let revision = self.revision(round, &d).map_err(|_| Illegal)?;
                        if let Some(id) = self.deal_id {
                            book::counter(sx.state, id, revision.clone()).map_err(|_| Illegal)?;
                            self.responses += 1;
                        } else {
                            let id = book::create_deal(sx.state, self.proposer, self.recipient, round, revision.clone()).map_err(|_| Illegal)?;
                            self.deal_id = Some(id);
                        }
                        self.current = Some(revision);
                        self.draft = None;
                        self.stage = DStage::Responding;
                        return Ok(());
                    }
                    other => {
                        if !Self::apply_item(round, &mut d, other) {
                            return Err(Illegal);
                        }
                    }
                }
                self.draft = Some(d);
                Ok(())
            }
            DStage::Responding => {
                let id = self.deal_id.ok_or(Illegal)?;
                let actor = self.actor();
                // A deal that cannot settle when accepted is void, like a decline.
                let accepted = opt == Opt::Accept && self.accept(sx.state, actor).is_ok();
                if accepted {
                    self.stage = DStage::Done;
                } else if matches!(opt, Opt::Decline | Opt::Accept) {
                    book::decline(sx.state, id, actor).map_err(|_| Illegal)?;
                    self.stage = DStage::Done;
                } else if opt == Opt::Counter {
                    // The answering seat builds from the terms on the table, its own side first.
                    let current = self.current.as_ref().ok_or(Illegal)?;
                    let (give, take) = if actor == self.proposer { (current.proposer_terms.clone(), current.recipient_terms.clone()) } else { (current.recipient_terms.clone(), current.proposer_terms.clone()) };
                    let other = if actor == self.proposer { self.recipient } else { self.proposer };
                    let mut d = Draft::new(actor, other);
                    d.give = give;
                    d.take = take;
                    self.draft = Some(d);
                    self.stage = DStage::Building;
                } else {
                    return Err(Illegal);
                }
                Ok(())
            }
        }
    }
}
