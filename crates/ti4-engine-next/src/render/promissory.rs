//! Renderer of the `promissory` area: `impl Family` for `ti4_engine_core::choice::promissory` (prompt, context,
//! option, parse) and its entry in the registry (`render/mod.rs`). Package W14 (lent-bodies lane 5).
//!
//! The one question is the placement of units from the reinforcements (`action_cards::place_units_choosing`, the planets of the
//! placer): the prompt, the option ids and labels are the string's byte for byte; the offer card, its facts and the captions of the
//! options (display only, and part of the compared question) are added by [`complete_place_units`] with the core question.

use ti4_engine_core::choice::promissory::{Ask, Opt, Subtype};
use ti4_engine_core::generated::{Planet, System, Unit, UnitBaseType};

use super::{Family, Render};
use crate::wire::choice::ChoiceOption;
use crate::generated::IdStr;

/// The kind of the placement options (the string `place_unit`).
pub const PLACE_KIND: &str = "place_unit";

/// The renderer of the placement question.
pub struct Fam;

/// `base_type` of a unit, as the string spells it.
#[must_use]
pub fn base_name(unit: Unit) -> &'static str {
    base_name_of(unit.data().base_type)
}

/// A base type, as the string spells it.
#[must_use]
pub const fn base_name_of(base: UnitBaseType) -> &'static str {
    match base {
        UnitBaseType::Carrier => "carrier",
        UnitBaseType::Cruiser => "cruiser",
        UnitBaseType::Destroyer => "destroyer",
        UnitBaseType::Dreadnought => "dreadnought",
        UnitBaseType::Fighter => "fighter",
        UnitBaseType::Flagship => "flagship",
        UnitBaseType::Infantry => "infantry",
        UnitBaseType::Mech => "mech",
        UnitBaseType::Pds => "pds",
        UnitBaseType::Spacedock => "spacedock",
        UnitBaseType::Warsun => "warsun",
    }
}

impl Family for Fam {
    type Ask = Ask;
    type Opt = Opt;
    type Subtype = Subtype;

    fn prompt(_: &Render, a: &Ask) -> String {
        match a {
            Ask::PlaceUnits { unit, fit } => format!("place up to {fit} {}", base_name(*unit)),
            Ask::C4rPds => "???_ERROR_ERROR_???: choose a planet you control".to_owned(),
            Ask::OrbitalPlanet => "Orbital Drop: onto which planet".to_owned(),
            Ask::OrbitalDeploy { planet, .. } => format!("Orbital Drop: deploy a mech on {}", planet.as_str()),
            Ask::StarForge => "Star Forge: place which units, and where".to_owned(),
            Ask::EmberColossus { .. } => "Ember Colossus: place 1 infantry with this mech".to_owned(),
        }
    }

    fn subtype(_: &Render, s: &Subtype) -> String {
        match s {
            Subtype::PlaceUnitsFromReinforcements => "place_units_from_reinforcements",
            Subtype::C4rPlanet => "c4r_planet",
            Subtype::OrbitalPlanet => "orbital_drop_choose_planet",
            Subtype::OrbitalDeploy => "orbital_drop_deploy_mech",
            Subtype::StarForge => "star_forge",
            Subtype::EmberColossus => "ember_colossus",
        }
        .to_owned()
    }

