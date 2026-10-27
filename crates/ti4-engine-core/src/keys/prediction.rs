//! The values of `GameState::agenda_predictions`: a predicted outcome, optionally tagged by the card
//! that made it, or the marker of a player barred from voting.

use super::outcome::Outcome;

/// The card (or ability) that recorded a prediction: the text after the `|` of
/// `<outcome>|<tag>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PredictionTag {
    ConstRider,
    DiploRider,
    KeleresRider,
    LeadRider,
    PoliticRider,
    Sanction,
    TechRider,
    TradeRider,
    WarRider,
}

impl PredictionTag {
    pub const ALL: [Self; 9] = [
        Self::ConstRider,
        Self::DiploRider,
        Self::KeleresRider,
        Self::LeadRider,
        Self::PoliticRider,
        Self::Sanction,
        Self::TechRider,
        Self::TradeRider,
        Self::WarRider,
    ];
}

/// Why a player has an entry without having predicted anything (`none|<bar>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum VoteBar {
    Assassin,
    PoliticalSecret,
}

impl VoteBar {
    pub const ALL: [Self; 2] = [Self::Assassin, Self::PoliticalSecret];
}

/// One entry of `agenda_predictions`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Prediction {
    /// `<outcome>`: Imperial Rider's and the Nomad's plain prediction.
    Plain(Outcome),
    /// `<outcome>|<tag>`.
    Tagged(Outcome, PredictionTag),
    /// `none|<bar>`.
    Barred(VoteBar),
}
