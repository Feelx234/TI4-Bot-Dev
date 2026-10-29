//! Decision types of the `explore` area: the questions exploration puts to a seat.
//!
//! Owner: package B5 (docs/ENGINE_CORE_STATE.md 4.4). Ported: which deck a two-trait planet explores into,
//! Distant Suns' pick of one of two cards, the Ion Storm's faceup side and the reward choices of the
//! instant cards (Abandoned Warehouses, Merchant Station, Functioning Base, Local Fabricators, Mercenary
//! Outfit). The adapter side is `ti4-engine/src/render/explore.rs`.

use crate::generated::{Explore, Planet, Wormhole};

/// The "card" of a string exploration question (`DecisionSource::Content(card)`): the steps that are not an
/// exploration card of their own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Step {
    /// `choose_deck`: "exploration".
    Exploration,
    IonStorm,
    AbandonedWarehouses,
    MerchantStation,
    FunctioningBase,
    LocalFabricators,
    MercenaryOutfit,
}

impl Step {
    pub const ALL: [Self; 7] = [
        Self::Exploration,
        Self::IonStorm,
        Self::AbandonedWarehouses,
        Self::MerchantStation,
        Self::FunctioningBase,
        Self::LocalFabricators,
        Self::MercenaryOutfit,
    ];
}

/// What the reward of a "mech or infantry" card is (the text of the question's offer).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PayReward {
    /// Cultural Archaeology ... "gain 1 trade good".
    TradeGood,
    /// Expedition: "ready this planet".
    ReadyPlanet,
    /// Volatile Fuel Source: "gain 1 command token".
    CommandToken,
}

/// What the area asks. Parameters only; the options are the window's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ask {
    /// A planet with two exploration traits: "choose which deck to explore this planet into".
    ChooseDeck,
    /// Distant Suns (Naaz): resolve the first card or the second. The first card is the question's source.
    DistantSuns { first: Explore, second: Explore },
    /// "Ion Storm: which side faceup".
    IonStorm,
    /// The reward of an instant card with a choice (a [`Step`] other than `Exploration` and `IonStorm`).
    Reward(Step),
    /// "remove 1 infantry from this planet to resolve the exploration card" (Cultural Archaeology, Expedition,
    /// Volatile Fuel Source): a mech on the planet pays without a question, an infantry only if the seat agrees.
    PayInfantry { card: Explore, planet: Planet, reward: PayReward },
}

/// One answer the area offers (not stored in the state).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Opt {
    /// A deck (an exploration trait).
    Deck(crate::generated::ExploreType),
    /// One of the two cards of Distant Suns.
    Card(Explore),
    /// The faceup side of the ion storm token (alpha or beta).
    Face(Wormhole),
    /// "gain ..." (commodities).
    Gain,
    /// "replenish commodities" (Merchant Station).
    Replenish,
    /// "convert ... commodities to trade goods".
    Convert,
    /// "spend 1 trade good ...".
    SpendTradeGood,
    /// "spend 1 commodity ...".
    SpendCommodity,
    /// Mercenary Outfit: place 1 infantry.
    Place,
    /// Mercenary Outfit: "place nothing" (its id is `decline`).
    PlaceNothing,
    /// Remove 1 infantry and gain the reward.
    RemoveInfantry,
    /// Keep the infantry (its id is `decline`).
    KeepInfantry,
}

/// The machine identifier of the question (`DecisionContext.subtype`): `{card}_choose_reward`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Subtype {
    /// Named by a [`Step`].
    Step(Step),
    /// Named by an exploration card (Distant Suns).
    Card(Explore),
}
