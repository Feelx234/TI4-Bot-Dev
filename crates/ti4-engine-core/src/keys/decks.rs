//! The closed key sets of the deck- and slice-shaped maps.

/// The key of `GameState::exploration_decks` (and of the deck an exploration card is drawn from).
/// Declared in text order of the keys (`CULTURAL < FRONTIER < HAZARDOUS < INDUSTRIAL`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum ExploreDeck {
    Cultural = 0,
    Frontier = 1,
    Hazardous = 2,
    Industrial = 3,
}

impl ExploreDeck {
    pub const COUNT: usize = 4;
    pub const ALL: [Self; Self::COUNT] = [
        Self::Cultural,
        Self::Frontier,
        Self::Hazardous,
        Self::Industrial,
    ];

    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }
}

/// The key of `GameState::expedition_slices` (Thunder's Edge). Declared in text order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Expedition {
    ActionCards = 0,
    Influence = 1,
    Resources = 2,
    Secret = 3,
    TechPlanet = 4,
    TradeGoods = 5,
}

impl Expedition {
    pub const COUNT: usize = 6;
    pub const ALL: [Self; Self::COUNT] = [
        Self::ActionCards,
        Self::Influence,
        Self::Resources,
        Self::Secret,
        Self::TechPlanet,
        Self::TradeGoods,
    ];

    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }
}
