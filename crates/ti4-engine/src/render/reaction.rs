//! Renderer of the `reaction` area: `impl Family` for `ti4_engine_core::choice::reaction` (prompt, context,
//! option, parse) and its entry in the registry (`render/mod.rs`). Owner: package A5.
//!
//! The window question is `"{when|after} {EVENT}"` with subtype `reaction_{when|after}_{EVENT}`; an
//! option is a registered ability (id = its registered text, kind `ability`, label = the printed name
//! plus the window in words, payload `{event}`), then the generic decline.
//!
//! Not represented in the typed ask, and so not produced here: the `reaction:` slot's card payload
//! (`cards`, `card_names`, `card`, `card_name`), the previews and the trigger of the context. Those are
//! display facts built from the state at the moment the window opens; the string resolver still builds
//! them, and the dual harness compares the identities and order of the options, not these.

use serde_json::Value;
use ti4_content::ContentStore;
use ti4_engine_core::choice::reaction::{Ask, Opt, Relation, Subtype};

use super::{Family, Render};
use crate::choice::ChoiceOption;
use crate::kernel_sync::text::{ability_id, ability_text, event_text, relation_text};

/// The renderer of the reaction windows.
pub struct Fam;

impl Family for Fam {
    type Ask = Ask;
    type Opt = Opt;
    type Subtype = Subtype;

    fn prompt(r: &Render, a: &Ask) -> String {
        let Ask::Window { event, relation } = a;
        format!("{} {}", relation_text(*relation), event_text(r, *event))
    }

    fn subtype(r: &Render, s: &Subtype) -> String {
        let Subtype::Window { event, relation } = s;
        format!("reaction_{}_{}", relation_text(*relation), event_text(r, *event))
    }

    fn option(r: &Render, a: &Ask, o: &Opt) -> ChoiceOption {
        let (Ask::Window { event, .. }, Opt::Ability(id)) = (a, o);
        let text = ability_text(r, *id);
        let label = crate::timing::ability_label(Some(ContentStore::embedded()), &text);
        ChoiceOption::labelled(&text, "ability", label).with("event", event_text(r, *event))
    }

    fn parse(r: &Render, _: &Ask, id: &str) -> Option<Opt> {
        // An ability id: `kind:...:EVENT:when|after`, known to the interner (registered).
        let parts: Vec<&str> = id.split(':').collect();
        if parts.len() < 4 || !matches!(parts[parts.len() - 1], "when" | "after") {
            return None;
        }
        r.find_code(id)?;
        Some(Opt::Ability(ability_id(r, id)))
    }

    fn details(_: &Render, _: &Ask) -> serde_json::Map<String, Value> {
        serde_json::Map::new()
    }
}

/// The relation of a lowercase word.
#[must_use]
pub fn relation_of(word: &str) -> Option<Relation> {
    match word {
        "when" => Some(Relation::When),
        "after" => Some(Relation::After),
        _ => None,
    }
}
