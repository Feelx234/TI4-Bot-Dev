//! Decision types of the `agenda` area: agenda phase, voting, laws.
//!
//! Owner: package B9 (docs/ENGINE_CORE_STATE.md 4.4). Ported so far: the questions of a vote
//! (`vote::VoteWindow`): naming an outcome, exhausting planets for it, spending trade goods, Predictive
//! Intelligence, Genetic Recombination's two questions and the speaker's tiebreak. The adapter side is
//! `ti4-engine/src/render/agenda.rs`; the renderer sees no state, so everything an option shows is a number
//! carried by the ask or the option. Never text; every type is `Copy`.

use crate::generated::{Agenda, Planet, System};
use crate::ids::Seat;
use crate::keys::Outcome;
use crate::state::Code;

/// Numbers an option shows that the position derives (the running tally of an outcome, the votes a planet
/// casts, the system a planet sits in). Two options are equal when they name the same thing: `Shown`
/// compares equal, orders equal and hashes to nothing, so an answer parsed from an id equals the option that
/// was offered.
#[derive(Debug, Clone, Copy, Default)]
pub struct Shown {
    /// The votes an outcome has (`current_votes`), or the votes a planet casts.
    pub votes: i32,
    /// Where a planet is, for the map (`system` payload of a planet outcome or planet option).
    pub system: Option<System>,
}

impl PartialEq for Shown {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}
impl Eq for Shown {}
impl PartialOrd for Shown {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Shown {
    fn cmp(&self, _: &Self) -> core::cmp::Ordering {
        core::cmp::Ordering::Equal
    }
}
impl core::hash::Hash for Shown {
    fn hash<H: core::hash::Hasher>(&self, _: &mut H) {}
}

/// The agenda a vote question is about and what it may show.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Frame {
    /// The agenda in play (`details.agenda`).
    pub agenda: Agenda,
    /// The face-down agenda under Covert Legislation (the outcomes are its).
    pub covert: Option<Agenda>,
    /// The speaker is the one asked: the face-down agenda is shown to them (`details.covert_agenda`).
    pub reveal: bool,
}

impl Frame {
    /// `vote::voted_agenda`: the agenda whose outcomes are voted on.
    #[must_use]
    pub fn voted(&self) -> Agenda {
        match self.covert {
            Some(hidden) if self.agenda == Agenda::CovertLegislation => hidden,
            _ => self.agenda,
        }
    }
}

/// The ledger of a vote so far, in the order of the offered outcomes (`details.tally`). A vote with more
/// than [`Tally::MAX`] outcomes is not expressed by the core.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Tally {
    pub len: u8,
    pub items: [(Outcome, i32); Tally::MAX],
}

impl Tally {
    pub const MAX: usize = 8;

    #[must_use]
    pub const fn empty() -> Self {
        Self { len: 0, items: [(Outcome::Empty, 0); Self::MAX] }
    }

    /// Append one outcome with its votes; `false` when the tally is full.
    pub fn push(&mut self, outcome: Outcome, votes: i32) -> bool {
        let at = usize::from(self.len);
        if at >= Self::MAX {
            return false;
        }
        self.items[at] = (outcome, votes);
        self.len += 1;
        true
    }

    #[must_use]
    pub fn as_slice(&self) -> &[(Outcome, i32)] {
        &self.items[..usize::from(self.len)]
    }
}

/// What the area asks. Parameters only; the option list is derived by the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ask {
    /// 8.10: which outcome to back (or abstain).
    Vote { frame: Frame },
    /// 8.11: which planet to exhaust for `outcome`, `votes` cast so far. `spender`: the seat may also
    /// exhaust for resources (Executive Order).
    Planets { frame: Frame, outcome: Outcome, votes: i32, spender: bool, tally: Tally },
    /// Spend trade goods for votes on `outcome`: `rate` votes each, `goods` available.
    TradeGoods { frame: Frame, outcome: Outcome, votes: i32, goods: i32, rate: u8 },
    /// Predictive Intelligence: 3 more votes for `outcome`.
    Predictive { frame: Frame, outcome: Outcome, votes: i32 },
    /// Genetic Recombination, asked of the holder before `voter` votes: which outcome they must back.
    Recombine { frame: Frame, voter: Seat },
    /// Genetic Recombination, asked of the voter: back `outcome` or return a fleet token.
    Tribute { frame: Frame, outcome: Outcome },
    /// 8.16: the speaker breaks a tie (or decides a silent table).
    Tiebreak { frame: Frame },
    /// 8.18, an agenda effect that names one player (Seed of an Empire): the speaker picks among the tied.
    NameTied,
    /// Homeland Defense Act, Against: the seat destroys one of its PDS.
    Structure,
    /// `laws::offer_discard`: the owner of `law` may discard it (the caller's `prompt`, interned, is the text).
    DiscardLaw { law: Agenda, prompt: Code },
    /// `Game::ask_to_use_note`: the holder of a card or ability (`source`) may use it on the agenda just revealed
    /// (`agenda`, shown under the question). `owner` is the seat the card belongs to (Political Secret's text
    /// names it).
    UseOnAgenda { source: UseSource, agenda: Agenda, owner: Seat },
    /// Committee Formation, after the owner discarded the card: which player the agenda elects (no vote follows).
    Committee { agenda: Agenda },
}

