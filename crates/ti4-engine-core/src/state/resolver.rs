//! The persistent part of the timing resolver (docs/todo/D2-prereqs.md D2-4b), carried by `Pending`.
//!
//! A [`crate::kernel::Resolver`] is rebuilt for every window from this and from the state: the registry is
//! **derived** (`AbilityTable::registry`), the clock and the seats (phase, round, turn, seating order, active
//! seat, speaker) are the state's own, and the trace and the event journal are the events view's. What is
//! left, and cannot be derived, is here: the spent frequency keys in the order they were spent, the
//! initiative order the resolver was made with (the state's changes once the strategy cards are chosen),
//! the open emissions of a window opened inside an emission, and the next trace id.

use crate::choice::reaction::AbilityId;
use crate::containers::{Cow, SmallList};
use crate::ids::Seat;

use super::EventKind;

/// The scope of a spent frequency key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum UsedScope {
    Trigger,
    Turn,
    Round,
}

/// What the resolver carries from one window to the next.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolverState {
    /// Spent frequency keys `(ability, scope, counter)`, in the order they were spent. Behind a `Cow`: the
    /// list only grows over a game and a checkpoint must not copy it.
    pub used: Cow<Vec<(AbilityId, UsedScope, u64)>>,
    /// The initiative order the resolver was made with.
    pub initiative: SmallList<Seat, 8>,
    /// The open emissions around the next window, outermost first.
    pub emission_stack: Vec<(EventKind, u64)>,
    /// The next trace id (one-based).
    pub next_id: u64,
}

impl Default for ResolverState {
    fn default() -> Self {
        Self { used: Cow::default(), initiative: SmallList::new(), emission_stack: Vec::new(), next_id: 1 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_clone_shares_the_spent_keys_until_one_side_spends() {
        let mut a = ResolverState::default();
        a.used.edit().push((AbilityId { kind: crate::choice::reaction::AbilityKind::Ability, key: super::super::Code(1) }, UsedScope::Round, 2));
        let mut b = a.clone();
        assert!(Cow::ptr_eq(&a.used, &b.used));
        b.used.edit().clear();
        assert_ne!(a, b);
        assert_eq!(a.used.len(), 1);
    }

    #[test]
    fn the_default_counts_trace_ids_from_one() {
        assert_eq!(ResolverState::default().next_id, 1);
    }
}
