//! From `strategy_cards.rs` of the legacy crate.
use ti4_content::ContentStore;
use ti4_model::content_types::ContentType;

/// Display only: the agenda card being placed, as printed.
pub fn agenda_details(content: &ContentStore, alias: &str) -> serde_json::Value {
    let record = content.get(ContentType::Agendas, alias);
    let field = |key: &str| record.as_ref().and_then(|record| record.text(key)).unwrap_or_default().to_owned();
    let name = Some(field("name")).filter(|name| !name.is_empty()).unwrap_or_else(|| alias.to_owned());
    serde_json::json!({
        "id": alias,
        "name": name,
        "type": field("type"),
        "target": field("target"),
        "text1": field("text1"),
        "text2": field("text2"),
    })
}
