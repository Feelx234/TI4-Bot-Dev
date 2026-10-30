//! Source hygiene of the typed core (trap T4 of docs/todo/D4-core-tests.md): the core is deterministic by construction, so its production
//! code (everything above the `#[cfg(test)]` module of a file) uses no hash map or set, no clock, no OS entropy, no process environment and
//! no file or console, and floats appear only where the string engine's own arithmetic is floating point (an allow-list that fails when it grows).

use std::path::{Path, PathBuf};

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = rd.filter_map(Result::ok).map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            if p.file_name().is_some_and(|n| n == "generated") {
                continue;
            }
            walk(&p, out);
        } else if p.extension().is_some_and(|e| e == "rs") {
            out.push(p);
        }
    }
}

/// `(path relative to crates/, line number, line)` of every non-comment production line of the two core crates.
fn production_lines() -> Vec<(String, usize, String)> {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("crates directory").to_path_buf();
    let mut files = Vec::new();
    for c in ["ti4-engine-core/src", "ti4-engine-core-factions/src"] {
        walk(&crates.join(c), &mut files);
    }
    assert!(files.len() > 200, "found only {} source files: the walk is broken", files.len());
    let mut out = Vec::new();
    for f in files {
        let text = std::fs::read_to_string(&f).expect("readable source");
        let rel = f.strip_prefix(&crates).expect("under crates").to_string_lossy().replace('\\', "/");
        for (i, line) in text.lines().enumerate() {
            let t = line.trim_start();
            if t.starts_with("#[cfg(test)]") {
                break;
            }
            if t.starts_with("//") {
                continue;
            }
            out.push((rel.clone(), i + 1, line.to_string()));
        }
    }
    out
}

fn hits(pattern: &[&str]) -> Vec<String> {
    production_lines()
        .into_iter()
        .filter(|(_, _, l)| pattern.iter().any(|p| contains_word(l, p)))
        .map(|(f, n, l)| format!("{f}:{n}: {}", l.trim()))
        .collect()
}

fn contains_word(line: &str, word: &str) -> bool {
    let bytes = line.as_bytes();
    let mut from = 0;
    while let Some(at) = line[from..].find(word) {
        let start = from + at;
        let end = start + word.len();
        let before = start == 0 || !(bytes[start - 1].is_ascii_alphanumeric() || bytes[start - 1] == b'_');
        let after = end >= bytes.len() || !(bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_');
        if before && after {
            return true;
        }
        from = end;
    }
    false
}

#[test]
fn the_core_uses_no_hash_map_or_hash_set() {
    assert_eq!(hits(&["HashMap", "HashSet"]), Vec::<String>::new(), "iteration order of a hash container is not deterministic");
}

#[test]
fn the_core_reads_no_clock_no_entropy_no_environment_no_file_and_prints_nothing() {
    let bad = hits(&["SystemTime", "Instant::now", "getrandom", "thread_rng", "OsRng", "println", "eprintln", "dbg"]);
    assert_eq!(bad, Vec::<String>::new());
    let bad = production_lines().into_iter().filter(|(_, _, l)| l.contains("std::env") || l.contains("std::fs") || l.contains("std::process")).map(|(f, n, l)| format!("{f}:{n}: {}", l.trim())).collect::<Vec<_>>();
    assert_eq!(bad, Vec::<String>::new());
}

#[test]
fn floats_are_confined_to_the_trade_deal_valuation_of_the_string_engine() {
    // D4-found-02: docs/todo/D4-core-tests.md T4 says the core has no floats; `query/trade_deals.rs` (B4d) has them, because the string deal
    // book weighs deals in f64 and rounds half to even like Python. They only decide which deals are offered and in what order, never state.
    let files: std::collections::BTreeSet<String> = hits(&["f32", "f64"]).into_iter().map(|h| h.split(':').next().unwrap_or("").to_string()).collect();
    assert_eq!(files.into_iter().collect::<Vec<_>>(), vec!["ti4-engine-core/src/query/trade_deals.rs".to_string()], "a new file with floating point needs a reason");
}

#[test]
fn the_core_has_no_unsafe_code() {
    let bad = production_lines().into_iter().filter(|(_, _, l)| contains_word(l, "unsafe") && !l.contains("forbid")).count();
    assert_eq!(bad, 0);
}

#[test]
fn the_core_names_the_wrapper_in_no_dependency() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf();
    for c in ["ti4-engine-core", "ti4-engine-core-factions"] {
        let manifest = std::fs::read_to_string(crates.join(c).join("Cargo.toml")).unwrap();
        let deps = manifest.split("[dev-dependencies]").next().unwrap();
        for forbidden in ["ti4-engine ", "ti4-engine=", "ti4-engine.", "ti4-model", "ti4-content", "serde"] {
            assert!(!deps.lines().filter(|l| !l.trim_start().starts_with('#')).any(|l| l.trim_start().starts_with(forbidden)), "{c} depends on {forbidden}");
        }
    }
}
