//! `cargo xtask gen-cards`: the committed output is current, a re-run is a no-op, a new record
//! only appends, a name collision fails.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use xtask::content_gen::{self, Config};

fn repo() -> Config {
    Config::for_repo(&Config::repo_root())
}

/// The committed generated files and names files match what the generator produces.
#[test]
fn generated_files_are_current() {
    if let Err(e) = content_gen::check(&repo()) {
        panic!("{e:#}");
    }
}

#[test]
fn a_rerun_is_a_no_op() {
    let cfg = repo();
    let first = content_gen::generate(&cfg).unwrap();
    assert!(
        first.notes.iter().all(|n| !n.contains("new id")),
        "{:?}",
        first.notes
    );
    let second = content_gen::generate(&cfg).unwrap();
    assert_eq!(first.files, second.files);
}

/// A scratch copy of the corpus and the names files, with outputs pointed into it.
fn scratch(name: &str) -> (Config, PathBuf) {
    let base = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("gen_cards_{name}"));
    let _ = std::fs::remove_dir_all(&base);
    let repo = repo();
    let content = base.join("content");
    let names = base.join("names");
    copy_dir(&repo.content_dir, &content, "json");
    copy_dir(&repo.names_dir, &names, "toml");
    let mut cfg = repo;
    cfg.content_dir = content;
    cfg.names_dir = names;
    let fields = base.join("card_fields.json");
    std::fs::copy(&cfg.fields_file, &fields).unwrap();
    cfg.fields_file = fields;
    cfg.content_tables_out = base.join("card_fields_tables.rs");
    cfg.core_out = base.join("core");
    cfg.adapter_out = base.join("adapter");
    (cfg, base)
}

fn copy_dir(from: &Path, to: &Path, ext: &str) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap().flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == ext) {
            std::fs::copy(&path, to.join(path.file_name().unwrap())).unwrap();
        }
    }
}

fn add_action_card(cfg: &Config, alias: &str, name: &str) {
    let path = cfg.content_dir.join("action_cards.json");
    let mut cards: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    cards.as_array_mut().unwrap().push(json!({
        "alias": alias, "name": name, "phase": "Action", "window": "Action",
        "text": "Test card.", "source": "base"
    }));
    std::fs::write(&path, serde_json::to_string(&cards).unwrap()).unwrap();
}

#[test]
fn adding_a_record_appends_one_name_and_renames_nothing() {
    let (cfg, _base) = scratch("append");
    let names_path = cfg.names_dir.join("action_cards.toml");
    let before = std::fs::read_to_string(&names_path).unwrap();
    add_action_card(&cfg, "mb5", "Morale Boost");
    let out = content_gen::generate(&cfg).unwrap();
    let after = &out.files[&names_path];
    let old: Vec<&str> = before.lines().collect();
    let new: Vec<&str> = after.lines().collect();
    assert_eq!(new.len(), old.len() + 1);
    let added: Vec<&&str> = new.iter().filter(|l| !old.contains(l)).collect();
    assert_eq!(added, [&"\"mb5\" = \"MoraleBoost5\""]);
    assert!(
        old.iter().all(|l| new.contains(l)),
        "an existing entry changed"
    );
    assert!(out.notes.iter().any(|n| n.contains("mb5 -> MoraleBoost5")));

    // The variant sits at the id's place in byte order: right after mb4.
    let core = &out.files[&cfg.core_out.join("action_cards.rs")];
    let at4 = core.find("    MoraleBoost4 = ").unwrap();
    let at5 = core.find("    MoraleBoost5 = ").unwrap();
    assert!(at5 > at4 && !core[at4..at5].lines().skip(1).any(|l| l.contains(" = ")));

    // Writing it and running again changes nothing.
    content_gen::write(&out).unwrap();
    let again = content_gen::generate(&cfg).unwrap();
    assert!(content_gen::stale(&again).is_empty());
    assert!(again.notes.iter().all(|n| !n.contains("new id")));
}

