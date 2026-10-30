//! The census of refusals (package D2-1, docs/todo/D2-prereqs.md item 3 and 4).
//!
//! Every [`Unmapped`](super::Unmapped) is built by [`bad`](super::error::bad) (and every
//! `core_adapter::Unmapped` by its `noted` constructor), which reports it here. Two counters exist
//! because the dual report is process-wide and parallel tests steal each other's counts (trap T6):
//!
//! * [`take`] drains the **process-wide** counter: gate binaries read it once per game;
//! * [`take_thread`] drains the **thread-local** one: a unit test reads it for its own calls only.
//!
//! **Strict mode** ([`set_strict`], or `TI4_UNMAPPED=strict` in the environment of a native binary)
//! turns the first refusal into a panic naming slice, reason and text, so the backtrace names the
//! call site. The gate binaries run strict. A test that refuses on purpose wraps the call in
//! [`allow_unmapped`]. Nothing here changes a decision: the counters only observe the `.ok()?`
//! fallbacks of the adapters.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use ti4_engine_core::state::Slice;

use super::error::Unmapped;

/// Refusals counted by `(slice, reason)`; `slice` is `Slice as u8`, adapter refusals use [`ADAPTER`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UnmappedCounts {
    pub total: u64,
    pub by_reason: BTreeMap<(u8, &'static str), u64>,
}

/// The slice number the refusals of `core_adapter::Unmapped` (which have no slice) are counted under.
pub const ADAPTER: u8 = u8::MAX;

impl UnmappedCounts {
    fn add(&mut self, slice: u8, what: &'static str) {
        self.total += 1;
        *self.by_reason.entry((slice, what)).or_default() += 1;
    }

    /// Fold another count in.
    pub fn merge(&mut self, other: &Self) {
        self.total += other.total;
        for (k, n) in &other.by_reason {
            *self.by_reason.entry(*k).or_default() += n;
        }
    }

    /// `"Players: faction"` -> count, the form the dual report prints and the shard files carry.
    #[must_use]
    pub fn by_name(&self) -> BTreeMap<String, u64> {
        self.by_reason.iter().map(|((s, what), n)| (reason_name(*s, what), *n)).collect()
    }
}

/// `"Players: faction"` (or `"adapter: Seats"`).
#[must_use]
pub fn reason_name(slice: u8, what: &str) -> String {
    match Slice::ALL.get(usize::from(slice)) {
        Some(s) => format!("{s:?}: {what}"),
        None => format!("adapter: {what}"),
    }
}

static PROCESS: Mutex<UnmappedCounts> = Mutex::new(UnmappedCounts { total: 0, by_reason: BTreeMap::new() });
static STRICT: AtomicBool = AtomicBool::new(false);

thread_local! {
    static THREAD: RefCell<UnmappedCounts> = const { RefCell::new(UnmappedCounts { total: 0, by_reason: BTreeMap::new() }) };
    static ALLOWED: Cell<u32> = const { Cell::new(0) };
    static STRICT_HERE: Cell<bool> = const { Cell::new(false) };
}

/// Whether the value of an environment variable (if set) is `want`.
fn env_is(value: Option<&str>, want: &str) -> bool {
    value.is_some_and(|v| v == want)
}

/// Whether strict mode is on: [`set_strict`], or `TI4_UNMAPPED=strict` (native only, read once).
#[must_use]
pub fn strict() -> bool {
    if STRICT.load(Ordering::Relaxed) || STRICT_HERE.with(Cell::get) {
        return true;
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        static ENV: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        return *ENV.get_or_init(|| env_is(std::env::var("TI4_UNMAPPED").ok().as_deref(), "strict"));
    }
    #[cfg(target_arch = "wasm32")]
    false
}

/// Make the first refusal of this process a panic (the gate binaries call this first).
// mutants:skip (the process-wide switch cannot be flipped in a test process: every parallel test would see it)
pub fn set_strict(on: bool) {
    STRICT.store(on, Ordering::Relaxed);
}

