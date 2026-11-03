//! The Fracture reads of the surface (`fracture.rs`, content only: the tile back is printed).

use ti4_content::ContentStore;
use ti4_model::content_types::{ContentType, SourceSet};
use ti4_model::id::SystemId;

const FRACTURE_BACK: &str = "fracture";

/// Every Fracture system, in corpus order.
#[must_use]
pub fn systems(content: &ContentStore, sources: SourceSet) -> Vec<SystemId> {
    content
        .from_sources(ContentType::Systems, sources)
        .filter(|record| record.text("tileBack") == Some(FRACTURE_BACK))
        .filter_map(|record| record.id().map(SystemId::new))
        .collect()
}

/// Whether a system belongs to the Fracture rather than the regular map.
#[must_use]
pub fn is_fracture_system(content: &ContentStore, sources: SourceSet, system: &SystemId) -> bool {
    ti4_content::galaxy::system(content, system.as_str(), sources).is_some_and(|tile| tile.record().text("tileBack") == Some(FRACTURE_BACK))
}
