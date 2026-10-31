//! Renderer of the questions of the tactical-island abilities lane M6d ported: the words of the string engine, byte for byte. Called by
//! `render/reaction.rs` for `reaction::Ask::Act`.

use ti4_engine_core::choice::activation::{Ask, Awakening, Look, Opt, Subtype};
use ti4_engine_core::choice::reaction::Opt as ROpt;

use super::Render;
use crate::choice::ChoiceOption;
use crate::generated::IdStr;

pub(super) fn prompt(r: &Render, a: &Ask) -> String {
    match a {
        Ask::Awaken { planet } => format!("Awaken: the sleeper token on {}", planet.as_str()),
        Ask::Ouranos => "Ouranos: which PDS to replace".to_owned(),
        Ask::SoAta { target } => format!("So Ata: what to look at on {}", r.seat_text(*target)),
        Ask::L1z1xCopy => "I48S (Ssruu): replace which infantry in the active system?".to_owned(),
    }
}

pub(super) fn subtype(s: &Subtype) -> String {
    match s {
        Subtype::Awaken => "awaken_replace",
        Subtype::Ouranos => "ouranos_pds",
        Subtype::SoAta => "so_ata_look",
        Subtype::L1z1xCopy => "leader_l1z1xagent_copy_planet",
    }
    .to_owned()
}

pub(super) fn option(r: &Render, a: &Ask, o: &Opt) -> ChoiceOption {
    match (a, o) {
        (Ask::Awaken { planet }, Opt::Awaken(Awakening::Pds)) => ChoiceOption::labelled("pds", "awaken", format!("replace the sleeper token on {} with 1 PDS", planet.as_str())),
        (Ask::Awaken { planet }, Opt::Awaken(Awakening::MechAndInfantry)) => {
            ChoiceOption::labelled("mech", "awaken", format!("replace the sleeper token on {} with 1 mech and 1 infantry (Hecatoncheires)", planet.as_str()))
        }
        (Ask::SoAta { target }, Opt::Look(look)) => {
            let (id, label) = match look {
                Look::ActionCards => ("action_cards", "action cards"),
                Look::PromissoryNotes => ("promissory_notes", "promissory notes"),
                Look::SecretObjectives => ("secret_objectives", "secret objectives"),
            };
            ChoiceOption::labelled(id, "commander", format!("look at {}'s {label}", r.seat_text(*target)))
        }
        (Ask::L1z1xCopy, Opt::Planet(p)) => {
            let content = ti4_content::ContentStore::embedded();
            let name = ti4_content::galaxy::planet(content, p.as_str(), ti4_model::content_types::FULL).and_then(|record| record.name()).unwrap_or(p.as_str());
            ChoiceOption::labelled(p.as_str(), "leader_l1z1xagent_copy_planet", format!("replace infantry on {name} with a mech")).with_planet(p.as_str(), p.data().system.map(|s| s.as_str()))
        }
        (Ask::Ouranos, Opt::Planet(p)) => ChoiceOption::labelled(p.as_str(), "pds", format!("replace the PDS on {} with Ouranos", p.as_str())),
        _ => panic!("option {o:?} in the question {a:?}"),
    }
}

pub(super) fn parse(r: &Render, a: &Ask, id: &str) -> Option<ROpt> {
    use ti4_engine_core::generated::Planet;
    let _ = r;
    match a {
        Ask::Awaken { .. } => match id {
            "pds" => Some(ROpt::Act(Opt::Awaken(Awakening::Pds))),
            "mech" => Some(ROpt::Act(Opt::Awaken(Awakening::MechAndInfantry))),
            _ => None,
        },
        Ask::Ouranos | Ask::L1z1xCopy => <Planet as IdStr>::from_id(id).map(|p| ROpt::Act(Opt::Planet(p))),
        Ask::SoAta { .. } => match id {
            "action_cards" => Some(ROpt::Act(Opt::Look(Look::ActionCards))),
            "promissory_notes" => Some(ROpt::Act(Opt::Look(Look::PromissoryNotes))),
            "secret_objectives" => Some(ROpt::Act(Opt::Look(Look::SecretObjectives))),
            _ => None,
        },
    }
}
