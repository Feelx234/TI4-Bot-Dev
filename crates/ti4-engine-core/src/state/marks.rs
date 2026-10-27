//! `Marks`: faction memory (`GameState.faction_marks`, 367 uses with `format!` keys) as typed keys.
//! The keys and values are package X1's ([`crate::keys::MarkKey`], [`crate::keys::MarkVal`]); per-faction
//! records move into the faction shards (C1-C6) as they are ported.

use crate::containers::Cow;
pub use crate::keys::{Family, MarkKey, MarkVal};

/// The marks in key order, which is the text order of the rendered keys (the `Ord` of [`MarkKey`]).
/// A sorted vector with binary search: a value is a tree ([`MarkVal`]), so it is not `Copy` and does not fit
/// `SortedMap`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MarkMap {
    entries: Vec<(MarkKey, MarkVal)>,
}

impl MarkMap {
    #[must_use]
    pub const fn new() -> Self {
        Self { entries: Vec::new() }
    }

    fn find(&self, key: &MarkKey) -> Result<usize, usize> {
        self.entries.binary_search_by(|(k, _)| k.cmp(key))
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
    pub fn get(&self, key: &MarkKey) -> Option<&MarkVal> {
        self.find(key).ok().map(|i| &self.entries[i].1)
    }

    #[must_use]
    pub fn contains_key(&self, key: &MarkKey) -> bool {
        self.find(key).is_ok()
    }

    /// Insert; the previous value if the key was present (as `BTreeMap::insert`).
    pub fn insert(&mut self, key: MarkKey, value: MarkVal) -> Option<MarkVal> {
        match self.find(&key) {
            Ok(i) => Some(std::mem::replace(&mut self.entries[i].1, value)),
            Err(i) => {
                self.entries.insert(i, (key, value));
                None
            }
        }
    }

    pub fn remove(&mut self, key: &MarkKey) -> Option<MarkVal> {
        self.find(key).ok().map(|i| self.entries.remove(i).1)
    }

    /// In key (text) order.
    pub fn iter(&self) -> impl Iterator<Item = (&MarkKey, &MarkVal)> {
        self.entries.iter().map(|(k, v)| (k, v))
    }
}

impl FromIterator<(MarkKey, MarkVal)> for MarkMap {
    fn from_iter<I: IntoIterator<Item = (MarkKey, MarkVal)>>(iter: I) -> Self {
        let mut map = Self::new();
        for (k, v) in iter {
            map.insert(k, v);
        }
        map
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Marks {
    pub tail: Cow<MarkMap>,
}
