//! P4, the replacement of a rule step: an offer to do a step differently, and the way to do it.

use super::{Q, Source};
use crate::kernel::{Cx, Next};

/// The steps of the rules a window declares replaceable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Step {
    UnitForm,
    ProductionExchange,
    PdsPlacement,
    SecondaryAsPrimary,
    SkipToCommit,
    BorrowedStats,
    CapturedReturn,
    FreeCapacity,
}

pub struct Replacement<S, O> {
    pub source: Source,
    pub step: Step,
    /// Whether (and how) this provider replaces the step here. The first `Some` wins, in registration order.
    pub offer: fn(Q<'_>, &S) -> Option<O>,
    /// Do the replaced step. `Next::Continue(ask)` suspends it on a question (the string `*_performed` hook that asks).
    pub perform: fn(&mut Cx<'_>, &S, O) -> Next,
}

impl<S, O> Clone for Replacement<S, O> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<S, O> Copy for Replacement<S, O> {}

pub struct Replacements<S, O> {
    items: Vec<Replacement<S, O>>,
}

impl<S, O> Clone for Replacements<S, O> {
    fn clone(&self) -> Self {
        Self { items: self.items.clone() }
    }
}

impl<S, O> Replacements<S, O> {
    #[must_use]
    pub const fn new() -> Self {
        Self { items: Vec::new() }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn push(&mut self, r: Replacement<S, O>) {
        self.items.push(r);
    }

    /// The first provider that offers, with what it offers.
    #[must_use]
    pub fn first(&self, q: Q<'_>, site: &S) -> Option<(Replacement<S, O>, O)> {
        self.items.iter().find_map(|r| (r.offer)(q, site).map(|o| (*r, o)))
    }

    /// The first offer only (the pure half; most windows only need to know that a replacement exists).
    #[must_use]
    pub fn offered(&self, q: Q<'_>, site: &S) -> Option<O> {
        self.first(q, site).map(|(_, o)| o)
    }
}

impl<S, O> Default for Replacements<S, O> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::Kernel;
    use crate::state::CoreState;

    #[test]
    fn the_first_offer_wins_in_registration_order() {
        let (k, s) = (Kernel::plain(), CoreState::all_loaded());
        let q = Q::new(&s, &k);
        let mut r = Replacements::<u8, u8>::new();
        let r1 = Replacement { source: Source::Printed, step: Step::UnitForm, offer: |_, x| (*x > 5).then_some(1), perform: |_, _, _| Next::Done };
        let r2 = Replacement { source: Source::Printed, step: Step::UnitForm, offer: |_, _| Some(2), perform: |_, _, _| Next::Done };
        r.push(r1);
        r.push(r2);
        assert_eq!(r.offered(q, &9), Some(1));
        assert_eq!(r.offered(q, &0), Some(2));
    }
}
