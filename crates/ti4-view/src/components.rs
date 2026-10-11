//! The printed side of what a player has in the play area: the faction abilities, the leaders,
//! the technologies, the relics and the legendary planet cards. A client asks for it once; a view
//! names these by id only.
//!
//! Nothing here decides a rule of the game. Whether a technology is a card that a player uses is
//! read from its printed text.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ti4_content::{ContentStore, Record};
use ti4_model::content_types::{ContentType, SourceSet};

use crate::exploration::{ExplorationCatalog, exploration_catalog};

/// One faction ability as printed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AbilityCard {
    pub id: String,
    pub name: String,
    /// When the ability is used, as printed: `ACTION`, or a timing. Empty for an ability that
    /// always holds.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub window: String,
    pub text: String,
}

/// One leader as printed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LeaderCard {
    pub id: String,
    /// `agent`, `commander` or `hero`.
    pub kind: String,
    pub name: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub window: String,
    pub text: String,
    /// How the leader is unlocked. Empty for a leader that is never locked.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub unlock: String,
}

/// A card that is a name and a text: a technology, a relic, the ability card of a legendary
/// planet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrintedCard {
    pub id: String,
    pub name: String,
    pub text: String,
    /// The card is used: its text has an `ACTION`, or it exhausts. A card that always holds is
    /// not.
    pub used: bool,
}

/// The printed cards of the play areas. It does not change during a game.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComponentCatalog {
    pub abilities: Vec<AbilityCard>,
    /// The abilities of each faction, in the order of the faction sheet.
    pub factions: BTreeMap<String, Vec<String>>,
    pub leaders: Vec<LeaderCard>,
    pub technologies: Vec<PrintedCard>,
    pub relics: Vec<PrintedCard>,
    /// The ability cards of the legendary planets, by the id of the planet.
    pub legendary: Vec<PrintedCard>,
}

/// Everything printed that a client is handed once.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    pub exploration: ExplorationCatalog,
    pub components: ComponentCatalog,
}

/// The catalogue of `sources`.
#[must_use]
pub fn catalog(content: &ContentStore, sources: SourceSet) -> Catalog {
    Catalog {
        exploration: exploration_catalog(content, sources),
        components: component_catalog(content, sources),
    }
}

fn text(record: &Record, key: &str) -> String {
    record.text(key).map(ToOwned::to_owned).unwrap_or_default()
}

/// Whether a card with this printed text is one that its owner uses.
fn is_used(text: &str) -> bool {
    text.contains("ACTION:") || text.to_ascii_lowercase().contains("exhaust this card")
}

