//! The eight strategy cards in play (`ti4_engine::setup::strategy_card_setup*`, pure content).

use std::collections::BTreeMap;

use ti4_content::ContentStore;
use ti4_model::content_types::{ContentType, Source, SourceSet};
use ti4_model::id::StrategyCardId;

/// The strategy-card set to use for a source scope, most recent expansion first (Thunder's Edge replaces the `PoK` set, which replaces the base set).
const STRATEGY_CARD_SETS: [(Source, &str); 3] = [(Source::ThundersEdge, "te"), (Source::Pok, "pok"), (Source::Base, "base_game")];

/// The eight cards of the set named `card_set` (`te`, `pok`, `base_game`, `base_game_codex1`), or of the set of `sources` when `None`, with
/// their initiative numbers, in the order of the set. `None` when no set matches.
#[must_use]
pub fn strategy_card_setup(content: &ContentStore, sources: SourceSet, card_set: Option<&str>) -> Option<(Vec<StrategyCardId>, BTreeMap<StrategyCardId, i32>)> {
    let chosen = match card_set {
        Some(alias) => content.get(ContentType::StrategyCardSets, alias)?,
        None => STRATEGY_CARD_SETS.iter().filter(|(source, _)| sources.contains(*source)).find_map(|(_, alias)| content.get(ContentType::StrategyCardSets, alias))?,
    };
    let mut ids = Vec::new();
    let mut initiative = BTreeMap::new();
    for card_id in chosen.strings("scIDs") {
        // Read initiative from the whole corpus, not the scoped view: a set may name a card whose own source tag is narrower than the set's.
        let Some(card) = content.get(ContentType::StrategyCards, card_id) else {
            continue;
        };
        let id = StrategyCardId::new(card_id);
        initiative.insert(id.clone(), i32::try_from(card.int("initiative").unwrap_or(99)).unwrap_or(99));
        ids.push(id);
    }
    Some((ids, initiative))
}