#[test]
fn a_name_collision_fails_and_asks_for_an_override() {
    let (cfg, _base) = scratch("collision");
    // Derives `Sabotage1`, which `sabo1` already holds.
    add_action_card(&cfg, "zz_sabotage", "Sabotage 1");
    let err = content_gen::generate(&cfg).unwrap_err().to_string();
    assert!(err.contains("collision in `action_cards`"), "{err}");
    assert!(err.contains("Sabotage1: sabo1, zz_sabotage"), "{err}");
    assert!(err.contains("override"), "{err}");

    // The override is the way out.
    let names_path = cfg.names_dir.join("action_cards.toml");
    let mut names = std::fs::read_to_string(&names_path).unwrap();
    names.push_str("\"zz_sabotage\" = \"SabotageTest\"  # override: test\n");
    std::fs::write(&names_path, names).unwrap();
    let out = content_gen::generate(&cfg).unwrap();
    assert!(out.files[&cfg.core_out.join("action_cards.rs")].contains("    SabotageTest = "));
}

#[test]
fn an_overridden_name_is_kept() {
    let (cfg, _base) = scratch("override");
    let names_path = cfg.names_dir.join("relics.toml");
    let text = std::fs::read_to_string(&names_path).unwrap();
    let edited = text.replace(
        "\"codex\" = \"TheCodex\"",
        "\"codex\" = \"Codex\"  # override: test",
    );
    assert_ne!(text, edited, "relics.toml should name codex TheCodex");
    std::fs::write(&names_path, &edited).unwrap();
    let out = content_gen::generate(&cfg).unwrap();
    assert_eq!(out.files[&names_path], edited);
    assert!(out.files[&cfg.core_out.join("relics.rs")].contains("    Codex = "));
}

#[test]
fn check_reports_a_stale_file() {
    let (cfg, _base) = scratch("stale");
    let out = content_gen::generate(&cfg).unwrap();
    content_gen::write(&out).unwrap();
    assert!(content_gen::check(&cfg).is_ok());
    add_action_card(&cfg, "zz_new", "Brand New");
    let err = content_gen::check(&cfg).unwrap_err().to_string();
    assert!(
        err.contains("action_cards.rs") && err.contains("action_cards.toml"),
        "{err}"
    );
    // A stray file in an output directory is stale too.
    content_gen::write(&content_gen::generate(&cfg).unwrap()).unwrap();
    std::fs::write(cfg.core_out.join("old_category.rs"), "").unwrap();
    let err = content_gen::check(&cfg).unwrap_err().to_string();
    assert!(err.contains("old_category.rs"), "{err}");
}

#[test]
fn the_core_output_has_no_string_literals() {
    let out = content_gen::generate(&repo()).unwrap();
    let core = repo().core_out;
    for (path, text) in &out.files {
        if path.starts_with(&core) {
            for (n, line) in text.lines().enumerate() {
                let code = line.split("//").next().unwrap_or(line);
                assert!(!code.contains('"'), "{}:{}: {line}", path.display(), n + 1);
            }
        }
    }
}

/// Replace `from` by `to` in a scratch copy of a corpus file (it must occur).
fn edit_corpus(cfg: &Config, file: &str, from: &str, to: &str) {
    let path = cfg.content_dir.join(file);
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains(from), "{file} should contain {from:?}");
    std::fs::write(&path, text.replacen(from, to, 1)).unwrap();
}

#[test]
fn an_unmapped_action_card_window_fails_the_generator_and_the_check() {
    let (cfg, _base) = scratch("unmapped_window");
    content_gen::write(&content_gen::generate(&cfg).unwrap()).unwrap();
    edit_corpus(
        &cfg,
        "action_cards.json",
        "At the start of the strategy phase",
        "At the very start of the strategy phase",
    );
    for err in [
        content_gen::generate(&cfg).unwrap_err(),
        content_gen::check(&cfg).unwrap_err(),
    ] {
        let err = format!("{err:#}");
        assert!(err.contains("Timing"), "{err}");
        assert!(
            err.contains("At the very start of the strategy phase"),
            "{err}"
        );
        assert!(err.contains("card_fields.json"), "{err}");
    }
}

