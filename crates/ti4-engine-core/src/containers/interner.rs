use std::sync::{Arc, OnceLock};

/// How many texts a table keeps in its private suffix before they are sealed into a new shared base.
const LOCAL_MAX: usize = 64;

/// The sealed part of a table: the texts in index order and the indices sorted by text, so a lookup
/// is a binary search, not a scan.
#[derive(Debug)]
struct Base {
    items: Vec<Arc<[u8]>>,
    sorted: Vec<u32>,
}

impl Base {
    fn of(items: Vec<Arc<[u8]>>) -> Self {
        let mut sorted: Vec<u32> = (0..items.len() as u32).collect();
        sorted.sort_unstable_by(|a, b| items[*a as usize].cmp(&items[*b as usize]));
        Self { items, sorted }
    }

    /// The base that is `older` followed by `added` (the texts are distinct): the sorted index of
    /// `older` merged with the sorted `added`, linear in the size of the table.
    fn grown(older: &Self, mut added: Vec<Arc<[u8]>>) -> Self {
        let first = older.items.len() as u32;
        let mut items = Vec::with_capacity(older.items.len() + added.len());
        items.extend(older.items.iter().cloned());
        items.append(&mut added);
        let mut fresh: Vec<u32> = (first..items.len() as u32).collect();
        fresh.sort_unstable_by(|a, b| items[*a as usize].cmp(&items[*b as usize]));
        let mut sorted = Vec::with_capacity(items.len());
        let (mut i, mut j) = (0, 0);
        while i < older.sorted.len() && j < fresh.len() {
            if items[older.sorted[i] as usize] <= items[fresh[j] as usize] {
                sorted.push(older.sorted[i]);
                i += 1;
            } else {
                sorted.push(fresh[j]);
                j += 1;
            }
        }
        sorted.extend_from_slice(&older.sorted[i..]);
        sorted.extend_from_slice(&fresh[j..]);
        Self { items, sorted }
    }

    fn find(&self, bytes: &[u8]) -> Option<u32> {
        self.sorted.binary_search_by(|i| (*self.items[*i as usize]).cmp(bytes)).ok().map(|at| self.sorted[at])
    }
}

fn empty_base() -> Arc<Base> {
    static EMPTY: OnceLock<Arc<Base>> = OnceLock::new();
    Arc::clone(EMPTY.get_or_init(|| Arc::new(Base::of(Vec::new()))))
}

/// An append-only table of texts (as bytes: the core holds no `str`): the text of index `i` never
/// changes and an index is never reused.
///
/// State-owned, not process-global (docs/todo/D2-prereqs.md D2-3): the table is a sealed, shared
/// **base** (`Arc`) plus a short private **local** suffix. A clone is one refcount bump and a copy of
/// the suffix (empty right after [`Self::compact`], which `intern` also does by itself every 64 new
/// texts); two clones that go on interning diverge, each resolving its own texts and the shared
/// base and nothing of the other's. Equality is by content in index order, never by pointer: equal
/// histories give equal tables.
///
/// The index is the caller's `Code(u16)` / `Atom::Blob(u32)`; the caller names the capacity of its
/// index type in [`Self::intern`].
#[derive(Clone)]
pub struct Interner {
    base: Arc<Base>,
    local: Vec<Arc<[u8]>>,
}

impl Default for Interner {
    fn default() -> Self {
        Self { base: empty_base(), local: Vec::new() }
    }
}

impl std::fmt::Debug for Interner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Interner").field("len", &self.len()).field("local", &self.local.len()).finish()
    }
}

impl PartialEq for Interner {
    fn eq(&self, other: &Self) -> bool {
        if self.len() != other.len() {
            return false;
        }
        if Arc::ptr_eq(&self.base, &other.base) {
            return self.local == other.local;
        }
        (0..self.len() as u32).all(|i| self.get(i) == other.get(i))
    }
}

impl Eq for Interner {}

