//! The types of `strategy.rs` of the legacy crate that the product surface names: the recorded result of a follower's secondary decision and
//! the reason a follower is not asked one. (The window that produces them, `StrategySecondaryWindow`, is the string flow; the core's strategic
//! arm replaces it.)

/// The recorded structural result of one eligible follower's secondary decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecondaryResolution {
    /// The follower chose not to use the secondary.
    Declined,
    /// The follower accepted the secondary. Its shared token cost, when applicable, was paid;
    /// the game driver immediately invokes the content-specific effect.
    Followed,
    /// The follower had no strategy token and was not offered the secondary.
    Ineligible,
}

/// Why a follower is not asked a card's secondary (see [`StrategySecondaryWindow::blocker`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum SecondaryBlocker {
    /// The secondary costs a strategy token and the seat has none (and no waiver).
    NoStrategyToken,
    /// Leadership: the seat cannot afford one command token.
    CannotAffordInfluence,
    /// Technology: the seat cannot pay the 4 resources.
    CannotPayResources,
    /// Technology: nothing is researchable.
    NothingToResearch,
    /// Diplomacy: no controlled planet is exhausted.
    NoExhaustedPlanet,
    /// Trade: commodities are already at the limit.
    CommoditiesFull,
    /// Warfare: the home system has no production.
    NoProduction,
    Other,
}
