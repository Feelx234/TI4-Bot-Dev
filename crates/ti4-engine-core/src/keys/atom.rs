//! The pieces a key or a value is made of.

use core::cmp::Ordering;

use super::decimal::DecimalTextOrd;
use super::destroy::DestroyReason;
use super::order;
use super::outcome::Outcome;
use super::word::Word;
use crate::generated::{
    ActionCard, Agenda, Breakthrough, Faction, Leader, Planet, PromissoryNote, Relic,
    SecretObjective, System, Technology, Unit,
};
use crate::ids::Seat;

/// One typed piece of a mark key or mark value. What the text of a piece looks like (padding,
/// separators) is the family's pattern in the adapter; here it is only which kind of thing it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Atom {
    /// An optional piece that is not there (the empty text between two separators).
    Absent,
    /// A plain count or sequence number (or one padded to a fixed width: the pattern knows).
    Num(u32),
    /// A number that sorts as its decimal text.
    Dec(DecimalTextOrd),
    Seat(Seat),
    System(System),
    Planet(Planet),
    Unit(Unit),
    Tech(Technology),
    Card(ActionCard),
    Note(PromissoryNote),
    Secret(SecretObjective),
    Agenda(Agenda),
    Faction(Faction),
    Leader(Leader),
    Relic(Relic),
    Breakthrough(Breakthrough),
    Word(Word),
    Outcome(Outcome),
    Destroy(DestroyReason),
    /// A value whose structure belongs to a later package (a staged event, a borrowed-agent record,
    /// a sustain target): an index into the adapter's table of texts. Never part of a key.
    Blob(u32),
}

impl Atom {
    /// The kind, as a small number (the order of the variants above).
    #[must_use]
    pub const fn kind(&self) -> u8 {
        match self {
            Self::Absent => 0,
            Self::Num(_) => 1,
            Self::Dec(_) => 2,
            Self::Seat(_) => 3,
            Self::System(_) => 4,
            Self::Planet(_) => 5,
            Self::Unit(_) => 6,
            Self::Tech(_) => 7,
            Self::Card(_) => 8,
            Self::Note(_) => 9,
            Self::Secret(_) => 10,
            Self::Agenda(_) => 11,
            Self::Faction(_) => 12,
            Self::Leader(_) => 13,
            Self::Relic(_) => 14,
            Self::Breakthrough(_) => 15,
            Self::Word(_) => 16,
            Self::Outcome(_) => 17,
            Self::Destroy(_) => 18,
            Self::Blob(_) => 19,
        }
    }

    /// Compare the texts of two pieces of the same place in a key, `term` and `other_term` being
    /// the byte that follows each in its key (0 at the end). See [`order`].
    #[must_use]
    pub fn cmp_followed(self, term: u8, other: Self, other_term: u8) -> Ordering {
        match (self, other) {
            (Self::Num(a), Self::Num(b)) => a.cmp(&b).then(term.cmp(&other_term)),
            (Self::Dec(a), Self::Dec(b)) => a.cmp_followed(term, b, other_term),
            (Self::Seat(a), Self::Seat(b)) => a.cmp(&b).then(term.cmp(&other_term)),
            (Self::System(a), Self::System(b)) => {
                order::cmp_indexed(order::SYSTEM, a.index(), term, b.index(), other_term)
            }
            (Self::Planet(a), Self::Planet(b)) => {
                order::cmp_indexed(order::PLANET, a.index(), term, b.index(), other_term)
            }
            (Self::Unit(a), Self::Unit(b)) => {
                order::cmp_indexed(order::UNIT, a.index(), term, b.index(), other_term)
            }
            (Self::Tech(a), Self::Tech(b)) => {
                order::cmp_indexed(order::TECHNOLOGY, a.index(), term, b.index(), other_term)
            }
            (Self::Card(a), Self::Card(b)) => {
                order::cmp_indexed(order::ACTION_CARD, a.index(), term, b.index(), other_term)
            }
            (Self::Note(a), Self::Note(b)) => {
                // The generic `<color>_an` notes are written `an`: they sort by that text.
                let (ra, rb) = (
                    usize::from(order::NOTE_RANK[a.index()]),
                    usize::from(order::NOTE_RANK[b.index()]),
                );
                order::cmp_indexed(order::PROMISSORY_NOTE, ra, term, rb, other_term)
            }
            (Self::Secret(a), Self::Secret(b)) => {
                order::cmp_indexed(order::NONE, a.index(), term, b.index(), other_term)
            }
            (Self::Agenda(a), Self::Agenda(b)) => {
                order::cmp_indexed(order::AGENDA, a.index(), term, b.index(), other_term)
            }
            (Self::Faction(a), Self::Faction(b)) => {
                order::cmp_indexed(order::FACTION, a.index(), term, b.index(), other_term)
            }
            (Self::Leader(a), Self::Leader(b)) => {
                order::cmp_indexed(order::LEADER, a.index(), term, b.index(), other_term)
            }
            (Self::Word(a), Self::Word(b)) => {
                order::cmp_indexed(order::WORD, a.index(), term, b.index(), other_term)
            }
            (Self::Absent, Self::Absent) => term.cmp(&other_term),
            // Pieces of different kinds are never at the same place of one family; order them
            // somehow (by kind) so the comparison stays total.
            (a, b) => a.kind().cmp(&b.kind()),
        }
    }
}

/// A type that is one kind of [`Atom`]: what a field of a [`super::MarkKey`] variant is made of.
pub trait AtomLike: Sized + Copy {
    fn to_atom(self) -> Atom;
    fn from_atom(atom: Atom) -> Option<Self>;
}

macro_rules! atom_like {
    ($($ty:ty => $variant:ident),* $(,)?) => {$(
        impl AtomLike for $ty {
            #[inline]
            fn to_atom(self) -> Atom { Atom::$variant(self) }
            #[inline]
            fn from_atom(atom: Atom) -> Option<Self> {
                if let Atom::$variant(x) = atom { Some(x) } else { None }
            }
        }
    )*};
}

atom_like! {
    u32 => Num,
    DecimalTextOrd => Dec,
    Seat => Seat,
    System => System,
    Planet => Planet,
    Unit => Unit,
    Technology => Tech,
    ActionCard => Card,
    PromissoryNote => Note,
    SecretObjective => Secret,
    Agenda => Agenda,
    Faction => Faction,
    Leader => Leader,
    Relic => Relic,
    Breakthrough => Breakthrough,
    Word => Word,
    Outcome => Outcome,
    DestroyReason => Destroy,
}

/// An optional trailing piece: `None` is [`Atom::Absent`].
impl<T: AtomLike> AtomLike for Option<T> {
    fn to_atom(self) -> Atom {
        self.map_or(Atom::Absent, AtomLike::to_atom)
    }
    fn from_atom(atom: Atom) -> Option<Self> {
        match atom {
            Atom::Absent => Some(None),
            other => T::from_atom(other).map(Some),
        }
    }
}
