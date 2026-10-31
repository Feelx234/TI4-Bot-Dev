//! Renderer of the questions of lane G3 (`ti4_engine_core::choice::silent3`): the words of the string engine, byte for byte. Called by `render/silent.rs`
//! for `silent::Ask::T`.

use ti4_engine_core::choice::reaction::Opt as ROpt;
use ti4_engine_core::choice::silent::Opt as GOpt;
use ti4_engine_core::choice::silent3::{Ask, Opt, Subtype};

use super::Render;
use crate::choice::ChoiceOption;
use crate::generated::IdStr;

pub(super) fn prompt(r: &Render, a: &Ask) -> String {
    let _ = r;
    match a {
        Ask::Ragh { planet } => format!("Ragh's Call: where do the Saar ground forces on {} go", planet.as_str()),
        Ask::Alastor { planet, unit } => format!("The Alastor: how many {} on {} fight as ships?", unit.as_str(), planet.as_str()),
    }
}

pub(super) fn subtype(s: &Subtype) -> String {
    match s {
        Subtype::Ragh => "ragh_destination",
        Subtype::Alastor => "alastor_participants",
    }
    .to_owned()
}

pub(super) fn option(r: &Render, a: &Ask, o: &Opt) -> ChoiceOption {
    let _ = r;
    match (a, o) {
        (Ask::Alastor { planet, unit }, Opt::Count(n)) => {
            ChoiceOption::labelled(format!("alastor|{n}"), "nekro_alastor", format!("{n} {} on {} fight as ships", unit.as_str(), planet.as_str())).with("planet", planet.as_str()).with("unit", unit.as_str()).with("count", i64::from(*n))
        }
        (Ask::Ragh { .. }, Opt::Site { system, planet }) => ChoiceOption::labelled(format!("{}|{}", system.as_str(), planet.as_str()), "planet", format!("move the Saar ground forces to {}", planet.as_str())),
        _ => panic!("option {o:?} in the question {a:?}"),
    }
}

pub(super) fn parse(r: &Render, a: &Ask, id: &str) -> Option<ROpt> {
    use ti4_engine_core::generated::{Planet, System};
    let _ = r;
    match a {
        Ask::Alastor { .. } => Some(ROpt::G(GOpt::T(Opt::Count(id.strip_prefix("alastor|")?.parse().ok()?)))),
        Ask::Ragh { .. } => {
            let (system, planet) = id.split_once('|')?;
            Some(ROpt::G(GOpt::T(Opt::Site { system: <System as IdStr>::from_id(system)?, planet: <Planet as IdStr>::from_id(planet)? })))
        }
    }
}
