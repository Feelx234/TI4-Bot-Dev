//! Renderer of the `turn` area: `impl Family` for `ti4_engine_core::choice::turn` (prompt, subtype, option, parse) and its
//! entry in the registry (`render/mod.rs`). The action menu (`Game::turn_options`) and the closing menu (`closing_options`):
//! the native options are `pass`, `tactical`, the strategic actions, the leaders' component actions and `end_turn`; the options of the sources that are
//! lent as reads render as a placeholder the adapter replaces with the option the string lists (`lent_options`).

use ti4_engine_core::choice::turn::{Ask, Opt, Subtype};

use super::{Family, Render};
use crate::choice::ChoiceOption;
use crate::game::{END_TURN_ID, END_TURN_KIND, TACTICAL_ACTION_ID};
use crate::generated::IdStr;
use crate::strategy::{ACTION_KIND, STRATEGIC_ACTION_ID};

/// The id a lent option renders with until the adapter puts the string's option in its place.
pub const LENT_ID: &str = "lent";

/// The id of a leader's component action, and that of a copy through Ssruu (`leaders::component_actions`, `borrowed_component_actions`).
const LEADER_PREFIX: &str = "component|leader|";
const BORROWED: &str = "yssarilagent|";
/// The id of a technology's component action (`technology::component_actions`).
const TECH_PREFIX: &str = "component|tech|";

/// The label of a technology's component action: Sling Relay's is the engine's own text.
fn tech_label(tech: ti4_engine_core::generated::Technology) -> String {
    match tech {
        ti4_engine_core::generated::Technology::SlingRelay => "use Sling Relay".to_owned(),
        other => format!("use {}", other.as_str()),
    }
}

/// The printed name of a leader, or its id: the label of its component action.
fn leader_name(leader: ti4_engine_core::generated::Leader) -> String {
    ti4_content::ContentStore::embedded()
        .get(ti4_model::ContentType::Leaders, leader.as_str())
        .and_then(|record| record.text("name"))
        .unwrap_or(leader.as_str())
        .to_owned()
}

pub struct Fam;

impl Family for Fam {
    type Ask = Ask;
    type Opt = Opt;
    type Subtype = Subtype;

    fn prompt(_: &Render, a: &Ask) -> String {
        match a {
            Ask::ActionMenu => "action phase".to_owned(),
            Ask::Closing { second: true } => "end your turn, or take your second action".to_owned(),
            Ask::Closing { second: false } => "end your turn".to_owned(),
            Ask::MinisterToken => "which command token comes back".to_owned(),
        }
    }

    fn subtype(_: &Render, s: &Subtype) -> String {
        match s {
            Subtype::ActionMenu => "action_menu".to_owned(),
            Subtype::EndTurn => "end_turn".to_owned(),
            Subtype::MinisterToken => "minister_token".to_owned(),
        }
    }

    fn option(_: &Render, _: &Ask, o: &Opt) -> ChoiceOption {
        match o {
            Opt::Pass => ChoiceOption::labelled("pass", ACTION_KIND, "pass"),
            Opt::Tactical => ChoiceOption::labelled(TACTICAL_ACTION_ID, ACTION_KIND, "take a tactical action"),
            Opt::Strategic { bare: true, .. } => ChoiceOption::labelled(STRATEGIC_ACTION_ID, ACTION_KIND, "take your strategic action"),
            Opt::Strategic { card, bare: false } => ChoiceOption::labelled(
                format!("{STRATEGIC_ACTION_ID}|{}", card.as_str()),
                ACTION_KIND,
                format!("take the strategic action of {}", crate::draft::strategy_card_label(ti4_content::ContentStore::embedded(), card.as_str())),
            ),
            Opt::EndTurn => ChoiceOption::labelled(END_TURN_ID, END_TURN_KIND, "end your turn"),
            Opt::Lent(_) => ChoiceOption::labelled(LENT_ID, "lent", "lent"),
            Opt::Leader(leader) => ChoiceOption::labelled(format!("{LEADER_PREFIX}{}", leader.as_str()), "component", leader_name(*leader)),
            Opt::Tech(tech) => ChoiceOption::labelled(format!("{TECH_PREFIX}{}", tech.as_str()), "component", tech_label(*tech)),
            Opt::Borrowed(agent) => ChoiceOption::labelled(
                format!("{LEADER_PREFIX}{BORROWED}{}", agent.as_str()),
                "component",
                format!("Ssruu: use {}'s ability", leader_name(*agent)),
            ),
            Opt::Token(system) => ChoiceOption::labelled(system.as_str(), "system", format!("retrieve your token from {}", system.as_str())),
        }
    }

    fn parse(_: &Render, a: &Ask, id: &str) -> Option<Opt> {
        if *a == Ask::MinisterToken {
            return ti4_engine_core::generated::System::from_id(id).map(Opt::Token);
        }
        match id {
            "pass" => Some(Opt::Pass),
            TACTICAL_ACTION_ID => Some(Opt::Tactical),
            END_TURN_ID => Some(Opt::EndTurn),
            _ => {
                if let Some(tech) = id.strip_prefix(TECH_PREFIX) {
                    return ti4_engine_core::generated::Technology::from_id(tech).map(Opt::Tech);
                }
                if let Some(rest) = id.strip_prefix(LEADER_PREFIX) {
                    return match rest.strip_prefix(BORROWED) {
                        Some(agent) => ti4_engine_core::generated::Leader::from_id(agent).map(Opt::Borrowed),
                        None => ti4_engine_core::generated::Leader::from_id(rest).map(Opt::Leader),
                    };
                }
                // A lone unused card keeps the bare id; which card it is the position's to say (the caller resolves it).
                id.strip_prefix(&format!("{STRATEGIC_ACTION_ID}|"))
                    .and_then(ti4_engine_core::generated::StrategyCard::from_id)
                    .map(|card| Opt::Strategic { card, bare: false })
            }
        }
    }
}
