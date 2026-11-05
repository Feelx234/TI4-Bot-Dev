//! The unit roster of a source scope and the scope mask (moved from the thin crate's `ties::technology_core`).

use ti4_content::ContentStore;
use ti4_engine_core::generated::{Faction, UnitBaseType, Source as CoreSource};
use ti4_engine_core::query::tech as q;
use ti4_engine_core::tables::{Scope, source_bit};
use ti4_model::content_types::{Source, SourceSet};

use crate::ids::IdStr;

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
