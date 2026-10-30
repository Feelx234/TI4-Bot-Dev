//! The text tables a game carries (docs/todo/D2-prereqs.md D2-3): the open texts behind `Code(u16)`
//! and `Atom::Blob(u32)`, and the owner table behind `Seat`. They are part of the state, so a
//! checkpoint (a clone) carries the tables its codes and blobs index into and a code means the same
//! text in every lineage that descends from the state that minted it.
//!
//! Not a slice: always valid, no `loaded` flag, no footprint.

use std::sync::{Arc, OnceLock};

use crate::containers::Interner;
use crate::ids::Seat;

/// A [`super::Code`] indexes `codes`; at most 65 535 texts (index 65 535 is never issued).
pub const CODE_CAPACITY: usize = u16::MAX as usize;
/// An `Atom::Blob` indexes `blobs`.
pub const BLOB_CAPACITY: usize = u32::MAX as usize;

/// The text of `Code(0)`: the diplomacy rules tag. A default `Diplomacy` (code 0) must equal a loaded
/// default `DiplomacyState`, so every table starts with it (a test checks it against `ti4_model`).
pub const DIPLOMACY_RULES_V1: &[u8] = b"diplomacy-rules-v1";

/// The open texts of one game.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Texts {
    /// Behind `Code(u16)`: reason texts, presentation phases, faction-mark keys, promise texts.
    pub codes: Interner,
    /// Behind `Atom::Blob(u32)`: values whose structure belongs to a later package.
    pub blobs: Interner,
}

impl Default for Texts {
    fn default() -> Self {
        static CODES: OnceLock<Interner> = OnceLock::new();
        let codes = CODES.get_or_init(|| Interner::sealed([DIPLOMACY_RULES_V1])).clone();
        Self { codes, blobs: Interner::default() }
    }
}

impl Texts {
    /// Seal both suffixes (a step boundary: the next clone copies nothing).
    pub fn compact(&mut self) {
        self.codes.compact();
        self.blobs.compact();
    }
}

/// The owner ids of one game in text order, as bytes: seat `i` is the `i`-th (docs/ENGINE_CORE_STATE.md
/// 1.3). Set once when the state is loaded and never changed: players are fixed for a game. A clone
/// shares the ids.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnerTable {
    ids: Arc<Vec<Box<[u8]>>>,
}

impl Default for OwnerTable {
    fn default() -> Self {
        static EMPTY: OnceLock<OwnerTable> = OnceLock::new();
        EMPTY.get_or_init(|| Self { ids: Arc::new(Vec::new()) }).clone()
    }
}

impl OwnerTable {
    /// The table of these ids, which the caller gives in text order, without duplicates.
    #[must_use]
    pub fn new<'a>(ids: impl IntoIterator<Item = &'a [u8]>) -> Self {
        Self { ids: Arc::new(ids.into_iter().map(Box::from).collect()) }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.ids.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    /// The id of a seat.
    #[must_use]
    pub fn id(&self, seat: Seat) -> Option<&[u8]> {
        self.ids.get(usize::from(seat.0)).map(|b| &**b)
    }

    /// The seat of an id.
    #[must_use]
    pub fn seat(&self, id: &[u8]) -> Option<Seat> {
        self.ids.binary_search_by(|probe| (**probe).cmp(id)).ok().map(|i| Seat(i as u8))
    }

    /// The ids in seat order.
    pub fn iter(&self) -> impl Iterator<Item = &[u8]> + '_ {
        self.ids.iter().map(|b| &**b)
    }

    /// Whether both tables share the same ids (a clone).
    #[must_use]
    pub fn shares_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.ids, &other.ids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_default_table_holds_the_diplomacy_tag_at_code_zero() {
        let t = Texts::default();
        assert_eq!(t.codes.get(0), Some(DIPLOMACY_RULES_V1));
        assert_eq!(t.codes.len(), 1);
        assert!(t.blobs.is_empty());
        assert!(t.codes.shares_base_with(&Texts::default().codes));
    }

    #[test]
    fn compact_seals_both_suffixes() {
        let mut t = Texts::default();
        t.codes.intern(b"c", CODE_CAPACITY);
        t.blobs.intern(b"b", BLOB_CAPACITY);
        assert_eq!((t.codes.local_len(), t.blobs.local_len()), (1, 1));
        t.compact();
        assert_eq!((t.codes.local_len(), t.blobs.local_len()), (0, 0));
        assert_eq!((t.codes.find(b"c"), t.blobs.find(b"b")), (Some(1), Some(0)));
    }

    #[test]
    fn the_diplomacy_tag_is_the_models() {
        assert_eq!(DIPLOMACY_RULES_V1, ti4_model::diplomacy::DIPLOMACY_RULES_V1.as_bytes());
    }

    #[test]
    fn owners_resolve_both_ways_in_text_order() {
        let t = OwnerTable::new([&b"a"[..], b"b", b"c"]);
        assert_eq!(t.seat(b"b"), Some(Seat(1)));
        assert_eq!(t.id(Seat(2)), Some(&b"c"[..]));
        assert_eq!(t.seat(b"d"), None);
        assert_eq!(t.id(Seat(3)), None);
        assert_eq!(t.len(), 3);
        assert!(OwnerTable::default().is_empty());
        assert!(!t.is_empty());
        assert!(t.shares_with(&t.clone()));
        assert!(!t.shares_with(&OwnerTable::new([&b"a"[..], b"b", b"c"])), "equal ids, not the same table");
        assert_eq!(t, OwnerTable::new([&b"a"[..], b"b", b"c"]));
    }
}
