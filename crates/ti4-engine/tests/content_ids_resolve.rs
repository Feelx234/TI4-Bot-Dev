//! Every content id hardcoded in engine production code must exist in the corpus.
//!
//! `relics.rs` guarded the Stellar Converter with `alias == "mecatol_rex"`. No planet has that id
//! -- Mecatol Rex is `mr` in the base game and `mrte` in Thunder's Edge -- so the guard was dead
//! from the day it was written and the relic could destroy Mecatol. Nothing failed, because a
//! comparison against a string that matches nothing is not an error, it is just always false.
//!
//! That is a class, not an incident: a literal id is unchecked at compile time and silent at
//! runtime. This test is the check. It scans production code for lines that handle content ids and
//! asserts every id-shaped literal on them resolves.
//!
//! # Scope, and why it is narrow
//!
//! Only lines that already mention an id accessor or constructor are scanned. The engine is full
//! of lowercase string literals that are not content ids -- option kinds, payload keys, decision
//! names -- and scanning every literal would drown the signal in vocabulary that is correct by
//! construction. Proximity keeps the false-positive rate at zero while still catching the
//! comparison form the Stellar Converter used.
//!
//! Test modules are skipped: a fixture may legitimately name something the corpus does not carry.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use ti4_content::ContentStore;
use ti4_model::content_types::{ContentType, FULL};

/// Lines that touch a content id in a way a wrong literal would silently break.
const ID_CONTEXTS: [&str; 6] = [
    "PlanetId::new(",
    "SystemId::new(",
    ".id()",
    "alias",
    "planets_in(",
    "home_system(",
];

/// Literals on those lines that are engine vocabulary rather than corpus ids.
///
/// Each is a name the engine defines for itself, so the corpus cannot be expected to carry it.
/// Anything not here and not in the corpus is either a typo or a dead branch.
const ENGINE_VOCABULARY: [&str; 23] = [
    // The label of an id in the core_sync codecs' error text (`id::<AgendaId>(S, "agenda", text)`).
    "agenda",
    "decline",
    "activate",
    "system",
    "planet",
    "build",
    "produce",
    "placement",
    "component",
    "expedition",
    "discard",
    "colour",
    "annex",
    "structure",
    "unknown",
    // Payload keys and option kinds, not corpus identities.
    "card",
    "return",
    "alias",
    "objective", // PUBLIC_OBJECTIVE_SCORED payload field, not a corpus identity.
    // The outcomes of a For/Against agenda and the option id / kind of a card offered on an agenda (the agenda reveal and
    // close chains, `agenda_reveal`, `agenda_close` and their tests).
    "for",
    "against",
    "use",
    "promissory_note",
];

/// Record accessors whose argument is a JSON field name, not a content id.
///
/// `record.text("alias")` asks for the field called `alias`; the corpus is not expected to carry
/// a record *named* "alias". Without this the check drowns in field names.
const FIELD_ACCESSORS: [&str; 9] = [
    "text(",
    "strings(",
    "int(",
    "float(",
    "flag(",
    "raw(",
    "number(",
    "with(",
    "payload_string(",
];

fn used_as_field_name(line: &str, literal: &str) -> bool {
    let needle = format!("\"{literal}\"");
    line.match_indices(&needle).any(|(at, _)| {
        let before = &line[..at];
        FIELD_ACCESSORS
            .iter()
            .any(|accessor| before.ends_with(accessor) || before.ends_with(&format!("{accessor} ")))
    })
}

fn is_id_shaped(token: &str) -> bool {
    token.len() >= 3
        && token
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        && token.chars().any(|c| c.is_ascii_alphabetic())
}

/// Literals on one line, in source order.
fn literals(line: &str) -> Vec<String> {
    let mut found = Vec::new();
    let bytes: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == '"' {
            let mut j = i + 1;
            let mut value = String::new();
            while j < bytes.len() && bytes[j] != '"' {
                if bytes[j] == '\\' {
                    j += 1;
                }
                if j < bytes.len() {
                    value.push(bytes[j]);
                }
                j += 1;
            }
            found.push(value);
            i = j + 1;
        } else {
            i += 1;
        }
    }
    found
}

