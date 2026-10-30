//! The deal book, the signals and the promise ledger of structured diplomacy (rules v1) over the Trade slice.
//! Package B4d. String side: `ti4_model::diplomacy::{DiplomacyState::create_deal, archive_deal, Deal::add_counter,
//! DealRevision::new}`, `ti4-engine/src/diplomacy/{signals,promises,window}.rs`.
//!
//! Everything here is a state transition over [`Diplomacy`]: no question is asked and no goods move (an accepted
//! deal's immediate transfers and a paid promise go through `rules::trade::exchange`, which the adapter drives
//! first). The facts the core cannot see come as arguments: whether an accepted deal was a fair transaction
//! (`worth_to_receiver`, string), which gameplay moment happened ([`Moment`]).

use crate::generated::{Agenda, Leader, System};
use crate::ids::Seat;
use crate::state::{
    Deal, DealId, DealRelationshipSnapshot, DealRevision, DealStatus, DealTerm, DiplomacyEvent, DiplomacyJournalEntry, PromiseStatus, Relationship,
    Signal, SignalId, SignalStatement, SignalStatus, TerminalDealSummary, TransferAsset, MAX_TERMS_PER_SIDE,
};
use crate::state::{CoreState, SignalKind};

use super::diplomacy::{apply_relationship_event, consume_initiation, RelEvent};

/// `diplomacy::MAX_COUNTERS`.
pub const MAX_COUNTERS: u8 = 2;
/// `diplomacy::MAX_TERMINAL_HISTORY`.
pub const MAX_TERMINAL_HISTORY: usize = 32;

/// `DiplomacyError`, the variants the book can raise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BookError {
    SelfPair,
    PlayerMissing,
    TooManyTerms,
    EmptyDeal,
    InvalidDeadline,
    InvalidAsset,
    TooManyCounters,
    InvalidRevision,
    TerminalDeal,
    UnknownDeal,
    IdExhausted,
}

impl From<super::diplomacy::SelfPair> for BookError {
    fn from(_: super::diplomacy::SelfPair) -> Self {
        Self::SelfPair
    }
}

fn pending(term: &DealTerm) -> PromiseStatus {
    if matches!(term, DealTerm::ImmediateTransfer(_)) {
        PromiseStatus::Fulfilled
    } else {
        PromiseStatus::Pending
    }
}

const fn deadline(term: &DealTerm) -> Option<u32> {
    match *term {
        DealTerm::ImmediateTransfer(_) => None,
        DealTerm::FuturePayment { deadline_round, .. }
        | DealTerm::DoNotActivate { deadline_round, .. }
        | DealTerm::DoNotAttack { deadline_round, .. }
        | DealTerm::Vote { deadline_round, .. }
        | DealTerm::Attack { deadline_round, .. }
        | DealTerm::ReplenishFor { deadline_round, .. }
        | DealTerm::UseLeaderFor { deadline_round, .. } => Some(deadline_round),
    }
}

fn valid_asset(asset: &TransferAsset) -> bool {
    match *asset {
        TransferAsset::TradeGoods(n)
        | TransferAsset::Commodities(n)
        | TransferAsset::CulturalFragments(n)
        | TransferAsset::HazardousFragments(n)
        | TransferAsset::IndustrialFragments(n)
        | TransferAsset::UnknownFragments(n) => n > 0,
        TransferAsset::PromissoryNote(_) | TransferAsset::ActionCard(_) | TransferAsset::SecretObjective(_) => true,
    }
}

