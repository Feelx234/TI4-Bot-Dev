//! P1, the modifier stack: a number built from a base and the deltas of every provider.

use super::data::ModifierRow;
use super::sites::Site;
use super::{Permute, Q, Source};
use crate::containers::SmallList;

/// What one provider does to the number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delta {
    /// Nothing (the provider does not apply to this site).
    None,
    /// Add to the running value. Adds commute.
    Add(i64),
    /// Replace the running value; order-sensitive, so it must say why.
    Set { to: i64, why: SetWhy },
}

/// Why a provider sets the value instead of adding (A5: "an absolute set must say why").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetWhy {
    /// A card or ability that says "instead" / "rolls N dice".
    Instead,
    /// A legacy `fold` slot of the string engine (it may do anything to the running value).
    LegacyFold,
}

/// The order of terms of today's expressions: printed, then technology, relic, law, faction, leader, card.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Layer {
    Printed,
    Technology,
    Relic,
    Law,
    Faction,
    Leader,
    Card,
}

/// The bound applied after the last provider (`.max(0)`, the roll range).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Clamp {
    None,
    Min(i64),
    Range(i64, i64),
}

impl Clamp {
    #[must_use]
    pub const fn apply(self, v: i64) -> i64 {
        match self {
            Self::None => v,
            Self::Min(lo) => {
                if v < lo {
                    lo
                } else {
                    v
                }
            }
            Self::Range(lo, hi) => {
                if v < lo {
                    lo
                } else if v > hi {
                    hi
                } else {
                    v
                }
            }
        }
    }
}

/// One entry of a stack. `f(q, site, running)` sees the value so far (a legacy fold needs it).
pub struct Provider<S> {
    pub source: Source,
    pub layer: Layer,
    /// Declared at registration: this provider only ever returns `None` or `Add`.
    pub adds_only: bool,
    pub f: fn(Q<'_>, &S, i64) -> Delta,
}

impl<S> Clone for Provider<S> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<S> Copy for Provider<S> {}

/// The result, with the parts that feed the UI facts (`Add` terms and the effect of each `Set`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Evaluated {
    pub value: i64,
    pub parts: SmallList<(Source, i64), 8>,
}

/// The providers of one number, ordered by `(layer, registration index)`.
pub struct Stack<S> {
    items: Vec<Entry<S>>,
}

/// A function provider or a data row (`ModifierRow`, read through the site).
pub enum Entry<S> {
    Fn(Provider<S>),
    Row(&'static ModifierRow),
}

impl<S> Clone for Entry<S> {
    fn clone(&self) -> Self {
        match self {
            Self::Fn(p) => Self::Fn(*p),
            Self::Row(r) => Self::Row(r),
        }
    }
}

impl<S> Entry<S> {
    fn layer(&self) -> Layer {
        match self {
            Self::Fn(p) => p.layer,
            Self::Row(r) => r.source.layer(),
        }
    }

    fn adds_only(&self) -> bool {
        match self {
            Self::Fn(p) => p.adds_only,
            Self::Row(_) => true,
        }
    }
}

impl<S> Clone for Stack<S> {
    fn clone(&self) -> Self {
        Self { items: self.items.clone() }
    }
}

impl<S> Default for Stack<S> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S> Stack<S> {
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

    /// The entries in evaluation order.
    #[must_use]
    pub fn entries(&self) -> &[Entry<S>] {
        &self.items
    }

    fn insert(&mut self, e: Entry<S>) {
        // After the last entry of the same or an earlier layer: stable by registration index.
        let layer = e.layer();
        let at = self.items.partition_point(|x| x.layer() <= layer);
        self.items.insert(at, e);
    }

    /// Register a data row; it is read through [`Site::row_times`] and adds `row.amount(times)`.
    pub fn push_row(&mut self, row: &'static ModifierRow) {
        self.insert(Entry::Row(row));
    }

