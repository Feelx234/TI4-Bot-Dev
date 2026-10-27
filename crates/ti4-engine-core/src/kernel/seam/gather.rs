//! P3, the contribution list: options or participants that providers add to (or edit out of) a list.

use super::{Permute, Q, Source};
use crate::containers::SmallList;

/// Whether the result keeps registration order or is sorted and deduplicated (`concat_sorted`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Order {
    Registration,
    Sorted,
}

/// The two passes the string engine has: the adds, then the in-place edits (`bastion_adjust_ship_spots`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pass {
    Add,
    Edit,
}

pub struct Contrib<S, T: Copy, const N: usize> {
    pub source: Source,
    pub pass: Pass,
    pub f: fn(Q<'_>, &S, &mut SmallList<T, N>),
}

impl<S, T: Copy, const N: usize> Clone for Contrib<S, T, N> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<S, T: Copy, const N: usize> Copy for Contrib<S, T, N> {}

pub struct Gather<S, T: Copy, const N: usize> {
    items: Vec<Contrib<S, T, N>>,
    order: Order,
}

impl<S, T: Copy, const N: usize> Clone for Gather<S, T, N> {
    fn clone(&self) -> Self {
        Self { items: self.items.clone(), order: self.order }
    }
}

impl<S, T: Copy + Ord, const N: usize> Gather<S, T, N> {
    #[must_use]
    pub const fn new(order: Order) -> Self {
        Self { items: Vec::new(), order }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn push(&mut self, source: Source, pass: Pass, f: fn(Q<'_>, &S, &mut SmallList<T, N>)) {
        self.items.push(Contrib { source, pass, f });
    }

    /// Every add in registration order, then every edit, then sort and dedup when the gather says so.
    #[must_use]
    pub fn collect(&self, q: Q<'_>, site: &S) -> SmallList<T, N> {
        let mut out = SmallList::default();
        for pass in [Pass::Add, Pass::Edit] {
            for c in self.items.iter().filter(|c| c.pass == pass) {
                (c.f)(q, site, &mut out);
            }
        }
        if self.order == Order::Sorted {
            out.as_mut_slice().sort();
            let mut kept: SmallList<T, N> = SmallList::default();
            for x in &out {
                if kept.last().is_none_or(|l| l != x) {
                    kept.push(*x);
                }
            }
            return kept;
        }
        out
    }

    /// A sorted gather does not depend on registration order (adds only); an ordered one does.
    pub fn permute_if_sorted(&mut self, p: Permute) {
        if self.order == Order::Sorted && self.items.iter().all(|c| c.pass == Pass::Add) {
            p.apply(&mut self.items);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::Kernel;
    use crate::state::CoreState;

    type G = Gather<u8, u8, 4>;

    #[test]
    fn adds_then_edits_and_sorting_is_explicit() {
        let (k, s) = (Kernel::plain(), CoreState::all_loaded());
        let q = Q::new(&s, &k);
        let mut g = G::new(Order::Registration);
        g.push(Source::Printed, Pass::Edit, |_, _, out| out.retain(|x| *x != 2));
        g.push(Source::Printed, Pass::Add, |_, _, out| out.extend([3, 2]));
        g.push(Source::Printed, Pass::Add, |_, _, out| out.extend([1, 3]));
        assert_eq!(g.collect(q, &0).as_slice(), &[3, 1, 3]);
        let mut sorted = G::new(Order::Sorted);
        sorted.push(Source::Printed, Pass::Add, |_, _, out| out.extend([3, 1]));
        sorted.push(Source::Printed, Pass::Add, |_, _, out| out.extend([2, 3]));
        assert_eq!(sorted.collect(q, &0).as_slice(), &[1, 2, 3]);
        sorted.permute_if_sorted(Permute::Reverse);
        assert_eq!(sorted.collect(q, &0).as_slice(), &[1, 2, 3]);
    }
}