/// `DealRevision::new`: a structurally valid revision with every promise pending.
///
/// # Errors
/// Too many terms, an empty deal, a deadline other than this round or the next, an asset of zero.
pub fn new_revision(number: u8, author: Seat, proposer_terms: &[DealTerm], recipient_terms: &[DealTerm], round: u32) -> Result<DealRevision, BookError> {
    if proposer_terms.len() > MAX_TERMS_PER_SIDE || recipient_terms.len() > MAX_TERMS_PER_SIDE {
        return Err(BookError::TooManyTerms);
    }
    if proposer_terms.is_empty() && recipient_terms.is_empty() {
        return Err(BookError::EmptyDeal);
    }
    let all = || proposer_terms.iter().chain(recipient_terms);
    if all().filter_map(deadline).any(|d| d < round || d > round.saturating_add(1)) {
        return Err(BookError::InvalidDeadline);
    }
    let assets = all().filter_map(|t| match t {
        DealTerm::ImmediateTransfer(a) | DealTerm::FuturePayment { asset: a, .. } => Some(a),
        _ => None,
    });
    if assets.into_iter().any(|a| !valid_asset(a)) {
        return Err(BookError::InvalidAsset);
    }
    Ok(DealRevision {
        number,
        author,
        proposer_terms: crate::containers::SmallList::from_slice(proposer_terms),
        recipient_terms: crate::containers::SmallList::from_slice(recipient_terms),
        proposer_statuses: proposer_terms.iter().map(pending).fold(crate::containers::SmallList::new(), |mut l, s| {
            l.push(s);
            l
        }),
        recipient_statuses: recipient_terms.iter().map(pending).fold(crate::containers::SmallList::new(), |mut l, s| {
            l.push(s);
            l
        }),
    })
}

fn has_row(core: &CoreState, who: Seat) -> bool {
    core.trade().diplomacy.relationships.keys().any(|(observer, _)| observer == who)
}

fn relationship(core: &CoreState, a: Seat, b: Seat) -> Relationship {
    core.trade().diplomacy.relationships.get(&(a, b)).copied().unwrap_or_default()
}

fn journal(core: &mut CoreState, event: DiplomacyEvent) {
    let round = core.meta().round;
    core.trade_mut().diplomacy.edit().journal.push(DiplomacyJournalEntry { round, event });
}

fn deal_of(core: &CoreState, id: DealId) -> Option<&Deal> {
    core.trade().diplomacy.active_deals.iter().find(|d| d.id == id)
}

fn deal_mut(core: &mut CoreState, id: DealId) -> Option<&mut Deal> {
    core.trade_mut().diplomacy.edit().active_deals.iter_mut().find(|d| d.id == id)
}

const fn terminal(status: DealStatus) -> bool {
    matches!(status, DealStatus::Declined | DealStatus::Fulfilled | DealStatus::Broken | DealStatus::Expired | DealStatus::Legacy)
}

/// `DiplomacyState::create_deal`: the opening offer, journaled with both seats' view of the other.
///
/// # Errors
/// Same seat twice, a seat with no row, a revision that is not the proposer's number 0, an exhausted id space.
pub fn create_deal(core: &mut CoreState, proposer: Seat, recipient: Seat, round: u32, revision: DealRevision) -> Result<DealId, BookError> {
    if proposer == recipient {
        return Err(BookError::SelfPair);
    }
    if !has_row(core, proposer) || !has_row(core, recipient) {
        return Err(BookError::PlayerMissing);
    }
    if revision.author != proposer || revision.number != 0 {
        return Err(BookError::InvalidRevision);
    }
    let id = DealId(core.trade().diplomacy.next_deal_id);
    let next = id.0.checked_add(1).ok_or(BookError::IdExhausted)?;
    let snapshot = DealRelationshipSnapshot {
        proposer_to_recipient: relationship(core, proposer, recipient),
        recipient_to_proposer: relationship(core, recipient, proposer),
    };
    let d = core.trade_mut().diplomacy.edit();
    d.next_deal_id = next;
    d.active_deals.push(Deal { id, proposer, recipient, created_round: round, revisions: vec![revision.clone()], status: DealStatus::Proposed });
    d.journal.push(DiplomacyJournalEntry { round, event: DiplomacyEvent::Offered { deal_id: id, proposer, recipient, relationship_snapshot: snapshot, revision } });
    Ok(id)
}

