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
    let Ask::CancelHit { staging, .. } = a;
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
        let Ask::CancelHit { system, .. } = a;
        format!("cancel a hit at {}", system.as_str())
    }

    fn subtype(_: &Render, s: &Subtype) -> String {
        match s {
            Subtype::SustainDamage => "sustain_damage".to_owned(),
        }
    }

    fn option(_: &Render, a: &Ask, o: &Opt) -> ChoiceOption {
        let Opt::Sustain { site, unit, galvanized } = o;
        ChoiceOption::labelled(format!("sustain|{}", location(*site)), SUSTAIN_KIND, label(a, *site, *unit, *galvanized))
            .with("unit", unit.as_str().to_owned())
    }

    fn parse(_: &Render, _: &Ask, _: &str) -> Option<Opt> {
        None
    }

    fn wording(_: &Render, a: &Ask, o: &AnyOpt) -> Wording {
        let Ask::CancelHit { non_fighters_first, .. } = a;
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
