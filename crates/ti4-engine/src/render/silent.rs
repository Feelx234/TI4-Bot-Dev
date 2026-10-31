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
        Ask::Reclaimer => "Reclaimer: place a structure".to_owned(),
        Ask::TradePolicy => "Hegemonic Trade Policy: swap which planet's values".to_owned(),
    }
}

pub(super) fn subtype(s: &Subtype) -> String {
    match s {
        Subtype::Landing(Landing::Warrior) => "lw2_return",
        Subtype::Landing(Landing::Legionnaire) => "legionnaire_return",
        Subtype::Mitosis => "mitosis_place",
        Subtype::Pillage => "pillage_take",
        Subtype::Reclaimer => "reclaimer",
        Subtype::TradePolicy => "htp_planet",
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
        (Ask::Reclaimer, Opt::Reclaimer { pds, system, planet }) => {
            let (base, word) = if *pds { ("pds", "PDS") } else { ("spacedock", "space dock") };
            ChoiceOption::labelled(format!("{base}|{}|{}", system.as_str(), planet.as_str()), "unit", format!("Reclaimer: place a {word} on {}", planet.as_str()))
                .with("system", system.as_str())
                .with("planet", planet.as_str())
        }
        (Ask::TradePolicy, Opt::Swap { system, planet }) => ChoiceOption::labelled(planet.as_str(), "planet", format!("swap the values of {}", planet.as_str())).with("system", system.as_str()),
        _ => panic!("option {o:?} in the question {a:?}"),
    }
}

pub(super) fn parse(r: &Render, a: &Ask, id: &str) -> Option<ROpt> {
    use ti4_engine_core::generated::{Planet, System};
    match a {
        Ask::Landing(_) => <Planet as IdStr>::from_id(id).map(|p| ROpt::G(Opt::Planet(p))),
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
