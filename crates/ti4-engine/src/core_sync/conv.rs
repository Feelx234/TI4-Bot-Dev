//! Small conversions shared by the codecs.

use ti4_engine_core::containers::{Opt32, SmallList};
use ti4_engine_core::generated::ExploreType;
use ti4_engine_core::state::Slice;

use super::error::{R, bad};
use crate::generated::IdStr;

/// The typed id of a corpus id text.
pub fn id<T: IdStr>(slice: Slice, what: &'static str, text: &str) -> R<T> {
    T::from_id(text).ok_or_else(|| bad(slice, what, text))
}

/// The corpus text of a typed id.
pub fn txt<T: IdStr>(t: T) -> &'static str {
    t.as_str()
}

/// A typed `Option<u32>` stamp; `u32::MAX` is not representable.
pub fn opt32(slice: Slice, what: &'static str, v: Option<u32>) -> R<Opt32> {
    match v {
        Some(u32::MAX) => Err(bad(slice, what, u32::MAX)),
        v => Ok(Opt32::new(v)),
    }
}

/// A narrowing conversion that rejects an out-of-range value instead of wrapping.
pub fn narrow<A: Copy + std::fmt::Display, B: TryFrom<A>>(slice: Slice, what: &'static str, v: A) -> R<B> {
    B::try_from(v).map_err(|_| bad(slice, what, v))
}

/// Collect typed ids into a small list.
pub fn list<T: IdStr + Copy, const N: usize, S: AsRef<str>>(
    slice: Slice,
    what: &'static str,
    texts: impl IntoIterator<Item = S>,
) -> R<SmallList<T, N>> {
    let mut out = SmallList::new();
    for t in texts {
        out.push(id::<T>(slice, what, t.as_ref())?);
    }
    Ok(out)
}

/// The exploration trait of a deck / log key (`"CULTURAL"`, `"FRONTIER"`, ..: the corpus id upper-cased).
/// Any other spelling is unmapped, so the key text is reproduced exactly on store.
pub fn explore_type(slice: Slice, text: &str) -> R<ExploreType> {
    try_explore_type(text).ok_or_else(|| bad(slice, "exploration trait", text))
}

/// [`explore_type`] for a caller that is asking whether a text names an exploration deck, not reading one off a state: the answer "no" is its
/// own and is NOT a refusal of the codecs, so it is not counted (and does not panic in strict mode). The Crown of Emphidia explores into the
/// deck named by a planet's first PLANET TYPE (both engines, `docs/ENGINE_KNOWN_ISSUES.md` B5-copy-04), which for a home planet is `FACTION`
/// and for Mecatol Rex `MR`: those texts are no deck, and a string flow that asks for one draws nothing.
#[must_use]
pub fn try_explore_type(text: &str) -> Option<ExploreType> {
    ExploreType::ALL.iter().copied().find(|t| explore_key(*t) == text)
}

/// The state's key of an exploration trait.
#[must_use]
pub fn explore_key(t: ExploreType) -> String {
    t.as_str().to_ascii_uppercase()
}
