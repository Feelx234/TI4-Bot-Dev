//! Renderer of the `turn` area: `impl Family` for `ti4_engine_core::choice::turn` (prompt, subtype, option, parse) and its
//! entry in the registry (`render/mod.rs`). The action menu (`Game::turn_options`) and the closing menu (`closing_options`):
//! the native options are `pass`, `tactical`, the strategic actions and `end_turn`; the options of the sources that are
//! lent as reads render as a placeholder the adapter replaces with the option the string lists (`lent_options`).

use ti4_engine_core::choice::turn::{Ask, Opt, Subtype};

use super::{Family, Render};
use crate::choice::ChoiceOption;
use crate::game::{END_TURN_ID, END_TURN_KIND, TACTICAL_ACTION_ID};
use crate::generated::IdStr;
use crate::strategy::{ACTION_KIND, STRATEGIC_ACTION_ID};

/// The id a lent option renders with until the adapter puts the string's option in its place.
pub const LENT_ID: &str = "lent";

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
            // A lone unused card keeps the bare id; which card it is the position's to say (the caller resolves it).
            _ => id
                .strip_prefix(&format!("{STRATEGIC_ACTION_ID}|"))
                .and_then(ti4_engine_core::generated::StrategyCard::from_id)
                .map(|card| Opt::Strategic { card, bare: false }),
        }
    }
}
