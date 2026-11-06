//! The stream domains: one independent ChaCha8 stream per purpose.
//!
//! The variants and their order are the core's `state::RngDomain` (the order matters: [`crate::Streams`] keeps its
//! streams sorted by domain and reports positions in that order, as the core does). The texts are the literal stream
//! names of the string engine (`ti4_engine::rng::domain`, setup `rng::domain_text`, the codec's `LITERALS`): a stream's
//! seed is `SHA-256(seed_le || text)`, so a text is part of every game's dice and must never change.

/// A random stream. The 22 literal domains of today's engine, plus `Dyn(family, n)`, a stream whose name is not a
/// literal (only old saves carry one; the text lives in the adapter's code table). A `Dyn` stream keeps its position
/// but is never drawn from ([`crate::Refused::Dyn`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Domain {
    /// Every die of the game (combat, bombardment, space cannon, abilities, Spy, Sanctions, ...).
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
    /// The Fracture entry roll.
    Fracture,
    /// Neuraloop: a random objective from both decks.
    Neuraloop,
    /// The Yssaril (Kyver) hero: random cards from a hand.
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

impl Domain {
    /// Every literal domain, in domain order.
    pub const LITERALS: [Self; 22] = [
        Self::Dice,
        Self::Objectives,
        Self::Agendas,
        Self::ActionCards,
        Self::Secrets,
        Self::Relics,
        Self::Galaxy,
        Self::Exploration,
        Self::Map,
        Self::Speaker,
        Self::Fracture,
        Self::Neuraloop,
        Self::YssarilHero,
        Self::Direct,
        Self::DeckA,
        Self::DeckB,
        Self::RetryProbe,
        Self::SeatingFactionAssignment,
        Self::SeatingLobbyDraw,
        Self::WideRoster,
        Self::KeleresVariant,
        Self::YinAscendant,
    ];

    /// The literal stream name; `None` for `Dyn`.
    #[must_use]
    pub const fn text(self) -> Option<&'static str> {
        Some(match self {
            Self::Dice => "dice",
            Self::Objectives => "deck:objectives",
            Self::Agendas => "deck:agendas",
            Self::ActionCards => "deck:action_cards",
            Self::Secrets => "deck:secrets",
            Self::Relics => "deck:relics",
            Self::Galaxy => "galaxy",
            Self::Exploration => "deck:exploration",
            Self::Map => "map",
            Self::Speaker => "setup:speaker",
            Self::Fracture => "fracture-breakthrough",
            Self::Neuraloop => "neuraloop",
            Self::YssarilHero => "yssarilhero",
            Self::Direct => "direct",
            Self::DeckA => "deck:a",
            Self::DeckB => "deck:b",
            Self::RetryProbe => "retry probe",
            Self::SeatingFactionAssignment => "seating:faction-assignment:v1",
            Self::SeatingLobbyDraw => "seating:lobby-faction-draw:v1",
            Self::WideRoster => "seating:wide-roster:v1",
            Self::KeleresVariant => "seating:keleres-variant:v1",
            Self::YinAscendant => "yinbt:unused_commander",
            Self::Dyn(..) => return None,
        })
    }

    /// The literal domain named `text`, if any.
    #[must_use]
    pub fn of_text(text: &str) -> Option<Self> {
        Self::LITERALS.into_iter().find(|d| d.text() == Some(text))
    }
}

/// `SHA-256(seed.to_le_bytes() || domain text)`: the 32-byte ChaCha8 seed of one named stream (`GameRng::derive_seed`).
/// Hashing rather than mixing the domain in: two names that differ by one bit must not give related streams.
#[must_use]
pub fn derive_seed(seed: u64, text: &str) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(seed.to_le_bytes());
    hasher.update(text.as_bytes());
    hasher.finalize().into()
}