fn source_files(dir: &Path, into: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        // `src/generated/` is the content generator's adapter output (id and text tables, not
        // compiled into the engine yet): data, not code that handles ids.
        if path.is_dir() && path.file_name().is_some_and(|n| n == "generated") {
            continue;
        }
        if path.is_dir() {
            source_files(&path, into);
        } else if path.extension().is_some_and(|e| e == "rs") {
            into.push(path);
        }
    }
}

/// The file with every `#[cfg(test)]` item replaced by blank lines (line numbers are kept).
///
/// An earlier version cut each file at its first `#[cfg(test)]`, which is often a test-only helper
/// in the middle of the file: 12,005 production lines were never scanned (game.rs alone 5,448).
/// The item after the attribute ends at its `;` when that comes before any `{`, else at the brace
/// that closes the first `{`. Braces inside string and char literals and comments are skipped.
fn production_code(text: &str) -> String {
    const ATTR: &str = "#[cfg(test)]";
    let bytes = text.as_bytes();
    let mut keep = vec![true; bytes.len()];
    let mut search = 0;
    while let Some(found) = text[search..].find(ATTR) {
        let start = search + found;
        let end = item_end(bytes, start + ATTR.len());
        keep[start..end].iter_mut().for_each(|k| *k = false);
        search = end;
    }
    text.char_indices()
        .map(|(i, c)| if keep[i] || c == '\n' { c } else { ' ' })
        .collect()
}

