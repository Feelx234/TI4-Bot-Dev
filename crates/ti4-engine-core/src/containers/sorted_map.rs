use std::fmt;

use super::small_list::SmallList;

/// A map over a sparse key set as a sorted small vec: iteration in key order (the order of the
/// `BTreeMap` it replaces when the key's `Ord` is the text order of the id), lookup by binary
/// search, inline up to `N` entries, `Copy` values only (no allocation on clone).
pub struct SortedMap<K: Ord + Copy, V: Copy, const N: usize> {
    entries: SmallList<(K, V), N>,
}

impl<K: Ord + Copy, V: Copy, const N: usize> SortedMap<K, V, N> {
    #[must_use]
    pub const fn new() -> Self {
        Self { entries: SmallList::new() }
    }

    fn find(&self, key: &K) -> Result<usize, usize> {
        self.entries.as_slice().binary_search_by(|(k, _)| k.cmp(key))
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[must_use]
    pub fn get(&self, key: &K) -> Option<&V> {
        self.find(key).ok().map(|i| &self.entries.as_slice()[i].1)
    }

    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        match self.find(key) {
            Ok(i) => Some(&mut self.entries.as_mut_slice()[i].1),
            Err(_) => None,
        }
    }

    #[must_use]
    pub fn contains_key(&self, key: &K) -> bool {
        self.find(key).is_ok()
    }

    /// Insert; the previous value if the key was present (as `BTreeMap::insert`).
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        match self.find(&key) {
            Ok(i) => Some(std::mem::replace(&mut self.entries.as_mut_slice()[i].1, value)),
            Err(i) => {
                self.entries.insert(i, (key, value));
                None
            }
        }
    }

    pub fn remove(&mut self, key: &K) -> Option<V> {
        match self.find(key) {
            Ok(i) => Some(self.entries.remove(i).1),
            Err(_) => None,
        }
    }

    /// The value for `key`, inserting `make()` first when absent (as `entry().or_insert_with`).
    pub fn get_or_insert_with(&mut self, key: K, make: impl FnOnce() -> V) -> &mut V {
        let i = match self.find(&key) {
            Ok(i) => i,
            Err(i) => {
                self.entries.insert(i, (key, make()));
                i
            }
        };
        &mut self.entries.as_mut_slice()[i].1
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn retain(&mut self, mut keep: impl FnMut(&K, &V) -> bool) {
        self.entries.retain(|(k, v)| keep(k, v));
    }

    /// Entries in key order.
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.entries.as_slice().iter().map(|(k, v)| (k, v))
    }

    pub fn keys(&self) -> impl Iterator<Item = K> + '_ {
        self.entries.as_slice().iter().map(|(k, _)| *k)
    }

    pub fn values(&self) -> impl Iterator<Item = V> + '_ {
        self.entries.as_slice().iter().map(|(_, v)| *v)
    }

    #[must_use]
    pub fn first(&self) -> Option<(K, V)> {
        self.entries.as_slice().first().copied()
    }

    #[must_use]
    pub fn as_slice(&self) -> &[(K, V)] {
        self.entries.as_slice()
    }
}

impl<K: Ord + Copy, V: Copy, const N: usize> Clone for SortedMap<K, V, N> {
    #[inline]
    fn clone(&self) -> Self {
        Self { entries: self.entries.clone() }
    }
}
impl<K: Ord + Copy, V: Copy, const N: usize> Default for SortedMap<K, V, N> {
    fn default() -> Self {
        Self::new()
    }
}
impl<K: Ord + Copy, V: Copy + PartialEq, const N: usize> PartialEq for SortedMap<K, V, N> {
    fn eq(&self, other: &Self) -> bool {
        self.entries == other.entries
    }
}
impl<K: Ord + Copy, V: Copy + Eq, const N: usize> Eq for SortedMap<K, V, N> {}
impl<K: Ord + Copy + fmt::Debug, V: Copy + fmt::Debug, const N: usize> fmt::Debug
    for SortedMap<K, V, N>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}
impl<K: Ord + Copy, V: Copy, const N: usize> FromIterator<(K, V)> for SortedMap<K, V, N> {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        let mut m = Self::new();
        for (k, v) in iter {
            m.insert(k, v);
        }
        m
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::DecimalTextOrd;

    #[test]
    fn behaves_like_a_btreemap() {
        let mut m: SortedMap<u8, i32, 2> = SortedMap::new();
        let mut b = std::collections::BTreeMap::new();
        for (k, v) in [(5u8, 1), (2, 2), (9, 3), (2, 4), (7, 5), (1, 6)] {
            assert_eq!(m.insert(k, v), b.insert(k, v));
        }
        assert_eq!(m.remove(&9), b.remove(&9));
        assert_eq!(m.remove(&100), None);
        *m.get_or_insert_with(3, || 0) += 10;
        *b.entry(3).or_insert(0) += 10;
        assert_eq!(m.iter().map(|(k, v)| (*k, *v)).collect::<Vec<_>>(), b.into_iter().collect::<Vec<_>>());
    }

    #[test]
    fn decimal_text_keys_iterate_in_text_order() {
        let mut m: SortedMap<DecimalTextOrd, u8, 4> = SortedMap::new();
        for n in [9u32, 10, 2, 100, 11] {
            m.insert(DecimalTextOrd(n), 0);
        }
        let got: Vec<u32> = m.keys().map(|k| k.0).collect();
        assert_eq!(got, vec![10, 100, 11, 2, 9]);
    }
}
