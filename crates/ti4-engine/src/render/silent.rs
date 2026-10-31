//! Renderer of the questions of the formerly silent abilities (lane G): the words of the string engine, byte for byte. Called by `render/reaction.rs`
//! for `reaction::Ask::G`.

use ti4_engine_core::choice::reaction::Opt as ROpt;
use ti4_engine_core::choice::silent::{Ask, Landing, Opt, Subtype};

use super::Render;
use crate::choice::ChoiceOption;
use crate::generated::IdStr;

pub(super) fn prompt(r: &Render, a: &Ask) -> String {
    let _ = r;
    match a {
        Ask::Landing(Landing::Warrior) => "Letani Warrior II: where in your home system".to_owned(),
        Ask::Landing(Landing::Legionnaire) => "Crimson Legionnaire II: where in your home system".to_owned(),
        Ask::Mitosis => "Mitosis: place an infantry or replace one with a mech".to_owned(),
        Ask::Pillage => "Pillage: take 1 trade good or commodity".to_owned(),
        Ask::T(t) => super::silent3::prompt(r, t),
        Ask::U(u) => super::silent4::prompt(r, u),
        Ask::Reclaimer => "Reclaimer: place a structure".to_owned(),
        Ask::TradePolicy => "Hegemonic Trade Policy: swap which planet's values".to_owned(),
        Ask::CommanderFighters => "Sai Seravus: how many fighters do you place from reinforcements".to_owned(),
        Ask::Voidwatch => "Voidwatch: which promissory note to give".to_owned(),
        Ask::Stillness => "The Stillness of Stars: which unit do you capture".to_owned(),
        Ask::NaazAgent | Ask::NaazCopy => "Garv and Gunn: explore which of your planets".to_owned(),
    }
}

pub(super) fn subtype(s: &Subtype) -> String {
    match s {
        Subtype::Landing(Landing::Warrior) => "lw2_return",
        Subtype::Landing(Landing::Legionnaire) => "legionnaire_return",
        Subtype::Mitosis => "mitosis_place",
        Subtype::Pillage => "pillage_take",
        Subtype::T(t) => return super::silent3::subtype(t),
        Subtype::U(u) => return super::silent4::subtype(u),
        Subtype::Reclaimer => "reclaimer",
        Subtype::TradePolicy => "htp_planet",
        Subtype::CommanderFighters => "commander_fighters",
        Subtype::Voidwatch => "voidwatch_note",
        Subtype::Stillness => "stillness_target",
        Subtype::NaazAgent => "agent_planet",
        Subtype::NaazCopy => "borrowed_agent_planet",
    }
    .to_owned()
}

