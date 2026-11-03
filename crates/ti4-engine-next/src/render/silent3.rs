//! Renderer of the questions of lane G3 (`ti4_engine_core::choice::silent3`): the words of the string engine, byte for byte. Called by `render/silent.rs`
//! for `silent::Ask::T`.

use ti4_engine_core::choice::reaction::Opt as ROpt;
use ti4_engine_core::choice::silent::Opt as GOpt;
use ti4_engine_core::choice::silent3::{Ask, Opt, Subtype};
use ti4_engine_core::generated::Wormhole;

use super::Render;
use crate::wire::choice::ChoiceOption;
use crate::generated::IdStr;

/// The text of a Creuss token (`tokens::CREUSS_TOKENS`).
fn token_text(kind: Wormhole) -> &'static str {
    match kind {
        Wormhole::Alpha => "ALPHA",
        Wormhole::Beta => "BETA",
        Wormhole::Delta => "DELTA",
        Wormhole::Epsilon => "EPSILON",
        Wormhole::Gamma => "GAMMA",
    }
}

pub(super) fn prompt(r: &Render, a: &Ask) -> String {
    let _ = r;
    match a {
        Ask::Ragh { planet } => format!("Ragh's Call: where do the Saar ground forces on {} go", planet.as_str()),
        Ask::Bio { so_far } => format!("Bioplasmosis: move an infantry ({so_far} so far)"),
        Ask::Scepter => "Scepter of Dominion: choose a system".to_owned(),
        Ask::Iff => "Creuss IFF: place or move a Creuss wormhole token".to_owned(),
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
        Subtype::Scepter => "scepter_system",
        Subtype::Iff => "creuss_token",
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
        (Ask::Scepter, Opt::Scepter { system, payers }) => {
            let system = system.as_str();
            let names: Vec<&str> = payers.iter().flatten().map(|f| f.as_str()).collect();
            let label = if names.is_empty() {
                format!("nobody places a command token in {system} (only other players with a token in the Mahact fleet pool place one)")
            } else {
                format!("{} {} a command token in {system}", names.join(", "), if names.len() == 1 { "places" } else { "place" })
            };
            ChoiceOption::labelled(system, "scepter", label)
        }
        (Ask::Iff, Opt::Token { kind, system }) => {
            let kind = token_text(*kind);
            ChoiceOption::labelled(format!("{kind}|{}", system.as_str()), "creuss_token", format!("Creuss IFF: the {kind} token into {}", system.as_str())).with("system", system.as_str())
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
        Ask::Bio { .. } | Ask::OracleGive | Ask::Scepter => None,
        Ask::Iff => {
            let (kind, system) = id.split_once('|')?;
            let kind = [Wormhole::Alpha, Wormhole::Beta].into_iter().find(|k| token_text(*k) == kind)?;
            Some(ROpt::G(GOpt::T(Opt::Token { kind, system: <ti4_engine_core::generated::System as IdStr>::from_id(system)? })))
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use ti4_engine_core::generated::{Faction, Planet, PromissoryNote, System, Unit};
    use ti4_engine_core::ids::Seat;
    use ti4_engine_core::state::NoteKey;

    fn r() -> Render {
        Render::new(vec!["a".to_owned(), "b".to_owned()])
    }

    /// An answer parses back to the typed option it was rendered from, where the id says everything.
    #[test]
    fn the_ids_that_name_their_option_parse_back_to_it() {
        let r = r();
        let planet = Planet::ALL[3];
        let system = System::ALL[4];
        let cases = [
            (Ask::Ragh { planet }, Opt::Site { system, planet }),
            (Ask::Alastor { planet, unit: Unit::InfantryI }, Opt::Count(2)),
            (Ask::SuffiAn { target: Seat(1) }, Opt::Exhaust),
            (Ask::SulaOpponent, Opt::Seat(Seat(1))),
        ];
        for (ask, opt) in cases {
            let id = option(&r, &ask, &opt).id;
            assert_eq!(parse(&r, &ask, &id), Some(ROpt::G(GOpt::T(opt))), "{ask:?} {id}");
        }
        // The index and note ids are matched against the rendered options of their question instead.
        for ask in [Ask::ExoSelf, Ask::ExoVictim { round: 0 }, Ask::SulaGive { to: Seat(0) }, Ask::Bio { so_far: 0 }, Ask::OracleGive, Ask::Scepter] {
            assert_eq!(parse(&r, &ask, "x"), None, "{ask:?}");
        }
        assert_eq!(parse(&r, &Ask::SuffiAn { target: Seat(1) }, "decline"), None);
    }

    /// The words of the string, for the questions no lockstep reaches (their string bodies are the source of each line).
    #[test]
    fn the_questions_and_answers_read_as_the_string_words_them() {
        let r = r();
        let (planet, system) = (Planet::ALL[3], System::ALL[4]);
        let key = NoteKey { note: PromissoryNote::Ceasefire, faction: Faction::TheFederationOfSol };
        assert_eq!(prompt(&r, &Ask::SuffiAn { target: Seat(1) }), "Suffi An: exhaust to draw 1 action card each with b");
        assert_eq!(option(&r, &Ask::SuffiAn { target: Seat(1) }, &Opt::Exhaust).label, "exhaust Suffi An: you and the pillaged player each draw 1 action card");
        assert_eq!(prompt(&r, &Ask::SulaOpponent), "S'ula Mentarion: choose an opponent");
        assert_eq!(option(&r, &Ask::SulaOpponent, &Opt::Seat(Seat(1))).label, "force b to give you a promissory note");
        assert_eq!(prompt(&r, &Ask::SulaGive { to: Seat(0) }), "Give a 1 promissory note from your hand");
        let give = option(&r, &Ask::SulaGive { to: Seat(0) }, &Opt::Note(key));
        assert_eq!((give.id.as_str(), give.kind.as_str(), give.label.as_str()), ("cf:sol", "give_note", "cf:sol"));
        let support = option(&r, &Ask::SulaGive { to: Seat(0) }, &Opt::Support(Faction::TheFederationOfSol));
        assert_eq!(support.id, "support:sol");
        assert_eq!(prompt(&r, &Ask::OracleGive), "The Oracle: which promissory note do you give");
        assert_eq!(option(&r, &Ask::OracleGive, &Opt::Note(key)).label, "give cf:sol to the Naalu player");
        assert_eq!(prompt(&r, &Ask::Bio { so_far: 2 }), "Bioplasmosis: move an infantry (2 so far)");
        let shift = option(&r, &Ask::Bio { so_far: 0 }, &Opt::Shift { index: 3, from: (system, planet), to: (system, planet) });
        assert_eq!(shift.id, "move|3");
        assert!(shift.label.starts_with("move 1 infantry from "), "{}", shift.label);
        assert_eq!(prompt(&r, &Ask::Scepter), "Scepter of Dominion: choose a system");
        let none = option(&r, &Ask::Scepter, &Opt::Scepter { system, payers: [None; 5] });
        assert!(none.label.starts_with("nobody places a command token in "), "{}", none.label);
        let two = option(&r, &Ask::Scepter, &Opt::Scepter { system, payers: [Some(Faction::TheFederationOfSol), Some(Faction::TheEmiratesOfHacan), None, None, None] });
        assert!(two.label.starts_with("sol, hacan place a command token in "), "{}", two.label);
        let one = option(&r, &Ask::Scepter, &Opt::Scepter { system, payers: [Some(Faction::TheFederationOfSol), None, None, None, None] });
        assert!(one.label.starts_with("sol places a command token in "), "{}", one.label);
        assert_eq!(prompt(&r, &Ask::ExoSelf), "Exotrireme II: which one is destroyed");
        assert_eq!(prompt(&r, &Ask::ExoVictim { round: 1 }), "Exotrireme II: destroy ship 2 of up to 2");
        let ship = option(&r, &Ask::ExoVictim { round: 0 }, &Opt::Ship { index: 1, owner: Seat(1), unit: Unit::CruiserI, damaged: true, victim: true });
        assert_eq!((ship.id.as_str(), ship.label.as_str()), ("ship|1", "destroy b's cruiser (damaged)"));
        assert_eq!(option(&r, &Ask::ExoSelf, &Opt::Ship { index: 0, owner: Seat(0), unit: Unit::ExotriremeII, damaged: false, victim: false }).id, "self|0");
    }
}
