//! Setup deck construction. The pure part (the shuffled decks of a source scope, drawn from the seed) lives in the setup crate
//! (`ti4_engine_core_setup::deck`) and is shared with the native core setup; what is here reads or writes a string position.

pub use ti4_engine_core_setup::deck::{
    EXPLORATION_TRAITS, Exclusions, StartingDecks, UNOFFICIAL_ACTION_CARDS, build_starting_decks, build_starting_decks_excluding,
};

/// Take the [`UNOFFICIAL_ACTION_CARDS`] out of the action deck, from rules version
/// [`ti4_model::state::RULES_OFFICIAL_ACTION_DECK`] on; below it this does nothing.
///
/// Called before every draw from the action deck rather than when the deck is built: setup runs
/// before a host sets the game's rules version (a resumed offline game rebuilds its setup at the
/// current version and only then is given the version it was played with), so the deck it
/// builds cannot know which deck the game plays. Removing the cards in place keeps every other
/// card in its shuffled position, and the deck has its own RNG domain, so no other deck or die
/// moves.
pub fn drop_unofficial_action_cards(state: &mut ti4_model::state::GameState) {
    if state.rules_version < ti4_model::state::RULES_OFFICIAL_ACTION_DECK {
        return;
    }
    state
        .action_card_deck
        .retain(|card| !UNOFFICIAL_ACTION_CARDS.contains(&card.as_str()));
}

/// How many cards the action deck has for the game's rules version: the
/// [`UNOFFICIAL_ACTION_CARDS`] a new game still carries before its first draw are not counted.
/// For views; a draw removes them ([`drop_unofficial_action_cards`]).
#[must_use]
pub fn action_deck_len(state: &ti4_model::state::GameState) -> usize {
    if state.rules_version < ti4_model::state::RULES_OFFICIAL_ACTION_DECK {
        return state.action_card_deck.len();
    }
    state
        .action_card_deck
        .iter()
        .filter(|card| !UNOFFICIAL_ACTION_CARDS.contains(&card.as_str()))
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ti4_content::ContentStore;
    use ti4_model::content_types::{ContentType, FULL, POK, SourceSet};

    fn content() -> &'static ContentStore {
        ContentStore::embedded()
    }

    /// The ids of one official deck of `decks.json`.
    fn official(alias: &str) -> std::collections::BTreeSet<String> {
        content()
            .get(ContentType::Decks, alias)
            .expect("the deck is in decks.json")
            .strings("cardIDs")
            .into_iter()
            .map(str::to_owned)
            .collect()
    }

    fn new_game(sources: SourceSet, version: u32) -> ti4_model::state::GameState {
        let ids = [
            ti4_model::id::PlayerId::new("a"),
            ti4_model::id::PlayerId::new("b"),
        ];
        let mut state = crate::setup::start_game_seeded(content(), &ids, sources, None, 5)
            .expect("setup succeeds");
        state.rules_version = version;
        state
    }

    #[test]
    fn a_new_game_draws_from_the_official_action_deck() {
        // I01: the corpus has three Vetos (veto, veto3, veto4); every printed deck has one.
        for (sources, deck, size) in [
            (FULL, "action_cards_te", 140),
            (POK, "action_cards_pok", 120),
        ] {
            let mut state = new_game(sources, ti4_model::state::RULES_VERSION);
            let built = state.action_card_deck.clone();
            assert_eq!(built.len(), size + 2, "setup still builds the corpus deck");
            assert_eq!(action_deck_len(&state), size);
            drop_unofficial_action_cards(&mut state);
            let dealt: std::collections::BTreeSet<String> = state
                .action_card_deck
                .iter()
                .map(|card| card.as_str().to_owned())
                .collect();
            assert_eq!(state.action_card_deck.len(), size);
            assert_eq!(dealt, official(deck), "{deck}");
            // Every other card keeps its shuffled position.
            let kept: Vec<_> = built
                .into_iter()
                .filter(|card| !UNOFFICIAL_ACTION_CARDS.contains(&card.as_str()))
                .collect();
            assert_eq!(state.action_card_deck, kept);
        }
    }

    #[test]
    fn an_old_game_keeps_the_corpus_action_deck() {
        let mut state = new_game(FULL, ti4_model::state::RULES_OFFICIAL_ACTION_DECK - 1);
        let built = state.action_card_deck.clone();
        drop_unofficial_action_cards(&mut state);
        assert_eq!(state.action_card_deck, built);
        assert_eq!(state.action_card_deck.len(), 142);
        assert_eq!(action_deck_len(&state), 142);
    }
    // (`a_draw_skips_the_extra_vetos_only_from_the_new_version` drives the string `action_cards::draw` with a string table: it moved to
    // `ti4-engine-core-bridge/tests/wrapper_native.rs`, where the legacy crate may be named.)
}
