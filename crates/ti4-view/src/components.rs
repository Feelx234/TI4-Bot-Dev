//! The printed side of what a player has in the play area: the faction abilities, the leaders,
//! the technologies, the relics and the legendary planet cards. A client asks for it once; a view
//! names these by id only.
//!
//! Nothing here decides a rule of the game. Whether a technology is a card that a player uses is
//! read from its printed text.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ti4_content::{
    ActionCardTiming, ContentStore, PublicObjectiveGroup, Record, SecretObjectiveGroup,
    action_card_timing, public_objective_group, secret_objective_group,
};
use ti4_engine::deck::build_starting_decks;
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

/// One action card as printed. Copies of a card have ids of their own and one name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionCard {
    pub id: String,
    pub name: String,
    /// The phase of the card: `Action`, `Agenda`, `Strategy`, `Status` or `Any`.
    pub phase: String,
    /// When the card is played, as printed: `Action`, or a timing.
    pub window: String,
    /// The group of the timing, one of [`ComponentCatalog::action_card_groups`].
    pub group: String,
    pub text: String,
}

/// One secret objective as printed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SecretObjective {
    pub id: String,
    pub name: String,
    /// The phase in which the objective is scored, as printed: `Status`, `Action` or `Agenda`.
    pub phase: String,
    /// What the objective asks for, one of [`ComponentCatalog::secret_objective_groups`].
    pub group: String,
    pub text: String,
}

/// One public objective as printed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicObjectiveCard {
    pub id: String,
    pub name: String,
    /// What the objective is worth: 1 for stage I, 2 for stage II.
    pub points: i64,
    /// What the objective asks for, one of [`ComponentCatalog::public_objective_groups`].
    pub group: String,
    pub text: String,
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
    /// The action card deck as a game begins with it, sorted by id.
    pub action_cards: Vec<ActionCard>,
    /// The groups of the timings of the action cards, in the order in which a reader is shown them.
    pub action_card_groups: Vec<String>,
    /// The secret objective deck as a game begins with it, sorted by id.
    pub secret_objectives: Vec<SecretObjective>,
    /// The groups of what the secret objectives ask for, in the order in which a reader is shown
    /// them.
    pub secret_objective_groups: Vec<String>,
    /// Every public objective that a game can reveal: stage I, then stage II, each sorted by id.
    /// A game draws five of each stage, and no fact says which.
    pub public_objectives: Vec<PublicObjectiveCard>,
    /// The groups of what the public objectives ask for, in the order in which a reader is shown
    /// them.
    pub public_objective_groups: Vec<String>,
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

/// The group of a card whose timing `ti4-content` does not know. No card of the corpus has it.
const OTHER_TIMING: &str = "Other";