    fn option(_: &Render, a: &Ask, o: &Opt) -> ChoiceOption {
        match (a, o) {
            (Ask::PlaceUnits { unit, fit }, Opt::Spot(system, planet)) => {
                let (system, planet, base) = (system.as_str(), planet.as_str(), base_name(*unit));
                ChoiceOption::labelled(format!("{system}|{planet}"), PLACE_KIND, format!("place {fit}x {base} on {planet} in {system}"))
                    .with("system", system)
                    .with("count", i64::from(*fit))
            }
            (Ask::OrbitalPlanet, Opt::Spot(system, planet)) => {
                let (system, planet) = (system.as_str(), planet.as_str());
                ChoiceOption::labelled(planet, "planet", planet).with("system", system).with("planet", planet)
            }
            (Ask::OrbitalDeploy { system, planet, mech }, _) => {
                let (system, planet, mech) = (system.as_str(), planet.as_str(), mech.as_str());
                ChoiceOption::labelled(format!("deploy|{mech}|1"), crate::ties::production::PRODUCE_KIND, format!("deploy 1 {mech} for 3 resources"))
                    .with("unit", mech)
                    .with("count", 1)
                    .with("cost", 3)
                    .with("system", system)
                    .with("planet", planet)
                    .with("orbital_drop_deploy", true)
            }
            (_, Opt::Forge { system, fighters }) => {
                let (system, kind, what) = (system.as_str(), if *fighters { "fighters" } else { "destroyer" }, if *fighters { "2 fighters" } else { "1 destroyer" });
                ChoiceOption::labelled(format!("{system}|{kind}"), "star_forge", format!("place {what} in {system}")).with("system", system)
            }
            (Ask::EmberColossus { system, planet }, Opt::Ember) => {
                let (system, place) = (system.as_str(), planet.map_or("space", |p| p.as_str()));
                ChoiceOption::labelled(format!("{system}|{place}"), PLACE_KIND, format!("place 1 infantry with the mech on {place} in {system}"))
            }
            (_, Opt::Ember) => ChoiceOption::labelled("ember", PLACE_KIND, "place 1 infantry with the mech"),
            (_, Opt::Spot(system, planet)) => {
                ChoiceOption::labelled(format!("{}|{}", system.as_str(), planet.as_str()), "nekro_c4r_planet", format!("place a PDS on {} in {}", planet.as_str(), system.as_str()))
            }
            (_, Opt::Deploy) => ChoiceOption::labelled("deploy", "produce", "deploy"),
        }
    }

    fn parse(_: &Render, a: &Ask, id: &str) -> Option<Opt> {
        match a {
            // The planet question's id names the planet alone, and the deploy's names the mech: both are matched against the options.
            Ask::OrbitalPlanet | Ask::OrbitalDeploy { .. } => None,
            Ask::StarForge => {
                let (system, kind) = id.split_once('|')?;
                match kind {
                    "fighters" => Some(Opt::Forge { system: System::from_id(system)?, fighters: true }),
                    "destroyer" => Some(Opt::Forge { system: System::from_id(system)?, fighters: false }),
                    _ => None,
                }
            }
            Ask::EmberColossus { system, planet } => {
                let place = planet.map_or("space", |p| p.as_str());
                (id == format!("{}|{place}", system.as_str())).then_some(Opt::Ember)
            }
            Ask::PlaceUnits { .. } | Ask::C4rPds => {
                let (system, planet) = id.split_once('|')?;
                Some(Opt::Spot(System::from_id(system)?, Planet::from_id(planet)?))
            }
        }
    }
}

/// The display facts of the placement question (`Choice::offered`): the card, the unit and how many, and per planet its name
/// and system. Needs the options, so the adapter calls it with the core question.
pub fn complete_place_units(choice: &ti4_engine_core::choice::Choice, rendered: &mut crate::wire::choice::Choice) {
    use ti4_engine_core::choice::Opt as COpt;
    let ti4_engine_core::choice::Ask::Promissory(Ask::PlaceUnits { unit, fit }) = choice.ask else { return }; // the card's own question has no offer card
    let content = ti4_content::ContentStore::embedded();
    let base = base_name(unit);
    let captions: Vec<(String, serde_json::Value)> = choice
        .opts
        .iter()
        .filter_map(|o| {
            let COpt::Promissory(Opt::Spot(system, planet)) = o else { return None };
            let name = ti4_content::galaxy::planet(content, planet.as_str(), ti4_model::content_types::FULL)
                .and_then(|record| record.name())
                .unwrap_or(planet.as_str());
            Some((
                format!("{}|{}", system.as_str(), planet.as_str()),
                crate::wire::choice::offer_caption(&format!("{name} (system {})", system.as_str()), Some(&format!("Place {fit} {base} here"))),
            ))
        })
        .chain(choice.opts.iter().any(|o| matches!(o, COpt::Decline)).then(|| (crate::wire::choice::DECLINE_ID.to_owned(), crate::wire::choice::offer_caption("Place none", Some("Nothing is placed")))))
        .collect();
    let captions: Vec<(&str, serde_json::Value)> = captions.iter().map(|(id, c)| (id.as_str(), c.clone())).collect();
    rendered.details = crate::wire::choice::offer_details(
        crate::wire::choice::offer_card("Place units from your reinforcements", "placement", None, Some("Choose where they go.")),
        vec![crate::wire::choice::offer_fact_unit("Unit", base), crate::wire::choice::offer_fact("Up to", u64::from(fit))],
        &captions,
    );
}
