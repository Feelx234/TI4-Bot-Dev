//! [`Streams`]: the game seed and one ChaCha8 stream per [`Domain`], and the draws made from them.
//!
//! This is the core's `state::Rng` (and the thin crate's `GameRng`) moved down, with the seeding folded in: a stream
//! is created on first use from `SHA-256(seed_le || domain text)`, so no `SeedSource` is needed. Every draw goes
//! through the same `rand` calls with the same integer types as today (`random_range(1..=sides)` on a `u32`,
//! `random_range(0..=i)` on a `usize` for a shuffle). `rand` 0.9 happens to widen a `u8` range to `u32` (a `u8`
//! sampler was tried and drew the same faces), but the identity should not rest on that internal detail, so the
//! types stay today's.

use std::sync::Arc;

use rand::Rng as _;
use rand_chacha::ChaCha8Rng;
use rand_core::SeedableRng;

use crate::request::{Faces, Kind, MAX_FACES, Request};
use crate::{Domain, derive_seed};

/// One stream and where it stands.
#[cfg_attr(not(target_family = "wasm"), derive(Debug))]
#[derive(Clone, PartialEq, Eq)]
struct Stream {
    domain: Domain,
    rng: ChaCha8Rng,
}

/// The randomness of a game: the seed and the streams drawn from so far (a stream never drawn from is absent and
/// stands at 0). A clone shares the streams; the first draw after a clone copies the stream table once (a stream is
/// 136 bytes, and a checkpoint of the core must stay a refcount bump, as with the core's `Cow`).
#[derive(Clone, PartialEq, Eq, Default)]
pub struct Streams {
    seed: u64,
    /// Sorted by domain.
    streams: Arc<Vec<Stream>>,
}

/// The text of the core's old `state::Rng` (`Rng { seed: .., streams: Cow([..]) }`), which these streams replaced in lane C2: a hash
/// over a position's `Debug` text (perf_kernel's state hash, the `775f4136e60a99d5` pin) does not move with the type.
#[cfg(not(target_family = "wasm"))]
impl core::fmt::Debug for Streams {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        struct Cowed<'a>(&'a [Stream]);
        impl core::fmt::Debug for Cowed<'_> {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.debug_tuple("Cow").field(&self.0).finish()
            }
        }
        f.debug_struct("Rng").field("seed", &self.seed).field("streams", &Cowed(&self.streams)).finish()
    }
}

/// The browser build prints the streams as their seed only (no generator state in the wasm file; lane C3, the size budget).
#[cfg(target_family = "wasm")]
impl core::fmt::Debug for Streams {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Rng").field("seed", &self.seed).finish_non_exhaustive()
    }
}

/// Why a request or a draw cannot be answered. Every case is a defect of the asking site, not a game event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    /// More than [`MAX_FACES`] dice in one request (split it with [`Request::dice_groups`]).
    TooMany(u8),
    /// A die with no sides, or a choice among no elements.
    NoSides,
}

impl core::fmt::Display for Refused {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::TooMany(n) => write!(f, "{n} dice in one request (at most {MAX_FACES})"),
            Self::NoSides => f.write_str("a die with no sides"),
        }
    }
}

impl Streams {
    /// Every stream at its start.
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self { seed, streams: Arc::default() }
    }

    #[must_use]
    pub const fn seed(&self) -> u64 {
        self.seed
    }

    fn stream_mut(&mut self, domain: Domain) -> &mut ChaCha8Rng {
        let seed = self.seed;
        let streams = Arc::make_mut(&mut self.streams);
        let at = match streams.binary_search_by(|s| s.domain.cmp(&domain)) {
            Ok(at) => at,
            Err(at) => {
                streams.insert(at, Stream { domain, rng: ChaCha8Rng::from_seed(derive_seed(seed, domain.text())) });
                at
            }
        };
        &mut streams[at].rng
    }

    /// A die with `sides` sides, `1..=sides`: today's `Rng::die` / `GameRng::die`.
    ///
    /// # Errors
    /// `sides == 0`.
    pub fn try_die(&mut self, domain: Domain, sides: u32) -> Result<u32, Refused> {
        if sides == 0 {
            return Err(Refused::NoSides);
        }
        Ok(self.stream_mut(domain).random_range(1..=sides))
    }

    /// [`Self::try_die`].
    ///
    /// # Panics
    /// When it refuses (a defect of the caller).
    pub fn die(&mut self, domain: Domain, sides: u32) -> u32 {
        self.try_die(domain, sides).unwrap_or_else(|_| panic!("ti4-chance: a die with no sides"))
    }

    /// Fisher-Yates from the back on one domain's stream: today's `Rng::shuffle` / `GameRng::shuffle`, the deck
    /// permutations of the setup (decks are permuted once and drawn from the top; there is no in-play shuffle).
    pub fn shuffle<T>(&mut self, domain: Domain, items: &mut [T]) {
        let rng = self.stream_mut(domain);
        for i in (1..items.len()).rev() {
            let j = rng.random_range(0..=i);
            items.swap(i, j);
        }
    }

    /// A uniform index in `0..n` (`random_range(0..n)` on a `usize`): the setup's seeded speaker and the seating
    /// draws. Kept as its own form rather than `die(n) - 1`: the setup calls it this way, and it stays bit-identical
    /// without relying on how `rand` maps the two ranges.
    ///
    /// # Panics
    /// When `n == 0`.
    pub fn index(&mut self, domain: Domain, n: usize) -> usize {
        assert!(n > 0, "ti4-chance: an index among no elements");
        self.stream_mut(domain).random_range(0..n)
    }

    /// Each stream's word position, in domain order (`Rng::positions`).
    pub fn positions(&self) -> impl Iterator<Item = (Domain, u128)> + '_ {
        self.streams.iter().map(|s| (s.domain, s.rng.get_word_pos()))
    }

    /// The word position of one stream (0 when never drawn from).
    #[must_use]
    pub fn position(&self, domain: Domain) -> u128 {
        self.streams.iter().find(|s| s.domain == domain).map_or(0, |s| s.rng.get_word_pos())
    }

    /// Keep only the streams listed, creating missing ones and moving each to its position (`Rng::set_positions`).
    pub fn set_positions(&mut self, positions: &[(Domain, u128)]) {
        Arc::make_mut(&mut self.streams).retain(|s| positions.iter().any(|(d, _)| *d == s.domain));
        for (domain, pos) in positions {
            self.stream_mut(*domain).set_word_pos(*pos);
        }
    }

    /// Whether both share their stream table (a checkpoint nobody drew from since).
    #[must_use]
    pub fn shares_streams_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.streams, &other.streams)
    }
}

