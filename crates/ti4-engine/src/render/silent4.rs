//! Renderer of the questions of lane G4 (`ti4_engine_core::choice::silent4`): the words of the string engine, byte for byte. Called by `render/silent.rs`
//! for `silent::Ask::U`, and by `render/reaction.rs` for the display details of the sustain offer.

use ti4_engine_core::choice::reaction::Opt as ROpt;
use ti4_engine_core::choice::silent::Opt as GOpt;
use ti4_engine_core::choice::silent4::{Ask, Opt, Subtype};

use super::Render;
use crate::choice::ChoiceOption;
use crate::generated::IdStr;

/// The name of a planet as the string's questions show it.
fn planet_name(planet: ti4_engine_core::generated::Planet) -> String {
    ti4_content::galaxy::planet(ti4_content::ContentStore::embedded(), planet.as_str(), ti4_model::content_types::FULL).and_then(|record| record.name()).unwrap_or(planet.as_str()).to_owned()
}

pub(super) fn prompt(r: &Render, a: &Ask) -> String {
    match a {
        Ask::Stymie { mover } => format!("Stymie: where to put {}'s command token", r.seat_text(*mover)),
        Ask::MagenHit => "Magen Defense Grid: assign the hit to one of the opponent's ground forces".to_owned(),
        Ask::Foresight { .. } => "Foresight: where do your ships go".to_owned(),
        Ask::Place { base, fit } => format!("place up to {fit} {}", super::promissory::base_name_of(*base)),
        Ask::GroundSustain { unit, system, planet, staging } => {
            if *staging {
                format!("sustain the hit on {} on {} (system {})", unit.as_str(), planet_name(*planet), system.as_str())
            } else {
                format!("sustain the hit on {}", unit.as_str())
            }
        }
    }
}

pub(super) fn subtype(s: &Subtype) -> String {
    match s {
        Subtype::Stymie => "stymie_system",
        Subtype::MagenHit => "magen_defense_hit",
        Subtype::GroundSustain => "ground_effect_sustain",
        Subtype::Foresight => "destination",
        Subtype::Place => "place_units_from_reinforcements",
    }
    .to_owned()
}

/// The display details of the sustain offer (`Choice::offered`): the card, the unit and where, and the two answers' captions.
pub(super) fn details(r: &Render, a: &Ask) -> serde_json::Map<String, serde_json::Value> {
    let _ = r;
    match a {
        Ask::GroundSustain { unit, system, planet, .. } => crate::choice::offer_details(
            crate::choice::offer_card(
                "Sustain damage",
                "ground hit",
                Some("An effect has assigned a hit to this ground force"),
                Some("SUSTAIN DAMAGE cancels the hit and the unit stays, damaged. Without it the unit is destroyed."),
            ),
            vec![crate::choice::offer_fact_unit("Unit", unit.as_str()), crate::choice::offer_fact_planet("Where", planet.as_str(), system.as_str())],
            &[
                ("sustain", crate::choice::offer_caption("Sustain damage", Some("The unit stays on the planet, damaged"))),
                ("decline", crate::choice::offer_caption("Let it be destroyed", Some("The unit is removed"))),
            ],
        ),
        _ => serde_json::Map::new(),
    }
}

pub(super) fn option(r: &Render, a: &Ask, o: &Opt) -> ChoiceOption {
    match (a, o) {
        (Ask::Stymie { mover }, Opt::System(system)) => ChoiceOption::labelled(system.as_str(), "stymie", format!("place {}'s command token in {}", r.seat_text(*mover), system.as_str())),
        (Ask::MagenHit, Opt::Target { index, owner, unit, damaged, sturdy, cost_halves }) => {
            ChoiceOption::labelled(index.to_string(), "magen_defense_hit", format!("assign the hit to {}'s {}{}", r.seat_text(*owner), unit.as_str(), if *damaged { " (damaged)" } else { "" }))
                .with("unit", unit.as_str())
                .with("damaged", *damaged)
                .with("kills", !*sturdy)
                .with("cost", f64::from(*cost_halves) / 2.0)
        }
        (Ask::Foresight { from }, Opt::Destination(system)) => ChoiceOption::labelled(format!("system|{}", system.as_str()), "system", format!("move your ships from {} to {}", from.as_str(), system.as_str())),
        (Ask::Place { base, .. }, Opt::Spot { system, planet, fit }) => {
            let place = planet.map_or("space", |p| p.as_str());
            ChoiceOption::labelled(format!("{}|{place}", system.as_str()), "place_unit", format!("place {fit}x {} on {place} in {}", super::promissory::base_name_of(*base), system.as_str()))
                .with("system", system.as_str())
                .with("count", i64::from(*fit))
        }
        (Ask::GroundSustain { .. }, Opt::Sustain) => ChoiceOption::labelled("sustain", "ground_effect_sustain", "use SUSTAIN DAMAGE"),
        _ => panic!("option {o:?} in the question {a:?}"),
    }
}

pub(super) fn parse(r: &Render, a: &Ask, id: &str) -> Option<ROpt> {
    let _ = r;
    match a {
        Ask::Stymie { .. } => Some(ROpt::G(GOpt::U(Opt::System(<ti4_engine_core::generated::System as IdStr>::from_id(id)?)))),
        // The target is matched against the rendered options of its question (the id is an index).
        Ask::MagenHit => None,
        Ask::GroundSustain { .. } => (id == "sustain").then_some(ROpt::G(GOpt::U(Opt::Sustain))),
        Ask::Foresight { .. } => Some(ROpt::G(GOpt::U(Opt::Destination(<ti4_engine_core::generated::System as IdStr>::from_id(id.strip_prefix("system|")?)?)))),
        // The fit of a spot is the question's: the answer is matched against the rendered options.
        Ask::Place { .. } => None,
    }
}

