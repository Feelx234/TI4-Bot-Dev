//! Three-line pure reads of the content that the setup needs and that the legacy engine keeps in
//! modules this lane does not own (`transit`, `fracture`).

use ti4_content::ContentStore;
use ti4_content::units::catalogue;
use ti4_model::content_types::SourceSet;
use ti4_model::id::SystemId;
use ti4_model::units::Unit;

/// The capacity of one ship (`transit::capacity_of`).
#[must_use]
pub fn capacity_of(content: &ContentStore, sources: SourceSet, unit: &Unit) -> i64 {
    catalogue(content, sources)
        .get(unit.type_id.as_str())
        .map_or(0, ti4_content::units::UnitType::capacity)
}

/// The `tileBack` of the Fracture's systems (`fracture::FRACTURE_BACK`).
const FRACTURE_BACK: &str = "fracture";

/// Whether a system belongs to the Fracture rather than the regular map
/// (`fracture::is_fracture_system_text`).
#[must_use]
pub fn is_fracture_system(content: &ContentStore, sources: SourceSet, system: &SystemId) -> bool {
    ti4_content::galaxy::system(content, system.as_str(), sources)
        .is_some_and(|tile| tile.record().text("tileBack") == Some(FRACTURE_BACK))
}
