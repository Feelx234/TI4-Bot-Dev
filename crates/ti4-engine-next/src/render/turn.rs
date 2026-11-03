//! Renderer of the `turn` area: `impl Family` for `ti4_engine_core::choice::turn` (prompt, subtype, option, parse) and its
//! entry in the registry (`render/mod.rs`). The action menu (`Game::turn_options`) and the closing menu (`closing_options`):
//! every source of the menu has typed options (package KM0): `pass`, `tactical`, the strategic actions, the transactions, the relics, the
//! exploration cards, the technologies, the faction and leader component actions, the action cards and `end_turn`. Only the options of
//! dropped content (contacts, payments, the expedition) and of legacy ids are `Opt::Lent`: they render as a placeholder the dual
//! replaces with the option the string lists.

use ti4_engine_core::choice::turn::{Ask, Opt, Subtype};

use super::{Family, Render};
use crate::wire::choice::ChoiceOption;
use crate::ties::game::{END_TURN_ID, END_TURN_KIND, TACTICAL_ACTION_ID};
use crate::generated::IdStr;
use crate::ties::strategy::{ACTION_KIND, STRATEGIC_ACTION_ID};

/// The id a lent option renders with until the adapter puts the string's option in its place.
pub const LENT_ID: &str = "lent";

/// The id of a leader's component action, and that of a copy through Ssruu (`leaders::component_actions`, `borrowed_component_actions`).
const LEADER_PREFIX: &str = "component|leader|";
const BORROWED: &str = "yssarilagent|";
/// The id of a technology's component action (`technology::component_actions`).
const TECH_PREFIX: &str = "component|tech|";

/// The ids of the relic, exploration, action-card and transaction options (`relics`, `exploration`, `action_cards`, `transactions`).
const PURGE_PREFIX: &str = "purge|";
const RELIC_PREFIX: &str = "relic|";
const DEVICE_PREFIX: &str = "play_area:";
const PLAY_PREFIX: &str = "action_card|";
const SALVAGED_PREFIX: &str = "action_card|salvaged|";
const OPEN_PREFIX: &str = "component|trade|";
/// The kind of every component action of the sources above (`relics::ACTION_KIND`, `action_cards::ACTION_KIND`).
const COMPONENT_KIND: &str = "component";

