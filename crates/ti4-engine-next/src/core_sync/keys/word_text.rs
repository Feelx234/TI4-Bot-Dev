//! The text of each [`ti4_engine_core::keys::Word`], in the order of `Word::ALL` (text order).

use ti4_engine_core::keys::Word;

/// The text of every word, indexed by `Word::index`.
pub static WORD_TEXT: [&str; Word::COUNT] = [
    "action",            // Action
    "action_cards",      // ActionCards
    "breakthrough",      // Breakthrough
    "choice",            // Choice
    "control",           // Control
    "declined",          // Declined
    "destroyed",         // Destroyed
    "discarded",         // Discarded
    "exempt",            // Exempt
    "false",             // False
    "g",                 // Galvanized (the trailing token of a staged destruction of a galvanized unit)
    "granted",           // Granted
    "ground",            // Ground
    "insider:peek",      // InsiderPeek
    "mi",                // Mi
    "negotiating",       // Negotiating
    "open",              // Open
    "probe",             // Probe
    "promissory_notes",  // PromissoryNotes
    "relics",            // Relics
    "replace",           // Replace
    "secret_objectives", // SecretObjectives
    "space",             // Space
    "spynet",            // Spynet
    "standing",          // Standing
    "swap",              // Swap
    "taken",             // Taken
    "technologies",      // Technologies
    "true",              // True
    "used",              // Used
    "winnu:gained",      // WinnuGained
    "winnu:imperator",   // WinnuImperator
    "winnu:tactical",    // WinnuTactical
    "yes",               // Yes
    "yssarilcommander",  // YssarilCommander
    "yssarilhero",       // YssarilHero
];

/// The word with this text, among `allowed`.
#[must_use]
pub fn word_from(text: &str, allowed: &[Word]) -> Option<Word> {
    allowed
        .iter()
        .copied()
        .find(|w| WORD_TEXT[w.index()] == text)
}

/// The text of a word.
#[must_use]
pub fn word_text(word: Word) -> &'static str {
    WORD_TEXT[word.index()]
}