/// `Deal::add_counter` followed by the `Countered` journal row of the window.
///
/// # Errors
/// An unknown deal, a deal no longer proposed, a third counter, a revision out of turn.
pub fn counter(core: &mut CoreState, id: DealId, revision: DealRevision) -> Result<(), BookError> {
    let deal = deal_mut(core, id).ok_or(BookError::UnknownDeal)?;
    if deal.status != DealStatus::Proposed {
        return Err(BookError::TerminalDeal);
    }
    if deal.revisions.len().saturating_sub(1) >= usize::from(MAX_COUNTERS) {
        return Err(BookError::TooManyCounters);
    }
    let expected = u8::try_from(deal.revisions.len()).unwrap_or(u8::MAX);
    if revision.number != expected || (revision.author != deal.proposer && revision.author != deal.recipient) {
        return Err(BookError::InvalidRevision);
    }
    deal.revisions.push(revision.clone());
    journal(core, DiplomacyEvent::Countered { deal_id: id, revision });
    Ok(())
}

/// `DiplomacyState::archive_deal`: a terminal deal leaves the book for the bounded history.
///
/// # Errors
/// An unknown deal, a deal that is not terminal.
pub fn archive_deal(core: &mut CoreState, id: DealId, round: u32) -> Result<(), BookError> {
    let deal = deal_of(core, id).ok_or(BookError::UnknownDeal)?;
    if !terminal(deal.status) {
        return Err(BookError::TerminalDeal);
    }
    let d = core.trade_mut().diplomacy.edit();
    let at = d.active_deals.iter().position(|x| x.id == id).ok_or(BookError::UnknownDeal)?;
    let deal = d.active_deals.remove(at);
    d.history.push(TerminalDealSummary {
        id,
        proposer: deal.proposer,
        recipient: deal.recipient,
        created_round: deal.created_round,
        terminal_round: round,
        status: deal.status,
        latest_revision: deal.revisions.last().cloned(),
        legacy_promise: None,
        legacy_status: None,
    });
    if d.history.len() > MAX_TERMINAL_HISTORY {
        d.history.remove(0);
    }
    Ok(())
}

/// The decline of the window: the deal's status, the `Declined` row, the archive.
///
/// # Errors
/// An unknown deal.
pub fn decline(core: &mut CoreState, id: DealId, actor: Seat) -> Result<(), BookError> {
    let deal = deal_mut(core, id).ok_or(BookError::UnknownDeal)?;
    deal.status = DealStatus::Declined;
    let revision = deal.revisions.last().map_or(0, |r| r.number);
    journal(core, DiplomacyEvent::Declined { deal_id: id, actor, revision });
    let round = core.meta().round;
    archive_deal(core, id, round)
}