/// The printed name of an action card, or its id.
fn card_name(card: ti4_engine_core::generated::ActionCard) -> String {
    crate::ties::action_cards::name_of(ti4_content::ContentStore::embedded(), &ti4_model::id::ActionCardId::new(card.as_str()))
}

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

    fn option(r: &Render, _: &Ask, o: &Opt) -> ChoiceOption {
        match o {
            Opt::Pass => ChoiceOption::labelled("pass", ACTION_KIND, "pass"),
            Opt::Tactical => ChoiceOption::labelled(TACTICAL_ACTION_ID, ACTION_KIND, "take a tactical action"),
            Opt::Strategic { bare: true, .. } => ChoiceOption::labelled(STRATEGIC_ACTION_ID, ACTION_KIND, "take your strategic action"),
            Opt::Strategic { card, bare: false } => ChoiceOption::labelled(
                format!("{STRATEGIC_ACTION_ID}|{}", card.as_str()),
                ACTION_KIND,
                format!("take the strategic action of {}", crate::ties::draft::strategy_card_label(ti4_content::ContentStore::embedded(), card.as_str())),
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
            Opt::Purge(kind) => ChoiceOption::labelled(
                format!("{PURGE_PREFIX}{}", kind.as_str().to_ascii_uppercase()),
                COMPONENT_KIND,
                format!("purge 3 {} relic fragments for a relic", kind.as_str().to_ascii_lowercase()),
            ),
            Opt::UseRelic(relic) => ChoiceOption::labelled(format!("{RELIC_PREFIX}{}", relic.as_str()), COMPONENT_KIND, format!("use {}", relic.as_str())),
            Opt::Device(card) => ChoiceOption::labelled(
                format!("{DEVICE_PREFIX}{}", card.as_str()),
                COMPONENT_KIND,
                "Enigmatic Device: spend 6 resources to research a technology",
            ),
            Opt::Play { index, card } => ChoiceOption::labelled(format!("{PLAY_PREFIX}{index}"), COMPONENT_KIND, format!("play {}", card_name(*card))),
            Opt::Salvaged(card) => ChoiceOption::labelled(
                format!("{SALVAGED_PREFIX}{}", card.as_str()),
                COMPONENT_KIND,
                format!("play {} off the salvage yard", card_name(*card)),
            ),
            Opt::Open { faction, .. } => ChoiceOption::labelled(
                format!("{OPEN_PREFIX}{}", faction.as_str()),
                crate::ties::transactions::OPEN_KIND,
                format!("open a transaction with {}", faction.as_str()),
            ),
            Opt::Faction(action) => super::component::option(r, *action),
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
                // The relic, exploration and salvage options name what they act on. (`action_card|<index>`, a transaction and a faction
                // action name their card or partner only through the position: the option-id fallback of the live loop reads them.)
                if let Some(kind) = id.strip_prefix(PURGE_PREFIX) {
                    return ti4_engine_core::generated::ExploreType::ALL.iter().copied().find(|k| k.as_str().eq_ignore_ascii_case(kind)).map(Opt::Purge);
                }
                if let Some(relic) = id.strip_prefix(RELIC_PREFIX) {
                    return ti4_engine_core::generated::Relic::from_id(relic).map(Opt::UseRelic);
                }
                if let Some(card) = id.strip_prefix(DEVICE_PREFIX) {
                    return ti4_engine_core::generated::Explore::from_id(card).map(Opt::Device);
                }
                if let Some(card) = id.strip_prefix(SALVAGED_PREFIX) {
                    return ti4_engine_core::generated::ActionCard::from_id(card).map(Opt::Salvaged);
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

#[cfg(test)]
mod tests {
    use super::*;
    use ti4_engine_core::generated::{Leader, StrategyCard, System, Technology};

    fn parse(id: &str) -> Option<Opt> {
        Fam::parse(&Render::new(Vec::new()), &Ask::ActionMenu, id)
    }

    fn option(o: Opt) -> ChoiceOption {
        Fam::option(&Render::new(Vec::new()), &Ask::ActionMenu, &o)
    }

    #[test]
    fn every_native_option_parses_back_from_the_id_it_renders() {
        let card = StrategyCard::ALL[1];
        let leader = Leader::ALL[0];
        let borrowed = Leader::ALL[3];
        for o in [Opt::Pass, Opt::Tactical, Opt::EndTurn, Opt::Strategic { card, bare: false }, Opt::Leader(leader), Opt::Borrowed(borrowed), Opt::Tech(Technology::SlingRelay)] {
            assert_eq!(parse(&option(o).id), Some(o), "{o:?}");
        }
        // The lone card's bare id names no card: the position resolves it.
        assert_eq!(parse(STRATEGIC_ACTION_ID), None);
        assert_eq!(parse("component|leader|no-such-leader"), None);
        assert_eq!(parse("component|leader|yssarilagent|no-such-leader"), None);
        assert_eq!(parse("component|tech|no-such-tech"), None);
        assert_eq!(parse("faction|orbital_drop"), None, "an option of a source that is lent");
        let token = System::ALL[0];
        assert_eq!(Fam::parse(&Render::new(Vec::new()), &Ask::MinisterToken, token.as_str()), Some(Opt::Token(token)));
        assert_eq!(parse(token.as_str()), None, "a system is an answer to the Minister's question only");
    }

    #[test]
    fn the_leaders_options_are_labelled_as_the_string_labels_them() {
        let o = option(Opt::Leader(Leader::CarthOfGoldenSands));
        assert_eq!((o.id.as_str(), o.kind.as_str(), o.label.as_str()), ("component|leader|hacanagent", "component", "Carth of Golden Sands"));
        let o = option(Opt::Borrowed(Leader::CarthOfGoldenSands));
        assert_eq!((o.id.as_str(), o.label.as_str()), ("component|leader|yssarilagent|hacanagent", "Ssruu: use Carth of Golden Sands's ability"));
        let o = option(Opt::Tech(Technology::SlingRelay));
        assert_eq!((o.id.as_str(), o.kind.as_str(), o.label.as_str()), ("component|tech|sr", "component", "use Sling Relay"));
    }
}
