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
        Ask::Bio { so_far } => format!("Bioplasmosis: move an infantry ({so_far} so far)"),
        Ask::OracleGive => "The Oracle: which promissory note do you give".to_owned(),
        Ask::SulaOpponent => "S'ula Mentarion: choose an opponent".to_owned(),
        Ask::SulaGive { to } => format!("Give {} 1 promissory note from your hand", r.seat_text(*to)),
        Ask::SuffiAn { target } => format!("Suffi An: exhaust to draw 1 action card each with {}", r.seat_text(*target)),
        Ask::ExoSelf => "Exotrireme II: which one is destroyed".to_owned(),
        Ask::ExoVictim { round } => format!("Exotrireme II: destroy ship {} of up to 2", u32::from(*round) + 1),
        Ask::Alastor { planet, unit } => format!("The Alastor: how many {} on {} fight as ships?", unit.as_str(), planet.as_str()),
    }
}

pub(super) fn subtype(s: &Subtype) -> String {
    match s {
        Subtype::Ragh => "ragh_destination",
        Subtype::Alastor => "alastor_participants",
        Subtype::Bio => "bioplasmosis_move",
        Subtype::OracleGive => "give_note",
        Subtype::SulaOpponent => "commander_opponent",
        Subtype::SulaGive => "commander_give_note",
        Subtype::SuffiAn => "suffi_an",
        Subtype::ExoSelf => "exotrireme_self",
        Subtype::ExoVictim => "exotrireme_victim",
    }
    .to_owned()
}

pub(super) fn option(r: &Render, a: &Ask, o: &Opt) -> ChoiceOption {
    let _ = r;
    match (a, o) {
        (Ask::Bio { .. }, Opt::Shift { index, from, to }) => {
            ChoiceOption::labelled(format!("move|{index}"), "bioplasmosis", format!("move 1 infantry from {} in {} to {} in {}", from.1.as_str(), from.0.as_str(), to.1.as_str(), to.0.as_str()))
        }
        (Ask::OracleGive, Opt::Note(key)) => {
            let note = super::cards::note_text(*key);
            ChoiceOption::labelled(format!("note|{note}"), "promissory_note", format!("give {note} to the Naalu player"))
        }
        (Ask::SulaOpponent, Opt::Seat(who)) => {
            let who = r.seat_text(*who);
            ChoiceOption::labelled(who, "commander", format!("force {who} to give you a promissory note"))
        }
        (Ask::SulaGive { .. }, Opt::Note(key)) => {
            let id = super::diplomacy::dip_note_id(*key);
            ChoiceOption::labelled(id.clone(), "give_note", id)
        }
        (Ask::SulaGive { .. }, Opt::Support(faction)) => {
            let id = format!("support:{}", faction.as_str());
            ChoiceOption::labelled(id.clone(), "give_note", id)
        }
        (Ask::SuffiAn { .. }, Opt::Exhaust) => ChoiceOption::labelled("exhaust", "agent", "exhaust Suffi An: you and the pillaged player each draw 1 action card"),
        (Ask::ExoSelf | Ask::ExoVictim { .. }, Opt::Ship { index, owner, unit, damaged, victim }) => {
            let who = r.seat_text(*owner);
            let label = format!("destroy {who}'s {}{}", unit.as_str(), if *damaged { " (damaged)" } else { "" });
            let option = ChoiceOption::labelled(format!("{}|{index}", if *victim { "ship" } else { "self" }), "ship", label);
            if *victim { option.with("player", who.to_owned()) } else { option }
        }
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
        // The ships of the exotrireme questions are matched against the rendered options (the id is an index into the question's list).
        Ask::ExoSelf | Ask::ExoVictim { .. } => None,
        // The answers of these are matched against the rendered options (the id is an index, or the note's text).
        Ask::Bio { .. } | Ask::OracleGive => None,
        Ask::SulaOpponent => Some(ROpt::G(GOpt::T(Opt::Seat(r.seat(id)?)))),
        // The notes of the hand are matched against the rendered options (the id is the note's text).
        Ask::SulaGive { .. } => None,
        Ask::SuffiAn { .. } => (id == "exhaust").then_some(ROpt::G(GOpt::T(Opt::Exhaust))),
        Ask::Alastor { .. } => Some(ROpt::G(GOpt::T(Opt::Count(id.strip_prefix("alastor|")?.parse().ok()?)))),
        Ask::Ragh { .. } => {
            let (system, planet) = id.split_once('|')?;
            Some(ROpt::G(GOpt::T(Opt::Site { system: <System as IdStr>::from_id(system)?, planet: <Planet as IdStr>::from_id(planet)? })))
        }
    }
}
