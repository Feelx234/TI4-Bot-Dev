//! Plain events and the one ordered emission queue (package P1; docs/todo/P1.md).
//!
//! `Game::events` of the string engine is ONE ordered log of two kinds of lines: the typed events the timing resolver
//! emitted (copied there by name when the surrounding call returns, `mirror_timing_log`) and the plain events of
//! `Game::emit` (`QUASH_USED`, `LAW_ENACTED:{agenda}:{outcome}`, ...), pushed at once. The core keeps both in
//! [`Ledger::events`](super::Ledger), as [`Emit`]s, in the string's order: [`PlainRec`] is the data of a plain event
//! (a closed [`PlainKind`] and up to two typed [`PlainArg`]s); `render/events.rs` of the adapter makes the text.
//!
//! The catalogue is the census of `Game::emit` over `ti4-engine/src` at `core-d1a` (docs/todo/P1.md 3). A name outside
//! it is [`PlainKind::Untyped`] with its whole text as a [`Code`] in the first argument: it renders back verbatim and
//! the census counts it. No text lives here; every kind has a [`Shape`] (which arguments, which types) that the
//! renderer and the validity test share.

use super::{Code, EventRec, Outcome};
use crate::choice::strategy::Pool;
use crate::generated::{Agenda, Leader, Planet, StrategyCard, System, Technology};
use crate::ids::Seat;
use crate::state::ObjectiveRef;

/// One argument slot of a plain event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlainArg {
    None,
    Seat(Seat),
    System(System),
    Planet(Planet),
    Agenda(Agenda),
    Leader(Leader),
    Card(StrategyCard),
    Tech(Technology),
    Obj(ObjectiveRef),
    Outcome(Outcome),
    Pool(Pool),
    Count(u16),
    Code(Code),
}

/// What an argument slot of a kind holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    /// No argument (and no `:` in the text).
    None,
    Seat,
    System,
    /// A system or nothing: the text is `NAME` or `NAME:system`.
    MaybeSystem,
    Planet,
    Agenda,
    Leader,
    Card,
    Tech,
    Obj,
    Outcome,
    Pool,
    Count,
    /// The whole text of an [`PlainKind::Untyped`] event.
    Text,
}

impl Slot {
    /// Whether `arg` fits the slot.
    #[must_use]
    pub const fn accepts(self, arg: PlainArg) -> bool {
        matches!(
            (self, arg),
            (Self::None, PlainArg::None)
                | (Self::Seat, PlainArg::Seat(_))
                | (Self::System, PlainArg::System(_))
                | (Self::MaybeSystem, PlainArg::System(_) | PlainArg::None)
                | (Self::Planet, PlainArg::Planet(_))
                | (Self::Agenda, PlainArg::Agenda(_))
                | (Self::Leader, PlainArg::Leader(_))
                | (Self::Card, PlainArg::Card(_))
                | (Self::Tech, PlainArg::Tech(_))
                | (Self::Obj, PlainArg::Obj(_))
                | (Self::Outcome, PlainArg::Outcome(_))
                | (Self::Pool, PlainArg::Pool(_))
                | (Self::Count, PlainArg::Count(_))
                | (Self::Text, PlainArg::Code(_))
        )
    }
}

/// The argument slots of a kind: the text is `NAME[:a[:b]]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shape {
    pub a: Slot,
    pub b: Slot,
}

const fn shape(a: Slot, b: Slot) -> Shape {
    Shape { a, b }
}

macro_rules! plain_kinds {
    ($($kind:ident => ($a:ident, $b:ident);)*) => {
        /// The closed set of plain event names (docs/todo/P1.md 3), in alphabetical order of the variants. The renderer
        /// maps a kind to its text (`SCREAMING_SNAKE` of the variant, with the exceptions its table lists).
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum PlainKind {
            $($kind,)*
        }

        impl PlainKind {
            /// Every kind, in declaration order.
            pub const ALL: &'static [PlainKind] = &[$(PlainKind::$kind,)*];

            /// The argument slots of the kind.
            #[must_use]
            pub const fn shape(self) -> Shape {
                match self {
                    $(PlainKind::$kind => shape(Slot::$a, Slot::$b),)*
                }
            }
        }
    };
}