    /// Register a provider that only adds (a bonus, a malus).
    pub fn push_add(&mut self, source: Source, layer: Layer, f: fn(Q<'_>, &S, i64) -> Delta) {
        self.insert(Entry::Fn(Provider { source, layer, adds_only: true, f }));
    }

    /// Register a provider that may set the value.
    pub fn push_set(&mut self, source: Source, layer: Layer, f: fn(Q<'_>, &S, i64) -> Delta) {
        self.insert(Entry::Fn(Provider { source, layer, adds_only: false, f }));
    }

    /// Reorder the entries of each layer whose entries all only add (the audit; other layers keep their order).
    pub fn permute_commutative(&mut self, p: Permute) {
        let mut start = 0;
        while start < self.items.len() {
            let layer = self.items[start].layer();
            let end = start + self.items[start..].iter().take_while(|x| x.layer() == layer).count();
            if self.items[start..end].iter().all(Entry::adds_only) {
                p.apply(&mut self.items[start..end]);
            }
            start = end;
        }
    }
}

impl<S: Site> Stack<S> {
    /// `base`, then every provider and row in order, then `clamp`.
    #[must_use]
    pub fn eval(&self, q: Q<'_>, site: &S, base: i64, clamp: Clamp) -> Evaluated {
        let mut value = base;
        let mut parts = SmallList::default();
        for e in &self.items {
            let (source, delta) = match e {
                Entry::Fn(p) => (p.source, (p.f)(q, site, value)),
                Entry::Row(r) => {
                    let times = site.row_times(q, r);
                    (r.source, if times == 0 { Delta::None } else { Delta::Add(r.amount(times)) })
                }
            };
            match delta {
                Delta::None => {}
                Delta::Add(n) => {
                    value += n;
                    if n != 0 {
                        parts.push((source, n));
                    }
                }
                Delta::Set { to, .. } => {
                    if to != value {
                        parts.push((source, to - value));
                    }
                    value = to;
                }
            }
        }
        Evaluated { value: clamp.apply(value), parts }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::Kernel;
    use crate::state::CoreState;

    impl Site for u8 {}

    fn ids(st: &Stack<u8>) -> Vec<usize> {
        st.entries()
            .iter()
            .map(|e| match e {
                Entry::Fn(p) => p.f as usize,
                Entry::Row(r) => *r as *const _ as usize,
            })
            .collect()
    }

    fn plus_one(_: Q<'_>, _: &u8, _: i64) -> Delta {
        Delta::Add(1)
    }
    fn plus_ten(_: Q<'_>, _: &u8, _: i64) -> Delta {
        Delta::Add(10)
    }
    fn double(_: Q<'_>, _: &u8, v: i64) -> Delta {
        Delta::Set { to: v * 2, why: SetWhy::Instead }
    }

    #[test]
    fn layers_order_the_terms_and_registration_order_breaks_ties() {
        let (k, s) = (Kernel::plain(), CoreState::all_loaded());
        let q = Q::new(&s, &k);
        let mut st = Stack::<u8>::new();
        st.push_set(Source::Printed, Layer::Faction, double);
        st.push_add(Source::Printed, Layer::Technology, plus_one);
        // (3 + 1) * 2 = 8, even though the doubling was registered first.
        assert_eq!(st.eval(q, &0, 3, Clamp::None).value, 8);
        st.push_add(Source::Printed, Layer::Faction, plus_ten);
        assert_eq!(st.eval(q, &0, 3, Clamp::None).value, 18, "same layer: after the double");
        assert_eq!(st.eval(q, &0, 3, Clamp::Range(1, 10)).value, 10);
        assert_eq!(st.eval(q, &0, 3, Clamp::None).parts.len(), 3);
    }

    #[test]
    fn only_layers_made_of_adds_are_permuted() {
        let mut st = Stack::<u8>::new();
        st.push_add(Source::Printed, Layer::Law, plus_one);
        st.push_add(Source::Printed, Layer::Law, plus_ten);
        st.push_set(Source::Printed, Layer::Faction, double);
        st.push_add(Source::Printed, Layer::Faction, plus_one);
        let before = ids(&st);
        st.permute_commutative(Permute::Reverse);
        let after = ids(&st);
        assert_eq!(before[0], after[1]);
        assert_eq!(before[2..], after[2..], "a layer with a Set keeps its order");
    }
}
