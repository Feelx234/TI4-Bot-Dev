//! Renderer of the `cards` area: `impl Family` for `ti4_engine_core::choice::cards` (prompt, context,
//! option, parse) and its entry in the registry (`render/mod.rs`).
//!
//! Owner: package B6 (docs/ENGINE_CORE_STATE.md 4.4). Ported: the own-hand question of
//! `action_cards::choose_from_own_hand`, byte for byte (prompt, subtype, kind `own_hand_card`, label = the
//! printed name).

use ti4_content::ContentStore;
use ti4_engine_core::choice::cards::{Ask, Opt, Reason, Subtype};
use ti4_model::id::ActionCardId;

use super::{Family, Render};
use crate::choice::ChoiceOption;
use crate::generated::IdStr;

/// The renderer of the card questions.
pub struct Fam;

impl Family for Fam {
    type Ask = Ask;
    type Opt = Opt;
    type Subtype = Subtype;

    fn prompt(r: &Render, a: &Ask) -> String {
        match a {
            Ask::OwnHand { reason } => match reason {
                Reason::StallTactics => "Stall Tactics: discard 1 action card from your hand".to_owned(),
                Reason::DeepgloomStallTactics => {
                    "Stall Tactics (allowed by Deepgloom Executable): discard 1 action card".to_owned()
                }
                Reason::MalleonDiscard => "Nekro Malleon: discard 1 action card".to_owned(),
                Reason::C4rDiscard => "???_ERROR_ERROR_???: discard 1 action card".to_owned(),
                Reason::Show { viewer } => format!("show {} 1 action card from your hand", r.seat_text(*viewer)),
            },
        }
    }

    fn subtype(_: &Render, s: &Subtype) -> String {
        match s {
            Subtype::StallTacticsDiscard => "stall_tactics_discard",
            Subtype::DeepgloomStallTacticsDiscard => "deepgloom_stall_tactics_discard",
            Subtype::MalleonDiscard => "malleon_discard",
            Subtype::C4rDiscard => "c4r_discard",
            Subtype::ShowActionCard => "show_action_card",
        }
        .to_owned()
    }

    fn option(_: &Render, _: &Ask, o: &Opt) -> ChoiceOption {
        match o {
            Opt::Card(card) => {
                let id = ActionCardId::new(card.as_str());
                ChoiceOption::labelled(card.as_str(), crate::action_cards::OWN_HAND_KIND, crate::action_cards::name_of(ContentStore::embedded(), &id))
            }
        }
    }

    fn parse(_: &Render, _: &Ask, id: &str) -> Option<Opt> {
        ti4_engine_core::generated::ActionCard::from_id(id).map(Opt::Card)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ti4_engine_core::generated::ActionCard;

    #[test]
    fn prompts_subtypes_and_options_render_as_the_string_engine_words_them() {
        let r = Render::new(vec!["a".to_owned(), "b".to_owned()]);
        let ask = Ask::OwnHand { reason: Reason::DeepgloomStallTactics };
        assert_eq!(Fam::prompt(&r, &ask), "Stall Tactics (allowed by Deepgloom Executable): discard 1 action card");
        assert_eq!(Fam::subtype(&r, &Subtype::C4rDiscard), "c4r_discard");
        let opt = Fam::option(&r, &ask, &Opt::Card(ActionCard::Bribery));
        assert_eq!((opt.id.as_str(), opt.kind.as_str(), opt.label.as_str()), ("bribery", "own_hand_card", "Bribery"));
        assert_eq!(Fam::parse(&r, &ask, "bribery"), Some(Opt::Card(ActionCard::Bribery)));
        assert_eq!(Fam::parse(&r, &ask, "nonsense"), None);
    }
}
