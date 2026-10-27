//! `Objectives`: revealed and scored objectives, the feat bookkeeping.

use crate::containers::{Cow, Set, SeatSet, SmallList};
use crate::generated::{PublicObjective, SecretObjective};
use crate::ids::Seat;

/// A revealed objective: a public objective, or a secret objective made public (Classified Document
/// Leaks puts the secret id into `revealed_objectives`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ObjectiveRef {
    Public(PublicObjective),
    Secret(SecretObjective),
}

/// A seat's scored objectives (`ObjectiveId` is a public or a secret objective id). Iteration across
/// the two classes merges by id text, which only the adapter knows (`NAME_RANK`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ObjectiveSet {
    pub public: Set<PublicObjective, 1>,
    pub secret: Set<SecretObjective, 1>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Objectives {
    /// `GameState.revealed_objectives` in reveal order.
    pub revealed: SmallList<ObjectiveRef, 12>,
    /// `GameState.scored_objectives`, by seat (owner index).
    pub scored: [ObjectiveSet; 16],
    /// Seats with a key in the string map (an empty set is a present key).
    pub scored_present: SeatSet,
    /// `GameState.scored_feat_occurrences`: sorted `(seat, occurrence)` pairs.
    pub scored_feats: Cow<Vec<(Seat, u64)>>,
}

impl Default for Objectives {
    fn default() -> Self {
        Self {
            revealed: SmallList::new(),
            scored: [ObjectiveSet::default(); 16],
            scored_present: SeatSet::EMPTY,
            scored_feats: Cow::default(),
        }
    }
}
