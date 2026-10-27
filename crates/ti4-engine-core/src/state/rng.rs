//! `Rng`: the seed and the ChaCha8 streams by typed domain. Draw order, `shuffle`, `die` and word
//! positions run the same `rand` / `rand_chacha` code as the string engine's `GameRng`; the stream
//! seed is `SHA-256(seed_le || domain text)`, computed by the adapter's [`SeedSource`] (the core never
//! holds the text).

use rand::Rng as _;
use rand_chacha::ChaCha8Rng;
use rand_core::SeedableRng;

use crate::containers::Cow;

/// A random stream. The literal domains of the string engine are variants; the dynamically named ones
/// are `Dyn(family, n)`, the family's text being the adapter's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RngDomain {
    Dice,
    Objectives,
    Agendas,
    ActionCards,
    Secrets,
    Relics,
    Galaxy,
    Exploration,
    Map,
    Speaker,
    Fracture,
    Neuraloop,
    YssarilHero,
    Direct,
    DeckA,
    DeckB,
    RetryProbe,
    SeatingFactionAssignment,
    SeatingLobbyDraw,
    WideRoster,
    KeleresVariant,
    YinAscendant,
    Dyn(u16, u32),
}

/// Produces the 32-byte seed of a stream: `SHA-256(seed.to_le_bytes() || domain text)`.
pub trait SeedSource {
    fn seed_for(&self, seed: u64, domain: RngDomain) -> [u8; 32];
}

/// One stream and where it stands.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Stream {
    domain: RngDomain,
    rng: ChaCha8Rng,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Rng {
    seed: u64,
    /// Sorted by domain. Behind one `Cow` (a stream is 136 bytes; twelve of them would make every
    /// checkpoint a 1.6 KB copy): the first draw after a checkpoint copies the vec once.
    streams: Cow<Vec<Stream>>,
}

impl Rng {
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self { seed, streams: Cow::default() }
    }

    #[must_use]
    pub const fn seed(&self) -> u64 {
        self.seed
    }

    fn stream_mut(&mut self, domain: RngDomain, source: &dyn SeedSource) -> &mut ChaCha8Rng {
        let seed = self.seed;
        let streams = self.streams.edit();
        let at = match streams.binary_search_by(|s| s.domain.cmp(&domain)) {
            Ok(at) => at,
            Err(at) => {
                let rng = ChaCha8Rng::from_seed(source.seed_for(seed, domain));
                streams.insert(at, Stream { domain, rng });
                at
            }
        };
        &mut streams[at].rng
    }

    /// Fisher-Yates from the back, as `GameRng::shuffle`.
    pub fn shuffle<T>(&mut self, domain: RngDomain, source: &dyn SeedSource, items: &mut [T]) {
        let rng = self.stream_mut(domain, source);
        for i in (1..items.len()).rev() {
            let j = rng.random_range(0..=i);
            items.swap(i, j);
        }
    }

    /// A die with `sides` sides, `1..=sides`, as `GameRng::die`.
    pub fn die(&mut self, domain: RngDomain, source: &dyn SeedSource, sides: u32) -> u32 {
        self.stream_mut(domain, source).random_range(1..=sides)
    }

    /// Each stream's word position, in domain order (`GameRng::positions`).
    pub fn positions(&self) -> impl Iterator<Item = (RngDomain, u128)> + '_ {
        self.streams.iter().map(|s| (s.domain, s.rng.get_word_pos()))
    }

    /// Keep only the streams listed, creating missing ones and moving each to its position
    /// (`GameRng::set_positions`).
    pub fn set_positions(&mut self, positions: &[(RngDomain, u128)], source: &dyn SeedSource) {
        self.streams.edit().retain(|s| positions.iter().any(|(d, _)| *d == s.domain));
        for (domain, pos) in positions {
            self.stream_mut(*domain, source).set_word_pos(*pos);
        }
    }

    /// Whether both generators share their streams (a checkpoint nobody drew from).
    #[must_use]
    pub fn shares_streams_with(&self, other: &Self) -> bool {
        Cow::ptr_eq(&self.streams, &other.streams)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Plain;
    impl SeedSource for Plain {
        fn seed_for(&self, seed: u64, domain: RngDomain) -> [u8; 32] {
            let mut out = [0u8; 32];
            out[..8].copy_from_slice(&seed.to_le_bytes());
            out[8] = match domain {
                RngDomain::Dice => 1,
                RngDomain::Agendas => 2,
                _ => 9,
            };
            out
        }
    }

    #[test]
    fn draws_match_a_bare_chacha_with_the_same_seed() {
        let mut rng = Rng::new(7);
        let mut bare = ChaCha8Rng::from_seed(Plain.seed_for(7, RngDomain::Dice));
        for _ in 0..50 {
            assert_eq!(rng.die(RngDomain::Dice, &Plain, 10), bare.random_range(1..=10));
        }
        let mut items: Vec<u32> = (0..30).collect();
        let mut expect = items.clone();
        let mut bare = ChaCha8Rng::from_seed(Plain.seed_for(7, RngDomain::Agendas));
        for i in (1..expect.len()).rev() {
            let j = bare.random_range(0..=i);
            expect.swap(i, j);
        }
        rng.shuffle(RngDomain::Agendas, &Plain, &mut items);
        assert_eq!(items, expect);
    }

    #[test]
    fn positions_round_trip_and_clone_shares() {
        let mut a = Rng::new(3);
        a.die(RngDomain::Dice, &Plain, 10);
        a.die(RngDomain::Agendas, &Plain, 10);
        let b = a.clone();
        assert!(a.shares_streams_with(&b));
        let pos: Vec<_> = a.positions().collect();
        a.die(RngDomain::Dice, &Plain, 10);
        assert!(!a.shares_streams_with(&b));
        let mut c = Rng::new(3);
        c.set_positions(&pos, &Plain);
        assert_eq!(c.positions().collect::<Vec<_>>(), pos);
        assert_eq!(c.die(RngDomain::Dice, &Plain, 10), {
            let mut d = b.clone();
            d.die(RngDomain::Dice, &Plain, 10)
        });
    }
}
