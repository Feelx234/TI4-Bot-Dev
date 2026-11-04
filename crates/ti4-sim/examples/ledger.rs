//! Every coverage ledger this engine keeps, printed together.
fn main() {
    let content = ti4_content::ContentStore::embedded();
    let pok = ti4_model::content_types::DEFAULT;
    let scope = ti4_engine::seating::IN_SCOPE_FACTIONS;
    println!("{}", ti4_engine_legacy::registry::report(content, pok));
    println!(
        "leaders unimplemented (in scope): {:?}",
        ti4_engine_legacy::leaders::unimplemented(content, &scope)
    );
    println!(
        "faction abilities blocked: {:?}",
        ti4_engine_legacy::faction_abilities::blocked()
            .keys()
            .collect::<Vec<_>>()
    );
    println!(
        "reaction windows unsupported: {}",
        ti4_engine_legacy::reactions::unsupported_windows().len()
    );
    println!(
        "bot choice kinds unscored: {:?}",
        ti4_policy::bot::unscored_kinds()
    );
}
