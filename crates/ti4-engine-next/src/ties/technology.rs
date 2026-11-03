//! From `technology.rs` of the legacy crate (copies). The legacy answers each of these on the typed core first and on the content
//! records when the core cannot say (`dual_typed!`, backend `Core`); so do these.

use std::collections::BTreeMap;

use ti4_content::ContentStore;
use ti4_model::content_types::ContentType;
use ti4_model::id::TechnologyId;

use super::technology_core;

/// The four research tracks. Unit upgrades have no colour (90.7b), which is why
/// `UNITUPGRADE` is deliberately absent.
pub const COLOURS: [&str; 4] = ["BIOTIC", "CYBERNETIC", "PROPULSION", "WARFARE"];

/// The letter each colour is written as in a technology's `requirements` string.
///
/// The corpus spells prerequisites as e.g. `RRRY` — three warfare and one cybernetic — rather
/// than as counts per named track.
#[must_use]
pub fn colour_of(letter: char) -> Option<&'static str> {
    match letter.to_ascii_uppercase() {
        'G' => Some("BIOTIC"),
        'Y' => Some("CYBERNETIC"),
        'B' => Some("PROPULSION"),
        'R' => Some("WARFARE"),
        _ => None,
    }
}

/// Printed technology name used by learned choice labels.
#[must_use]
pub fn name(content: &ContentStore, alias: &TechnologyId) -> String {
    content
        .get(ContentType::Technologies, alias.as_str())
        .and_then(|record| record.text("name"))
        .unwrap_or_else(|| alias.as_str())
        .to_owned()
}

/// The colour a technology itself counts as, if any.
#[must_use]
pub fn colour_type(content: &ContentStore, alias: &TechnologyId) -> Option<&'static str> {
    technology_core::colour_type(alias).unwrap_or_else(|| colour_type_text(content, alias))
}

/// What a technology needs, as counts per colour.
#[must_use]
pub fn prerequisites(content: &ContentStore, alias: &TechnologyId) -> BTreeMap<&'static str, usize> {
    technology_core::prerequisites(alias).unwrap_or_else(|| prerequisites_text(content, alias))
}

/// [`colour_type`] from the content records and the string state: the oracle of the dual run.
#[doc(hidden)]
#[must_use]
pub fn colour_type_text(content: &ContentStore, alias: &TechnologyId) -> Option<&'static str> {
    let record = content.get(ContentType::Technologies, alias.as_str())?;
    let types = record.strings("types");
    COLOURS
        .iter()
        .find(|colour| types.contains(colour))
        .copied()
}

/// [`prerequisites`] from the content records and the string state: the oracle of the dual run.
#[doc(hidden)]
#[must_use]
pub fn prerequisites_text(
    content: &ContentStore,
    alias: &TechnologyId,
) -> BTreeMap<&'static str, usize> {
    let mut needs = BTreeMap::new();
    let Some(record) = content.get(ContentType::Technologies, alias.as_str()) else {
        return needs;
    };
    let printed = record.text("requirements").unwrap_or("").trim();
    // The corpus writes "no prerequisites" as the literal strings "null" and "None" as well as
    // an absent field. They are spelled out here rather than being caught by the
    // is-not-a-colour-letter fallback, because that fallback would make any future typo a free
    // technology instead of an error.
    if printed.is_empty()
        || printed.eq_ignore_ascii_case("null")
        || printed.eq_ignore_ascii_case("none")
    {
        return needs;
    }
    for letter in printed.chars() {
        if let Some(colour) = colour_of(letter) {
            *needs.entry(colour).or_insert(0) += 1;
        }
    }
    needs
}

/// Whether this is a unit upgrade, which has no colour (90.7b).
#[must_use]
pub fn is_unit_upgrade(content: &ContentStore, alias: &TechnologyId) -> bool {
    technology_core::is_unit_upgrade(alias).unwrap_or_else(|| is_unit_upgrade_text(content, alias))
}

/// [`is_unit_upgrade`] from the content records.
#[must_use]
pub fn is_unit_upgrade_text(content: &ContentStore, alias: &TechnologyId) -> bool {
    content.get(ContentType::Technologies, alias.as_str()).is_some_and(|record| record.strings("types").contains(&"UNITUPGRADE"))
}
