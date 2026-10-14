//! The per-game strategy card set.
//!
//! A server game plays one of the catalog's `strategy_card_sets`, chosen when it is created.
//! The choice is stored with the lobby and the init record; the cards themselves are baked into
//! the saved initial state, so a restart or a history replacement replays the same cards.
//!
//! Games saved before the option existed carry no value and keep the Prophecy of Kings cards
//! they were recorded with ([`LEGACY`]); only newly created games default to [`DEFAULT`].

/// Thunder's Edge cards: Construction and Warfare as printed in Thunder's Edge.
pub const TE: &str = "te";
/// Prophecy of Kings cards (Codex I Diplomacy).
pub const POK: &str = "pok";
/// Base game with the Codex I Diplomacy: the original Construction text.
pub const BASE_GAME_CODEX1: &str = "base_game_codex1";

/// The set a game created without naming one plays.
pub const DEFAULT: &str = TE;
/// The set a record with no value was recorded with.
pub const LEGACY: &str = POK;
/// Every set a player may choose, in the order the lobby lists them.
///
/// `base_game` is not offered: its Diplomacy (`base2`) prints "ready each exhausted planet you
/// control in that system" while the engine resolves the Codex I "ready up to 2 exhausted
/// planets", so the text shown would not be the effect played.
pub const OFFERED: [&str; 3] = [TE, POK, BASE_GAME_CODEX1];

/// A set name nobody offers.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown strategy_card_set {0:?} (expected one of: te, pok, base_game_codex1)")]
pub struct UnknownCardSet(pub String);

/// Checks a client-supplied name.
///
/// # Errors
/// [`UnknownCardSet`] for anything outside [`OFFERED`].
pub fn validate(name: &str) -> Result<&'static str, UnknownCardSet> {
    OFFERED
        .iter()
        .copied()
        .find(|offered| *offered == name)
        .ok_or_else(|| UnknownCardSet(name.to_owned()))
}

/// The set a stored value stands for: a missing value is a legacy record.
#[must_use]
pub fn resolve(stored: Option<&str>) -> &str {
    stored.unwrap_or(LEGACY)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_offered_sets_validate_and_missing_means_pok() {
        for set in OFFERED {
            assert_eq!(validate(set), Ok(set));
        }
        assert!(validate("base_game").is_err());
        assert!(validate("TE").is_err());
        assert_eq!(resolve(None), "pok");
        assert_eq!(resolve(Some("te")), "te");
        assert_eq!(DEFAULT, "te");
    }
}