impl Interner {
    /// A table that already holds `items` (index = position), sealed.
    #[must_use]
    pub fn sealed<'a>(items: impl IntoIterator<Item = &'a [u8]>) -> Self {
        let items: Vec<Arc<[u8]>> = items.into_iter().map(Arc::from).collect();
        Self { base: Arc::new(Base::of(items)), local: Vec::new() }
    }

    /// The index of `bytes`, adding it when new. `None` when it is new and the table already holds
    /// `capacity` texts (the caller's index type is full); nothing is added then.
    pub fn intern(&mut self, bytes: &[u8], capacity: usize) -> Option<u32> {
        if let Some(i) = self.find(bytes) {
            return Some(i);
        }
        if self.len() >= capacity {
            return None;
        }
        if self.local.len() >= LOCAL_MAX {
            self.compact();
        }
        let i = self.len() as u32;
        self.local.push(Arc::from(bytes));
        Some(i)
    }

    /// The index of `bytes` if the table holds it.
    #[must_use]
    pub fn find(&self, bytes: &[u8]) -> Option<u32> {
        if let Some(i) = self.base.find(bytes) {
            return Some(i);
        }
        let at = self.local.iter().position(|t| **t == *bytes)?;
        Some((self.base.items.len() + at) as u32)
    }

    /// The text of an index: the base first, then the suffix.
    #[must_use]
    pub fn get(&self, i: u32) -> Option<&[u8]> {
        let i = i as usize;
        match self.base.items.get(i) {
            Some(t) => Some(t),
            None => self.local.get(i - self.base.items.len()).map(|t| &**t),
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.base.items.len() + self.local.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Seal the suffix into a new base (indices are kept). Cheap to skip, cheap to repeat; call it at a
    /// step boundary so the next clone copies nothing.
    pub fn compact(&mut self) {
        if self.local.is_empty() {
            return;
        }
        self.base = Arc::new(Base::grown(&self.base, std::mem::take(&mut self.local)));
    }

    /// How many texts are in the private suffix (what a clone has to copy).
    #[must_use]
    pub fn local_len(&self) -> usize {
        self.local.len()
    }

    /// Whether both tables share the same sealed base (a clone that did not compact since).
    #[must_use]
    pub fn shares_base_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.base, &other.base)
    }

    /// Whether `earlier` is a prefix of this table: every text of `earlier` has the same index here.
    /// What a table must satisfy to continue the history of `earlier`.
    #[must_use]
    pub fn extends(&self, earlier: &Self) -> bool {
        earlier.len() <= self.len() && (0..earlier.len() as u32).all(|i| self.get(i) == earlier.get(i))
    }

    /// Every text in index order.
    pub fn iter(&self) -> impl Iterator<Item = &[u8]> + '_ {
        self.base.items.iter().chain(self.local.iter()).map(|t| &**t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CAP: usize = 1 << 20;

    #[test]
    fn interning_is_idempotent_and_indices_are_dense() {
        let mut t = Interner::default();
        assert_eq!(t.intern(b"a", CAP), Some(0));
        assert_eq!(t.intern(b"b", CAP), Some(1));
        assert_eq!(t.intern(b"a", CAP), Some(0));
        assert_eq!(t.intern(b"", CAP), Some(2));
        assert_eq!(t.get(1), Some(&b"b"[..]));
        assert_eq!(t.get(2), Some(&b""[..]));
        assert_eq!(t.get(3), None);
        assert_eq!(t.len(), 3);
    }

    #[test]
    fn a_full_table_refuses_new_texts_and_keeps_old_ones() {
        let mut t = Interner::default();
        assert_eq!(t.intern(b"a", 2), Some(0));
        assert_eq!(t.intern(b"b", 2), Some(1));
        assert_eq!(t.intern(b"c", 2), None);
        assert_eq!(t.len(), 2);
        assert_eq!(t.intern(b"b", 2), Some(1));
    }

    #[test]
    fn compact_keeps_every_index_and_empties_the_suffix() {
        let mut t = Interner::default();
        for i in 0..10u32 {
            t.intern(format!("t{i}").as_bytes(), CAP);
        }
        assert_eq!(t.local_len(), 10);
        let before: Vec<Vec<u8>> = t.iter().map(<[u8]>::to_vec).collect();
        t.compact();
        assert_eq!(t.local_len(), 0);
        assert_eq!(t.iter().map(<[u8]>::to_vec).collect::<Vec<_>>(), before);
        assert_eq!(t.find(b"t7"), Some(7));
        assert_eq!(t.intern(b"t3", CAP), Some(3));
        assert_eq!(t.intern(b"new", CAP), Some(10));
    }

    #[test]
    fn the_suffix_is_sealed_by_itself_so_it_stays_short() {
        let mut t = Interner::default();
        for i in 0..1000u32 {
            assert_eq!(t.intern(format!("text {i}").as_bytes(), CAP), Some(i));
            assert!(t.local_len() <= LOCAL_MAX);
        }
        assert_eq!(t.len(), 1000);
        assert_eq!(t.find(b"text 999"), Some(999));
        assert_eq!(t.find(b"text 0"), Some(0));
        assert_eq!(t.find(b"text 1000"), None);
    }

    #[test]
    fn a_clone_shares_the_base_and_diverges_on_its_own_suffix() {
        let mut a = Interner::default();
        a.intern(b"shared", CAP);
        a.compact();
        let mut b = a.clone();
        assert!(a.shares_base_with(&b));
        assert_eq!(a.intern(b"only a", CAP), Some(1));
        assert_eq!(b.intern(b"only b", CAP), Some(1));
        // the same index names different texts in the two lineages, and each resolves its own
        assert_eq!(a.get(1), Some(&b"only a"[..]));
        assert_eq!(b.get(1), Some(&b"only b"[..]));
        assert_eq!(a.get(0), b.get(0));
        assert_eq!(a.find(b"only b"), None);
        assert_eq!(b.find(b"only a"), None);
        assert_ne!(a, b);
    }

    #[test]
    fn equal_histories_are_equal_whatever_the_sealing() {
        let mut a = Interner::default();
        let mut b = Interner::default();
        for t in ["x", "y", "z"] {
            a.intern(t.as_bytes(), CAP);
            b.intern(t.as_bytes(), CAP);
        }
        b.compact();
        assert_eq!(a, b);
        a.intern(b"w", CAP);
        assert_ne!(a, b);
        b.intern(b"w", CAP);
        assert_eq!(a, b);
        let mut c = Interner::default();
        for t in ["y", "x", "z", "w"] {
            c.intern(t.as_bytes(), CAP);
        }
        assert_ne!(a, c, "the same texts in another order are another history");
    }

    #[test]
    fn extends_is_the_prefix_relation() {
        let mut a = Interner::default();
        a.intern(b"1", CAP);
        let early = a.clone();
        a.intern(b"2", CAP);
        assert!(a.extends(&early));
        assert!(!early.extends(&a));
        let mut other = Interner::default();
        other.intern(b"x", CAP);
        assert!(!a.extends(&other));
    }

    #[test]
    fn a_sealed_table_resolves_and_interns_like_a_grown_one() {
        let sealed = Interner::sealed([&b"b"[..], b"a", b"c"]);
        assert_eq!(sealed.find(b"a"), Some(1));
        assert_eq!(sealed.find(b"c"), Some(2));
        let mut grown = Interner::default();
        for t in ["b", "a", "c"] {
            grown.intern(t.as_bytes(), CAP);
        }
        assert_eq!(sealed, grown);
    }

    #[test]
    fn growing_a_sealed_base_equals_sealing_it_whole() {
        let texts: Vec<String> = (0..200u32).map(|i| format!("{:x}", i.wrapping_mul(2_654_435_761))).collect();
        let mut grown = Interner::default();
        for (n, t) in texts.iter().enumerate() {
            assert_eq!(grown.intern(t.as_bytes(), CAP), Some(n as u32));
            if n % 7 == 0 {
                grown.compact();
            }
        }
        let whole = Interner::sealed(texts.iter().map(|t| t.as_bytes()));
        assert_eq!(grown, whole);
        for (n, t) in texts.iter().enumerate() {
            assert_eq!(grown.find(t.as_bytes()), Some(n as u32));
            assert_eq!(whole.find(t.as_bytes()), Some(n as u32));
        }
    }

    #[test]
    fn the_default_table_allocates_no_base() {
        let a = Interner::default();
        let b = Interner::default();
        assert!(a.shares_base_with(&b));
        assert!(a.is_empty());
    }
}
