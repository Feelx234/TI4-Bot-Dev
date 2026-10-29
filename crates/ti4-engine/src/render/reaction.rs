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
use ti4_engine_core::choice::reaction::{Ask, Opt, Relation, Subtype, Take};
use ti4_engine_core::generated::ActionCard;

use super::{Family, Render};
use crate::generated::IdStr;
use crate::choice::ChoiceOption;
use crate::kernel_sync::text::{ability_id, ability_text, event_text, relation_text};

/// The renderer of the reaction windows.
pub struct Fam;

impl Family for Fam {
    type Ask = Ask;
    type Opt = Opt;
    type Subtype = Subtype;

    fn prompt(r: &Render, a: &Ask) -> String {
        match a {
            Ask::Window { event, relation } => format!("{} {}", relation_text(*relation), event_text(r, *event)),
            Ask::PlayCard { event, relation } => format!("play an action card ({} {})", relation_text(*relation), event_text(r, *event)),
            Ask::InstinctTraining { card } => format!("Instinct Training: exhaust and spend a strategy token to cancel {}", card.as_str()),
            Ask::Legionnaire { .. } => "Crimson Legionnaire: gain 1 commodity or convert 1 to a trade good".to_owned(),
            Ask::Cavalry => "The Cavalry: which non-fighter ship takes the Nomad flagship's abilities".to_owned(),
            Ask::Assimilate => "gain a technology, or assimilate instead".to_owned(),
            Ask::AgentSwap { target } => format!(
                "I48S: exhaust to let {} replace 1 infantry in the active system with a mech",
                target.map_or_else(|| "the active player".to_owned(), |t| r.seat_text(t).to_owned())
            ),
        }
    }

    fn subtype(r: &Render, s: &Subtype) -> String {
        match s {
            Subtype::Window { event, relation } => format!("reaction_{}_{}", relation_text(*relation), event_text(r, *event)),
            Subtype::PlayCard { event, relation } => format!("play_reaction_{}_{}", relation_text(*relation), event_text(r, *event)),
            Subtype::InstinctTraining => "instinct_training_cancel".to_owned(),
            Subtype::AgentSwap => "l1z1x_agent_swap".to_owned(),
            Subtype::Legionnaire => "legionnaire_payment".to_owned(),
            Subtype::Cavalry => "cavalry_ship".to_owned(),
            Subtype::Assimilate => "assimilate".to_owned(),
        }
    }

    fn option(r: &Render, a: &Ask, o: &Opt) -> ChoiceOption {
        match (a, o) {
            (Ask::Window { event, .. }, Opt::Ability(id)) => {
                let text = ability_text(r, *id);
                let label = crate::timing::ability_label(Some(ContentStore::embedded()), &text);
                ChoiceOption::labelled(&text, "ability", label).with("event", event_text(r, *event))
            }
            (Ask::PlayCard { .. }, Opt::Card(card)) => {
                let content = ContentStore::embedded();
                let id = card.as_str();
                let name = crate::action_cards::name_of(content, &ti4_model::id::ActionCardId::new(id));
                ChoiceOption::labelled(id, crate::reactions::ACTION_CARD_KIND, format!("play {name}")).with("card", id).with("card_name", name)
            }
            (Ask::InstinctTraining { .. }, Opt::Use) => ChoiceOption::labelled("use", "technology", "cancel it"),
            (Ask::AgentSwap { .. }, Opt::Use) => ChoiceOption::labelled("use", "leader", "exhaust the agent"),
            (Ask::Cavalry, Opt::Ship(unit)) => {
                let id = unit.as_str();
                ChoiceOption::labelled(id, "cavalry_ship", format!("treat a {id} as the Nomad flagship")).with("unit", id)
            }
            (Ask::Legionnaire { .. }, Opt::Gain) => ChoiceOption::labelled("gain", "economy", "gain 1 commodity"),
            (Ask::Legionnaire { .. }, Opt::Convert) => ChoiceOption::labelled("convert", "economy", "convert 1 commodity to a trade good"),
            (Ask::Assimilate, Opt::Take { how, tech, source }) => {
                let (tech_id, source) = (tech.as_str(), r.seat_text(*source));
                let name = crate::technology::name(ContentStore::embedded(), &ti4_model::id::TechnologyId::new(tech_id));
                let (kind, label) = match how {
                    Take::Gain => ("gain", format!("gain {name} from {source}")),
                    Take::TokenX => ("x", format!("place the X token on {name} of {source}")),
                    Take::TokenY => ("y", format!("place the Y token on {name} of {source}")),
                };
                ChoiceOption::labelled(format!("{kind}|{source}|{tech_id}"), "nekro_assimilate", label)
            }
            _ => panic!("option {o:?} in the question {a:?}"),
        }
    }

    fn parse(r: &Render, a: &Ask, id: &str) -> Option<Opt> {
        if let Ask::PlayCard { .. } = a {
            return ActionCard::from_id(id).map(Opt::Card);
        }
        if let Ask::Cavalry = a {
            return <ti4_engine_core::generated::Unit as IdStr>::from_id(id).map(Opt::Ship);
        }
        if let Ask::Assimilate = a {
            let mut parts = id.splitn(3, '|');
            let (kind, source, tech) = (parts.next()?, parts.next()?, parts.next()?);
            let how = match kind {
                "gain" => Take::Gain,
                "x" => Take::TokenX,
                "y" => Take::TokenY,
                _ => return None,
            };
            return Some(Opt::Take {
                how,
                tech: <ti4_engine_core::generated::Technology as IdStr>::from_id(tech)?,
                source: r.seat(source)?,
            });
        }
        if let Ask::Legionnaire { .. } = a {
            return match id {
                "gain" => Some(Opt::Gain),
                "convert" => Some(Opt::Convert),
                _ => None,
            };
        }
        if !matches!(a, Ask::Window { .. }) {
            return (id == "use").then_some(Opt::Use);
        }
        // An ability id: `kind:...:EVENT:when|after|produce`, known to the interner (registered).
        let parts: Vec<&str> = id.split(':').collect();
        if parts.len() < 4 {
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
