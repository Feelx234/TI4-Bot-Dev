//! Renderer of the `cards` area: `impl Family` for `ti4_engine_core::choice::cards` (prompt, context,
//! option, parse) and its entry in the registry (`render/mod.rs`).
//!
//! Owner: package B6 (docs/ENGINE_CORE_STATE.md 4.4). Ported: the own-hand question of
//! `action_cards::choose_from_own_hand`, byte for byte (prompt, subtype, kind `own_hand_card`, label = the
//! printed name).

use ti4_content::ContentStore;
use ti4_engine_core::choice::cards::{Ask, Opt, Reason, Subtype, What};
use ti4_engine_core::generated::{ActionCard, Planet, System};
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
            Ask::Pick { card, what } => pick_prompt(*card, *what).to_owned(),
            Ask::Predict { card, .. } => format!("{}: predict the agenda outcome", rider_name(*card)),
        }
    }

    fn subtype(_: &Render, s: &Subtype) -> String {
        match s {
            Subtype::StallTacticsDiscard => "stall_tactics_discard",
            Subtype::DeepgloomStallTacticsDiscard => "deepgloom_stall_tactics_discard",
            Subtype::MalleonDiscard => "malleon_discard",
            Subtype::C4rDiscard => "c4r_discard",
            Subtype::ShowActionCard => "show_action_card",
            Subtype::PredictOutcome => "predict_agenda_outcome",
            Subtype::Pick { card, what } => {
                return match what {
                    What::Elect => "confusing_legal_text_elect".to_owned(),
                    What::Silence => "silence_choose_system".to_owned(),
                    _ => format!("{}_pick_{}", card.as_str(), pick_kind(*what)),
                };
            }
        }
        .to_owned()
    }

    fn option(r: &Render, a: &Ask, o: &Opt) -> ChoiceOption {
        match o {
            Opt::Seat(seat) => {
                let id = r.seat_text(*seat);
                let (card, what) = pick_of(a);
                ChoiceOption::labelled(id, pick_kind(what), format!("{}{id}", seat_label(card, what)))
            }
            Opt::System(system) => {
                let id = system.as_str();
                match pick_of(a).1 {
                    What::Silence => ChoiceOption::labelled(id, "silence", format!("ships in {id} may pass blockades")),
                    What::Bombard => ChoiceOption::labelled(id, "system", format!("bombard {id}")),
                    What::Deploy => ChoiceOption::labelled(id, "system", format!("cruiser into {id}")),
                    _ => ChoiceOption::labelled(id, "recall", format!("recall your token from {id}")),
                }
            }
            Opt::Spot(system, planet) => ChoiceOption::labelled(format!("{}|{}", system.as_str(), planet.as_str()), "planet", planet.as_str())
                .with_planet(planet.as_str(), Some(system.as_str())),
            Opt::Note(key) => {
                let id = note_text(*key);
                ChoiceOption::labelled(id.clone(), "promissory note", id)
            }
            Opt::Outcome(outcome) => {
                let Ask::Predict { frame, .. } = a else { return ChoiceOption::labelled("", "", "") };
                let named = super::agenda::outcome_label(r, frame, *outcome);
                ChoiceOption::labelled(super::agenda::outcome_text(r, *outcome), "prediction", format!("predict {named}"))
            }
            Opt::Card(card) => {
                let id = ActionCardId::new(card.as_str());
                ChoiceOption::labelled(card.as_str(), crate::action_cards::OWN_HAND_KIND, crate::action_cards::name_of(ContentStore::embedded(), &id))
            }
        }
    }

    fn parse(r: &Render, a: &Ask, id: &str) -> Option<Opt> {
        match a {
            Ask::OwnHand { .. } => ActionCard::from_id(id).map(Opt::Card),
            Ask::Pick { what: What::Player | What::Elect, .. } => r.seat(id).map(Opt::Seat),
            Ask::Pick { what: What::Recall | What::Silence | What::Bombard | What::Deploy, .. } => System::from_id(id).map(Opt::System),
            Ask::Pick { what: What::Planet, .. } => {
                let (system, planet) = id.split_once('|')?;
                Some(Opt::Spot(System::from_id(system)?, Planet::from_id(planet)?))
            }
            Ask::Pick { what: What::Note, .. } => parse_note(id).map(Opt::Note),
            Ask::Predict { frame, .. } => super::agenda::parse_outcome(r, frame, id).map(Opt::Outcome),
        }
    }
}

