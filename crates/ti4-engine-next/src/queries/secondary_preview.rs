//! The types of `secondary_preview.rs` of the legacy crate (the preview of a follower's strategy-card secondary), with the thin `Choice`.
//! `preview_secondary` itself is NOT here: it runs the string flow (`StrategySecondaryWindow`, `strategy_cards::follow`) on a throwaway copy of
//! the position; its twin runs the core's strategic arm (`live/strategic.rs`) on a fork of the wrapper's core state and belongs to lane D
//! (docs/D28B2_RESUME.md). `MAX_ANSWERS` is in `kinds`.

use serde::{Deserialize, Serialize};
use ti4_content::StrategyCardKind;

use super::strategy::SecondaryBlocker;
use crate::wire::choice::Choice;

pub use crate::kinds::secondary_preview::MAX_ANSWERS;

/// What the follower would be asked, as of the position given.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
#[allow(
    clippy::large_enum_variant,
    reason = "one short-lived value per request; boxing the choice would only complicate every reader"
)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum SecondaryPreview {
    /// The next question: the window question when `step` is 0, a follow-up after that.
    Question {
        choice: Choice,
        step: usize,
        /// Technology: how the engine would pay the 4 resources once a technology is chosen.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        payment: Option<PaymentPreview>,
        /// Faction prompts the real flow would put first that the preview declined to get here.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        skipped: Vec<SkippedPrompt>,
    },
    /// The window would not open for this seat at all.
    WouldNotBeAsked { blocker: SecondaryBlocker },
    /// The answers given end the secondary: nothing further would be asked.
    Complete {
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        skipped: Vec<SkippedPrompt>,
        /// Scripted answers after the first that the flow never asked for (it ended earlier than
        /// the script assumed: a purchase of more tokens than the seat can afford). Payment
        /// answers for a payment the engine settled by itself (a lone option) do not count.
        #[serde(default, skip_serializing_if = "is_zero")]
        unused_answers: usize,
    },
    /// A scripted answer is not offered by the question the engine asks at that point: the
    /// planned answers do not fit the game as it is now. `choice` is that question.
    Rejected {
        /// Index into the given `answers` of the answer that is not offered.
        at: usize,
        answer: String,
        choice: Box<Choice>,
    },
    /// The preview cannot say (an unexpected question, an answer that is not offered, ...).
    Unavailable { detail: String },
}

const fn is_zero(count: &usize) -> bool {
    *count == 0
}

/// How the engine would pay a Technology secondary's resources: the first plan of
/// [`crate::payment::plans`], which is the one `paid_research` applies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PaymentPreview {
    /// Resources owed.
    pub cost: i64,
    /// Planets that would be exhausted.
    pub planets: Vec<String>,
    /// Trade goods that would be spent.
    pub trade_goods: i32,
    /// What the plan is worth (at least `cost`).
    pub worth: i64,
}

/// A question outside the card's own that the real flow would ask first, answered "decline" here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SkippedPrompt {
    pub subtype: String,
    pub prompt: String,
}

/// Which decision subtypes are the card's own follow-up questions (the ones a client prepares).
/// Empty for the cards whose follow-up is not previewed.
#[must_use]
pub fn follow_up_subtypes(card: StrategyCardKind) -> &'static [&'static str] {
    match card {
        StrategyCardKind::Technology => &["research_technology"],
        StrategyCardKind::Diplomacy => &["ready_planet"],
        StrategyCardKind::Construction => &["place_structure"],
        // Production: the build list, then (after a scripted build) the payment or placement
        // question the engine asks next, if it asks one at all.
        StrategyCardKind::Warfare => &["produce_unit", "pay_resources", "place_unit"],
        // Leadership: the payment of each token is the player's choice, one planet or trade good
        // per question (`pay_influence`; the engine has no automatic rule for it), then the
        // token's pool, then the "again?" question. A client scripts the whole purchase to have
        // the engine check it. Trade, Politics and Imperial resolve without a question (or draw
        // cards, which a preview must not run).
        StrategyCardKind::Leadership => &[
            "pay_influence",
            "gain_command_token",
            "buy_token_with_influence",
        ],
        StrategyCardKind::Politics | StrategyCardKind::Trade | StrategyCardKind::Imperial => &[],
    }
}
