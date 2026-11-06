//! A FROZEN copy of the old implementation the bottom crate replaced (lane C2, docs/FORK_RNG_THEORY.md 4.7 U): the core's
//! `state::Rng` with its `SeedSource` and the setup's `rng::Literals` / `domain_text` / `derive_seed` / `seeded_speaker_index`, as
//! they stood at `core-integration-28` (f22ece5e8, `crates/ti4-engine-core/src/state/rng.rs` and
//! `crates/ti4-engine-core-setup/src/rng.rs`), minus the dynamic domains (decision D3) and the probe census.
//!
//! Until C2 the bit-identity tests compared against the live core and setup crates (dev-dependencies). Since C2 the core's streams
//! ARE `ti4_chance::Streams`, so comparing against the core would compare the crate with itself; the reference is kept here
//! instead, verbatim in its algorithm (the same `rand` calls, the same integer types, the same seeding). Do not "fix" it: it is the
//! definition of today's dice.

use rand::Rng as _;
use rand_chacha::ChaCha8Rng;
use rand_core::SeedableRng;
use sha2::{Digest, Sha256};

/// The old `state::RngDomain` (without `Dyn`), in its order.
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
}

/// The old `state::SeedSource`.
pub trait SeedSource {
    fn seed_for(&self, seed: u64, domain: RngDomain) -> [u8; 32];
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Stream {
    domain: RngDomain,
    rng: ChaCha8Rng,
}

/// The old `state::Rng` (its `Cow<Vec<Stream>>` is a plain `Vec` here: sharing did not change a draw).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Rng {
    seed: u64,
    streams: Vec<Stream>,
}

impl Rng {
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self { seed, streams: Vec::new() }
    }

    fn stream_mut(&mut self, domain: RngDomain, source: &dyn SeedSource) -> &mut ChaCha8Rng {
        let seed = self.seed;
        let at = match self.streams.binary_search_by(|s| s.domain.cmp(&domain)) {
            Ok(at) => at,
            Err(at) => {
                let rng = ChaCha8Rng::from_seed(source.seed_for(seed, domain));
                self.streams.insert(at, Stream { domain, rng });
                at
            }
        };
        &mut self.streams[at].rng
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

    pub fn positions(&self) -> impl Iterator<Item = (RngDomain, u128)> + '_ {
        self.streams.iter().map(|s| (s.domain, s.rng.get_word_pos()))
    }

    pub fn set_positions(&mut self, positions: &[(RngDomain, u128)], source: &dyn SeedSource) {
        self.streams.retain(|s| positions.iter().any(|(d, _)| *d == s.domain));
        for (domain, pos) in positions {
            self.stream_mut(*domain, source).set_word_pos(*pos);
        }
    }
}

/// The old setup `rng::domain_text`.
#[must_use]
pub const fn domain_text(domain: RngDomain) -> &'static str {
    match domain {
        RngDomain::Dice => "dice",
        RngDomain::Objectives => "deck:objectives",
        RngDomain::Agendas => "deck:agendas",
        RngDomain::ActionCards => "deck:action_cards",
        RngDomain::Secrets => "deck:secrets",
        RngDomain::Relics => "deck:relics",
        RngDomain::Galaxy => "galaxy",
        RngDomain::Exploration => "deck:exploration",
        RngDomain::Map => "map",
        RngDomain::Speaker => "setup:speaker",
        RngDomain::Fracture => "fracture-breakthrough",
        RngDomain::Neuraloop => "neuraloop",
        RngDomain::YssarilHero => "yssarilhero",
        RngDomain::Direct => "direct",
        RngDomain::DeckA => "deck:a",
        RngDomain::DeckB => "deck:b",
        RngDomain::RetryProbe => "retry probe",
        RngDomain::SeatingFactionAssignment => "seating:faction-assignment:v1",
        RngDomain::SeatingLobbyDraw => "seating:lobby-faction-draw:v1",
        RngDomain::WideRoster => "seating:wide-roster:v1",
        RngDomain::KeleresVariant => "seating:keleres-variant:v1",
        RngDomain::YinAscendant => "yinbt:unused_commander",
    }
}

/// The old setup `rng::derive_seed`.
#[must_use]
pub fn derive_seed(seed: u64, domain: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(seed.to_le_bytes());
    hasher.update(domain.as_bytes());
    hasher.finalize().into()
}

/// The old setup `rng::Literals`.
#[derive(Debug, Clone, Copy, Default)]
pub struct Literals;

impl SeedSource for Literals {
    fn seed_for(&self, seed: u64, domain: RngDomain) -> [u8; 32] {
        derive_seed(seed, domain_text(domain))
    }
}

/// The old setup `rng::seeded_speaker_index`.
#[must_use]
pub fn seeded_speaker_index(seats: usize, seed: u64) -> Option<usize> {
    if seats == 0 {
        return None;
    }
    let mut rng = ChaCha8Rng::from_seed(derive_seed(seed, "setup:speaker"));
    Some(rng.random_range(0..seats))
}
