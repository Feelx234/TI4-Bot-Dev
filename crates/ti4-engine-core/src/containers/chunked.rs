use super::cow::Cow;

/// A fixed-length array of `CHUNK * N` cells in `N` copy-on-write chunks. A clone bumps `N`
/// refcounts and copies nothing; an edit copies one chunk (`CHUNK` cells) if it is shared.
#[derive(Debug)]
pub struct Chunked<T: Clone, const CHUNK: usize, const N: usize> {
    chunks: [Cow<[T; CHUNK]>; N],
}

impl<T: Clone, const CHUNK: usize, const N: usize> Chunked<T, CHUNK, N> {
    /// Every cell `fill`; all chunks share one allocation until first written.
    #[must_use]
    pub fn new(fill: &T) -> Self {
        let shared = Cow::new(std::array::from_fn::<T, CHUNK, _>(|_| fill.clone()));
        Self { chunks: std::array::from_fn(|_| shared.clone()) }
    }

    /// Number of cells (`CHUNK * N`).
    pub const CAPACITY: usize = CHUNK * N;

    #[inline]
    #[must_use]
    pub fn get(&self, index: usize) -> &T {
        &self.chunks[index / CHUNK][index % CHUNK]
    }

    /// Mutable access; copies the chunk first if another clone shares it.
    #[inline]
    pub fn edit(&mut self, index: usize) -> &mut T {
        &mut self.chunks[index / CHUNK].edit()[index % CHUNK]
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.chunks.iter().flat_map(|c| c.iter())
    }

    /// Whether the chunk holding `index` is shared with the same chunk of `other`.
    #[must_use]
    pub fn chunk_shared_with(&self, other: &Self, index: usize) -> bool {
        Cow::ptr_eq(&self.chunks[index / CHUNK], &other.chunks[index / CHUNK])
    }

    /// How many chunks are shared with `other`.
    #[must_use]
    pub fn shared_chunks(&self, other: &Self) -> usize {
        self.chunks.iter().zip(other.chunks.iter()).filter(|(a, b)| Cow::ptr_eq(a, b)).count()
    }
}

impl<T: Clone, const CHUNK: usize, const N: usize> Clone for Chunked<T, CHUNK, N> {
    #[inline]
    fn clone(&self) -> Self {
        Self { chunks: self.chunks.clone() }
    }
}

impl<T: Clone + Default, const CHUNK: usize, const N: usize> Default for Chunked<T, CHUNK, N> {
    fn default() -> Self {
        Self::new(&T::default())
    }
}

impl<T: Clone + PartialEq, const CHUNK: usize, const N: usize> PartialEq for Chunked<T, CHUNK, N> {
    fn eq(&self, other: &Self) -> bool {
        self.chunks == other.chunks
    }
}
impl<T: Clone + Eq, const CHUNK: usize, const N: usize> Eq for Chunked<T, CHUNK, N> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clone_shares_every_chunk_and_an_edit_copies_one() {
        let mut a: Chunked<u32, 16, 15> = Chunked::new(&0);
        assert_eq!(Chunked::<u32, 16, 15>::CAPACITY, 240);
        *a.edit(37) = 7;
        let b = a.clone();
        assert_eq!(b.shared_chunks(&a), 15);
        *a.edit(38) = 8;
        assert_eq!(a.shared_chunks(&b), 14);
        assert!(!a.chunk_shared_with(&b, 37));
        assert!(a.chunk_shared_with(&b, 200));
        assert_eq!((*a.get(37), *a.get(38)), (7, 8));
        assert_eq!((*b.get(37), *b.get(38)), (7, 0));
        assert_eq!(a.iter().count(), 240);
    }
}
