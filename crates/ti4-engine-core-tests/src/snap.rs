//! File snapshots for scenario tests. `assert_snapshot("area/name", &text)` compares with `tests/snapshots/area/name.snap`.
//!
//! * `BLESS=<area> cargo test -p ti4-engine-core-tests <filter>` (or `BLESS=1`) rewrites the file and prints its path; review it as an ordinary git diff.
//! * A missing file FAILS (it is never silently created); so does a stale file that no test names ([`stale_snapshots`]).
//! * No hidden information goes into a snapshot (never a deck order or another seat's hand that the scenario did not author).

use std::path::{Path, PathBuf};

/// The directory of the snapshots of this crate.
#[must_use]
pub fn snapshot_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join("snapshots")
}

/// `BLESS=1` blesses every snapshot of the run; `BLESS=<text>` only those whose name contains the text (so a run of the whole suite can
/// bless one area without touching the tests of the kit that expect a missing or differing snapshot to fail).
fn blessing(name: &str) -> bool {
    std::env::var("BLESS").is_ok_and(|v| v == "1" || (!v.is_empty() && name.contains(&v)))
}

/// Compare `text` with the snapshot `name` (`area/name`, no extension). Trailing whitespace of lines and a missing final newline do not count.
///
/// # Panics
/// When the file is missing or differs (unless `BLESS=1`).
pub fn assert_snapshot(name: &str, text: &str) {
    assert!(!name.contains(".."), "a snapshot name stays inside the snapshot directory");
    let path = snapshot_dir().join(format!("{name}.snap"));
    let norm = |t: &str| t.lines().map(str::trim_end).collect::<Vec<_>>().join("\n");
    let want = norm(text);
    if blessing(name) {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).expect("create the snapshot directory");
        }
        std::fs::write(&path, format!("{want}\n")).expect("write the snapshot");
        eprintln!("blessed {}", path.display());
        return;
    }
    let have = match std::fs::read_to_string(&path) {
        Ok(h) => norm(&h),
        Err(e) => panic!("snapshot {} is missing ({e}); run with BLESS=1 and review the file\n--- actual ---\n{want}", path.display()),
    };
    assert!(have == want, "snapshot {name} differs\n--- expected ({}) ---\n{have}\n--- actual ---\n{want}", path.display());
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = rd.filter_map(Result::ok).map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            walk(&p, out);
        } else {
            out.push(p);
        }
    }
}

/// The snapshot files whose name no `.rs` file under `tests/` mentions as a string literal (`"area/name"`).
#[must_use]
pub fn stale_snapshots() -> Vec<String> {
    let root = snapshot_dir();
    let tests = root.parent().expect("tests directory").to_path_buf();
    let mut sources = Vec::new();
    let mut files = Vec::new();
    walk(&tests, &mut files);
    for f in &files {
        if f.extension().is_some_and(|e| e == "rs") {
            sources.push(std::fs::read_to_string(f).unwrap_or_default());
        }
    }
    let mut stale = Vec::new();
    for f in files {
        let Ok(rel) = f.strip_prefix(&root) else { continue };
        if rel.extension().is_none_or(|e| e != "snap") {
            continue;
        }
        let name = rel.with_extension("").to_string_lossy().replace('\\', "/");
        let quoted = format!("\"{name}\"");
        if !sources.iter().any(|s| s.contains(&quoted)) {
            stale.push(name);
        }
    }
    stale
}
