//! `GameState::faction_marks`: the typed key of every family of mark, and the typed tree a value
//! is parsed into.
//!
//! The families are declared in the text order of their literal prefixes (`mark_keys!` below), so
//! the derived family order and the field-wise comparison of [`MarkKey`]'s `Ord` give text order.
//! Each field names its type and the byte that follows it in the key text (0 at the end), which
//! the comparison needs for ids that are prefixes of other ids ([`super::order`]). The text of the
//! families (their literals and separators) is the adapter's table, in the same order.

use core::cmp::Ordering;

use super::atom::{Atom, AtomLike};
use super::decimal::DecimalTextOrd;
use super::word::Word;
use crate::generated::{Faction, Leader, Planet, PromissoryNote, System, Technology, Unit};
use crate::ids::Seat;

/// The most fields a key has.
pub const MAX_FIELDS: usize = 6;

macro_rules! mark_keys {
    ($( $(#[$meta:meta])* $name:ident ( $($f:ident : $ty:ty = $term:expr),* ) ),* $(,)?) => {
        /// The family of a mark key: which pattern of text it has.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[repr(u16)]
        pub enum Family { $($name),* }

        impl Family {
            /// Every family, in text order.
            pub const ALL: &'static [Family] = &[$(Family::$name),*];
            /// The number of families.
            pub const COUNT: usize = Self::ALL.len();

            /// The index into [`Family::ALL`].
            #[must_use]
            pub const fn index(self) -> usize { self as usize }

            /// The byte that follows each field in the key text (0 for the last).
            #[must_use]
            pub const fn terms(self) -> &'static [u8] {
                match self { $(Family::$name => &[$($term),*]),* }
            }
        }

        /// A key of `faction_marks`, one variant per family.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum MarkKey { $( $(#[$meta])* $name($($ty),*) ),* }

        impl MarkKey {
            /// The family of this key.
            #[must_use]
            pub const fn family(&self) -> Family {
                match self { $(Self::$name(..) => Family::$name),* }
            }

            /// The fields as atoms, and how many there are.
            #[must_use]
            #[allow(unused_assignments, unused_mut, unused_variables)]
            pub fn atoms(&self) -> ([Atom; MAX_FIELDS], usize) {
                match self { $(
                    Self::$name($($f),*) => {
                        let mut out = [Atom::Absent; MAX_FIELDS];
                        let mut n = 0;
                        $( out[n] = AtomLike::to_atom(*$f); n += 1; )*
                        (out, n)
                    }
                ),* }
            }

            /// The key of `family` whose fields are `atoms`; `None` when they do not fit.
            #[must_use]
            #[allow(unused_mut, unused_variables)]
            pub fn from_atoms(family: Family, atoms: &[Atom]) -> Option<Self> {
                match family { $(
                    Family::$name => {
                        let mut it = atoms.iter().copied();
                        let key = Self::$name($(<$ty as AtomLike>::from_atom(it.next()?)?),*);
                        if it.next().is_some() { return None; }
                        Some(key)
                    }
                ),* }
            }
        }
    };
}

mark_keys! {
    AllowPass(),
    ArborecLw2(owner: Seat = 0),
    ArborecPsychosporeExhausted(owner: Seat = 0),
    ArgentProduction(owner: Seat = b':', system: System = 0),
    ArgentWingTransfer(owner: Seat = 0),
    BastionIconExhausted(owner: Seat = 0),
    BastionLost(system: System = b':', owner: Seat = b':', unit: Unit = 0),
    BastionRetreated(system: System = b':', owner: Seat = 0),
    BonusFrom(),
    CabalCrucible(owner: Seat = 0),
    /// `cabal:devour:{system}` or, with a planet, `cabal:devour:{system}|{planet}`.
    CabalDevour(system: System = b'|', planet: Option<Planet> = 0),
    /// `cards:reveal:{action|choice|standing}|{source}|{from}|{to}|{deck}`.
    CardsReveal(kind: Word = b'|', source: Word = b'|', from: Seat = b'|', to: Seat = b'|', deck: Word = 0),
    /// Zero padded to 8 digits.
    CardsStaged(seq: u32 = 0),
    CombatSustainTarget(),
    CommanderAbility(holder: Seat = b':', commander: Leader = 0),
    CrimsonBreachActive(system: System = 0),
    CrimsonBtExhausted(owner: Seat = 0),
    CrimsonCard(owner: Seat = 0),
    CrimsonDeployed(invader: Seat = b':', seq: DecimalTextOrd = 0),
    CrimsonSever(note: PromissoryNote = b':', faction: Faction = 0),
    DeepwroughtBtExhausted(owner: Seat = 0),
    DeepwroughtCarrina(owner: Seat = 0),
    DeepwroughtOcean(planet: Planet = 0),
    DeepwroughtPurgedTech(tech: Technology = 0),
    DeepwroughtShare(note: PromissoryNote = b':', faction: Faction = 0),
    EmpyreanAetherstream(),
    EmpyreanPassage(),
    EmpyreanTetherAllow(),
    EmpyreanTether(owner: Seat = 0),
    FirmamentAgent(),
    FirmamentAgentMoved(),
    FirmamentEnervate(),
    FirmamentSowing(owner: Seat = 0),
    GhostAgentLink(),
    Inf2Card(owner: Seat = 0),
    KeleresOdlynn(owner: Seat = b':', agenda_seq: DecimalTextOrd = 0),
    KeleresOmniopiares(seq: DecimalTextOrd = b':', planet: Planet = 0),
    KeleresAgentGoods(payer: Seat = 0),
    KeleresAsnUsed(owner: Seat = 0),
    KeleresExecutiveOrderPending(),
    KeleresLawsBlank(),
    LinkFrom(),
    LinkTo(),
    LinkX(),
    LinkY(),
    MahactBenediction(),
    MahactCl2(owner: Seat = 0),
    MahactFleet(mahact: Seat = 0),
    MahactVaultsExhausted(owner: Seat = 0),
    /// Zero padded to 4 digits.
    MapEdit(seq: u32 = 0),
    MentakHero(system: System = 0),
    MuaatWarsunMoved(owner: Seat = 0),
    NaaluGiftRound(),
    NaazSupercharge(owner: Seat = 0),
    NekroAlastor(seq: DecimalTextOrd = b'|', system: System = b'|', owner: Seat = b'|', planet: Planet = b'|', unit: Unit = 0),
    NekroPropagation(owner: Seat = 0),
    NekroThreat(owner: Seat = 0),
    NekroThreatPhase(owner: Seat = 0),
    /// `nekro:token:{owner}:Z`.
    NekroToken(owner: Seat = b':'),
    NekroTs(system: System = b'|', planet: Option<Planet> = 0),
    NomadArtuno(owner: Seat = 0),
    NomadCavalry(holder: Seat = 0),
    NomadHero(owner: Seat = 0),
    NomadTcsNegotiating(owner: Seat = b':', other: Seat = 0),
    Paid(owner: Seat = b':', tech: Technology = 0),
    PrivateBorrowedScheming(owner: Seat = 0),
    PrivateCombatReplayRoll(),
    PrivateGhostCommander(owner: Seat = b':', seq: DecimalTextOrd = 0),
    PrivateMoved(seq: DecimalTextOrd = 0),
    /// `private:#seen:{breakthrough|relics|technologies}:{seat}`.
    PrivateSeen(kind: Word = b':', seat: Seat = 0),
    /// Zero padded to 6 digits.
    PrivateStagedEvent(seq: u32 = 0),
    /// Zero padded to 8 digits.
    PrivateStagedGround(seq: u32 = 0),
    /// `private:{seat}:{insider:peek|winnu:gained|winnu:imperator|winnu:tactical}`.
    PrivateSeat(seat: Seat = b':', kind: Word = 0),
    ProductionValueSwapSeq(),
    RalnelNanoExhausted(owner: Seat = 0),
    RalnelSkim(owner: Seat = 0),
    SaarAgentBoost(owner: Seat = 0),
    SaarDeorbitBarrageExhausted(owner: Seat = 0),
    SardakkAgentTactical(owner: Seat = 0),
    SardakkHeroSkip(),
    SardakkTekklarCombat(),
    SardakkTekklarRound(),
    TitansAllowanceAsked(owner: Seat = b':', round: DecimalTextOrd = 0),
    TitansCoalescence(planet: Planet = 0),
    TitansSleeper(planet: Planet = 0),
    WhX(),
    WhY(),
    YinAbilityUsed(owner: Seat = 0),
    YssarilBt(kind: Word = b':', owner: Seat = b':', user: Seat = 0),
    YssarilSsruuRoundAgent(borrower: Seat = b':', agent: Leader = b':', seq: DecimalTextOrd = 0),
}

impl PartialOrd for MarkKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for MarkKey {
    /// Text order of the rendered keys.
    fn cmp(&self, other: &Self) -> Ordering {
        match self.family().cmp(&other.family()) {
            Ordering::Equal => {}
            unequal => return unequal,
        }
        let (a, na) = self.atoms();
        let (b, nb) = other.atoms();
        let terms = self.family().terms();
        let term_after = |atoms: &[Atom; MAX_FIELDS], n: usize, i: usize| -> u8 {
            // An absent optional tail takes its separator with it.
            if i + 1 < n && atoms[i + 1] == Atom::Absent {
                0
            } else {
                terms[i]
            }
        };
        for i in 0..na.min(nb) {
            let ordering = a[i].cmp_followed(term_after(&a, na, i), b[i], term_after(&b, nb, i));
            if ordering != Ordering::Equal {
                return ordering;
            }
        }
        Ordering::Equal
    }
}

/// A parsed mark value: a tree of typed atoms. The shape of the tree (which separators, which atom
/// kinds in which place) is the family's value schema in the adapter. Values are never ordered.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MarkVal {
    Atom(Atom),
    /// Fields of different kinds joined by one separator (`06|mr`).
    Rec(Vec<MarkVal>),
    /// Items of one kind joined by one separator (`a,b,c`); possibly none (the empty text).
    List(Vec<MarkVal>),
}
