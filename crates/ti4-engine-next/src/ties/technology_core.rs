//! From `technology_core.rs` of the legacy crate: the printed-data queries of `ti4_engine_core::query::tech` by alias, and the unit roster
//! of a source scope (copies; the research position queries stay with the string rules).

use std::collections::BTreeMap;

use ti4_content::ContentStore;
use ti4_engine_core::generated::{Faction, FactionTag, TechType, Technology, UnitBaseType, Source as CoreSource};
use ti4_engine_core::query::tech::{self as q, Colours};
use ti4_engine_core::state::TechSet;
use ti4_engine_core::tables::{Scope, source_bit};
use ti4_model::content_types::{Source, SourceSet};
use ti4_model::id::TechnologyId;

use crate::generated::IdStr;

/// The typed technology of an alias.
#[inline]
#[must_use]
pub fn tech(alias: &str) -> Option<Technology> {
    <Technology as IdStr>::from_id(alias)
}

/// The scope mask of a source set (`core_adapter::scope` without the text round trip).
#[must_use]
pub fn scope(sources: SourceSet) -> Scope {
    Scope(sources.iter().fold(0, |mask, s| {
        mask | source_bit(match s {
            Source::Base => CoreSource::Base,
            Source::Codex1 => CoreSource::Codex1,
            Source::Codex2 => CoreSource::Codex2,
            Source::Codex3 => CoreSource::Codex3,
            Source::Codex4 => CoreSource::Codex4,
            Source::Pok => CoreSource::Pok,
            Source::ThundersEdge => CoreSource::ThundersEdge,
        })
    }))
}

/// The corpus spelling of a research track (`technology::COLOURS`).
#[must_use]
pub fn colour_name(t: TechType) -> &'static str {
    <TechType as IdStr>::as_str(t)
}

/// The colour map of the string engine (`BTreeMap<&str, usize>`, zeros absent).
#[must_use]
pub fn colour_map(counts: Colours) -> BTreeMap<&'static str, usize> {
    q::COLOURS
        .into_iter()
        .zip(counts)
        .filter(|(_, n)| *n > 0)
        .map(|(c, n)| (colour_name(c), usize::from(n)))
        .collect()
}

/// A colour name of the string engine (`BIOTIC` ...) as a track.
#[must_use]
pub fn track(name: &str) -> Option<usize> {
    <TechType as IdStr>::from_id(name).and_then(q::colour_index)
}

/// Ids of a typed set, in id order (the order of the string engine's sorted `Vec<TechnologyId>`).
#[must_use]
pub fn ids(set: &TechSet) -> Vec<TechnologyId> {
    set.iter().map(|t| TechnologyId::new(<Technology as IdStr>::as_str(t))).collect()
}

// --- printed data ------------------------------------------------------------------------------

#[must_use]
pub fn colour_type(alias: &TechnologyId) -> Option<Option<&'static str>> {
    Some(q::colour_type(tech(alias.as_str())?).map(colour_name))
}

#[must_use]
pub fn is_unit_upgrade(alias: &TechnologyId) -> Option<bool> {
    Some(q::is_unit_upgrade(tech(alias.as_str())?))
}

#[must_use]
pub fn faction_of(alias: &TechnologyId) -> Option<Option<&'static str>> {
    Some(q::faction_of(tech(alias.as_str())?).map(<FactionTag as IdStr>::as_str))
}

#[must_use]
pub fn replaced_for_faction(faction: &str, alias: &TechnologyId) -> Option<bool> {
    let faction = <Faction as IdStr>::from_id(faction)?;
    Some(q::replaced_for_faction(faction, tech(alias.as_str())?))
}

#[must_use]
pub fn prerequisites(alias: &TechnologyId) -> Option<BTreeMap<&'static str, usize>> {
    Some(colour_map(q::prerequisites(tech(alias.as_str())?)))
}

#[must_use]
pub fn active_aliases(sources: SourceSet) -> std::collections::BTreeSet<TechnologyId> {
    q::active(scope(sources)).iter().map(|t| TechnologyId::new(<Technology as IdStr>::as_str(t))).collect()
}

// --- the string state as a research context ----------------------------------------------------


/// The faction units of a source scope, resolved by the store's reprint rules; built once per scope
/// (`None`: a store that is not the embedded one, whose catalogue the cache cannot share).
pub fn roster_for(content: &ContentStore, sources: SourceSet) -> Option<&'static q::UnitRoster> {
    static ROSTERS: [std::sync::OnceLock<Option<q::UnitRoster>>; 128] = [const { std::sync::OnceLock::new() }; 128];
    if !content.is_embedded() {
        return None;
    }
    let slot = ROSTERS.get(usize::try_from(sources.as_u32()).ok()?)?;
    slot.get_or_init(|| {
        let embedded = ContentStore::embedded();
        let mut roster = q::UnitRoster::default();
        for faction in Faction::ALL {
            for base in UnitBaseType::ALL {
                let found = ti4_content::units::faction_unit(
                    embedded,
                    <Faction as IdStr>::as_str(faction),
                    <UnitBaseType as IdStr>::as_str(base),
                    sources,
                );
                let unit = match found {
                    Some(u) => Some(<ti4_engine_core::generated::Unit as IdStr>::from_id(u.id())?),
                    None => None,
                };
                roster.set(faction, base, unit);
            }
        }
        Some(roster)
    })
    .as_ref()
}
