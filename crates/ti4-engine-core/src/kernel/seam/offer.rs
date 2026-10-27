//! P6, the offer point: a named position inside a window where providers may put a question.
//!
//! The window is a stage `Offering { point, idx }`: it asks the offers of the point in registration order,
//! one question at a time, and resumes at `idx + 1` after the answer. No `Resolver` involvement and no
//! frequency scopes (those are for reaction windows, which are events).

use super::{Q, Source};
use crate::choice::{Ask, Opt};
use crate::generated::{Planet, System};
use crate::ids::Seat;
use crate::kernel::{Cx, Next};

/// The named points of the windows. Closed: each window lists its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Point {
    PaymentWindow,
    ProductionStart,
    ProductionEnd,
    CombatStart,
    AfterRollDice,
    AfterBarrage,
    RoundEnd,
    BeforeCommit,
    CommitToll,
    AfterActivation,
}

impl Point {
    pub const COUNT: usize = 10;
}

/// Where the offer is made: who is offered, in which system and on which planet, which round.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OfferSite {
    pub who: Seat,
    pub system: Option<System>,
    pub planet: Option<Planet>,
    pub round: u32,
}

pub struct Offer {
    pub source: Source,
    /// `None`: nothing to offer to this seat now.
    pub ask: fn(Q<'_>, &OfferSite) -> Option<Ask>,
    /// The answer to the question.
    pub apply: fn(&mut Cx<'_>, &OfferSite, Opt) -> Next,
}

impl Clone for Offer {
    fn clone(&self) -> Self {
        *self
    }
}
impl Copy for Offer {}

#[derive(Clone)]
pub struct Offers {
    at: [Vec<Offer>; Point::COUNT],
}

impl Default for Offers {
    fn default() -> Self {
        Self { at: std::array::from_fn(|_| Vec::new()) }
    }
}

impl Offers {
    #[must_use]
    pub fn len(&self) -> usize {
        self.at.iter().map(Vec::len).sum()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn push(&mut self, point: Point, offer: Offer) {
        self.at[point as usize].push(offer);
    }

    /// The first offer at or after `from` that has a question: its index and the question.
    #[must_use]
    pub fn next_ask(&self, q: Q<'_>, point: Point, from: usize, site: &OfferSite) -> Option<(usize, Ask)> {
        self.at[point as usize].iter().enumerate().skip(from).find_map(|(i, o)| (o.ask)(q, site).map(|a| (i, a)))
    }

    /// Apply the answer to the offer at `idx`.
    pub fn apply(&self, cx: &mut Cx<'_>, point: Point, idx: usize, site: &OfferSite, answer: Opt) -> Next {
        match self.at[point as usize].get(idx) {
            Some(o) => (o.apply)(cx, site, answer),
            None => Next::Done,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::Kernel;
    use crate::state::CoreState;

    #[test]
    fn offers_are_iterated_in_registration_order_skipping_silent_ones() {
        let (k, s) = (Kernel::plain(), CoreState::all_loaded());
        let q = Q::new(&s, &k);
        let site = OfferSite { who: Seat(0), system: None, planet: None, round: 1 };
        let mut o = Offers::default();
        o.push(Point::PaymentWindow, Offer { source: Source::Printed, ask: |_, _| None, apply: |_, _, _| Next::Done });
        o.push(
            Point::PaymentWindow,
            Offer { source: Source::Printed, ask: |_, _| Some(Ask::Generic(crate::choice::generic::Ask::Confirm)), apply: |_, _, _| Next::Done },
        );
        let (idx, _) = o.next_ask(q, Point::PaymentWindow, 0, &site).expect("the second offer asks");
        assert_eq!(idx, 1);
        assert!(o.next_ask(q, Point::PaymentWindow, 2, &site).is_none(), "resume after the last one");
        assert!(o.next_ask(q, Point::ProductionEnd, 0, &site).is_none());
        assert_eq!(o.len(), 2);
    }
}
