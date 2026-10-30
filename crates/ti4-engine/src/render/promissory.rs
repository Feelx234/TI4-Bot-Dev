//! Renderer of the `promissory` area: `impl Family` for `ti4_engine_core::choice::promissory` (prompt, context,
//! option, parse) and its entry in the registry (`render/mod.rs`). Package W14 (lent-bodies lane 5).
//!
//! The one question is the placement of units from the reinforcements (`action_cards::place_units_choosing`, the planets of the
//! placer): the prompt, the option ids and labels are the string's byte for byte; the offer card, its facts and the captions of the
//! options (display only, and part of the compared question) are added by [`complete_place_units`] with the core question.

use ti4_engine_core::choice::promissory::{Ask, Opt, Subtype};
use ti4_engine_core::generated::{Planet, System, Unit, UnitBaseType};

use super::{Family, Render};
use crate::choice::ChoiceOption;
use crate::generated::IdStr;

/// The kind of the placement options (the string `place_unit`).
pub const PLACE_KIND: &str = "place_unit";

/// The renderer of the placement question.
pub struct Fam;

/// `base_type` of a unit, as the string spells it.
#[must_use]
pub fn base_name(unit: Unit) -> &'static str {
    match unit.data().base_type {
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
        let Ask::PlaceUnits { unit, fit } = a;
        format!("place up to {fit} {}", base_name(*unit))
    }

    fn subtype(_: &Render, s: &Subtype) -> String {
        match s {
            Subtype::PlaceUnitsFromReinforcements => "place_units_from_reinforcements",
        }
        .to_owned()
    }

    fn option(_: &Render, a: &Ask, o: &Opt) -> ChoiceOption {
        let (Ask::PlaceUnits { unit, fit }, Opt::Spot(system, planet)) = (a, o);
        let (system, planet, base) = (system.as_str(), planet.as_str(), base_name(*unit));
        ChoiceOption::labelled(format!("{system}|{planet}"), PLACE_KIND, format!("place {fit}x {base} on {planet} in {system}"))
            .with("system", system)
            .with("count", i64::from(*fit))
    }

    fn parse(_: &Render, _: &Ask, id: &str) -> Option<Opt> {
        let (system, planet) = id.split_once('|')?;
        Some(Opt::Spot(System::from_id(system)?, Planet::from_id(planet)?))
    }
}

/// The display facts of the placement question (`Choice::offered`): the card, the unit and how many, and per planet its name
/// and system. Needs the options, so the adapter calls it with the core question.
pub fn complete_place_units(choice: &ti4_engine_core::choice::Choice, rendered: &mut crate::choice::Choice) {
    use ti4_engine_core::choice::Opt as COpt;
    let ti4_engine_core::choice::Ask::Promissory(Ask::PlaceUnits { unit, fit }) = choice.ask else { return };
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
                crate::choice::offer_caption(&format!("{name} (system {})", system.as_str()), Some(&format!("Place {fit} {base} here"))),
            ))
        })
        .chain(choice.opts.iter().any(|o| matches!(o, COpt::Decline)).then(|| (crate::choice::DECLINE_ID.to_owned(), crate::choice::offer_caption("Place none", Some("Nothing is placed")))))
        .collect();
    let captions: Vec<(&str, serde_json::Value)> = captions.iter().map(|(id, c)| (id.as_str(), c.clone())).collect();
    rendered.details = crate::choice::offer_details(
        crate::choice::offer_card("Place units from your reinforcements", "placement", None, Some("Choose where they go.")),
        vec![crate::choice::offer_fact_unit("Unit", base), crate::choice::offer_fact("Up to", u64::from(fit))],
        &captions,
    );
}
