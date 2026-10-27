use std::ops::Deref;
use std::sync::Arc;

/// A shared, copy-on-write cell: `Arc<T>` with `make_mut` as the only way to edit. Cloning bumps a
/// refcount; the first edit of a shared cell copies the value (the checkpoint cost model).
#[derive(Debug)]
pub struct Cow<T: Clone>(Arc<T>);

impl<T: Clone> Cow<T> {
    #[must_use]
    pub fn new(value: T) -> Self {
        Self(Arc::new(value))
    }

    /// Mutable access; copies the value first if another clone shares it.
    #[inline]
    pub fn edit(&mut self) -> &mut T {
        Arc::make_mut(&mut self.0)
    }

    /// Whether both cells share one allocation (a clone nobody has edited yet).
    #[must_use]
    pub fn ptr_eq(a: &Self, b: &Self) -> bool {
        Arc::ptr_eq(&a.0, &b.0)
    }

    /// The underlying `Arc` (containers that seal a cell into a shared chain).
    #[must_use]
    pub fn into_arc(self) -> Arc<T> {
        self.0
    }

    /// Number of cells sharing this value.
    #[must_use]
    pub fn share_count(&self) -> usize {
        Arc::strong_count(&self.0)
    }
}

impl<T: Clone> Clone for Cow<T> {
    #[inline]
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl<T: Clone> Deref for Cow<T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T: Clone + Default> Default for Cow<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T: Clone + PartialEq> PartialEq for Cow<T> {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0) || *self.0 == *other.0
    }
}

impl<T: Clone + Eq> Eq for Cow<T> {}

impl<T: Clone> From<T> for Cow<T> {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clone_shares_and_edit_splits() {
        let mut a = Cow::new(vec![1, 2, 3]);
        let b = a.clone();
        assert!(Cow::ptr_eq(&a, &b));
        assert_eq!(a.share_count(), 2);
        a.edit().push(4);
        assert!(!Cow::ptr_eq(&a, &b));
        assert_eq!(*b, vec![1, 2, 3]);
        assert_eq!(*a, vec![1, 2, 3, 4]);
        // An unshared cell is edited in place.
        let before = Arc::as_ptr(&a.0);
        a.edit().push(5);
        assert_eq!(before, Arc::as_ptr(&a.0));
    }
}
