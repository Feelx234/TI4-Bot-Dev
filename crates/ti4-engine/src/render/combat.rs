//! Renderer of the `combat` area: `impl Family` for `ti4_engine_core::choice::combat` (prompt, context,
//! option, parse) and its entry in the registry (`render/mod.rs`). Owner: package B1.
//!
//! Ported: the sustain offer. Prompt `"cancel a hit at {system}"`, subtype `sustain_damage`; an option is
//! `sustain|{location}` (kind `sustain`, payload `{unit}`), then the generic decline worded "take the hit"
//! (payload `non_fighters_first` when the loss is bound to non-fighters).
//!
//! Not represented here: the previews (`ShipsInSystem` deltas), the `sustainers` detail and the context
//! (hits owed, target). The string resolver builds those; the dual compares ids, kinds, labels and payload.
//! `parse` cannot recover the unit type from an id without the state, so it answers `None`: the typed side
//! answers by index.

use ti4_engine_core::choice::combat::{Ask, Opt, Subtype, SustainSite};
use ti4_engine_core::choice::Opt as AnyOpt;
use ti4_engine_core::generated::Unit;

use super::{Family, Render, Wording};
use crate::choice::ChoiceOption;
use crate::generated::IdStr;

pub const SUSTAIN_KIND: &str = "sustain";

/// The renderer of the combat questions.
pub struct Fam;

/// The id suffix of a site (`space:3`, `planet:x`, `planet:x:variant:2`).
#[must_use]
pub fn location(site: SustainSite) -> String {
    match site {
        SustainSite::Space(i) => format!("space:{i}"),
        SustainSite::Planet { planet, index, variant: true } => format!("planet:{}:variant:{index}", planet.as_str()),
        SustainSite::Planet { planet, variant: false, .. } => format!("planet:{}", planet.as_str()),
    }
}

fn label(a: &Ask, site: SustainSite, unit: Unit, galvanized: bool) -> String {
    let staging = match a {
        Ask::CancelHit { staging, .. } => staging,
        Ask::AssignHit | Ask::RaidDamage { .. } => &false,
    };
    if let (true, SustainSite::Planet { planet, .. }) = (*staging, site) {
        let id = planet.as_str();
        let place = ti4_content::galaxy::planet(ti4_content::ContentStore::embedded(), id, ti4_model::content_types::FULL)
            .and_then(|record| record.name())
            .unwrap_or(id);
        if unit == Unit::EidolonMaximum {
            let form = if galvanized { "galvanized" } else { "plain" };
            return format!("sustain damage on Maximum on {place} ({form})");
        }
        return format!("sustain damage on {} on {place}", unit.as_str());
    }
    format!("sustain damage on {}", unit.as_str())
}

impl Family for Fam {
    type Ask = Ask;
    type Opt = Opt;
    type Subtype = Subtype;

    fn prompt(_: &Render, a: &Ask) -> String {
        match a {
            Ask::CancelHit { system, .. } => format!("cancel a hit at {}", system.as_str()),
            Ask::AssignHit => "assign a hit".to_owned(),
            Ask::RaidDamage { .. } => "Raid Formation: choose a ship with SUSTAIN DAMAGE to become damaged".to_owned(),
        }
    }

    fn subtype(_r: &Render, s: &Subtype) -> String {
        match s {
            Subtype::SustainDamage => "sustain_damage".to_owned(),
            Subtype::Casualty(code) => _r.code_text(*code),
        }
    }

    fn option(r: &Render, a: &Ask, o: &Opt) -> ChoiceOption {
        let (site, unit, galvanized) = match o {
            Opt::RaidDamage { unit } => {
                let target = match a {
                    Ask::RaidDamage { target } => r.seat_text(*target),
                    _ => "",
                };
                return ChoiceOption::labelled(unit.as_str().to_owned(), "raid_formation_damage", format!("damage 1 {} of {target}", unit.as_str()));
            }
            Opt::Destroy { index, unit, damaged } => {
                let tag = if *damaged { " (damaged)" } else { "" };
                return ChoiceOption::labelled(
                    format!("destroy|{index}"),
                    "casualty",
                    format!("destroy {}{tag}", unit.as_str()),
                )
                .with("unit", unit.as_str().to_owned())
                .with("damaged", *damaged);
            }
            Opt::Sustain { site, unit, galvanized } => (site, unit, galvanized),
        };
        ChoiceOption::labelled(format!("sustain|{}", location(*site)), SUSTAIN_KIND, label(a, *site, *unit, *galvanized))
            .with("unit", unit.as_str().to_owned())
    }

    fn parse(_: &Render, a: &Ask, id: &str) -> Option<Opt> {
        match a {
            Ask::RaidDamage { .. } => Some(Opt::RaidDamage { unit: <Unit as crate::generated::IdStr>::from_id(id)? }),
            _ => None,
        }
    }

    fn wording(_: &Render, a: &Ask, o: &AnyOpt) -> Wording {
        let non_fighters_first = match a {
            Ask::CancelHit { non_fighters_first, .. } => non_fighters_first,
            Ask::AssignHit | Ask::RaidDamage { .. } => &false,
        };
        let mut w = Wording::default();
        if matches!(o, AnyOpt::Decline) {
            w.label = Some("take the hit".to_owned());
            if *non_fighters_first {
                w.payload.insert("non_fighters_first".to_owned(), serde_json::Value::Bool(true));
            }
        }
        w
    }
}