/// What an accepted deal does to the book once its immediate transfers have moved (`window::apply_acceptance`):
/// the status, the `Accepted` and `ImmediateApplied` rows, the relationship events, and the archive of a deal with
/// no promise left. `fair` is the string engine's pricing of the immediate legs (`worth_to_receiver`).
///
/// # Errors
/// An unknown deal or a relationship event that names a seat twice.
pub fn accept(core: &mut CoreState, id: DealId, actor: Seat, fair: bool) -> Result<(), BookError> {
    let deal = deal_of(core, id).ok_or(BookError::UnknownDeal)?.clone();
    let revision = deal.revisions.last().ok_or(BookError::InvalidRevision)?.clone();
    deal_mut(core, id).ok_or(BookError::UnknownDeal)?.status = DealStatus::Accepted;
    journal(core, DiplomacyEvent::Accepted { deal_id: id, actor, revision: revision.number });
    let all = || revision.proposer_terms.as_slice().iter().chain(revision.recipient_terms.as_slice());
    for (index, term) in all().enumerate() {
        if matches!(term, DealTerm::ImmediateTransfer(_)) {
            journal(core, DiplomacyEvent::ImmediateApplied { deal_id: id, term_index: u8::try_from(index).unwrap_or(u8::MAX) });
        }
    }
    let future = all().any(|t| !matches!(t, DealTerm::ImmediateTransfer(_)));
    deal_mut(core, id).ok_or(BookError::UnknownDeal)?.status = if future { DealStatus::Active } else { DealStatus::Fulfilled };
    let (a, b) = (deal.proposer, deal.recipient);
    if future {
        apply_relationship_event(core, RelEvent::DealAccepted { a, b })?;
    } else if fair {
        apply_relationship_event(core, RelEvent::FairTransaction { a, b })?;
    }
    for term in revision.recipient_terms.as_slice() {
        if let DealTerm::Attack { player: target, .. } = *term
            && target != a
            && target != b
        {
            apply_relationship_event(core, RelEvent::AntiThirdPartyDeal { payer: a, attacker: b, target })?;
        }
    }
    if !future {
        journal(core, DiplomacyEvent::DealSettled { deal_id: id, status: DealStatus::Fulfilled });
        let round = core.meta().round;
        archive_deal(core, id, round)?;
    }
    Ok(())
}

// -- signals ---------------------------------------------------------------------------------------

/// `SignalError`, the variants the core can raise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalError {
    Disabled,
    SelfSignal,
    PlayerMissing,
    InvalidExpiry,
    AlreadyInitiated,
    IdExhausted,
    SelfPair,
}

/// `SignalStatement::kind`.
#[must_use]
pub const fn kind_of(statement: &SignalStatement) -> SignalKind {
    match statement {
        SignalStatement::WillVote { .. } => SignalKind::Assurance,
        SignalStatement::AttackIfYouActivate { .. } => SignalKind::Warning,
    }
}

/// `emit_signal` (`consume_allowance`) and `emit_signal_in_open_contact` (not): one public signal.
///
/// # Errors
/// Disabled diplomacy, a seat signalling itself, an unseated party, an expiry other than this round or the next, a
/// pair that already made contact this turn, an exhausted id space.
pub fn emit_signal(core: &mut CoreState, speaker: Seat, target: Seat, statement: SignalStatement, expires_round: u32, consume_allowance: bool) -> Result<SignalId, SignalError> {
    if !core.trade().diplomacy.enabled {
        return Err(SignalError::Disabled);
    }
    if speaker == target {
        return Err(SignalError::SelfSignal);
    }
    if core.seats().player_of(speaker).is_none() || core.seats().player_of(target).is_none() {
        return Err(SignalError::PlayerMissing);
    }
    let round = core.meta().round;
    if expires_round < round || expires_round > round.saturating_add(1) {
        return Err(SignalError::InvalidExpiry);
    }
    if consume_allowance && !consume_initiation(core, speaker, target) {
        return Err(SignalError::AlreadyInitiated);
    }
    let id = SignalId(core.trade().diplomacy.next_signal_id);
    let next = id.0.checked_add(1).ok_or(SignalError::IdExhausted)?;
    let kind = kind_of(&statement);
    let d = core.trade_mut().diplomacy.edit();
    d.next_signal_id = next;
    d.recent_signals.push(Signal { id, speaker, target, kind, statement, status: SignalStatus::Open, created_round: round, expires_round });
    journal(core, DiplomacyEvent::SignalEmitted { signal_id: id });
    apply_relationship_event(core, RelEvent::SignalMade { speaker, target, kind }).map_err(|_| SignalError::SelfPair)?;
    Ok(id)
}

fn is_settled(status: SignalStatus) -> bool {
    !matches!(status, SignalStatus::Open | SignalStatus::Triggered)
}

