use std::sync::Arc;

use super::cow::Cow;

/// An append-only log. A clone shares every sealed chunk and the tail chunk (two refcount bumps);
/// the first append after a clone copies the tail chunk only (at most `CHUNK` records), which is
/// the one small allocation the checkpoint cost model allows.
pub struct Log<T: Clone, const CHUNK: usize = 32> {
    /// Sealed full chunks, newest first.
    sealed: Option<Arc<Node<T>>>,
    /// The open chunk (fewer than `CHUNK` records).
    tail: Option<Cow<Vec<T>>>,
    len: u32,
}

struct Node<T> {
    items: Arc<Vec<T>>,
    prev: Option<Arc<Node<T>>>,
}

impl<T> Drop for Node<T> {
    fn drop(&mut self) {
        // Unlink iteratively: a long chain must not recurse on drop.
        let mut next = self.prev.take();
        while let Some(arc) = next {
            match Arc::try_unwrap(arc) {
                Ok(mut node) => next = node.prev.take(),
                Err(_) => break,
            }
        }
    }
}

impl<T: Clone, const CHUNK: usize> Log<T, CHUNK> {
    #[must_use]
    pub const fn new() -> Self {
        Self { sealed: None, tail: None, len: 0 }
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.len as usize
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn push(&mut self, item: T) {
        let tail = self.tail.get_or_insert_with(|| Cow::new(Vec::with_capacity(CHUNK)));
        tail.edit().push(item);
        self.len += 1;
        if tail.len() == CHUNK {
            let full = self.tail.take().expect("tail present");
            self.sealed = Some(Arc::new(Node { items: full.into_arc(), prev: self.sealed.take() }));
        }
    }

    fn sealed_count(&self) -> usize {
        self.len() / CHUNK
    }

    fn tail_slice(&self) -> &[T] {
        self.tail.as_ref().map_or(&[], |t| t.as_slice())
    }

    #[must_use]
    pub fn get(&self, index: usize) -> Option<&T> {
        if index >= self.len() {
            return None;
        }
        let chunk = index / CHUNK;
        if chunk >= self.sealed_count() {
            return self.tail_slice().get(index % CHUNK);
        }
        let mut steps = self.sealed_count() - 1 - chunk;
        let mut node = self.sealed.as_ref()?;
        while steps > 0 {
            node = node.prev.as_ref()?;
            steps -= 1;
        }
        node.items.get(index % CHUNK)
    }

    #[must_use]
    pub fn last(&self) -> Option<&T> {
        self.len().checked_sub(1).and_then(|i| self.get(i))
    }

    /// Records in append order.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        let mut chunks: Vec<&[T]> = Vec::with_capacity(self.sealed_count() + 1);
        let mut node = self.sealed.as_ref();
        while let Some(n) = node {
            chunks.push(n.items.as_slice());
            node = n.prev.as_ref();
        }
        chunks.reverse();
        chunks.push(self.tail_slice());
        chunks.into_iter().flatten()
    }

    #[must_use]
    pub fn to_vec(&self) -> Vec<T> {
        self.iter().cloned().collect()
    }

    /// Whether both logs share their sealed prefix (a clone nobody has appended to far enough to
    /// seal a new chunk).
    #[must_use]
    pub fn shares_prefix_with(&self, other: &Self) -> bool {
        match (&self.sealed, &other.sealed) {
            (None, None) => true,
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }

    /// Whether the open chunk is the same allocation in both logs.
    #[must_use]
    pub fn shares_tail_with(&self, other: &Self) -> bool {
        match (&self.tail, &other.tail) {
            (None, None) => true,
            (Some(a), Some(b)) => Cow::ptr_eq(a, b),
            _ => false,
        }
    }
}

impl<T: Clone, const CHUNK: usize> Clone for Log<T, CHUNK> {
    #[inline]
    fn clone(&self) -> Self {
        Self { sealed: self.sealed.clone(), tail: self.tail.clone(), len: self.len }
    }
}
impl<T: Clone, const CHUNK: usize> Default for Log<T, CHUNK> {
    fn default() -> Self {
        Self::new()
    }
}
impl<T: Clone + PartialEq, const CHUNK: usize> PartialEq for Log<T, CHUNK> {
    fn eq(&self, other: &Self) -> bool {
        self.len == other.len && self.iter().zip(other.iter()).all(|(a, b)| a == b)
    }
}
impl<T: Clone + Eq, const CHUNK: usize> Eq for Log<T, CHUNK> {}
impl<T: Clone + std::fmt::Debug, const CHUNK: usize> std::fmt::Debug for Log<T, CHUNK> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}
impl<T: Clone, const CHUNK: usize> Extend<T> for Log<T, CHUNK> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for x in iter {
            self.push(x);
        }
    }
}
impl<T: Clone, const CHUNK: usize> FromIterator<T> for Log<T, CHUNK> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut l = Self::new();
        l.extend(iter);
        l
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_order_and_random_access() {
        let mut l: Log<u32, 4> = Log::new();
        for i in 0..11 {
            l.push(i);
        }
        assert_eq!(l.len(), 11);
        assert_eq!(l.to_vec(), (0..11).collect::<Vec<_>>());
        for i in 0..11 {
            assert_eq!(l.get(i), Some(&(i as u32)));
        }
        assert_eq!(l.get(11), None);
        assert_eq!(l.last(), Some(&10));
    }

    #[test]
    fn clone_shares_the_prefix_and_the_tail_until_appended() {
        let mut a: Log<u32, 4> = (0..10).collect();
        let b = a.clone();
        assert!(a.shares_prefix_with(&b) && a.shares_tail_with(&b));
        a.push(10);
        // The tail was copied for `a`, the sealed prefix is untouched.
        assert!(a.shares_prefix_with(&b));
        assert!(!a.shares_tail_with(&b));
        assert_eq!(b.to_vec(), (0..10).collect::<Vec<_>>());
        assert_eq!(a.to_vec(), (0..11).collect::<Vec<_>>());
        a.push(11);
        a.push(12);
        // A new chunk is sealed on `a`; `b` keeps the old head, the older chunks stay shared.
        assert!(!a.shares_prefix_with(&b));
        assert_eq!(b.len(), 10);
        assert_eq!(a.get(3), b.get(3));
    }

    #[test]
    fn a_very_long_chain_drops_without_recursion() {
        let mut l: Log<u8, 2> = Log::new();
        for _ in 0..400_000 {
            l.push(1);
        }
        drop(l);
    }
}
