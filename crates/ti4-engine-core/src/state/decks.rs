//! `Decks`: one `Deck<T>` per card deck. A draw advances the head without copying.

use crate::containers::Deck;
use crate::generated::{ActionCard, Agenda, Explore, ExploreType, PublicObjective, Relic, SecretObjective};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Decks {
    pub action: Deck<ActionCard>,
    pub secret: Deck<SecretObjective>,
    pub agenda: Deck<Agenda>,
    pub relic: Deck<Relic>,
    /// `GameState.objective_deck` (stage I then stage II).
    pub objective: Deck<PublicObjective>,
    /// `GameState.exploration_decks`, by [`ExploreType`] (cultural, frontier, hazardous, industrial).
    pub explore: [Deck<Explore>; ExploreType::COUNT],
    /// Bit `i` set: the string map has a key for `ExploreType` `i` (an empty deck is a present key).
    pub explore_present: u8,
}