#[test]
fn an_unmapped_agenda_target_leader_window_or_strategy_card_id_fails() {
    for (name, file, from, to, enm) in [
        (
            "target",
            "agendas.json",
            "Elect Law",
            "Elect Laws",
            "AgendaElects",
        ),
        (
            "leader",
            "leaders.json",
            "During the action phase:",
            "During the action phase,",
            "LeaderWindow",
        ),
        (
            "strategy",
            "strategy_cards.json",
            "\"pok6warfare\"",
            "\"pok6warfare2\"",
            "StrategyCardKind",
        ),
    ] {
        let (cfg, _base) = scratch(&format!("unmapped_{name}"));
        edit_corpus(&cfg, file, from, to);
        let err = format!("{:#}", content_gen::generate(&cfg).unwrap_err());
        assert!(
            err.contains(enm) && err.contains("card_fields.json"),
            "{err}"
        );
    }
}

#[test]
fn a_table_row_the_corpus_no_longer_prints_fails() {
    let (cfg, _base) = scratch("dead_row");
    let fields = std::fs::read_to_string(&cfg.fields_file).unwrap();
    let row = "[\"Elect Law\", \"Law\"],";
    assert!(fields.contains(row));
    std::fs::write(
        &cfg.fields_file,
        fields.replace(
            row,
            &format!("{row}\n      [\"Elect Nobody\", \"Player\"],"),
        ),
    )
    .unwrap();
    let err = format!("{:#}", content_gen::generate(&cfg).unwrap_err());
    assert!(
        err.contains("does not print") && err.contains("Elect Nobody"),
        "{err}"
    );
}

#[test]
fn an_odd_researchable_text_fails_and_the_marker_list_is_checked() {
    let (cfg, _base) = scratch("researchable");
    edit_corpus(
        &cfg,
        "technologies.json",
        "This technology cannot be researched.",
        "This technology cannot be researched. (Not even once.)",
    );
    let err = format!("{:#}", content_gen::generate(&cfg).unwrap_err());
    assert!(err.contains("cannot be researched"), "{err}");

    let (cfg, _base) = scratch("researchable_list");
    let fields = std::fs::read_to_string(&cfg.fields_file).unwrap();
    std::fs::write(&cfg.fields_file, fields.replace("\"parasite-obs\", ", "")).unwrap();
    let err = format!("{:#}", content_gen::generate(&cfg).unwrap_err());
    assert!(err.contains("not_researchable"), "{err}");
}

#[test]
fn the_content_tables_are_the_data_file_and_the_enum_orders_are_stable() {
    let out = content_gen::generate(&repo()).unwrap();
    let core = &out.files[&repo().core_out.join("values.rs")];
    // Timing is numbered in the byte order of the printed sentence: Action first.
    assert!(
        core.contains("pub enum Timing {\n    Action = 0,"),
        "{core}"
    );
    // Covert Legislation's hidden-agenda target has its own variant.
    assert!(core.contains("    HiddenNextAgenda = "));
    let tables = &out.files[&repo().content_tables_out];
    assert!(tables.contains("action_card_windows! {\n    Action = \"Action\","));
    assert!(tables.contains("const ELECT_TYPE_ROWS: &[(&str, ElectType)]"));
    // Every category has a rank table in the core.
    for cat in content_gen::spec::categories() {
        let file = &out.files[&repo().core_out.join(format!("{}.rs", cat.key))];
        assert!(
            file.contains(&format!("pub static {}_NAME_RANK", cat.upper)),
            "{}",
            cat.key
        );
    }
}
