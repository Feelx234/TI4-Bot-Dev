//! The engine's own card registries, as JSON, for `tools/card_audit/card_audit.py`.
//!
//! Read-only: asks each registry what it implements (the same functions `coverage_report` and
//! `registry::ledger` use) and prints one JSON object on stdout. The audit script joins it with its
//! static scan of the sources; without it the script falls back to literal-reference counts alone.
//!
//! ```text
//! cargo run -p ti4-engine-legacy --example card_registry_dump > docs/card_audit/registry_dump.json
//! ```

use std::collections::BTreeMap;

use serde_json::{Value, json};
use ti4_content::ContentStore;
use ti4_model::content_types::{ContentType, DEFAULT};
use ti4_model::id::{ActionCardId, ObjectiveId};

fn strings<I: IntoIterator<Item = S>, S: ToString>(items: I) -> Value {
    let mut out: Vec<String> = items.into_iter().map(|s| s.to_string()).collect();
    out.sort();
    out.dedup();
    json!(out)
}

fn ids_of(content: &ContentStore, kind: ContentType) -> Vec<String> {
    content
        .records(kind)
        .iter()
        .filter_map(|r| r.id().map(ToOwned::to_owned))
        .collect()
}

fn main() {
    let content = ContentStore::embedded();
    let sources = DEFAULT;

    // Action cards: an effect (`effect_for`) and, for a reaction card, a window bound by its
    // printed `window` text (`reactions::window_for`).
    let mut action_windows = BTreeMap::new();
    for id in ids_of(content, ContentType::ActionCards) {
        let alias = ActionCardId::new(id.clone());
        let printed = content
            .get(ContentType::ActionCards, &id)
            .and_then(|r| r.text("window"))
            .unwrap_or_default()
            .trim()
            .to_owned();
        let window = ti4_engine_legacy::reactions::window_for(content, &alias);
        action_windows.insert(
            id,
            json!({
                "printed_window": printed,
                "bound_event": window.map(|w| w.event),
                "has_effect": ti4_engine_legacy::action_cards::effect_for(&alias).is_some(),
            }),
        );
    }
    let reachable = ti4_engine_legacy::reactions::reachable(content, sources);

    // Public objectives: a predicate or a price.
    let mut public = BTreeMap::new();
    for id in ids_of(content, ContentType::PublicObjectives) {
        let oid = ObjectiveId::new(id.clone());
        public.insert(
            id,
            json!({
                "requirement": ti4_engine_legacy::objectives::requirement_for(&oid).is_some(),
                "cost": ti4_engine_legacy::objectives::cost_of(&oid).is_some(),
            }),
        );
    }

    // Faction assets: every faction of the corpus, not only the six trained ones.
    let mut assets = BTreeMap::new();
    for faction in content.records(ContentType::Factions) {
        let Some(alias) = faction.id() else { continue };
        let rows: Vec<Value> = ti4_engine_legacy::factions::assets(content, sources, alias)
            .into_iter()
            .map(|asset| {
                json!({
                    "kind": format!("{:?}", asset.kind),
                    "id": asset.id,
                    "implemented": ti4_engine_legacy::factions::implemented(&asset),
                })
            })
            .collect();
        assets.insert(alias.to_owned(), json!(rows));
    }
    let all_factions: Vec<String> = content
        .records(ContentType::Factions)
        .iter()
        .filter_map(|r| r.id().map(ToOwned::to_owned))
        .collect();
    let faction_refs: Vec<&str> = all_factions.iter().map(String::as_str).collect();

    let decks = ti4_engine_legacy::deck::build_starting_decks(content, sources, 1);

    let out = json!({
        "sources": "DEFAULT",
        "action_cards": {
            "registered": strings(ti4_engine_legacy::action_cards::registered_aliases()),
            "unimplemented": strings(ti4_engine_legacy::action_cards::unimplemented(content).iter().map(|a| a.as_str().to_owned())),
            "reaction_reachable": strings(reachable.iter().map(|a| a.as_str().to_owned())),
            "windows": action_windows,
        },
        "agendas": {
            "registered": strings(ti4_engine_legacy::agenda_effects::registered_aliases()),
            "unimplemented": strings(ti4_engine_legacy::agenda_effects::unimplemented(content, sources)),
            "laws_unenforced": strings(ti4_engine_legacy::laws::unimplemented(content, sources)),
        },
        "explores": {
            "unimplemented": strings(ti4_engine_legacy::exploration::unimplemented(content, sources)),
        },
        "relics": {
            "registered": strings(ti4_engine_legacy::relics::registered_aliases()),
            "unimplemented": strings(ti4_engine_legacy::relics::unimplemented(content, sources).iter().map(|a| a.as_str().to_owned())),
        },
        "secret_objectives": {
            "registered": strings(ti4_engine_legacy::secrets::registered_aliases()),
            "unimplemented": strings(ti4_engine_legacy::secrets::unimplemented(content, sources).iter().map(|a| a.as_str().to_owned())),
        },
        "public_objectives": {
            "registered": strings(ti4_engine_legacy::objectives::registered_aliases()),
            "bought": strings(ti4_engine_legacy::objectives::bought_aliases()),
            "per_card": public,
        },
        "abilities": {
            "registered": strings(ti4_engine_legacy::faction_abilities::registered()),
            "unimplemented": strings(ti4_engine_legacy::faction_abilities::unimplemented(content, sources)),
        },
        "leaders": {
            "registered": strings(ti4_engine_legacy::leaders::registered_abilities()),
            "modifiers": strings(ti4_engine_legacy::leaders::modifiers().into_keys()),
            "unimplemented_all_factions": strings(ti4_engine_legacy::leaders::unimplemented(content, &faction_refs).iter().map(|a| a.as_str().to_owned())),
        },
        "breakthroughs": {
            "registered": strings(ti4_engine_legacy::breakthroughs::registered_aliases()),
            "module_claimed": strings(ti4_engine_legacy::factions::registered_breakthroughs()),
        },
        "faction_assets": assets,
        "decks_seed1": {
            "action_cards": strings(decks.action_cards.iter().map(|a| a.as_str().to_owned())),
            "agendas": strings(decks.agendas.iter()),
            "relics": strings(decks.relics.iter().map(|a| a.as_str().to_owned())),
            "secrets": strings(decks.secrets.iter().map(|a| a.as_str().to_owned())),
            "exploration": decks.exploration.iter().map(|(k, v)| (k.clone(), strings(v.iter()))).collect::<BTreeMap<_, _>>(),
        },
    });
    println!("{}", serde_json::to_string_pretty(&out).expect("json"));
}
