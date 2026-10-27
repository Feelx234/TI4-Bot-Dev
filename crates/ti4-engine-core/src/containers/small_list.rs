use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::Deref;

/// An ordered list stored inline up to `N` elements and on the heap beyond. Insertion order is
/// kept (hand order, unit order and the JSON depend on it); equality ignores the representation.
pub struct SmallList<T: Copy, const N: usize> {
    repr: Repr<T, N>,
}

#[derive(Clone)]
enum Repr<T: Copy, const N: usize> {
    Empty,
    Inline { len: u8, items: [T; N] },
    /// Boxed so the spill costs one pointer in the enum, not three words.
    Heap(Box<Vec<T>>),
}

impl<T: Copy, const N: usize> SmallList<T, N> {
    #[must_use]
    pub const fn new() -> Self {
        Self { repr: Repr::Empty }
    }

    #[must_use]
    pub fn as_slice(&self) -> &[T] {
        match &self.repr {
            Repr::Empty => &[],
            Repr::Inline { len, items } => &items[..usize::from(*len)],
            Repr::Heap(v) => v,
        }
    }

    pub fn as_mut_slice(&mut self) -> &mut [T] {
        match &mut self.repr {
            Repr::Empty => &mut [],
            Repr::Inline { len, items } => &mut items[..usize::from(*len)],
            Repr::Heap(v) => v,
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.as_slice().len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Whether the elements live on the heap (spilled past `N`).
    #[must_use]
    pub const fn spilled(&self) -> bool {
        matches!(self.repr, Repr::Heap(_))
    }

    pub fn push(&mut self, x: T) {
        match &mut self.repr {
            Repr::Empty if N > 0 => self.repr = Repr::Inline { len: 1, items: [x; N] },
            Repr::Empty => self.repr = Repr::Heap(Box::new(vec![x])),
            Repr::Inline { len, items } => {
                if usize::from(*len) < N {
                    items[usize::from(*len)] = x;
                    *len += 1;
                } else {
                    let mut v = Vec::with_capacity(N * 2);
                    v.extend_from_slice(&items[..]);
                    v.push(x);
                    self.repr = Repr::Heap(Box::new(v));
                }
            }
            Repr::Heap(v) => v.push(x),
        }
    }

    pub fn pop(&mut self) -> Option<T> {
        match &mut self.repr {
            Repr::Empty => None,
            Repr::Inline { len, items } => {
                *len -= 1;
                let x = items[usize::from(*len)];
                if *len == 0 {
                    self.repr = Repr::Empty;
                }
                Some(x)
            }
            Repr::Heap(v) => v.pop(),
        }
    }

    /// Insert at `index`, shifting later elements right.
    pub fn insert(&mut self, index: usize, x: T) {
        debug_assert!(index <= self.len());
        self.push(x);
        self.as_mut_slice()[index..].rotate_right(1);
    }

    /// Remove at `index`, shifting later elements left (order preserved).
    pub fn remove(&mut self, index: usize) -> T {
        debug_assert!(index < self.len());
        self.as_mut_slice()[index..].rotate_left(1);
        self.pop().expect("a non-empty list")
    }

    pub fn clear(&mut self) {
        self.repr = Repr::Empty;
    }

    pub fn truncate(&mut self, len: usize) {
        while self.len() > len {
            self.pop();
        }
    }

    /// Keep the elements the predicate accepts, in order.
    pub fn retain(&mut self, mut keep: impl FnMut(&T) -> bool) {
        let old: Self = self.clone();
        self.clear();
        for x in old.iter() {
            if keep(x) {
                self.push(*x);
            }
        }
    }

    pub fn extend_from_slice(&mut self, items: &[T]) {
        for x in items {
            self.push(*x);
        }
    }

    #[must_use]
    pub fn from_slice(items: &[T]) -> Self {
        let mut l = Self::new();
        l.extend_from_slice(items);
        l
    }
}

impl<T: Copy, const N: usize> Clone for SmallList<T, N> {
    #[inline]
    fn clone(&self) -> Self {
        Self { repr: self.repr.clone() }
    }
}
impl<T: Copy, const N: usize> Default for SmallList<T, N> {
    fn default() -> Self {
        Self::new()
    }
}
impl<T: Copy, const N: usize> Deref for SmallList<T, N> {
    type Target = [T];
    fn deref(&self) -> &[T] {
        self.as_slice()
    }
}
impl<T: Copy + PartialEq, const N: usize> PartialEq for SmallList<T, N> {
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}
impl<T: Copy + Eq, const N: usize> Eq for SmallList<T, N> {}
impl<T: Copy + Hash, const N: usize> Hash for SmallList<T, N> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_slice().hash(state);
    }
}
impl<T: Copy + fmt::Debug, const N: usize> fmt::Debug for SmallList<T, N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.as_slice()).finish()
    }
}
impl<T: Copy, const N: usize> FromIterator<T> for SmallList<T, N> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut l = Self::new();
        for x in iter {
            l.push(x);
        }
        l
    }
}
impl<T: Copy, const N: usize> Extend<T> for SmallList<T, N> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for x in iter {
            self.push(x);
        }
    }
}
impl<'a, T: Copy, const N: usize> IntoIterator for &'a SmallList<T, N> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.as_slice().iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_insertion_order_across_the_spill() {
        let mut l: SmallList<u8, 3> = SmallList::new();
        for x in [9, 1, 5] {
            l.push(x);
        }
        assert!(!l.spilled());
        l.push(7);
        assert!(l.spilled());
        assert_eq!(&*l, &[9, 1, 5, 7]);
        l.insert(1, 2);
        assert_eq!(&*l, &[9, 2, 1, 5, 7]);
        assert_eq!(l.remove(0), 9);
        assert_eq!(&*l, &[2, 1, 5, 7]);
        l.retain(|x| *x != 1);
        assert_eq!(&*l, &[2, 5, 7]);
        let same: SmallList<u8, 3> = [2u8, 5, 7].into_iter().collect();
        assert_eq!(l, same);
    }

    #[test]
    fn inline_clone_copies_in_place() {
        let l: SmallList<u16, 12> = (0..12).collect();
        assert!(!l.spilled());
        let c = l.clone();
        assert_eq!(l, c);
        let mut e: SmallList<u16, 4> = SmallList::new();
        assert_eq!(e.pop(), None);
        e.push(1);
        assert_eq!(e.pop(), Some(1));
        assert!(e.is_empty());
    }
}
