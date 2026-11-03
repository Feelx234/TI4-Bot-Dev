//! From `timing.rs` of the legacy crate.

/// The constants live in `crate::kinds` (lane B); this module re-exports them.
pub use crate::kinds::timing::{ACTIVE_DETAIL};
use serde::{Deserialize, Serialize};

/// The point relative to an event at which an ability may resolve.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Relation {
    /// Resolve before the event's ordinary effect.
    When,
    /// Resolve after the event's ordinary effect.
    After,
}


/// The label of a reaction option for the player: the printed name of the faction ability,
/// leader, breakthrough, technology or unit behind an ability id such as
/// `ability:empyrean:aetherpassage:SYSTEM_ACTIVATED:after` or
/// `leader:mentak:mentakagent:PILLAGE_USED:after`, plus the window in words ("after system
/// activated"). A key with a clause suffix (`empyreanbt_allow`) names its card and the clause
/// ("Void Tether: allow"). The option's id is never changed; other ids (rules, `reaction:` cards)
/// keep their id as the label, and a card play replaces it afterwards. Without `content` the
/// name is the id's own key, capitalised.
pub fn ability_label(content: Option<&ti4_content::ContentStore>, id: &str) -> String {
    let parts: Vec<&str> = id.split(':').collect();
    let kind = parts.first().copied().unwrap_or("");
    use ti4_model::content_types::ContentType;
    let content_type = match kind {
        "ability" => ContentType::Abilities,
        "leader" => ContentType::Leaders,
        "breakthrough" => ContentType::Breakthroughs,
        "technology" => ContentType::Technologies,
        "unit" => ContentType::Units,
        _ => return id.to_owned(),
    };
    if parts.len() < 4 {
        return id.to_owned();
    }
    // The last two segments are the window: EVENT_TYPE and when/after.
    let relation = parts[parts.len() - 1];
    let event = parts[parts.len() - 2];
    let window = format!("{relation} {}", event.to_lowercase().replace('_', " "));
    let keys = &parts[1..parts.len() - 2];
    let printed = |content: &ti4_content::ContentStore, key: &str| {
        let record = content.get(content_type, key)?;
        let name = record.text("name")?.to_owned();
        Some(match (kind, record.text("type")) {
            ("leader", Some(role)) => format!("{} {name}", capitalised(role)),
            _ => name,
        })
    };
    // The key is the last segment that names a record (a copied leader names the owner first).
    let named = content.and_then(|content| {
        keys.iter().rev().find_map(|key| {
            printed(content, key).or_else(|| {
                let (card, clause) = key.rsplit_once('_')?;
                Some(format!("{}: {}", printed(content, card)?, clause))
            })
        })
    });
    let name = named.unwrap_or_else(|| {
        let key = keys.last().copied().unwrap_or(id);
        capitalised(&key.replace('_', " "))
    });
    format!("{name} ({window})")
}

pub fn capitalised(text: &str) -> String {
    let mut chars = text.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

/// The wire name of a relation (`when` / `after`).
#[must_use]
pub const fn relation_name(relation: Relation) -> &'static str {
    match relation {
        Relation::When => "when",
        Relation::After => "after",
    }
}
