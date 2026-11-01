//! From `action_cards_view.rs` of the legacy crate (copy).

/// [`reveal_set`] from the lines (base type texts) of the seat's units in the active system: the live loop reads them from the core
/// position (`live::plays`), so both sides share the matching, quirks included.
pub fn reveal_set_of_lines(lines: &[&str], sources: &ti4_model::content_types::SourceSet) -> ti4_engine_core::state::TechSet {
    use ti4_model::content_types::ContentType;
    let content = ti4_content::ContentStore::embedded();
    let mut out = ti4_engine_core::state::TechSet::default();
    if lines.is_empty() {
        return out;
    }
    let types = ti4_content::units::catalogue(content, sources.clone());
    for tech in ti4_engine_core::generated::Technology::ALL {
        let alias = ti4_model::id::TechnologyId::new(crate::generated::IdStr::as_str(tech));
        if !crate::ties::technology::is_unit_upgrade(content, &alias) {
            continue;
        }
        let Some(record) = content.get(ContentType::Technologies, alias.as_str()) else { continue };
        let base = record.text("baseUpgrade").unwrap_or_default().to_owned();
        let matches = if base.is_empty() {
            let name = record.text("name").unwrap_or_default().to_lowercase().replace(" ii", "").replace(' ', "_");
            lines.iter().any(|line| name == *line || name.ends_with(&format!("_{line}")))
        } else {
            let subject_line = types.get(base.as_str()).map(ti4_content::units::UnitType::base_type);
            lines.iter().any(|line| Some(*line) == subject_line) || subject_line.is_none() && lines.iter().any(|line| **line == base)
        };
        if matches {
            out.insert(tech);
        }
    }
    out
}
