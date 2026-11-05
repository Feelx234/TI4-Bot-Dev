//! The `covers!` registry (docs/todo/D4-core-tests.md 6.2): a test that drives a window or an effect declares it, and the coverage
//! report (`tools/engine_core/d4/coverage.py`, lane B) joins the declarations with the inventory of `impl Window` types.
//!
//! Every declaration is kept in a process-wide set and appended as one tab-separated line `kind<TAB>label<TAB>test` to the file named by
//! `D4_COVERS_FILE`, else `$CARGO_TARGET_DIR/d4-covers.txt`, else `target/d4-covers.txt` (append mode under a mutex; no exit hook is
//! needed because the report deduplicates).

use std::collections::BTreeSet;
use std::io::Write as _;
use std::sync::Mutex;

static REGISTRY: Mutex<BTreeSet<(String, String)>> = Mutex::new(BTreeSet::new());
static FILE: Mutex<()> = Mutex::new(());

/// Where the lines go.
#[must_use]
pub fn covers_file() -> std::path::PathBuf {
    if let Ok(p) = std::env::var("D4_COVERS_FILE") {
        return p.into();
    }
    let dir = std::env::var("CARGO_TARGET_DIR").unwrap_or_else(|_| "target".into());
    std::path::Path::new(&dir).join("d4-covers.txt")
}

/// Record that the calling test covers `label` of `kind` (`window`, `card`, `slot`, `stage`).
pub fn register(kind: &str, label: &str, test: &str) {
    REGISTRY.lock().unwrap_or_else(|e| e.into_inner()).insert((kind.to_string(), label.to_string()));
    let _guard = FILE.lock().unwrap_or_else(|e| e.into_inner());
    let path = covers_file();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(f, "{kind}\t{label}\t{test}");
    }
}

/// Like [`register`], but only the first time this process sees the pair (the stage probe reports every step of every window: the file
/// would otherwise grow by a line per step).
fn register_once(kind: &str, label: &str, test: &str) {
    let new = REGISTRY.lock().unwrap_or_else(|e| e.into_inner()).insert((kind.to_string(), label.to_string()));
    if !new {
        return;
    }
    let _guard = FILE.lock().unwrap_or_else(|e| e.into_inner());
    let path = covers_file();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(f, "{kind}\t{label}\t{test}");
    }
}

/// The stage probe of the core (`ti4_engine_core::driver::probe`): every window that runs in a test reports its type and its stage, so a
/// test that drives a window through `driver::live`, a job of the turn flow or a `Driver` of its own counts as covering it, with the stage
/// variants it passed (the `Rig` registers the same lines; the report deduplicates).
fn trace(window: &'static str, stage: &dyn std::fmt::Debug) {
    let test = std::thread::current().name().unwrap_or("?").to_string();
    register_once("stage", &format!("{window}::{stage:?}"), &test);
    register_once("window", window, &test);
}

/// Install the stage probe on the calling thread (the scenario builder and the rig do it: a test thread that builds a position is traced).
pub fn install_probe() {
    ti4_engine_core::driver::probe::set(Some(trace));
}

/// What this process has registered so far.
#[must_use]
pub fn registered() -> Vec<(String, String)> {
    REGISTRY.lock().unwrap_or_else(|e| e.into_inner()).iter().cloned().collect()
}

/// `covers!("rules::invasion::stages::CommitWindow")`: the test drives that window.
#[macro_export]
macro_rules! covers {
    ($label:expr) => {
        $crate::covers::register("window", $label, module_path!())
    };
}

/// `covers_card!(ActionCard::Spy)`: the test drives the play of that action card.
#[macro_export]
macro_rules! covers_card {
    ($card:expr) => {
        $crate::covers::register("card", &format!("{:?}", $card), module_path!())
    };
}

/// `covers_slot!("movement.gravity_drive")`: the test drives that hook slot.
#[macro_export]
macro_rules! covers_slot {
    ($label:expr) => {
        $crate::covers::register("slot", $label, module_path!())
    };
}
