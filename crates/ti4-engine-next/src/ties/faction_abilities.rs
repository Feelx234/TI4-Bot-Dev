//! From `faction_abilities.rs` of the legacy crate (copies).

use ti4_content::ContentStore;
use ti4_model::content_types::ContentType;

/// Display only: a faction ability as printed (its name, when it can be used and what it does).
pub fn ability_card(content: &ContentStore, id: &str) -> serde_json::Value {
    let record = content.get(ContentType::Abilities, id);
    let field = |key: &str| record.as_ref().and_then(|record| record.text(key));
    serde_json::json!({
        "name": field("name"),
        "window": field("window"),
        "effect": field("windowEffect"),
    })
}

/// The cost of Munitions Reserves, paid at each combat round's opening window.
pub const MUNITIONS_COST: i32 = 2;
