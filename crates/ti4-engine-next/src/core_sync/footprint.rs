use ti4_engine_core::state::{Slice, SliceSet};

/// What a ported entry point reads and writes, declared per call site as a `const`.
/// A lend loads `reads | writes` (a written slice is loaded too, so the core sees it whole) and
/// stores back `writes` only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Footprint {
    pub reads: SliceSet,
    pub writes: SliceSet,
}

impl Footprint {
    #[must_use]
    pub const fn new(reads: &[Slice], writes: &[Slice]) -> Self {
        Self { reads: SliceSet::of(reads), writes: SliceSet::of(writes) }
    }

    /// Read-only.
    #[must_use]
    pub const fn reads(reads: &[Slice]) -> Self {
        Self::new(reads, &[])
    }

    /// Every slice read and written.
    pub const ALL: Self = Self { reads: SliceSet::ALL, writes: SliceSet::ALL };

    /// The slices a lend has to load.
    #[must_use]
    pub const fn loads(self) -> SliceSet {
        self.reads.union(self.writes)
    }

    /// The union of two footprints (a region over several calls).
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self { reads: self.reads.union(other.reads), writes: self.writes.union(other.writes) }
    }
}