fn judge(core: &mut CoreState, index: usize, status: SignalStatus) -> Result<(), BookError> {
    let signal = {
        let s = &mut core.trade_mut().diplomacy.edit().recent_signals[index];
        s.status = status;
        *s
    };
    journal(core, DiplomacyEvent::SignalJudged { signal });
    let (speaker, target) = (signal.speaker, signal.target);
    let event = match status {
        SignalStatus::Honoured => Some(RelEvent::AssuranceKept { speaker, target }),
        SignalStatus::Broken => Some(RelEvent::AssuranceBroken { speaker, target }),
        SignalStatus::Heeded if signal.kind == SignalKind::Request => Some(RelEvent::RequestHeeded { speaker, target }),
        SignalStatus::Ignored => Some(RelEvent::RequestIgnored { speaker, target }),
        SignalStatus::CarriedOut => Some(RelEvent::ThreatCarriedOut { speaker, target }),
        SignalStatus::Bluffed => Some(RelEvent::ThreatBluffed { speaker, target }),
        SignalStatus::Open | SignalStatus::Triggered | SignalStatus::Heeded => None,
    };
    if let Some(event) = event {
        apply_relationship_event(core, event)?;
    }
    Ok(())
}

/// One gameplay moment (`DiplomacyEventContext`), over typed ids.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Moment {
    SystemActivated { player: Seat, system: System },
    HostileEngagement { attacker: Seat, victim: Seat, activation_seq: u32 },
    VotesRecorded { agenda: Agenda },
    CommoditiesReplenished { by: Seat, beneficiary: Seat },
    LeaderUsedFor { user: Seat, leader: Leader, beneficiary: Seat },
}

fn voted(core: &CoreState, seat: Seat, outcome: crate::state::Outcome) -> bool {
    core.agenda().votes.get(&seat).is_some_and(|v| *v == outcome)
}

/// `signals::judge_event`: every undecided signal against one moment.
///
/// # Errors
/// A relationship event that names a seat twice.
pub fn judge_event(core: &mut CoreState, moment: Moment) -> Result<(), BookError> {
    let round = core.meta().round;
    let mut outcomes = Vec::new();
    for (index, signal) in core.trade().diplomacy.recent_signals.iter().enumerate() {
        if round > signal.expires_round {
            continue;
        }
        let (speaker, target) = (signal.speaker, signal.target);
        let next = match (signal.statement, signal.status, moment) {
            (SignalStatement::WillVote { agenda, outcome }, SignalStatus::Open, Moment::VotesRecorded { agenda: actual }) if agenda == actual => {
                Some(if voted(core, speaker, outcome) { SignalStatus::Honoured } else { SignalStatus::Broken })
            }
            (SignalStatement::AttackIfYouActivate { .. }, SignalStatus::Triggered, Moment::HostileEngagement { attacker, victim, .. })
                if attacker == speaker && victim == target =>
            {
                Some(SignalStatus::CarriedOut)
            }
            (SignalStatement::AttackIfYouActivate { system }, SignalStatus::Open, Moment::SystemActivated { player, system: activated })
                if player == target && activated == system =>
            {
                Some(SignalStatus::Triggered)
            }
            _ => None,
        };
        if let Some(status) = next {
            outcomes.push((index, status));
        }
    }
    for (index, status) in outcomes {
        judge(core, index, status)?;
    }
    Ok(())
}

/// `signals::settle_deadlines`: the undecided signals that expire with the completed round.
///
/// # Errors
/// A relationship event that names a seat twice.
pub fn settle_signals(core: &mut CoreState, completed_round: u32) -> Result<(), BookError> {
    let due: Vec<(usize, SignalStatus)> = core
        .trade()
        .diplomacy
        .recent_signals
        .iter()
        .enumerate()
        .filter(|(_, s)| s.expires_round <= completed_round && !is_settled(s.status))
        .map(|(i, s)| {
            let status = match (s.statement, s.status) {
                (SignalStatement::WillVote { .. }, _) => SignalStatus::Broken,
                (_, SignalStatus::Triggered) => SignalStatus::Bluffed,
                _ => SignalStatus::Heeded,
            };
            (i, status)
        })
        .collect();
    for (index, status) in due {
        judge(core, index, status)?;
    }
    Ok(())
}

