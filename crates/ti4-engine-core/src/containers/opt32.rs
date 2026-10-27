use std::num::NonZeroU32;

/// `Option<u32>` in 4 bytes (round and activation stamps): the value `v` is stored as `v + 1`, so
/// every `u32` but `u32::MAX` is representable (`u32::MAX` is never a round).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Opt32(Option<NonZeroU32>);

impl Opt32 {
    pub const NONE: Self = Self(None);

    #[inline]
    #[must_use]
    pub fn some(value: u32) -> Self {
        debug_assert!(value != u32::MAX);
        Self(NonZeroU32::new(value.wrapping_add(1)))
    }

    #[inline]
    #[must_use]
    pub fn new(value: Option<u32>) -> Self {
        value.map_or(Self::NONE, Self::some)
    }

    #[inline]
    #[must_use]
    pub fn get(self) -> Option<u32> {
        self.0.map(|n| n.get() - 1)
    }

    #[inline]
    #[must_use]
    pub const fn is_some(self) -> bool {
        self.0.is_some()
    }

    #[inline]
    pub fn set(&mut self, value: Option<u32>) {
        *self = Self::new(value);
    }

    #[inline]
    pub fn take(&mut self) -> Option<u32> {
        let old = self.get();
        *self = Self::NONE;
        old
    }

    /// Whether the stamp equals `value`.
    #[inline]
    #[must_use]
    pub fn is(self, value: u32) -> bool {
        self.get() == Some(value)
    }
}

impl From<Option<u32>> for Opt32 {
    fn from(value: Option<u32>) -> Self {
        Self::new(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_bytes_and_zero_is_a_value() {
        assert_eq!(std::mem::size_of::<Opt32>(), 4);
        assert_eq!(Opt32::some(0).get(), Some(0));
        assert_eq!(Opt32::NONE.get(), None);
        assert_eq!(Opt32::some(7).get(), Some(7));
        let mut o = Opt32::some(3);
        assert_eq!(o.take(), Some(3));
        assert!(!o.is_some());
    }
}
