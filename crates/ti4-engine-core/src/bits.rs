//! Fixed-size bitsets over the id spaces. Iteration is in index order, which is the text order of
//! the ids, so a bitset iterates like the `BTreeSet<String>` it replaces.

use crate::tables::{SYSTEM_COUNT, UNIT_COUNT};

/// A bitset of `W` 64-bit words.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Bits<const W: usize> {
    words: [u64; W],
}

impl<const W: usize> std::fmt::Debug for Bits<W> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_set().entries(self.iter()).finish()
    }
}

impl<const W: usize> Default for Bits<W> {
    fn default() -> Self {
        Self::EMPTY
    }
}

impl<const W: usize> Bits<W> {
    pub const EMPTY: Self = Self { words: [0; W] };

    #[inline]
    #[must_use]
    pub const fn contains(&self, i: usize) -> bool {
        self.words[i >> 6] & (1 << (i & 63)) != 0
    }

    /// Insert; whether the bit was new (as `BTreeSet::insert`).
    #[inline]
    pub fn insert(&mut self, i: usize) -> bool {
        let word = &mut self.words[i >> 6];
        let bit = 1 << (i & 63);
        let new = *word & bit == 0;
        *word |= bit;
        new
    }

    #[inline]
    pub fn remove(&mut self, i: usize) {
        self.words[i >> 6] &= !(1 << (i & 63));
    }

    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.words.iter().all(|w| *w == 0)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.words.iter().map(|w| w.count_ones() as usize).sum()
    }

    #[inline]
    pub fn union_with(&mut self, other: &Self) {
        for (a, b) in self.words.iter_mut().zip(other.words.iter()) {
            *a |= *b;
        }
    }

    #[inline]
    pub fn intersect_with(&mut self, other: &Self) {
        for (a, b) in self.words.iter_mut().zip(other.words.iter()) {
            *a &= *b;
        }
    }

    #[inline]
    #[must_use]
    pub fn intersects(&self, other: &Self) -> bool {
        self.words
            .iter()
            .zip(other.words.iter())
            .any(|(a, b)| a & b != 0)
    }

    /// Indices in ascending order.
    pub fn iter(&self) -> impl Iterator<Item = usize> + '_ {
        self.words.iter().enumerate().flat_map(|(w, &word)| {
            let mut rest = word;
            std::iter::from_fn(move || {
                if rest == 0 {
                    return None;
                }
                let bit = rest.trailing_zeros() as usize;
                rest &= rest - 1;
                Some(w * 64 + bit)
            })
        })
    }
}

pub const SYS_WORDS: usize = SYSTEM_COUNT.div_ceil(64);
pub const UNIT_WORDS: usize = UNIT_COUNT.div_ceil(64);

/// A set of systems.
pub type SysSet = Bits<SYS_WORDS>;
/// A set of unit types.
pub type UnitSet = Bits<UNIT_WORDS>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iterates_in_index_order_like_a_btreeset() {
        let mut s = SysSet::EMPTY;
        for i in [130, 3, 64, 0, 63, 200] {
            s.insert(i);
        }
        assert_eq!(s.iter().collect::<Vec<_>>(), vec![0, 3, 63, 64, 130, 200]);
        assert!(!s.insert(3));
        s.remove(3);
        assert!(!s.contains(3));
        assert_eq!(s.len(), 5);
    }
}