// -- the promise ledger ------------------------------------------------------------------------------

/// `PromiseError`, the variants the core can raise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromiseError {
    UnknownDeal,
    UnknownTerm,
    AlreadySettled,
    SelfPair,
    Book(BookError),
}

impl From<BookError> for PromiseError {
    fn from(e: BookError) -> Self {
        Self::Book(e)
    }
}

impl From<super::diplomacy::SelfPair> for PromiseError {
    fn from(_: super::diplomacy::SelfPair) -> Self {
        Self::SelfPair
    }
}

/// One settlement the scan found: (proposer's side, index on the side, index across both sides, status, promiser,
/// beneficiary).
type Settle = (bool, usize, usize, PromiseStatus, Seat, Seat);

fn settle_term(core: &mut CoreState, id: DealId, proposer_side: bool, index: usize, global: usize, status: PromiseStatus, promiser: Seat, beneficiary: Seat) -> Result<(), PromiseError> {
    let deal = deal_mut(core, id).ok_or(PromiseError::UnknownDeal)?;
    let revision = deal.revisions.last_mut().ok_or(PromiseError::UnknownTerm)?;
    let statuses = if proposer_side { &mut revision.proposer_statuses } else { &mut revision.recipient_statuses };
    let slot = statuses.as_mut_slice().get_mut(index).ok_or(PromiseError::UnknownTerm)?;
    if *slot != PromiseStatus::Pending {
        return Err(PromiseError::AlreadySettled);
    }
    *slot = status;
    journal(core, DiplomacyEvent::PromiseSettled { deal_id: id, term_index: u8::try_from(global).unwrap_or(u8::MAX), status });
    match status {
        PromiseStatus::Fulfilled => apply_relationship_event(core, RelEvent::PromiseFulfilled { promiser, beneficiary })?,
        PromiseStatus::Broken => apply_relationship_event(core, RelEvent::PromiseBroken { promiser, beneficiary })?,
        PromiseStatus::Pending | PromiseStatus::Expired => {}
    }
    Ok(())
}

fn update_terminal(core: &mut CoreState, id: DealId) -> Result<(), PromiseError> {
    let deal = deal_of(core, id).ok_or(PromiseError::UnknownDeal)?;
    let latest = deal.revisions.last().ok_or(PromiseError::UnknownTerm)?;
    let all: Vec<PromiseStatus> = latest.proposer_statuses.as_slice().iter().chain(latest.recipient_statuses.as_slice()).copied().collect();
    let status = if all.contains(&PromiseStatus::Broken) {
        Some(DealStatus::Broken)
    } else if all.iter().all(|s| *s == PromiseStatus::Fulfilled) {
        Some(DealStatus::Fulfilled)
    } else if all.iter().all(|s| *s != PromiseStatus::Pending) {
        Some(DealStatus::Expired)
    } else {
        None
    };
    if let Some(status) = status {
        deal_mut(core, id).ok_or(PromiseError::UnknownDeal)?.status = status;
        journal(core, DiplomacyEvent::DealSettled { deal_id: id, status });
        let round = core.meta().round;
        archive_deal(core, id, round)?;
    }
    Ok(())
}

