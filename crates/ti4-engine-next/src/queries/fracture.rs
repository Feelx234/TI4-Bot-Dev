//! The Fracture reads of the surface (`fracture.rs`, content only: the tile back is printed).

use ti4_content::ContentStore;
use ti4_model::content_types::{ContentType, SourceSet};
use ti4_model::id::SystemId;
pub use crate::builders::support::is_fracture_system;

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