/// The cards of the play areas of `sources`, as printed.
#[must_use]
pub fn component_catalog(content: &ContentStore, sources: SourceSet) -> ComponentCatalog {
    let records = |kind: ContentType| {
        content
            .records(kind)
            .iter()
            .filter(move |record| record.in_sources(sources))
    };
    let card = |id: &str, name: &str, printed: String| PrintedCard {
        id: id.to_owned(),
        name: name.to_owned(),
        used: is_used(&printed),
        text: printed,
    };
    let named = |record: &Record| {
        let id = record.id()?;
        Some(card(
            id,
            record.text("name").unwrap_or(id),
            text(record, "text"),
        ))
    };
    ComponentCatalog {
        abilities: records(ContentType::Abilities)
            .filter_map(|record| {
                let id = record.id()?;
                Some(AbilityCard {
                    id: id.to_owned(),
                    name: record.text("name").unwrap_or(id).to_owned(),
                    window: text(record, "window"),
                    text: [
                        text(record, "permanentEffect"),
                        text(record, "windowEffect"),
                    ]
                    .into_iter()
                    .filter(|part| !part.is_empty())
                    .collect::<Vec<_>>()
                    .join("\n"),
                })
            })
            .collect(),
        factions: ti4_content::factions::catalogue(content, sources)
            .into_iter()
            .map(|(alias, faction)| {
                (
                    alias.to_owned(),
                    faction
                        .abilities()
                        .into_iter()
                        .map(ToOwned::to_owned)
                        .collect(),
                )
            })
            .collect(),
        leaders: records(ContentType::Leaders)
            .filter_map(|record| {
                let id = record.id()?;
                Some(LeaderCard {
                    id: id.to_owned(),
                    kind: text(record, "type"),
                    name: record.text("name").unwrap_or(id).to_owned(),
                    title: text(record, "title"),
                    window: text(record, "abilityWindow"),
                    text: text(record, "abilityText"),
                    unlock: Some(text(record, "unlockCondition"))
                        .filter(|unlock| !unlock.eq_ignore_ascii_case("Always Unlocked"))
                        .unwrap_or_default(),
                })
            })
            .collect(),
        technologies: records(ContentType::Technologies)
            .filter_map(named)
            .collect(),
        relics: records(ContentType::Relics).filter_map(named).collect(),
        legendary: records(ContentType::Planets)
            .filter_map(|record| {
                let name = record.text("legendaryAbilityName")?;
                Some(card(
                    record.id()?,
                    name,
                    text(record, "legendaryAbilityText"),
                ))
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ti4_model::content_types::POK;

    fn printed() -> ComponentCatalog {
        component_catalog(ContentStore::embedded(), POK)
    }

    #[test]
    fn a_faction_has_its_abilities_as_printed() {
        let catalog = printed();
        assert_eq!(catalog.factions["sol"], ["orbital_drop", "versatile"]);
        for (faction, abilities) in &catalog.factions {
            for id in abilities {
                let card = catalog
                    .abilities
                    .iter()
                    .find(|card| card.id == *id)
                    .unwrap_or_else(|| panic!("{id} of {faction} is in the catalogue"));
                assert!(!card.text.is_empty(), "{id} has its text");
            }
        }
        let drop = catalog
            .abilities
            .iter()
            .find(|card| card.id == "orbital_drop")
            .expect("Orbital Drop");
        assert_eq!(drop.name, "Orbital Drop");
        assert_eq!(drop.window, "ACTION");
        assert!(drop.text.contains("2 infantry"));
    }

    #[test]
    fn a_leader_says_how_it_is_unlocked() {
        let catalog = printed();
        let leader = |id: &str| {
            catalog
                .leaders
                .iter()
                .find(|card| card.id == id)
                .unwrap_or_else(|| panic!("no leader {id}"))
        };
        let agent = leader("solagent");
        assert_eq!(agent.kind, "agent");
        assert_eq!(agent.name, "Evelyn DeLouis");
        assert!(agent.unlock.is_empty(), "an agent is never locked");
        assert!(agent.text.contains("1 additional die"));
        assert!(leader("solcommander").unlock.contains("12 resources"));
        assert_eq!(leader("solhero").kind, "hero");
        assert_eq!(leader("solhero").unlock, "Have 3 scored objectives.");
    }

    #[test]
    fn a_technology_that_always_holds_is_not_used() {
        let catalog = printed();
        let used = |id: &str| {
            catalog
                .technologies
                .iter()
                .find(|card| card.id == id)
                .unwrap_or_else(|| panic!("no technology {id}"))
                .used
        };
        assert!(used("sr"), "Sling Relay has an ACTION");
        assert!(used("bs"), "Bio-Stims exhausts");
        assert!(!used("amd"));
        assert!(!used("nm"));
        assert!(!used("cr2"), "a unit upgrade");
    }

    #[test]
    fn relics_and_legendary_planets_have_their_text() {
        let catalog = printed();
        let core = catalog
            .relics
            .iter()
            .find(|card| card.id == "dynamiscore")
            .expect("Dynamis Core");
        assert_eq!(core.name, "Dynamis Core");
        assert!(core.used, "Dynamis Core has an ACTION");
        let shard = catalog.relics.iter().find(|card| card.id == "shard");
        assert!(!shard.expect("Shard of the Throne").used);
        assert!(!catalog.legendary.is_empty());
        for card in &catalog.legendary {
            assert!(!card.text.is_empty(), "{} has its text", card.id);
        }
    }
}