/// The `kind` of a pick question's options.
const fn pick_kind(what: What) -> &'static str {
    match what {
        What::Player => "player",
        What::Elect => "elect",
        What::Recall => "recall",
        What::Bombard | What::Deploy => "system",
        What::Silence => "silence",
        What::Note => "promissory note",
        What::Planet => "planet",
    }
}

fn pick_of(a: &Ask) -> (ActionCard, What) {
    match a {
        Ask::Pick { card, what } => (*card, *what),
        Ask::Predict { card, .. } => (*card, What::Player),
        Ask::OwnHand { .. } => (ActionCard::Spy, What::Player),
    }
}

/// The prompt of a card's pick question, as `action_cards.rs` words it.
const fn pick_prompt(card: ActionCard, what: What) -> &'static str {
    use ActionCard as C;
    match (card, what) {
        (C::Insubordination, _) => "Insubordination: whose tactic pool",
        (C::AssassinateRepresentative, _) => "Assassinate Representative: choose 1 player",
        (C::AncientBurialSites, _) => "Ancient Burial Sites: choose 1 player",
        (C::ForwardSupplyBase, _) => "Forward Supply Base: choose another player",
        (C::ConfusingLegalText, _) => "Confusing Legal Text: who is the elected player instead",
        (C::UnexpectedAction, _) => "Unexpected Action: recall your token from where",
        (C::MiningInitiative, _) => "Mining Initiative: mine which planet",
        (C::WarEffort, _) => "War Effort: into which system",
        (C::FrontlineDeployment, _) => "Frontline Deployment: onto which planet",
        (C::TacticalBombardment, _) => "Tactical Bombardment: which system",
        (C::InTheSilenceOfSpace, _) => "In The Silence Of Space: whose ships ignore blockades",
        (C::DiplomaticPressure1 | C::DiplomaticPressure2 | C::DiplomaticPressure3 | C::DiplomaticPressure4, What::Note) => {
            "Diplomatic Pressure: which note do you give"
        }
        (C::DiplomaticPressure1 | C::DiplomaticPressure2 | C::DiplomaticPressure3 | C::DiplomaticPressure4, _) => {
            "Diplomatic Pressure: choose another player"
        }
        (C::Uprising, _) => "Uprising: which planet",
        (C::CrippleDefenses, _) => "Cripple Defenses: which planet",
        (C::ReactorMeltdown, _) => "Reactor Meltdown: which space dock",
        _ => "",
    }
}

/// The label prefix of a seat option.
const fn seat_label(card: ActionCard, what: What) -> &'static str {
    use ActionCard as C;
    match (card, what) {
        (_, What::Elect) => "elect ",
        (C::Insubordination, _) => "take a tactic token from ",
        (C::AssassinateRepresentative, _) => "assassinate ",
        (C::AncientBurialSites, _) => "target ",
        (C::ForwardSupplyBase, _) => "pay ",
        (C::DiplomaticPressure1 | C::DiplomaticPressure2 | C::DiplomaticPressure3 | C::DiplomaticPressure4, _) => "pressure ",
        _ => "",
    }
}

/// The name a Rider's prediction prompt starts with.
const fn rider_name(card: ActionCard) -> &'static str {
    use ActionCard as C;
    match card {
        C::ImperialRider => "Imperial Rider",
        C::ConstructionRider => "Construction Rider",
        C::DiplomacyRider => "Diplomacy Rider",
        C::LeadershipRider => "Leadership Rider",
        C::PoliticsRider => "Politics Rider",
        C::TechnologyRider => "Technology Rider",
        C::TradeRider => "Trade Rider",
        C::WarfareRider => "Warfare Rider",
        _ => "Sanction",
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

/// The state's text of a promissory note (`"{alias}:{faction}"`).
pub(crate) fn note_text(key: ti4_engine_core::state::NoteKey) -> String {
    format!("{}:{}", crate::core_sync::keys::text::note_text(key.note), key.faction.as_str())
}

/// The note a state text names.
fn parse_note(id: &str) -> Option<ti4_engine_core::state::NoteKey> {
    let (alias, faction) = id.split_once(':')?;
    let note = crate::core_sync::keys::text::parse_note(alias)?;
    let key = ti4_engine_core::state::NoteKey { note, faction: ti4_engine_core::generated::Faction::from_id(faction)? };
    (note_text(key) == id).then_some(key)
}
