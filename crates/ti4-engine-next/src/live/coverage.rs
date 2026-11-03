//! How many live steps each native arm ran (trap T-Z1: a zero bail count that never ran a native arm proves nothing). Process-wide,
//! read by the `playthrough` bin; `live_step` counts every successful step.

use std::collections::BTreeMap;
use std::sync::Mutex;

static RUN: Mutex<BTreeMap<&'static str, u64>> = Mutex::new(BTreeMap::new());

/// One live step of the arm named `arm` ran.
pub fn note(arm: &'static str) {
    if let Ok(mut m) = RUN.lock() {
        *m.entry(arm).or_default() += 1;
    }
}

/// The counts by arm name, cleared.
#[must_use]
pub fn take() -> BTreeMap<&'static str, u64> {
    RUN.lock().map(|mut m| std::mem::take(&mut *m)).unwrap_or_default()
}
