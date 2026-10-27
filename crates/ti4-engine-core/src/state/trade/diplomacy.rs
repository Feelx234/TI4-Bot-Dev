//! `DiplomacyState` (`ti4_model::diplomacy`), typed. Owned by B4; F1 fixes the shapes.

use crate::containers::{Cow, SeatSet, SmallList, SortedMap};
use crate::generated::{ActionCard, Agenda, Leader, SecretObjective, System};
use crate::ids::Seat;

use super::super::agenda::Outcome;
use super::super::cards::NoteKey;

/// `diplomacy::MAX_TERMS_PER_SIDE`.
pub const MAX_TERMS_PER_SIDE: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct DealId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct SignalId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Relationship {
    pub trust: i16,
    pub threat: u8,
    pub cooperation: i16,
    pub hostility: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferAsset {
    TradeGoods(u8),
    Commodities(u8),
    CulturalFragments(u8),
    HazardousFragments(u8),
    IndustrialFragments(u8),
    UnknownFragments(u8),
    PromissoryNote(NoteKey),
    ActionCard(ActionCard),
    SecretObjective(SecretObjective),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DealTerm {
    ImmediateTransfer(TransferAsset),
    FuturePayment { asset: TransferAsset, deadline_round: u32 },
    DoNotActivate { system: System, deadline_round: u32 },
    DoNotAttack { player: Seat, deadline_round: u32 },
    Vote { agenda: Agenda, outcome: Outcome, deadline_round: u32 },
    Attack { player: Seat, deadline_round: u32 },
    ReplenishFor { beneficiary: Seat, deadline_round: u32 },
    UseLeaderFor { leader: Leader, beneficiary: Seat, deadline_round: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromiseStatus {
    Pending,
    Fulfilled,
    Broken,
    Expired,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DealRevision {
    pub number: u8,
    pub author: Seat,
    pub proposer_terms: SmallList<DealTerm, 4>,
    pub recipient_terms: SmallList<DealTerm, 4>,
    pub proposer_statuses: SmallList<PromiseStatus, 4>,
    pub recipient_statuses: SmallList<PromiseStatus, 4>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DealStatus {
    Proposed,
    Accepted,
    Active,
    Declined,
    Fulfilled,
    Broken,
    Expired,
    Legacy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Deal {
    pub id: DealId,
    pub proposer: Seat,
    pub recipient: Seat,
    pub created_round: u32,
    pub revisions: Vec<DealRevision>,
    pub status: DealStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalDealSummary {
    pub id: DealId,
    pub proposer: Seat,
    pub recipient: Seat,
    pub created_round: u32,
    pub terminal_round: u32,
    pub status: DealStatus,
    pub latest_revision: Option<DealRevision>,
    /// `legacy_promise` text, adapter-interned.
    pub legacy_promise: Option<super::Promise>,
    pub legacy_status: Option<PromiseStatus>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalKind {
    Request,
    Threat,
    Assurance,
    Warning,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalStatement {
    WillVote { agenda: Agenda, outcome: Outcome },
    AttackIfYouActivate { system: System },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SignalStatus {
    #[default]
    Open,
    Honoured,
    Broken,
    Heeded,
    Ignored,
    Triggered,
    CarriedOut,
    Bluffed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Signal {
    pub id: SignalId,
    pub speaker: Seat,
    pub target: Seat,
    pub kind: SignalKind,
    pub statement: SignalStatement,
    pub status: SignalStatus,
    pub created_round: u32,
    pub expires_round: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DealRelationshipSnapshot {
    pub proposer_to_recipient: Relationship,
    pub recipient_to_proposer: Relationship,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiplomacyEvent {
    Offered { deal_id: DealId, proposer: Seat, recipient: Seat, relationship_snapshot: DealRelationshipSnapshot, revision: DealRevision },
    Countered { deal_id: DealId, revision: DealRevision },
    Accepted { deal_id: DealId, actor: Seat, revision: u8 },
    Declined { deal_id: DealId, actor: Seat, revision: u8 },
    ImmediateApplied { deal_id: DealId, term_index: u8 },
    PromiseSettled { deal_id: DealId, term_index: u8, status: PromiseStatus },
    DealSettled { deal_id: DealId, status: DealStatus },
    SignalEmitted { signal_id: SignalId },
    SignalJudged { signal: Signal },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiplomacyJournalEntry {
    pub round: u32,
    pub event: DiplomacyEvent,
}

/// The diplomacy rules tag (`DiplomacyState.rules_version` is a text such as `"v1"`): adapter-interned.
pub type DiplomacyRules = super::Promise;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diplomacy {
    pub enabled: bool,
    pub rules: DiplomacyRules,
    pub relationships: SortedMap<(Seat, Seat), Relationship, 8>,
    pub active_deals: Vec<Deal>,
    pub history: Vec<TerminalDealSummary>,
    pub recent_signals: Vec<Signal>,
    pub journal: Vec<DiplomacyJournalEntry>,
    pub next_deal_id: u64,
    pub next_signal_id: u64,
    pub initiations_this_turn: SortedMap<Seat, SeatSet, 8>,
    pub last_attacks: SortedMap<(Seat, Seat), u32, 8>,
    pub last_breaches: SortedMap<(Seat, Seat), u32, 8>,
    pub judged_attacks: SortedMap<(u32, Seat, Seat), (), 8>,
}

impl Default for Diplomacy {
    fn default() -> Self {
        Self {
            enabled: false,
            rules: super::Promise::default(),
            relationships: SortedMap::new(),
            active_deals: Vec::new(),
            history: Vec::new(),
            recent_signals: Vec::new(),
            journal: Vec::new(),
            next_deal_id: 1,
            next_signal_id: 1,
            initiations_this_turn: SortedMap::new(),
            last_attacks: SortedMap::new(),
            last_breaches: SortedMap::new(),
            judged_attacks: SortedMap::new(),
        }
    }
}

/// Keeps `Cow` in scope for the doc link above.
pub type SharedDiplomacy = Cow<Diplomacy>;