fn inspect(core: &CoreState, moment: Moment, terms: &[DealTerm], statuses: &[PromiseStatus], promiser: Seat, beneficiary: Seat, proposer_side: bool, offset: usize, out: &mut Vec<Settle>) {
    for (index, (term, status)) in terms.iter().zip(statuses).enumerate() {
        if *status != PromiseStatus::Pending {
            continue;
        }
        let result = match (*term, moment) {
            (DealTerm::DoNotActivate { system, .. }, Moment::SystemActivated { player, system: actual }) if player == promiser && actual == system => {
                Some(PromiseStatus::Broken)
            }
            (DealTerm::DoNotAttack { player, .. }, Moment::HostileEngagement { attacker, victim, .. }) if attacker == promiser && victim == player => {
                Some(PromiseStatus::Broken)
            }
            (DealTerm::Attack { player, .. }, Moment::HostileEngagement { attacker, victim, .. }) if attacker == promiser && victim == player => {
                Some(PromiseStatus::Fulfilled)
            }
            (DealTerm::Vote { agenda, outcome, .. }, Moment::VotesRecorded { agenda: actual }) if agenda == actual && voted(core, promiser, outcome) => {
                Some(PromiseStatus::Fulfilled)
            }
            (DealTerm::ReplenishFor { beneficiary: promised, .. }, Moment::CommoditiesReplenished { by, beneficiary: who }) if by == promiser && who == promised => {
                Some(PromiseStatus::Fulfilled)
            }
            (DealTerm::UseLeaderFor { leader, beneficiary: helped, .. }, Moment::LeaderUsedFor { user, leader: used, beneficiary: actual })
                if user == promiser && used == leader && actual == helped =>
            {
                Some(PromiseStatus::Fulfilled)
            }
            _ => None,
        };
        if let Some(result) = result {
            out.push((proposer_side, index, offset + index, result, promiser, beneficiary));
        }
    }
}

/// `promises::evaluate_event`: a moment against every pending promise of an active deal, then against the signals.
/// A hostile engagement is first a direct attack for the relationship table.
///
/// # Errors
/// A deal transition that is not valid.
pub fn evaluate_event(core: &mut CoreState, moment: Moment) -> Result<(), PromiseError> {
    if !core.trade().diplomacy.enabled {
        return Ok(());
    }
    if let Moment::HostileEngagement { attacker, victim, activation_seq } = moment {
        apply_relationship_event(core, RelEvent::DirectAttack { attacker, victim, activation_seq })?;
    }
    let ids: Vec<DealId> = core.trade().diplomacy.active_deals.iter().map(|d| d.id).collect();
    for id in ids {
        let Some(deal) = deal_of(core, id) else { continue };
        if deal.status != DealStatus::Active {
            continue;
        }
        let (Some(revision), proposer, recipient) = (deal.revisions.last().cloned(), deal.proposer, deal.recipient) else { continue };
        let mut found = Vec::new();
        inspect(core, moment, revision.proposer_terms.as_slice(), revision.proposer_statuses.as_slice(), proposer, recipient, true, 0, &mut found);
        inspect(core, moment, revision.recipient_terms.as_slice(), revision.recipient_statuses.as_slice(), recipient, proposer, false, revision.proposer_terms.len(), &mut found);
        for (side, index, global, status, promiser, beneficiary) in found {
            settle_term(core, id, side, index, global, status, promiser, beneficiary)?;
        }
        update_terminal(core, id)?;
    }
    judge_event(core, moment)?;
    Ok(())
}

fn settle_due(core: &mut CoreState, until: Option<u32>) -> Result<(), PromiseError> {
    if !core.trade().diplomacy.enabled {
        return Ok(());
    }
    let ids: Vec<DealId> = core.trade().diplomacy.active_deals.iter().map(|d| d.id).collect();
    for id in ids {
        let Some(deal) = deal_of(core, id) else { continue };
        if deal.status != DealStatus::Active {
            continue;
        }
        let (Some(revision), proposer, recipient) = (deal.revisions.last().cloned(), deal.proposer, deal.recipient) else { continue };
        let mut found = Vec::new();
        for (terms, statuses, promiser, beneficiary, side, offset) in [
            (revision.proposer_terms.as_slice(), revision.proposer_statuses.as_slice(), proposer, recipient, true, 0),
            (revision.recipient_terms.as_slice(), revision.recipient_statuses.as_slice(), recipient, proposer, false, revision.proposer_terms.len()),
        ] {
            for (index, (term, status)) in terms.iter().zip(statuses).enumerate() {
                if *status != PromiseStatus::Pending {
                    continue;
                }
                let due = until.is_some_and(|round| deadline(term).is_some_and(|d| d <= round));
                let result = if due {
                    match term {
                        DealTerm::DoNotActivate { .. } | DealTerm::DoNotAttack { .. } => PromiseStatus::Fulfilled,
                        _ => PromiseStatus::Broken,
                    }
                } else if until.is_none() {
                    PromiseStatus::Expired
                } else {
                    continue;
                };
                found.push((side, index, offset + index, result, promiser, beneficiary));
            }
        }
        for (side, index, global, status, promiser, beneficiary) in found {
            settle_term(core, id, side, index, global, status, promiser, beneficiary)?;
        }
        update_terminal(core, id)?;
    }
    Ok(())
}