plain_kinds! {
    ActionPhaseBegan => (None, None);
    AgendaDiscarded => (Agenda, None);
    AgendaEffectDeferred => (Agenda, None);
    AgendaEffectResolved => (Agenda, None);
    AgendaEffectUnresolved => (Agenda, None);
    AgendaOutcomeRedirected => (Outcome, Seat);
    AgendaPhaseBegan => (None, None);
    AgendaPhaseResolved => (None, None);
    AgendaPredictionCorrect => (Seat, None);
    AgendaResolved => (Agenda, Outcome);
    AgendaRevealed => (Agenda, None);
    CeasefireUsed => (None, None);
    CommandTokenGained => (Pool, None);
    CommandTokensGained => (None, None);
    ComponentActionFailed => (None, None);
    ComponentActionResolved => (None, None);
    CovertAgendaResolved => (Agenda, Outcome);
    Counteroffer => (None, None);
    DiplomacyPaymentFulfilled => (None, None);
    EResSiphons => (Seat, None);
    EntropicScarTechnologyGained => (Seat, Tech);
    ExpeditionSliceClaimed => (None, None);
    ExtremeDuress => (Seat, None);
    FractureEnteredPlay => (None, None);
    FreeTacticalAction => (None, None);
    FrontierExplored => (None, None);
    GameFinished => (None, None);
    GenesisInfantry => (Seat, System);
    InvasionResolved => (None, None);
    LawEnacted => (Agenda, Outcome);
    LeaderAbilityResolved => (None, None);
    LeaderUnlocked => (Leader, None);
    MilitarySupportUsed => (None, None);
    ObjectiveReplaced => (Obj, Obj);
    ObjectiveScored => (Obj, None);
    PlayerPassed => (None, None);
    PoliticalFavorUsed => (None, None);
    PoliticalSecretUsed => (Seat, None);
    ProductionResolved => (None, None);
    QuantumDatahubSwap => (Seat, Seat);
    QuashUsed => (None, None);
    RoundBegan => (None, None);
    ScanlinkExplored => (Planet, None);
    ShipLostToGravityRift => (None, None);
    ShipMoved => (None, None);
    SpaceCombatResolved => (None, None);
    SpatialConduit => (Seat, System);
    SpecOpsLost => (Seat, None);
    SpecOpsReturned => (Seat, Count);
    SpecOpsSurvived => (Seat, None);
    StartingTechnologyChosen => (None, None);
    StatusBookkeepingResolved => (None, None);
    StatusPhaseBegan => (None, None);
    StatusPhaseResolved => (None, None);
    StatusScoringBegan => (None, None);
    StrategicActionBegan => (None, None);
    StrategicActionCancelled => (Card, None);
    StrategicActionComplete => (None, None);
    StrategyCardChosen => (None, None);
    StrategyCardsSwapped => (Seat, Seat);
    StrategySecondaryDeclined => (None, None);
    StrategySecondaryFollowed => (None, None);
    StrategySecondaryPreviewComplete => (None, None);
    SupportForTheThroneReturned => (Seat, None);
    SystemActivated => (MaybeSystem, None);
    TacticalActionBegan => (None, None);
    TacticalActionComplete => (None, None);
    Transaction => (None, None);
    TransactionAbandoned => (None, None);
    TransactionOffered => (None, None);
    TransactionRefused => (None, None);
    TransactionRejected => (None, None);
    TurnClosing => (None, None);
    TurnEndedByMahactCommander => (Seat, None);
    TurnEndedByMinisterOfPeace => (None, None);
    TurnEndedByNullificationField => (Seat, None);
    TurnEndedByStarlancer => (Seat, None);
    TurnPassed => (None, None);
    TurnPuppet => (Seat, None);
    TurnRetained => (None, None);
    TurnSkipped => (Seat, None);
    Untyped => (Text, None);
}

/// A plain event: its kind and the arguments the kind's [`Shape`] names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlainRec {
    pub kind: PlainKind,
    pub a: PlainArg,
    pub b: PlainArg,
}

impl PlainRec {
    /// A kind with no argument.
    #[must_use]
    pub const fn bare(kind: PlainKind) -> Self {
        Self { kind, a: PlainArg::None, b: PlainArg::None }
    }

    /// A kind with one argument.
    #[must_use]
    pub const fn one(kind: PlainKind, a: PlainArg) -> Self {
        Self { kind, a, b: PlainArg::None }
    }

    /// A kind with two arguments.
    #[must_use]
    pub const fn two(kind: PlainKind, a: PlainArg, b: PlainArg) -> Self {
        Self { kind, a, b }
    }

    /// An event whose name the catalogue does not hold: the whole text, as an adapter code.
    #[must_use]
    pub const fn untyped(text: Code) -> Self {
        Self::one(PlainKind::Untyped, PlainArg::Code(text))
    }

    /// Whether the arguments fit the kind's shape.
    #[must_use]
    pub const fn is_valid(&self) -> bool {
        let shape = self.kind.shape();
        shape.a.accepts(self.a) && shape.b.accepts(self.b)
    }
}

/// One line of the ordered emission queue (`Game::events`).
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::large_enum_variant, reason = "the queue is a chunked `Log` of lines written under the dual run; a box would cost an allocation per mirrored typed event")]
pub enum Emit {
    /// A typed event the resolver emitted, as the string mirrors it (by name, when the surrounding call returns).
    Typed(EventRec),
    /// A plain event of `Game::emit`.
    Plain(PlainRec),
}

