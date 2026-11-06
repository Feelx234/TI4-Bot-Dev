//! The random streams of the setup: the bottom crate's `ti4_chance::Streams` (the same `SHA-256(seed_le || domain text)`
//! seeding as the string engine's `GameRng`, bit-identical draws and shuffles; docs/FORK_RNG_THEORY.md 4.1, lane C2). Setup
//! draws happen before the engine plays and are a pure function of the seed: they are not chance questions.

use ti4_engine_core::state::RngDomain;

/// The text of a stream name (`ti4_engine::rng::domain`). Every domain is a literal (decision D3: no dynamic domains).
#[must_use]
pub const fn domain_text(domain: RngDomain) -> &'static str {
    domain.text()
}

/// `SHA-256(seed_le || domain)`: the seed of one named stream.
#[must_use]
pub fn derive_seed(seed: u64, domain: &str) -> [u8; 32] {
    ti4_chance::derive_seed(seed, domain)
}

/// The opening speaker drawn from `seed`: a uniformly chosen seat index, the same one for the same seed and table
/// (its own stream, `setup:speaker`, so choosing it shifts no deck or die). `None` for an empty table.
#[must_use]
pub fn seeded_speaker_index(seats: usize, seed: u64) -> Option<usize> {
    if seats == 0 {
        return None;
    }
    Some(ti4_chance::Streams::new(seed).index(RngDomain::Speaker, seats))
}
