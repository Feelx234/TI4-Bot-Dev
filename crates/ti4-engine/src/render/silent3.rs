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
    }
}

pub(super) fn subtype(s: &Subtype) -> String {
    match s {
        Subtype::Ragh => "ragh_destination",
    }
    .to_owned()
}

pub(super) fn option(r: &Render, a: &Ask, o: &Opt) -> ChoiceOption {
    let _ = r;
    match (a, o) {
        (Ask::Ragh { .. }, Opt::Site { system, planet }) => ChoiceOption::labelled(format!("{}|{}", system.as_str(), planet.as_str()), "planet", format!("move the Saar ground forces to {}", planet.as_str())),
    }
}

pub(super) fn parse(r: &Render, a: &Ask, id: &str) -> Option<ROpt> {
    use ti4_engine_core::generated::{Planet, System};
    let _ = r;
    match a {
        Ask::Ragh { .. } => {
            let (system, planet) = id.split_once('|')?;
            Some(ROpt::G(GOpt::T(Opt::Site { system: <System as IdStr>::from_id(system)?, planet: <Planet as IdStr>::from_id(planet)? })))
        }
    }
}