pub(super) fn option(r: &Render, a: &Ask, o: &Opt) -> ChoiceOption {
    match (a, o) {
        (Ask::Landing(Landing::Warrior), Opt::Planet(p)) => ChoiceOption::labelled(p.as_str(), "lw2", format!("place a Letani Warrior on {}", p.as_str())),
        (Ask::Landing(Landing::Legionnaire), Opt::Planet(p)) => ChoiceOption::labelled(p.as_str(), "cl2", format!("place a Crimson Legionnaire on {}", p.as_str())),
        (Ask::Mitosis, Opt::Mitosis { replace, system, planet }) => {
            let place = planet.map_or("space", |p| p.as_str());
            let id = format!("{}|{}|{place}", if *replace { "mech" } else { "infantry" }, system.as_str());
            let label = if *replace {
                format!("Mitosis: replace an infantry on {place} in {} with a mech", system.as_str())
            } else {
                format!("Mitosis: place 1 infantry on {place} in {}", system.as_str())
            };
            ChoiceOption::labelled(id, "mitosis", label)
        }
        (Ask::Pillage, Opt::Pillage { commodity, target }) => {
            let target = r.seat_text(*target);
            let (kind, word) = if *commodity { ("commodity", "commodity") } else { ("trade_good", "trade good") };
            ChoiceOption::labelled(format!("{kind}|{target}"), "pillage", format!("take 1 {word} from {target}"))
        }
        (Ask::T(a), Opt::T(o)) => super::silent3::option(r, a, o),
        (Ask::U(a), Opt::U(o)) => super::silent4::option(r, a, o),
        (Ask::Reclaimer, Opt::Reclaimer { pds, system, planet }) => {
            let (base, word) = if *pds { ("pds", "PDS") } else { ("spacedock", "space dock") };
            ChoiceOption::labelled(format!("{base}|{}|{}", system.as_str(), planet.as_str()), "unit", format!("Reclaimer: place a {word} on {}", planet.as_str()))
                .with("system", system.as_str())
                .with("planet", planet.as_str())
        }
        (Ask::TradePolicy, Opt::Swap { system, planet }) => ChoiceOption::labelled(planet.as_str(), "planet", format!("swap the values of {}", planet.as_str())).with("system", system.as_str()),
        (Ask::CommanderFighters, Opt::Fighters { count, system }) => {
            ChoiceOption::labelled(format!("fighters|{count}"), "fighter", format!("place {count} fighter(s) in {}", system.as_str())).with("count", i64::from(*count))
        }
        (Ask::Voidwatch, Opt::Note(key)) => {
            let note = super::cards::note_text(*key);
            ChoiceOption::labelled(format!("note|{note}"), "promissory", format!("give {note} to the Empyrean"))
        }
        (Ask::Voidwatch, Opt::Support(faction)) => {
            let note = format!("support:{}", faction.as_str());
            ChoiceOption::labelled(format!("note|{note}"), "promissory", format!("give {note} to the Empyrean"))
        }
        (Ask::Stillness, Opt::Capture { unit, victim }) => ChoiceOption::labelled(unit.as_str(), "stillness", format!("capture a {} from {}'s reinforcements", unit.as_str(), r.seat_text(*victim))),
        (Ask::NaazAgent, Opt::Explore(planet)) => ChoiceOption::labelled(planet.as_str(), "planet", format!("explore {}", planet.as_str())),
        (Ask::NaazCopy, Opt::Explore(planet)) => {
            let content = ti4_content::ContentStore::embedded();
            let name = ti4_content::galaxy::planet(content, planet.as_str(), ti4_model::content_types::FULL).and_then(|record| record.name()).unwrap_or(planet.as_str());
            ChoiceOption::labelled(planet.as_str(), "planet", format!("explore {name}"))
        }
        _ => panic!("option {o:?} in the question {a:?}"),
    }
}

pub(super) fn parse(r: &Render, a: &Ask, id: &str) -> Option<ROpt> {
    use ti4_engine_core::generated::{Planet, System};
    match a {
        Ask::T(t) => super::silent3::parse(r, t, id),
        Ask::U(u) => super::silent4::parse(r, u, id),
        Ask::Landing(_) => <Planet as IdStr>::from_id(id).map(|p| ROpt::G(Opt::Planet(p))),
        Ask::NaazAgent | Ask::NaazCopy => <Planet as IdStr>::from_id(id).map(|p| ROpt::G(Opt::Explore(p))),
        Ask::Pillage => {
            let (kind, target) = id.split_once('|')?;
            let commodity = match kind {
                "commodity" => true,
                "trade_good" => false,
                _ => return None,
            };
            Some(ROpt::G(Opt::Pillage { commodity, target: r.seat(target)? }))
        }
        Ask::Reclaimer => {
            let mut parts = id.splitn(3, '|');
            let (base, system, planet) = (parts.next()?, parts.next()?, parts.next()?);
            let pds = match base {
                "pds" => true,
                "spacedock" => false,
                _ => return None,
            };
            Some(ROpt::G(Opt::Reclaimer { pds, system: <System as IdStr>::from_id(system)?, planet: <Planet as IdStr>::from_id(planet)? }))
        }
        // The system is not in the option id: the option is matched against the rendered ones of its question.
        // The victim is not in the option id: the option is matched against the rendered ones of its question.
        Ask::CommanderFighters | Ask::Voidwatch | Ask::Stillness => None,
        // The system is an attribute of the rendered option: the option is matched against the rendered ones of its question.
        Ask::TradePolicy => None,
        Ask::Mitosis => {
            let mut parts = id.splitn(3, '|');
            let (kind, system, place) = (parts.next()?, parts.next()?, parts.next()?);
            let replace = match kind {
                "mech" => true,
                "infantry" => false,
                _ => return None,
            };
            let planet = if place == "space" { None } else { Some(<Planet as IdStr>::from_id(place)?) };
            Some(ROpt::G(Opt::Mitosis { replace, system: <System as IdStr>::from_id(system)?, planet }))
        }
    }
}