/// Answer `req` from `streams` in place: one die per face, in order, from the request's domain. The hot path: the
/// play-mode resolver (`ti4-engine-chance`) answers every chance question of the core with it.
///
/// # Errors
/// See [`Refused`]. Nothing is drawn on an error.
pub fn try_answer_mut(req: &Request, streams: &mut Streams) -> Result<Faces, Refused> {
    let (n, sides) = match req.kind {
        Kind::Dice { n, sides, .. } => (n, sides),
        Kind::Index { n } => (1, n),
    };
    if usize::from(n) > MAX_FACES {
        return Err(Refused::TooMany(n));
    }
    if sides == 0 && n > 0 {
        return Err(Refused::NoSides);
    }
    let mut faces = Faces::default();
    if n == 0 {
        return Ok(faces);
    }
    let rng = streams.stream_mut(req.domain);
    let range = 1..=u32::from(sides);
    for _ in 0..n {
        // A face is at most `sides`, which is a `u8`.
        #[allow(clippy::cast_possible_truncation)]
        let face = rng.random_range(range.clone()) as u8;
        let _ = faces.try_push(face);
    }
    Ok(faces)
}

/// [`try_answer_mut`].
///
/// # Panics
/// When it refuses (a defect of the asking site).
pub fn answer_mut(req: &Request, streams: &mut Streams) -> Faces {
    // (A fixed message: formatting the request would link its `Debug` into the browser build.)
    try_answer_mut(req, streams).unwrap_or_else(|_| panic!("ti4-chance: a request it cannot answer (too many faces or no sides)"))
}

/// The pure form of [`answer_mut`]: the faces and the advanced streams; `streams` is untouched.
///
/// # Panics
/// As [`answer_mut`].
#[must_use]
pub fn answer(req: &Request, streams: &Streams) -> (Faces, Streams) {
    let mut next = streams.clone();
    let faces = answer_mut(req, &mut next);
    (faces, next)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bare(seed: u64, domain: Domain) -> ChaCha8Rng {
        ChaCha8Rng::from_seed(derive_seed(seed, domain.text()))
    }

    #[test]
    fn every_sides_and_count_matches_a_bare_chacha() {
        // docs/FORK_RNG_THEORY.md 4.1: every `sides` in 1..=255 x n in 1..=15, against `ChaCha8Rng::random_range`.
        for seed in 0..48u64 {
            let mut s = Streams::new(seed);
            let mut b = bare(seed, Domain::Dice);
            for sides in 1..=255u8 {
                for n in 1..=12u8 {
                    let f = answer_mut(&Request::new(Domain::Dice, Kind::Dice { n, sides, hits_on: None }), &mut s);
                    for &face in f.as_slice() {
                        assert_eq!(u32::from(face), b.random_range(1..=u32::from(sides)), "seed {seed} sides {sides} n {n}");
                    }
                }
            }
            assert_eq!(s.position(Domain::Dice), b.get_word_pos());
        }
    }

    #[test]
    fn refusals_draw_nothing() {
        let mut s = Streams::new(1);
        let many = Request::new(Domain::Dice, Kind::Dice { n: 13, sides: 10, hits_on: None });
        assert_eq!(try_answer_mut(&many, &mut s), Err(Refused::TooMany(13)));
        let none = Request::new(Domain::Dice, Kind::Index { n: 0 });
        assert_eq!(try_answer_mut(&none, &mut s), Err(Refused::NoSides));
        assert_eq!(s, Streams::new(1));
        let empty = Request::new(Domain::Dice, Kind::Dice { n: 0, sides: 10, hits_on: None });
        assert!(answer_mut(&empty, &mut s).is_empty());
        assert_eq!(s.positions().count(), 0, "an empty group creates no stream");
    }
}
