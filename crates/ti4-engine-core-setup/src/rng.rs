//! The random streams of the setup: the same `SHA-256(seed_le || domain text)` seeding as the string engine's `GameRng`
//! (`ti4_engine::rng`), on the core's own generator (`ti4_engine_core::state::Rng`, bit-identical draws and shuffles).

use rand::{Rng as _, SeedableRng};
use rand_chacha::ChaCha8Rng;
use sha2::{Digest, Sha256};
use ti4_engine_core::state::{RngDomain, SeedSource};

/// The text of a literal stream name (`ti4_engine::rng::domain`); `None` for a dynamic domain, which no setup draws from.
#[must_use]
pub const fn domain_text(domain: RngDomain) -> Option<&'static str> {
    Some(match domain {
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
        RngDomain::Dyn(..) => return None,
    })
}

/// `SHA-256(seed_le || domain)`: the seed of one named stream.
#[must_use]
pub fn derive_seed(seed: u64, domain: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(seed.to_le_bytes());
    hasher.update(domain.as_bytes());
    hasher.finalize().into()
}

/// The seeding of every literal domain.
#[derive(Debug, Clone, Copy, Default)]
pub struct Literals;

impl SeedSource for Literals {
    fn seed_for(&self, seed: u64, domain: RngDomain) -> [u8; 32] {
        let text = domain_text(domain).expect("the setup draws from literal domains only");
        derive_seed(seed, text)
    }
}

/// The opening speaker drawn from `seed`: a uniformly chosen seat index, the same one for the same seed and table
/// (its own stream, `setup:speaker`, so choosing it shifts no deck or die). `None` for an empty table.
#[must_use]
pub fn seeded_speaker_index(seats: usize, seed: u64) -> Option<usize> {
    if seats == 0 {
        return None;
    }
    let mut rng = ChaCha8Rng::from_seed(derive_seed(seed, "setup:speaker"));
    Some(rng.random_range(0..seats))
}