/// The end (exclusive) of the item starting at `from`.
fn item_end(bytes: &[u8], from: usize) -> usize {
    let mut i = from;
    let mut depth = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'/' if bytes.get(i + 1) == Some(&b'/') => {
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
                continue;
            }
            b'r' if (bytes.get(i + 1) == Some(&b'"') || bytes.get(i + 1) == Some(&b'#'))
                && (i == 0 || !(bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_')) =>
            {
                // A raw string `r#"..."#`: ends at a quote followed by as many hashes.
                let mut j = i + 1;
                let mut hashes = 0;
                while bytes.get(j) == Some(&b'#') {
                    hashes += 1;
                    j += 1;
                }
                if bytes.get(j) == Some(&b'"') {
                    j += 1;
                    while j < bytes.len()
                        && !(bytes[j] == b'"'
                            && bytes[j + 1..]
                                .iter()
                                .take(hashes)
                                .filter(|b| **b == b'#')
                                .count()
                                == hashes)
                    {
                        j += 1;
                    }
                    i = j + hashes;
                }
            }
            b'"' => {
                i += 1;
                while i < bytes.len() && bytes[i] != b'"' {
                    if bytes[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
            }
            // A char literal (`'{'`, `'\''`); a lifetime (`'a`) has no closing quote nearby.
            b'\'' if bytes.get(i + 2) == Some(&b'\'') => i += 2,
            b'\'' if bytes.get(i + 1) == Some(&b'\\') && bytes.get(i + 3) == Some(&b'\'') => i += 3,
            b';' if depth == 0 => return i + 1,
            b'{' => depth += 1,
            b'}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return i + 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    bytes.len()
}

#[test]
fn test_items_are_blanked_wherever_they_sit() {
    let text = "fn a() {}\n#[cfg(test)]\nfn helper() { let s = \"}\"; let c = '{'; }\nfn b() { x(\"kept\") }\n#[cfg(test)]\nuse foo;\n#[cfg(test)]\nmod tests {\n    fn t() { \"gone\" }\n}\nfn c() {}\n";
    let production = production_code(text);
    assert_eq!(production.lines().count(), text.lines().count());
    assert!(
        production.contains("fn a()")
            && production.contains("\"kept\"")
            && production.contains("fn c()")
    );
    assert!(
        !production.contains("helper")
            && !production.contains("gone")
            && !production.contains("use foo")
    );
}

#[test]
fn every_hardcoded_content_id_resolves_in_the_corpus() {
    let content = ContentStore::embedded();
    // Every identity the corpus carries, across every category: a hardcoded id may name a relic,
    // a technology or a breakthrough as legitimately as a planet, and checking only two
    // categories would report the other twenty-six as dead.
    let mut known: BTreeSet<String> = BTreeSet::new();
    for category in <ti4_model::content_types::ContentType as strum::IntoEnumIterator>::iter() {
        for record in content.records(category) {
            if let Some(id) = record.id() {
                known.insert(id.to_owned());
            }
            if let Some(alias) = record.text("alias") {
                known.insert(alias.to_owned());
            }
            for alias in record.strings("aliases") {
                known.insert(alias.to_owned());
            }
        }
    }
    for planet in ti4_content::galaxy::all_planets(content, FULL).into_keys() {
        known.insert(planet.to_owned());
    }
    for system in ti4_content::galaxy::all_systems(content, FULL).into_keys() {
        known.insert(system.to_owned());
    }
    let vocabulary: BTreeSet<&str> = ENGINE_VOCABULARY.into_iter().collect();

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    source_files(&root, &mut files);
    assert!(!files.is_empty(), "no engine sources found under {root:?}");

    let mut dead: Vec<String> = Vec::new();
    for file in &files {
        let Ok(text) = std::fs::read_to_string(file) else {
            continue;
        };
        // A fixture may name something the corpus does not carry, and that is its business:
        // every `#[cfg(test)]` item is blanked, wherever it sits in the file.
        let production = production_code(&text);
        let name = file.file_name().and_then(|n| n.to_str()).unwrap_or("?");
        for (number, line) in production.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") || trimmed.starts_with("///") {
                continue;
            }
            if !ID_CONTEXTS.iter().any(|needle| line.contains(needle)) {
                continue;
            }
            for literal in literals(line) {
                if !is_id_shaped(&literal)
                    || vocabulary.contains(literal.as_str())
                    || known.contains(&literal)
                    || used_as_field_name(line, &literal)
                {
                    continue;
                }
                dead.push(format!("{name}:{}  {literal:?}", number + 1));
            }
        }
    }

    assert!(
        dead.is_empty(),
        "content ids that match nothing in the corpus -- a comparison against one is always \
         false and a construction of one names a planet or system that does not exist:\n  {}",
        dead.join("\n  ")
    );
}

/// A file that is compiled only for tests: under a `*_tests/` directory (declared by a
/// `#[cfg(test)] mod`), `fixtures.rs`, or a file that starts with `#![cfg(test)]`.
fn is_test_only_file(path: &Path) -> bool {
    path.components().any(|c| {
        c.as_os_str()
            .to_str()
            .is_some_and(|s| s.ends_with("_tests"))
    }) || path.file_name().is_some_and(|n| n == "fixtures.rs")
        // The test module of the core_sync seam (`#[cfg(test)] mod tests;`).
        || path.ends_with("core_sync/tests.rs")
        || std::fs::read_to_string(path).is_ok_and(|t| t.contains("#![cfg(test)]"))
}

/// Typed id constructors and the categories their literal must come from.
const TYPED_CONSTRUCTORS: [(&str, &[ContentType]); 12] = [
    ("PlanetId::new(\"", &[ContentType::Planets]),
    ("SystemId::new(\"", &[ContentType::Systems]),
    ("LeaderId::new(\"", &[ContentType::Leaders]),
    ("UnitTypeId::new(\"", &[ContentType::Units]),
    ("TechnologyId::new(\"", &[ContentType::Technologies]),
    ("ActionCardId::new(\"", &[ContentType::ActionCards]),
    ("FactionId::new(\"", &[ContentType::Factions]),
    (
        "SecretObjectiveId::new(\"",
        &[ContentType::SecretObjectives],
    ),
    (
        "ObjectiveId::new(\"",
        &[ContentType::PublicObjectives, ContentType::SecretObjectives],
    ),
    ("StrategyCardId::new(\"", &[ContentType::StrategyCards]),
    ("BreakthroughId::new(\"", &[ContentType::Breakthroughs]),
    ("RelicId::new(\"", &[ContentType::Relics]),
];

/// A literal passed to a typed id constructor in production code is an id of *that* category.
///
/// The test above accepts an id of any category, and 36 ids are spelled the same in two
/// categories (`mr` is a technology and a planet, `sar` a technology and a secret), so a
/// `TechnologyId::new("mr")` meant as a planet would pass it. This one checks the category.
#[test]
fn typed_id_literals_name_their_own_category() {
    let content = ContentStore::embedded();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    source_files(&root, &mut files);
    let mut wrong = Vec::new();
    let mut checked = 0;
    for file in files.iter().filter(|f| !is_test_only_file(f)) {
        let Ok(text) = std::fs::read_to_string(file) else {
            continue;
        };
        let production = production_code(&text);
        let name = file.file_name().and_then(|n| n.to_str()).unwrap_or("?");
        for (constructor, categories) in TYPED_CONSTRUCTORS {
            for (at, _) in production.match_indices(constructor) {
                // `FactionId::new(` must not also match inside `SecretObjectiveId::new(` etc.
                let before = production[..at].chars().next_back();
                if before.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_') {
                    continue;
                }
                let rest = &production[at + constructor.len()..];
                let literal = &rest[..rest.find('"').unwrap_or(0)];
                checked += 1;
                if !categories
                    .iter()
                    .any(|c| content.get(*c, literal).is_some())
                {
                    let line = production[..at].lines().count();
                    wrong.push(format!(
                        "{name}:{line}  {constructor}{literal}\") not in {categories:?}"
                    ));
                }
            }
        }
    }
    assert!(checked > 0, "no typed id literals found to check");
    assert!(
        wrong.is_empty(),
        "typed id literals that are not ids of their category:\n  {}",
        wrong.join("\n  ")
    );
}

/// Constructors whose argument becomes a `DecisionSource::ActionCard` id.
const ACTION_CARD_SOURCES: [&str; 2] = ["DecisionSource::ActionCard(", "played_card_source("];

/// Every action-card decision source the engine emits names a card in `action_cards.json`.
///
/// Night 1 (F4) found the same card under two ids in the decision traces: the effects built their
/// sources from names (`public_disgrace`, `skilled_retreat`, `exchange_program`,
/// `in_the_silence_of_space`) while the play itself carried the content id (`disgrace`,
/// `s_retreat1..4`, `exchangeprogram`, `silence_space`). Sources now come from the played card;
/// a literal is allowed only as the fallback for a direct call, and it must still be a real id.
#[test]
fn every_action_card_decision_source_names_an_action_card() {
    let content = ContentStore::embedded();
    let cards: BTreeSet<String> = content
        .records(ti4_model::content_types::ContentType::ActionCards)
        .iter()
        .filter_map(|record| record.text("alias").map(str::to_owned))
        .collect();
    assert!(cards.contains("disgrace"), "action card corpus not loaded");

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    source_files(&root, &mut files);
    let mut wrong: Vec<String> = Vec::new();
    let mut checked = 0;
    for file in &files {
        let Ok(text) = std::fs::read_to_string(file) else {
            continue;
        };
        let production = production_code(&text);
        let name = file.file_name().and_then(|n| n.to_str()).unwrap_or("?");
        for constructor in ACTION_CARD_SOURCES {
            for (at, _) in production.match_indices(constructor) {
                if production[..at].ends_with("fn ") {
                    continue; // the helper's own definition
                }
                // The argument may sit on the next line. Only a string literal is checked: an
                // expression (the played card's id) is correct by construction.
                let argument = production[at + constructor.len()..].trim_start();
                let Some(literal) = argument.strip_prefix('"') else {
                    continue;
                };
                let literal = &literal[..literal.find('"').unwrap_or(0)];
                checked += 1;
                if !cards.contains(literal) {
                    let line = production[..at].lines().count();
                    wrong.push(format!("{name}:{line}  {literal:?}"));
                }
            }
        }
    }
    assert!(
        checked > 0,
        "no action-card decision sources found to check"
    );
    assert!(
        wrong.is_empty(),
        "action-card decision sources that are not action_cards.json ids:\n  {}",
        wrong.join("\n  ")
    );
}