/// What a [`Ask::UseOnAgenda`] question offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum UseSource {
    /// The Xxcha ability Quash.
    Quash,
    /// The promissory note Political Favor.
    Favor,
    /// The promissory note Political Secret.
    Secret,
}

impl Ask {
    /// The agenda a vote question is about; the questions of an effect carry none.
    #[must_use]
    pub const fn frame(&self) -> Option<&Frame> {
        match self {
            Self::Vote { frame }
            | Self::Planets { frame, .. }
            | Self::TradeGoods { frame, .. }
            | Self::Predictive { frame, .. }
            | Self::Recombine { frame, .. }
            | Self::Tribute { frame, .. }
            | Self::Tiebreak { frame } => Some(frame),
            Self::NameTied | Self::Structure | Self::DiscardLaw { .. } | Self::UseOnAgenda { .. } | Self::Committee { .. } => None,
        }
    }
}

/// Which face of a planet a vote exhausts it for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Face {
    Influence,
    Resources,
}

/// One answer the area offers (not stored in the state).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Opt {
    /// Back an outcome (`Vote`), or the speaker picks it (`Tiebreak`).
    Back(Outcome, Shown),
    /// Exhaust a planet card; `shown.votes` is what it casts.
    Exhaust { planet: Planet, face: Face, shown: Shown },
    /// Spend this many trade goods.
    Spend(u8),
    /// Cast the 3 additional votes.
    CastPredictive,
    /// Genetic Recombination: the voter must back this outcome (or return a fleet token).
    Demand(Outcome),
    /// Genetic Recombination: cast at least one vote for the demanded outcome.
    TributeVote,
    /// Genetic Recombination: return a fleet token instead.
    TributeToken,
    /// `NameTied`: this player is the one the agenda names.
    Elect(Seat),
    /// `Structure`: the PDS in position `index` of the planet's units (the option id), on this planet.
    Structure { index: u8, planet: Planet, system: System },
    /// `DiscardLaw`: discard the law.
    DiscardLaw,
    /// `UseOnAgenda`: use the card or ability.
    UseCard,
}

/// The machine identifier of the question (`DecisionContext.subtype`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Subtype {
    CastVote,
    ExhaustPlanet,
    SpendTradeGoods,
    PredictiveIntelligence,
    RecombinationOutcome,
    RecombinationTribute,
    Tiebreak,
    ElectTiebreak,
    DefenseStructure,
    OfferDiscardLaw,
    UseQuash,
    UseFavor,
    UseSecret,
    CommitteeElect,
}

impl Ask {
    /// The subtype of the question.
    #[must_use]
    pub const fn subtype(&self) -> Subtype {
        match self {
            Self::Vote { .. } => Subtype::CastVote,
            Self::Planets { .. } => Subtype::ExhaustPlanet,
            Self::TradeGoods { .. } => Subtype::SpendTradeGoods,
            Self::Predictive { .. } => Subtype::PredictiveIntelligence,
            Self::Recombine { .. } => Subtype::RecombinationOutcome,
            Self::Tribute { .. } => Subtype::RecombinationTribute,
            Self::Tiebreak { .. } => Subtype::Tiebreak,
            Self::NameTied => Subtype::ElectTiebreak,
            Self::Structure => Subtype::DefenseStructure,
            Self::DiscardLaw { .. } => Subtype::OfferDiscardLaw,
            Self::UseOnAgenda { source: UseSource::Quash, .. } => Subtype::UseQuash,
            Self::UseOnAgenda { source: UseSource::Favor, .. } => Subtype::UseFavor,
            Self::UseOnAgenda { source: UseSource::Secret, .. } => Subtype::UseSecret,
            Self::Committee { .. } => Subtype::CommitteeElect,
        }
    }
}