/// The group of a secret objective that `ti4-content` does not know. No objective of the corpus
/// has it.
const OTHER_GROUP: &str = "Other";

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
        action_cards: {
            let mut deck = build_starting_decks(content, sources, 0).action_cards;
            deck.sort();
            deck.iter()
                .map(|id| {
                    let record = content.get(ContentType::ActionCards, id.as_str());
                    let printed = |key: &str| record.map(|record| text(record, key));
                    let phase = printed("phase").unwrap_or_default();
                    let window = printed("window").unwrap_or_default();
                    ActionCard {
                        id: id.as_str().to_owned(),
                        name: printed("name").unwrap_or_else(|| id.as_str().to_owned()),
                        group: action_card_timing(&phase, &window)
                            .map_or(OTHER_TIMING, ActionCardTiming::label)
                            .to_owned(),
                        phase,
                        window,
                        text: printed("text").unwrap_or_default(),
                    }
                })
                .collect()
        },
        action_card_groups: ActionCardTiming::ALL
            .iter()
            .map(|timing| timing.label())
            .chain([OTHER_TIMING])
            .map(ToOwned::to_owned)
            .collect(),
        secret_objectives: {
            let mut deck = build_starting_decks(content, sources, 0).secrets;
            deck.sort();
            deck.iter()
                .map(|id| {
                    let record = content.get(ContentType::SecretObjectives, id.as_str());
                    let printed = |key: &str| record.map(|record| text(record, key));
                    SecretObjective {
                        id: id.as_str().to_owned(),
                        name: printed("name").unwrap_or_else(|| id.as_str().to_owned()),
                        phase: printed("phase").unwrap_or_default(),
                        group: secret_objective_group(id.as_str())
                            .map_or(OTHER_GROUP, SecretObjectiveGroup::label)
                            .to_owned(),
                        text: printed("text").unwrap_or_default(),
                    }
                })
                .collect()
        },
        secret_objective_groups: SecretObjectiveGroup::ALL
            .iter()
            .map(|group| group.label())
            .chain([OTHER_GROUP])
            .map(ToOwned::to_owned)
            .collect(),
        public_objectives: {
            let mut cards: Vec<PublicObjectiveCard> = records(ContentType::PublicObjectives)
                .filter_map(|record| {
                    Some(PublicObjectiveCard {
                        id: record.id()?.to_owned(),
                        name: text(record, "name"),
                        points: record.int("points")?,
                        group: public_objective_group(record.id()?)
                            .map_or(OTHER_GROUP, PublicObjectiveGroup::label)
                            .to_owned(),
                        text: text(record, "text"),
                    })
                })
                .collect();
            cards.sort_by(|a, b| (a.points, &a.id).cmp(&(b.points, &b.id)));
            cards
        },
        public_objective_groups: PublicObjectiveGroup::ALL
            .iter()
            .map(|group| group.label())
            .chain([OTHER_GROUP])
            .map(ToOwned::to_owned)
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
    fn the_public_objectives_are_as_printed_by_stage() {
        let catalog = printed();
        let stage = |points| {
            catalog
                .public_objectives
                .iter()
                .filter(|card| card.points == points)
                .count()
        };
        assert_eq!((stage(1), stage(2)), (20, 20));
        assert_eq!(catalog.public_objectives.len(), 40);
        assert!(
            catalog
                .public_objectives
                .is_sorted_by_key(|card| card.points)
        );
        let borders = catalog
            .public_objectives
            .iter()
            .find(|card| card.id == "expand_borders")
            .expect("Expand Borders");
        assert_eq!(borders.name, "Expand Borders");
        assert_eq!(borders.group, "Planets you control");
        for card in &catalog.public_objectives {
            assert!(catalog.public_objective_groups.contains(&card.group));
            assert_ne!(card.group, OTHER_GROUP, "{} has no group", card.id);
        }
        assert_eq!(borders.text, "Control 6 planets in non-home systems.");
    }

    #[test]
    fn the_secret_objectives_are_as_printed_and_have_their_group() {
        let catalog = component_catalog(ContentStore::embedded(), POK);
        assert_eq!(catalog.secret_objectives.len(), 40);
        for objective in &catalog.secret_objectives {
            assert!(!objective.text.is_empty(), "{} has its text", objective.id);
            assert!(
                catalog.secret_objective_groups.contains(&objective.group)
                    && objective.group != OTHER_GROUP,
                "{} has its group",
                objective.id
            );
        }
        let gatekeeper = catalog
            .secret_objectives
            .iter()
            .find(|objective| objective.id == "btgk")
            .expect("Become the Gatekeeper");
        assert_eq!(gatekeeper.name, "Become the Gatekeeper");
        assert_eq!(gatekeeper.phase, "Status");
        assert_eq!(gatekeeper.group, "Ships on the board");
    }

    #[test]
    fn the_action_card_deck_has_every_copy_as_printed() {
        let catalog = printed();
        assert!(!catalog.action_cards.is_empty());
        for card in &catalog.action_cards {
            assert!(!card.window.is_empty(), "{} has its window", card.id);
            assert!(!card.text.is_empty(), "{} has its text", card.id);
            assert!(
                catalog.action_card_groups.contains(&card.group) && card.group != OTHER_TIMING,
                "{} has its group",
                card.id
            );
        }
        let boosts: Vec<_> = catalog
            .action_cards
            .iter()
            .filter(|card| card.name == "Morale Boost")
            .collect();
        assert_eq!(boosts.len(), 4);
        assert_eq!(boosts[0].window, "At the start of a combat round");
        assert_eq!(boosts[0].phase, "Action");
        assert_eq!(boosts[0].group, "Combat");
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
