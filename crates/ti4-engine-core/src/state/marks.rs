//! `Marks`: faction memory (`GameState.faction_marks`, 367 uses with `format!` keys) as typed keys.
//! Key families and value meanings are pinned by package X1 (`MarkFamily` numbers are assigned
//! there); per-faction records move into the faction shards (C1-C6) as they are ported.

use crate::containers::{Cow, DecimalTextOrd, SortedMap};

/// A key family (one per `format!` pattern of the corpus). Numbered by X1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MarkFamily(pub u16);

/// A mark key: the family and up to two arguments (seats, ids by index, or sequence numbers).
/// `seq` iterates in decimal *text* order, as the string keys do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MarkKey {
    pub family: MarkFamily,
    pub a: u16,
    pub b: u16,
    pub seq: DecimalTextOrd,
}

/// A mark value; its meaning (flag, count, seat, id index) is per family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MarkVal(pub i32);

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Marks {
    pub tail: Cow<SortedMap<MarkKey, MarkVal, 32>>,
}