impl Emit {
    /// The plain event, when this is one.
    #[must_use]
    pub const fn plain(&self) -> Option<&PlainRec> {
        match self {
            Self::Plain(p) => Some(p),
            Self::Typed(_) => None,
        }
    }
}

/// The kinds of the emission queue in order, `Ok` for a plain event and `Err` for a mirrored typed one (tests).
#[cfg(test)]
pub(crate) fn kinds(s: &super::CoreState) -> Vec<Result<PlainKind, super::EventKind>> {
    s.ledger.events.iter().map(|e| match e {
        Emit::Plain(p) => Ok(p.kind),
        Emit::Typed(t) => Err(t.kind),
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::containers::Log;

    #[test]
    fn a_plain_record_is_small_and_copy() {
        assert!(std::mem::size_of::<PlainRec>() <= 16, "{}", std::mem::size_of::<PlainRec>());
        assert!(std::mem::size_of::<PlainArg>() <= 8, "{}", std::mem::size_of::<PlainArg>());
        // An `Emit` is not larger than the typed record it carries by more than a word.
        assert!(std::mem::size_of::<Emit>() <= std::mem::size_of::<EventRec>() + 8);
    }

    #[test]
    fn every_kind_has_a_shape_and_a_bare_or_filled_record_of_it_is_valid() {
        let sample = |slot: Slot| match slot {
            Slot::None => PlainArg::None,
            Slot::Seat => PlainArg::Seat(Seat(1)),
            Slot::System | Slot::MaybeSystem => PlainArg::System(System::ALL[0]),
            Slot::Planet => PlainArg::Planet(Planet::ALL[0]),
            Slot::Agenda => PlainArg::Agenda(Agenda::ALL[0]),
            Slot::Leader => PlainArg::Leader(Leader::ALL[0]),
            Slot::Card => PlainArg::Card(StrategyCard::ALL[0]),
            Slot::Tech => PlainArg::Tech(Technology::ALL[0]),
            Slot::Obj => PlainArg::Obj(ObjectiveRef::Public(crate::generated::PublicObjective::ALL[0])),
            Slot::Outcome => PlainArg::Outcome(Outcome::For),
            Slot::Pool => PlainArg::Pool(Pool::Fleet),
            Slot::Count => PlainArg::Count(3),
            Slot::Text => PlainArg::Code(Code(7)),
        };
        assert!(PlainKind::ALL.len() >= 70, "the catalogue has about 70 names: {}", PlainKind::ALL.len());
        for kind in PlainKind::ALL {
            let shape = kind.shape();
            let rec = PlainRec { kind: *kind, a: sample(shape.a), b: sample(shape.b) };
            assert!(rec.is_valid(), "{kind:?}");
            // A swapped or missing argument is not.
            if shape.a != Slot::None && shape.a != Slot::MaybeSystem {
                assert!(!PlainRec { a: PlainArg::None, ..rec }.is_valid(), "{kind:?} needs its first argument");
            }
            if shape.a == Slot::None {
                assert!(!PlainRec { a: PlainArg::Count(1), ..rec }.is_valid(), "{kind:?} takes no argument");
            }
        }
        let mut sorted: Vec<_> = PlainKind::ALL.to_vec();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), PlainKind::ALL.len());
        assert_eq!(PlainKind::ALL.last(), Some(&PlainKind::Untyped), "Untyped closes the list");
    }

    #[test]
    fn the_system_of_a_free_tactical_activation_is_optional() {
        assert!(PlainRec::bare(PlainKind::SystemActivated).is_valid());
        assert!(PlainRec::one(PlainKind::SystemActivated, PlainArg::System(System::ALL[3])).is_valid());
        assert!(!PlainRec::one(PlainKind::SystemActivated, PlainArg::Seat(Seat(0))).is_valid());
    }

    #[test]
    fn a_cloned_queue_shares_its_prefix_and_appends_stay_apart() {
        let mut a: Log<Emit> = Log::new();
        a.extend((0..70).map(|i| Emit::Plain(PlainRec::one(PlainKind::TurnSkipped, PlainArg::Seat(Seat(i % 6))))));
        let mut b = a.clone();
        assert!(a.shares_prefix_with(&b));
        b.push(Emit::Plain(PlainRec::bare(PlainKind::QuashUsed)));
        assert_eq!((a.iter().count(), b.iter().count()), (70, 71));
        assert_eq!(b.last().and_then(Emit::plain).map(|p| p.kind), Some(PlainKind::QuashUsed));
        assert!(a.last().and_then(Emit::plain).is_some_and(|p| p.kind == PlainKind::TurnSkipped));
    }
}
