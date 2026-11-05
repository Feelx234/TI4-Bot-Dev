//! The leaders a faction begins with (pure content: the thin crate's `builders::leaders` re-exports these and writes them to a string seat).

use ti4_content::ContentStore;
use ti4_model::content_types::{ContentType, SourceSet};
use ti4_model::id::LeaderId;
use ti4_model::state::LeaderStatus;

use crate::factions::tag_belongs_to;

/// The `type` of an agent.
pub const AGENT: &str = "agent";

/// What kind of leader this is, lower case (`leaders::kind_of`).
#[must_use]
pub fn kind_of(content: &ContentStore, leader: &LeaderId) -> Option<String> {
    content
        .get(ContentType::Leaders, leader.as_str())
        .and_then(|record| record.text("type"))
        .map(str::to_ascii_lowercase)
}

/// A leader record's id.
fn record_id(record: &ti4_content::Record) -> Option<&str> {
    record.text("id").or_else(|| record.text("alias"))
}

/// The leaders a faction has, in corpus order (`leaders::for_faction`).
///
/// A record that replaces another (`homebrewReplacesID`) excludes the replaced one from scope. The
/// faction sheet names its leaders; alternates it does not list are dealt only where the sheet's
/// own leader of that type is out of scope.
#[must_use]
pub fn for_faction(content: &ContentStore, sources: SourceSet, faction: &str) -> Vec<LeaderId> {
    let records = content
        .from_sources(ContentType::Leaders, sources)
        .filter(|record| {
            record
                .text("faction")
                // Equivalent on the corpus: no leader record is tagged `keleresm`, the one tag `unit` adds.
                .is_some_and(|owner| tag_belongs_to(owner, faction, false)) // mutants:skip
        })
        .collect::<Vec<_>>();
    let replaced: std::collections::BTreeSet<&str> = records
        .iter()
        .filter_map(|record| record.text("homebrewReplacesID"))
        .collect();
    let sheet: Vec<&str> = ti4_content::factions::get(content, faction)
        .map(|record| record.leaders())
        .unwrap_or_default();
    let available: std::collections::BTreeSet<&str> =
        records.iter().filter_map(|record| record_id(record)).collect();
    let missing_types: std::collections::BTreeSet<String> = sheet
        .iter()
        .filter(|listed| !available.contains(*listed))
        .filter_map(|listed| kind_of(content, &LeaderId::new(*listed)))
        .collect();
    let on_sheet = |record: &&ti4_content::Record| {
        sheet.is_empty()
            || record_id(record).is_some_and(|id| sheet.contains(&id))
            || record
                .text("homebrewReplacesID")
                .is_some_and(|original| sheet.contains(&original))
            || record
                .text("type")
                .is_some_and(|kind| missing_types.contains(&kind.to_ascii_lowercase()))
    };
    records
        .into_iter()
        .filter(on_sheet)
        .filter_map(|record| record.text("id").or_else(|| record.text("alias")))
        .map(LeaderId::new)
        .filter(|leader| !replaced.contains(leader.as_str()))
        .collect()
}

/// 51.2a: a faction begins with its agents readied and everything else locked.
#[must_use]
pub fn starting_states(
    content: &ContentStore,
    sources: SourceSet,
    faction: &str,
) -> Vec<(LeaderId, LeaderStatus)> {
    for_faction(content, sources, faction)
        .into_iter()
        .map(|leader| {
            let status = if kind_of(content, &leader).as_deref() == Some(AGENT) {
                LeaderStatus::Readied
            } else {
                LeaderStatus::Locked
            };
            (leader, status)
        })
        .collect()
}
