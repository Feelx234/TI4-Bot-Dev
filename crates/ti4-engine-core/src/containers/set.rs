use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

use super::idx::Idx;
use crate::ids::Seat;

/// A set over an id enum as `W` 64-bit words (`W * 64 >= T::COUNT`). Iteration is in index order,
/// which is the text order of the ids, so it visits ids exactly as `BTreeSet<String>` did.
pub struct Set<T: Idx, const W: usize> {
    words: [u64; W],
    _t: PhantomData<T>,
}

impl<T: Idx, const W: usize> Set<T, W> {
    pub const EMPTY: Self = Self { words: [0; W], _t: PhantomData };

    #[must_use]
    pub const fn new() -> Self {
        Self::EMPTY
    }

    #[inline]
    #[must_use]
    pub fn contains(&self, t: T) -> bool {
        let i = t.index();
        debug_assert!(i < W * 64);
        self.words[i >> 6] & (1 << (i & 63)) != 0
    }

    /// Insert; whether the element was new (as `BTreeSet::insert`).
    #[inline]
    pub fn insert(&mut self, t: T) -> bool {
        let i = t.index();
        debug_assert!(i < W * 64);
        let bit = 1u64 << (i & 63);
        let word = &mut self.words[i >> 6];
        let new = *word & bit == 0;
        *word |= bit;
        new
    }

    /// Remove; whether the element was present.
    #[inline]
    pub fn remove(&mut self, t: T) -> bool {
        let i = t.index();
        debug_assert!(i < W * 64);
        let bit = 1u64 << (i & 63);
        let word = &mut self.words[i >> 6];
        let had = *word & bit != 0;
        *word &= !bit;
        had
    }

    pub fn clear(&mut self) {
        self.words = [0; W];
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.words.iter().all(|w| *w == 0)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.words.iter().map(|w| w.count_ones() as usize).sum()
    }

    pub fn union_with(&mut self, other: &Self) {
        for (a, b) in self.words.iter_mut().zip(other.words.iter()) {
            *a |= *b;
        }
    }

    pub fn intersect_with(&mut self, other: &Self) {
        for (a, b) in self.words.iter_mut().zip(other.words.iter()) {
            *a &= *b;
        }
    }

    pub fn subtract(&mut self, other: &Self) {
        for (a, b) in self.words.iter_mut().zip(other.words.iter()) {
            *a &= !*b;
        }
    }

    #[must_use]
    pub fn intersects(&self, other: &Self) -> bool {
        self.words.iter().zip(other.words.iter()).any(|(a, b)| a & b != 0)
    }

    #[must_use]
    pub fn is_subset(&self, other: &Self) -> bool {
        self.words.iter().zip(other.words.iter()).all(|(a, b)| a & !b == 0)
    }

    /// Elements in ascending index order.
    pub fn iter(&self) -> impl Iterator<Item = T> + '_ {
        self.words.iter().enumerate().flat_map(|(w, &word)| {
            let mut rest = word;
            std::iter::from_fn(move || {
                if rest == 0 {
                    return None;
                }
                let bit = rest.trailing_zeros() as usize;
                rest &= rest - 1;
                T::from_index(w * 64 + bit)
            })
        })
    }

    /// The smallest element.
    #[must_use]
    pub fn first(&self) -> Option<T> {
        self.iter().next()
    }

    /// The raw words (codec and hashing use).
    #[must_use]
    pub const fn words(&self) -> &[u64; W] {
        &self.words
    }
}

impl<T: Idx, const W: usize> Clone for Set<T, W> {
    #[inline]
    fn clone(&self) -> Self {
        *self
    }
}
impl<T: Idx, const W: usize> Copy for Set<T, W> {}
impl<T: Idx, const W: usize> Default for Set<T, W> {
    fn default() -> Self {
        Self::EMPTY
    }
}
impl<T: Idx, const W: usize> PartialEq for Set<T, W> {
    fn eq(&self, other: &Self) -> bool {
        self.words == other.words
    }
}
impl<T: Idx, const W: usize> Eq for Set<T, W> {}
impl<T: Idx, const W: usize> Hash for Set<T, W> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.words.hash(state);
    }
}
impl<T: Idx + fmt::Debug, const W: usize> fmt::Debug for Set<T, W> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set().entries(self.iter()).finish()
    }
}
impl<T: Idx, const W: usize> FromIterator<T> for Set<T, W> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut set = Self::EMPTY;
        for t in iter {
            set.insert(t);
        }
        set
    }
}

/// A set of seats (owners): up to 16.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct SeatSet(pub u16);

impl SeatSet {
    pub const EMPTY: Self = Self(0);

    #[inline]
    #[must_use]
    pub const fn contains(self, s: Seat) -> bool {
        self.0 & s.bit() != 0
    }

    #[inline]
    pub fn insert(&mut self, s: Seat) -> bool {
        let new = !self.contains(s);
        self.0 |= s.bit();
        new
    }

    #[inline]
    pub fn remove(&mut self, s: Seat) -> bool {
        let had = self.contains(s);
        self.0 &= !s.bit();
        had
    }

    #[must_use]
    pub const fn len(self) -> usize {
        self.0.count_ones() as usize
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    #[must_use]
    pub const fn intersection(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }

    #[must_use]
    pub const fn without(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }

    pub fn iter(self) -> impl Iterator<Item = Seat> {
        let mut rest = self.0;
        std::iter::from_fn(move || {
            if rest == 0 {
                return None;
            }
            let bit = rest.trailing_zeros() as u8;
            rest &= rest - 1;
            Some(Seat(bit))
        })
    }
}

impl fmt::Debug for SeatSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set().entries(self.iter().map(|s| s.0)).finish()
    }
}
impl FromIterator<Seat> for SeatSet {
    fn from_iter<I: IntoIterator<Item = Seat>>(iter: I) -> Self {
        let mut set = Self::EMPTY;
        for s in iter {
            set.insert(s);
        }
        set
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated::{Technology, Planet};

    #[test]
    fn iterates_in_index_order_like_a_btreeset() {
        let mut s: Set<Technology, 2> = Set::new();
        for i in [101, 3, 64, 0, 63, 70] {
            s.insert(Technology::from_index(i).unwrap());
        }
        let got: Vec<usize> = s.iter().map(Idx::index).collect();
        assert_eq!(got, vec![0, 3, 63, 64, 70, 101]);
        assert!(!s.insert(Technology::from_index(3).unwrap()));
        assert!(s.remove(Technology::from_index(3).unwrap()));
        assert!(!s.remove(Technology::from_index(3).unwrap()));
        assert_eq!(s.len(), 5);
        assert_eq!(std::mem::size_of::<Set<Technology, 2>>(), 16);
        let planets: Set<Planet, 3> = Planet::ALL.iter().copied().collect();
        assert_eq!(planets.len(), Planet::COUNT);
    }

    #[test]
    fn seat_set() {
        let mut s = SeatSet::EMPTY;
        s.insert(Seat(5));
        s.insert(Seat(1));
        s.insert(Seat(15));
        assert_eq!(s.iter().map(|x| x.0).collect::<Vec<_>>(), vec![1, 5, 15]);
        assert_eq!(s.len(), 3);
    }
}
