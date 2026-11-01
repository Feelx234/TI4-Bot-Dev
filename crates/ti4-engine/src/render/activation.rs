//! Renderer of the questions of the tactical-island abilities lane M6d ported: the words of the string engine, byte for byte. Called by
//! `render/reaction.rs` for `reaction::Ask::Act`.

use ti4_engine_core::choice::activation::{Ask, Awakening, Look, Opt, Subtype};
use ti4_engine_core::choice::reaction::Opt as ROpt;
use ti4_engine_core::generated::Planet;

use super::Render;
use crate::choice::ChoiceOption;
use crate::generated::IdStr;

pub(super) fn prompt(r: &Render, a: &Ask) -> String {
    match a {
        Ask::Awaken { planet } => format!("Awaken: the sleeper token on {}", planet.as_str()),
        Ask::Ouranos => "Ouranos: which PDS to replace".to_owned(),
        Ask::SoAta { target } => format!("So Ata: what to look at on {}", r.seat_text(*target)),
        Ask::L1z1xCopy => "I48S (Ssruu): replace which infantry in the active system?".to_owned(),
        Ask::MercerTake { n } => format!("Field Marshal Mercer: remove a ground force ({n} of 2)"),
        Ask::MercerPlace { unit } => format!("Field Marshal Mercer: place {} on a planet", unit.as_str()),
    }
}

pub(super) fn subtype(s: &Subtype) -> String {
    match s {
        Subtype::Awaken => "awaken_replace",
        Subtype::Ouranos => "ouranos_pds",
        Subtype::SoAta => "so_ata_look",
        Subtype::L1z1xCopy => "leader_l1z1xagent_copy_planet",
        Subtype::MercerTake => "mercer_take",
        Subtype::MercerPlace => "mercer_place",
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
        (Ask::MercerTake { .. }, Opt::Take { system, planet, unit, damaged }) => {
            let at = planet.map_or_else(|| format!("the space area of {}", system.as_str()), |p| p.as_str().to_owned());
            ChoiceOption::labelled(take_id(*system, *planet, *unit, *damaged), "mercer_take", format!("remove {} from {at}", unit.as_str())).with("unit", unit.as_str()).with("system", system.as_str())
        }
        (Ask::MercerPlace { unit }, Opt::Planet(p)) => {
            let system = p.data().system.map(|s| s.as_str()).unwrap_or_default();
            ChoiceOption::labelled(p.as_str(), "mercer_place", format!("place {} on {}", unit.as_str(), p.as_str())).with("unit", unit.as_str()).with("system", system)
        }
        (Ask::Ouranos, Opt::Planet(p)) => ChoiceOption::labelled(p.as_str(), "pds", format!("replace the PDS on {} with Ouranos", p.as_str())),
        _ => panic!("option {o:?} in the question {a:?}"),
    }
}

pub(super) fn parse(r: &Render, a: &Ask, id: &str) -> Option<ROpt> {
    let _ = r;
    match a {
        Ask::Awaken { .. } => match id {
            "pds" => Some(ROpt::Act(Opt::Awaken(Awakening::Pds))),
            "mech" => Some(ROpt::Act(Opt::Awaken(Awakening::MechAndInfantry))),
            _ => None,
        },
        Ask::MercerTake { .. } => {
            let mut parts = id.splitn(5, '|');
            let (Some("take"), Some(system), Some(place), Some(unit), Some(damaged)) = (parts.next(), parts.next(), parts.next(), parts.next(), parts.next()) else { return None };
            let planet = if place == "space" { None } else { Some(<Planet as IdStr>::from_id(place)?) };
            Some(ROpt::Act(Opt::Take { system: <ti4_engine_core::generated::System as IdStr>::from_id(system)?, planet, unit: <ti4_engine_core::generated::Unit as IdStr>::from_id(unit)?, damaged: damaged == "1" }))
        }
        Ask::Ouranos | Ask::L1z1xCopy | Ask::MercerPlace { .. } => <Planet as IdStr>::from_id(id).map(|p| ROpt::Act(Opt::Planet(p))),
        Ask::SoAta { .. } => match id {
            "action_cards" => Some(ROpt::Act(Opt::Look(Look::ActionCards))),
            "promissory_notes" => Some(ROpt::Act(Opt::Look(Look::PromissoryNotes))),
            "secret_objectives" => Some(ROpt::Act(Opt::Look(Look::SecretObjectives))),
            _ => None,
        },
    }
}

/// The id of the option that takes one ground force off the board (`nomad_agents::take_id`).
fn take_id(system: ti4_engine_core::generated::System, planet: Option<Planet>, unit: ti4_engine_core::generated::Unit, damaged: bool) -> String {
    format!("take|{}|{}|{}|{}", system.as_str(), planet.map_or("space", |p| p.as_str()), unit.as_str(), u8::from(damaged))
}