/// The display facts of the placement question (`Choice::offered`): the card, the unit and how many, and per spot its name and how many go there. Needs the options, so the adapter calls it with the
/// core question (`live::details::complete`).
pub fn complete(choice: &ti4_engine_core::choice::Choice, rendered: &mut crate::choice::Choice) {
    use ti4_engine_core::choice::Opt as COpt;
    let ti4_engine_core::choice::Ask::Reaction(ti4_engine_core::choice::reaction::Ask::G(ti4_engine_core::choice::silent::Ask::U(Ask::Place { base, fit }))) = choice.ask else { return };
    let base = super::promissory::base_name_of(base);
    let mut captions: Vec<(String, serde_json::Value)> = Vec::new();
    for o in choice.opts.iter() {
        match o {
            COpt::Reaction(ROpt::G(GOpt::U(Opt::Spot { system, planet, fit }))) => {
                let place = planet.map_or_else(|| format!("In the space of system {}", system.as_str()), |p| format!("{} (system {})", planet_name(p), system.as_str()));
                let id = format!("{}|{}", system.as_str(), planet.map_or("space", |p| p.as_str()));
                captions.push((id, crate::choice::offer_caption(&place, Some(&format!("Place {fit} {base} here")))));
            }
            COpt::Decline => captions.push((crate::choice::DECLINE_ID.to_owned(), crate::choice::offer_caption("Place none", Some("Nothing is placed")))),
            _ => {}
        }
    }
    let captions: Vec<(&str, serde_json::Value)> = captions.iter().map(|(id, c)| (id.as_str(), c.clone())).collect();
    rendered.details.extend(crate::choice::offer_details(
        crate::choice::offer_card("Place units from your reinforcements", "placement", None, Some("Choose where they go.")),
        vec![crate::choice::offer_fact_unit("Unit", base), crate::choice::offer_fact("Up to", u64::from(fit))],
        &captions,
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use ti4_engine_core::generated::{Planet, System, Unit};
    use ti4_engine_core::ids::Seat;

    fn r() -> Render {
        Render::new(vec!["a".to_owned(), "b".to_owned()])
    }

    #[test]
    fn the_questions_and_answers_read_as_the_string_words_them() {
        let r = r();
        let (planet, system) = (Planet::ALL[3], System::ALL[4]);
        assert_eq!(prompt(&r, &Ask::Stymie { mover: Seat(1) }), "Stymie: where to put b's command token");
        let spot = option(&r, &Ask::Stymie { mover: Seat(1) }, &Opt::System(system));
        assert_eq!((spot.id.as_str(), spot.kind.as_str()), (system.as_str(), "stymie"));
        assert_eq!(spot.label, format!("place b's command token in {}", system.as_str()));
        assert_eq!(prompt(&r, &Ask::MagenHit), "Magen Defense Grid: assign the hit to one of the opponent's ground forces");
        let target = option(&r, &Ask::MagenHit, &Opt::Target { index: 2, owner: Seat(1), unit: Unit::InfantryI, damaged: true, sturdy: false, cost_halves: 1 });
        assert_eq!((target.id.as_str(), target.kind.as_str(), target.label.as_str()), ("2", "magen_defense_hit", "assign the hit to b's infantry (damaged)"));
        assert_eq!(target.payload["cost"], serde_json::json!(0.5));
        assert_eq!(target.payload["kills"], serde_json::json!(true));
        assert_eq!(target.payload["damaged"], serde_json::json!(true));
        let fresh = option(&r, &Ask::MagenHit, &Opt::Target { index: 0, owner: Seat(0), unit: Unit::FactionlessMech, damaged: false, sturdy: true, cost_halves: 4 });
        assert_eq!((fresh.label.as_str(), fresh.payload["kills"].clone(), fresh.payload["cost"].clone()), ("assign the hit to a's mech", serde_json::json!(false), serde_json::json!(2.0)));
        let plain = Ask::GroundSustain { unit: Unit::FactionlessMech, system, planet, staging: false };
        let staged = Ask::GroundSustain { unit: Unit::FactionlessMech, system, planet, staging: true };
        assert_eq!(prompt(&r, &plain), "sustain the hit on mech");
        assert!(prompt(&r, &staged).starts_with("sustain the hit on mech on ") && prompt(&r, &staged).ends_with(&format!(" (system {})", system.as_str())), "{}", prompt(&r, &staged));
        assert_eq!(option(&r, &plain, &Opt::Sustain).label, "use SUSTAIN DAMAGE");
        assert_eq!(subtype(&Subtype::GroundSustain), "ground_effect_sustain");
        assert_eq!(details(&r, &plain)["kind"], "offer");
        assert!(details(&r, &Ask::MagenHit).is_empty());
    }

    #[test]
    fn the_ids_that_name_their_option_parse_back_to_it() {
        let r = r();
        let system = System::ALL[4];
        let ask = Ask::Stymie { mover: Seat(1) };
        let id = option(&r, &ask, &Opt::System(system)).id;
        assert_eq!(parse(&r, &ask, &id), Some(ROpt::G(GOpt::U(Opt::System(system)))));
        assert_eq!(parse(&r, &Ask::MagenHit, "0"), None);
        let sustain = Ask::GroundSustain { unit: Unit::FactionlessMech, system, planet: Planet::ALL[3], staging: false };
        assert_eq!(parse(&r, &sustain, "sustain"), Some(ROpt::G(GOpt::U(Opt::Sustain))));
        assert_eq!(parse(&r, &sustain, "decline"), None);
    }
}
