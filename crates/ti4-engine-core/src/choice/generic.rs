//! The generic decision families: the options that belong to no rule area (decline, done, a number)
//! and the asks and facts shared by every area. Owner: F4.
//!
//! The text of a generic option (its `kind`, `label`, payload) depends on the ask it appears in
//! ("Decline" vs "take the hit"); the core only knows the typed value, the adapter's
//! `Family::wording` supplies the text.

use crate::generated::{
    Ability, ActionCard, Attachment, Breakthrough, Explore, GenericCard, Leader, PromissoryNote, Relic, Technology,
};
use crate::state::Code;

/// Asks that belong to no area. Scaffolding for the generic families and for tests; areas have
/// their own asks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ask {
    /// "Done / decline": the options are [`super::Opt::Done`] and/or [`super::Opt::Decline`].
    Confirm,
    /// Pick a count: one [`super::Opt::Number`] per value in `lo..=hi`.
    Count { lo: i32, hi: i32 },
}

/// Subtypes that belong to no area.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Subtype {
    /// A question whose area is not ported yet. Only the corpus harness builds it (to check the
    /// other facts of a recorded context); a ported area never produces it.
    Unclassified,
}

/// The LRR reference a rule-sourced decision cites (`DecisionSource::Rule`). The set seen in the
/// decision corpus; append-only, through the foundation steward. Compound and non-numeric references
/// are variants of their own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Rule {
    R2_4,
    R8,
    R8_10,
    R8_11,
    R8_16,
    R8_18,
    R22,
    R22_1,
    R26,
    R27_2,
    R34_2,
    /// `34.3/75.2/75.3`
    R34_3_75_2_75_3,
    R42,
    R45_4,
    R49,
    R52_3,
    R52_4,
    R60,
    R61_6,
    R68,
    R78_4,
    R78_7,
    R78_9,
    R81_5,
    R82,
    R89_1,
    R89_2,
    R95,
    /// `end of turn`
    EndOfTurn,
    /// A reference not in the list above (the adapter keeps the text in its code table).
    Other(Code),
}

/// A content record named by an alias in `DecisionSource::{FactionAbility, Content}`. The adapter
/// finds the record kind by looking the alias up in the generated tables; an alias that no table has
/// stays [`Content::Other`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Content {
    Ability(Ability),
    Leader(Leader),
    Relic(Relic),
    Explore(Explore),
    Breakthrough(Breakthrough),
    Promissory(PromissoryNote),
    Technology(Technology),
    GenericCard(GenericCard),
    Attachment(Attachment),
    ActionCard(ActionCard),
    Other(Code),
}