/// `promises::settle_deadlines`: the promises due with the completed round, then the signals.
///
/// # Errors
/// A deal transition that is not valid.
pub fn settle_deadlines(core: &mut CoreState, completed_round: u32) -> Result<(), PromiseError> {
    settle_due(core, Some(completed_round))?;
    if core.trade().diplomacy.enabled {
        settle_signals(core, completed_round)?;
    }
    Ok(())
}

/// `promises::settle_game_end`: what was never reached expires.
///
/// # Errors
/// A deal transition that is not valid.
pub fn settle_game_end(core: &mut CoreState) -> Result<(), PromiseError> {
    settle_due(core, None)
}

/// The part of `fulfill_payment` before the goods move: which promise `term_index` of `deal` is, and whether
/// `promiser` may pay it now. Reads only.
///
/// # Errors
/// An unknown deal, a deal not active, a term that is not the promiser's, not pending, or not a payment.
pub fn payment_due(core: &CoreState, id: DealId, promiser: Seat, term_index: usize) -> Result<PaymentDue, PromiseError> {
    let deal = deal_of(core, id).ok_or(PromiseError::UnknownDeal)?;
    if deal.status != DealStatus::Active {
        return Err(PromiseError::AlreadySettled);
    }
    let revision = deal.revisions.last().ok_or(PromiseError::UnknownTerm)?;
    let count = revision.proposer_terms.len();
    let (proposer_side, local, beneficiary, term, status) = if term_index < count {
        (true, term_index, deal.recipient, revision.proposer_terms.as_slice().get(term_index), revision.proposer_statuses.as_slice().get(term_index))
    } else {
        let local = term_index - count;
        (false, local, deal.proposer, revision.recipient_terms.as_slice().get(local), revision.recipient_statuses.as_slice().get(local))
    };
    let expected = if proposer_side { deal.proposer } else { deal.recipient };
    if expected != promiser {
        return Err(PromiseError::UnknownTerm);
    }
    if status != Some(&PromiseStatus::Pending) {
        return Err(PromiseError::AlreadySettled);
    }
    let Some(DealTerm::FuturePayment { asset, .. }) = term.copied() else { return Err(PromiseError::UnknownTerm) };
    Ok(PaymentDue { proposer_side, local, beneficiary, asset })
}

/// The promise `payment_due` found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaymentDue {
    pub proposer_side: bool,
    pub local: usize,
    pub beneficiary: Seat,
    pub asset: TransferAsset,
}

/// The part of `fulfill_payment` after the goods moved: the term is fulfilled and the deal may end.
///
/// # Errors
/// A term already settled, a deal transition that is not valid.
pub fn payment_made(core: &mut CoreState, id: DealId, promiser: Seat, term_index: usize) -> Result<(), PromiseError> {
    let due = payment_due(core, id, promiser, term_index)?;
    settle_term(core, id, due.proposer_side, due.local, term_index, PromiseStatus::Fulfilled, promiser, due.beneficiary)?;
    update_terminal(core, id)
}
