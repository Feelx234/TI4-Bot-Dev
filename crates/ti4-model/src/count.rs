//! Call counters for the view side of the rule bot (docs/todo/D3-bots.md D3-0 item 5), behind the feature
//! `observed-count` (off by default; `--features ti4-engine/observed-count`). Without the feature `bump` and
//! `set_scope` are empty inline functions: no behaviour, no cost.
//!
//! A counter is a static name (`"Observed::seat"`, `"PlayerId::new"`). It is attributed to the *scope* set on the
//! calling thread (the subtype of the decision the bot is answering) and ignored when no scope is set, so only
//! the traffic inside the bot's answer is counted.

#[cfg(feature = "observed-count")]
mod on {
    use std::cell::{Cell, RefCell};
    use std::collections::{BTreeMap, HashMap};
    use std::sync::Mutex;

    type Table = BTreeMap<String, BTreeMap<&'static str, u64>>;
    static GLOBAL: Mutex<Table> = Mutex::new(BTreeMap::new());

    thread_local! {
        static ACTIVE: Cell<bool> = const { Cell::new(false) };
        static SCOPE: RefCell<String> = const { RefCell::new(String::new()) };
        static LOCAL: RefCell<HashMap<&'static str, u64>> = RefCell::new(HashMap::new());
    }

    fn flush() {
        LOCAL.with(|local| {
            let mut local = local.borrow_mut();
            if local.is_empty() {
                return;
            }
            SCOPE.with(|scope| {
                let mut global = GLOBAL.lock().unwrap();
                let slot = global.entry(scope.borrow().clone()).or_default();
                for (name, n) in local.drain() {
                    *slot.entry(name).or_default() += n;
                }
            });
        });
    }

    pub fn bump(name: &'static str) {
        if ACTIVE.with(Cell::get) {
            LOCAL.with(|local| *local.borrow_mut().entry(name).or_default() += 1);
        }
    }

    pub fn set_scope(scope: Option<&str>) {
        flush();
        match scope {
            Some(text) => SCOPE.with(|s| {
                let mut s = s.borrow_mut();
                s.clear();
                s.push_str(text);
                ACTIVE.with(|a| a.set(true));
            }),
            None => ACTIVE.with(|a| a.set(false)),
        }
    }

    pub fn take() -> Table {
        flush();
        std::mem::take(&mut *GLOBAL.lock().unwrap())
    }
}

/// Count one call of `name` in the current scope.
#[cfg(feature = "observed-count")]
pub use on::bump;
/// Make `scope` (a subtype text) the scope of the following counts on this thread; `None` stops counting.
#[cfg(feature = "observed-count")]
pub use on::set_scope;
/// Drain the counts of every thread: scope -> name -> calls. Call `set_scope(None)` on the counting thread first.
#[cfg(feature = "observed-count")]
pub use on::take;

/// Count one call of `name` in the current scope (the feature `observed-count` is off: nothing).
#[cfg(not(feature = "observed-count"))]
#[inline(always)]
pub fn bump(_name: &'static str) {}

/// Set the scope of the counts (the feature `observed-count` is off: nothing).
#[cfg(not(feature = "observed-count"))]
#[inline(always)]
pub fn set_scope(_scope: Option<&str>) {}

/// Drain the counts (the feature `observed-count` is off: none).
#[cfg(not(feature = "observed-count"))]
#[must_use]
pub fn take() -> std::collections::BTreeMap<String, std::collections::BTreeMap<&'static str, u64>> {
    std::collections::BTreeMap::new()
}

/// Whether the counters are compiled in.
pub const ENABLED: bool = cfg!(feature = "observed-count");
