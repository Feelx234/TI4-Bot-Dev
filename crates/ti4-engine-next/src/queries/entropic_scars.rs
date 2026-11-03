//! `entropic_scars.rs` of the legacy crate, as far as the product surface uses it. Plain reads of the printed records, copied.

use ti4_content::ContentStore;
use ti4_model::content_types::SourceSet;
use ti4_model::id::SystemId;

/// Whether this system is an entropic scar.
#[must_use]
pub fn is_scar(content: &ContentStore, sources: SourceSet, system: &SystemId) -> bool {
    ti4_content::galaxy::system(content, system.as_str(), sources).is_some_and(|tile| tile.is_scar())
}

/// Whether a unit ability may be used, given where it is used from and what it is used against (rules 2 and 4). `against` is `None` for an
/// ability with no target outside its own system, in which case only rule 2 applies.
#[must_use]
pub fn abilities_usable(content: &ContentStore, sources: SourceSet, from: &SystemId, against: Option<&SystemId>) -> bool {
    if is_scar(content, sources, from) {
        return false; // rule 2
    }
    match against {
        Some(target) => !is_scar(content, sources, target), // rule 4
        None => true,
    }
}