/// The gate binaries' first call: strict, unless the environment says `TI4_UNMAPPED=count` (a run that only
/// wants the census, printed by [`DualReport`](crate::dual::DualReport)).
// mutants:skip (sets the process-wide switch; the decision is `gate_strict`, tested)
pub fn set_strict_for_gate() {
    #[cfg(not(target_arch = "wasm32"))]
    set_strict(gate_strict(std::env::var("TI4_UNMAPPED").ok().as_deref()));
    #[cfg(target_arch = "wasm32")]
    set_strict(true);
}

/// Whether a gate binary runs strict, given `TI4_UNMAPPED`: unless it says `count`.
fn gate_strict(env: Option<&str>) -> bool {
    !env_is(env, "count")
}

fn record(slice: u8, what: &'static str, text: &str) {
    PROCESS.lock().unwrap_or_else(std::sync::PoisonError::into_inner).add(slice, what);
    THREAD.with(|t| t.borrow_mut().add(slice, what));
    if strict() && ALLOWED.with(Cell::get) == 0 {
        panic!("unmapped (strict mode): {} {text:?}", reason_name(slice, what));
    }
}

/// Count a refusal of the codecs (called by `bad`).
pub fn note(u: &Unmapped) {
    record(u.slice as u8, u.what, &u.text);
}

/// Count a refusal of `core_adapter` (no slice).
pub fn note_adapter(what: &'static str, text: &str) {
    record(ADAPTER, what, text);
}

/// Drain the process-wide counter.
pub fn take() -> UnmappedCounts {
    std::mem::take(&mut *PROCESS.lock().unwrap_or_else(std::sync::PoisonError::into_inner))
}

/// A copy of the process-wide counter.
#[must_use]
pub fn peek() -> UnmappedCounts {
    PROCESS.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone()
}

/// Drain the counter of this thread (a unit test reads its own calls only).
pub fn take_thread() -> UnmappedCounts {
    THREAD.with(|t| std::mem::take(&mut *t.borrow_mut()))
}

/// Run `f` with strict mode on for this thread only (the tests of the strict mode itself: the process-wide switch
/// would make every parallel test panic on its own refusals).
pub fn strict_on_this_thread<T>(f: impl FnOnce() -> T) -> T {
    struct Guard(bool);
    impl Drop for Guard {
        fn drop(&mut self) {
            STRICT_HERE.with(|s| s.set(self.0));
        }
    }
    let _guard = Guard(STRICT_HERE.with(|s| s.replace(true)));
    f()
}

/// Run `f` with strict mode suspended on this thread (a test that refuses on purpose); still counted.
pub fn allow_unmapped<T>(f: impl FnOnce() -> T) -> T {
    struct Guard;
    impl Drop for Guard {
        fn drop(&mut self) {
            ALLOWED.with(|a| a.set(a.get() - 1));
        }
    }
    ALLOWED.with(|a| a.set(a.get() + 1));
    let _guard = Guard;
    f()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_environment_switches_read_their_exact_values() {
        assert!(env_is(Some("strict"), "strict"));
        assert!(!env_is(Some("count"), "strict"));
        assert!(!env_is(None, "strict"));
        assert!(gate_strict(None));
        assert!(gate_strict(Some("strict")));
        assert!(!gate_strict(Some("count")));
    }

    #[test]
    fn counts_merge_by_sum_and_by_reason() {
        let mut a = UnmappedCounts::default();
        a.add(2, "x");
        a.add(2, "x");
        let mut b = UnmappedCounts::default();
        b.add(2, "x");
        b.add(3, "y");
        a.merge(&b);
        assert_eq!(a.total, 4);
        assert_eq!(a.by_reason.get(&(2, "x")), Some(&3));
        assert_eq!(a.by_reason.get(&(3, "y")), Some(&1));
    }
}
