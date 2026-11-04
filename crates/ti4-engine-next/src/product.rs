//! Product assertions (package W17 of `docs/todo/lent-bodies.md`): positions the product never reaches, which the core port
//! turns from a quiet bail into a panic once the string engine is gone.
//!
//! Three families were counted as lends although no player can ever cause them:
//!
//! * **XF-6, the codec refuses the position.** The fixture faction `generic`, more than 16 owners (a table seats at most 8), an
//!   owner id that is a prefix of another, an id with no typed form, content that is not generated. A real game has none of
//!   them, so every [`Unmapped`] a lend reports is a bug of the product ([`refused`]).
//! * **XF-7, irreproducible dice.** Test faces (`Dice::preloaded`), a pending stream restore (turn redo, switched off and
//!   refused at the load), a seed that is not the state's. A real game's dice are the state's streams ([`reproducible`]).
//! * **L19, trade guards.** A faction played by two seats (the lobby seats a faction once), a transaction window inside a battle
//!   or an invasion, a transaction with no map ([`unreachable_here`] at the call sites).
//! * **L21, the factions the lobby never seats** (Keleres, Deepwrought, Firmament, Obsidian, Ral Nel) and the two dropped with
//!   Last Bastion and the Crimson Rebellion ([`lobby_faction`]).
//!
//! # Strict mode
//!
//! The assertions are **off by default**: the string engine still plays every product game until D2, the dev tools (training,
//! review, self-play with the wide roster, the unit tests with the fixture faction) keep running tables the product never
//! creates, and each of those positions stays a counted bail with its reason. They are switched on by
//! [`set_strict`] (D2 calls it where the host starts a game), by the environment variable `TI4_CORE_STRICT=1` (the gates and the
//! nightly sweep), or for one thread by [`with_strict`] (the tests).

use std::cell::Cell;
use std::sync::atomic::{AtomicU8, Ordering};

use crate::core_sync::Unmapped;

const UNSET: u8 = 0;
const OFF: u8 = 1;
const ON: u8 = 2;

static PROCESS: AtomicU8 = AtomicU8::new(UNSET);

thread_local! {
    static THREAD: Cell<Option<bool>> = const { Cell::new(None) };
}

/// Whether the assertions panic: the thread's override, else the process setting, else `TI4_CORE_STRICT` (read once).
#[must_use]
pub fn strict() -> bool {
    if let Some(on) = THREAD.with(Cell::get) {
        return on;
    }
    match PROCESS.load(Ordering::Relaxed) {
        ON => true,
        OFF => false,
        _ => {
            let on = std::env::var_os("TI4_CORE_STRICT").is_some_and(|v| !v.is_empty() && v != "0");
            PROCESS.store(if on { ON } else { OFF }, Ordering::Relaxed);
            on
        }
    }
}

/// Switch the assertions on or off for the whole process.
pub fn set_strict(on: bool) {
    PROCESS.store(if on { ON } else { OFF }, Ordering::Relaxed);
}

/// Run `body` with the assertions on or off for this thread only (restored afterwards, also when `body` panics).
pub fn with_strict<T>(on: bool, body: impl FnOnce() -> T) -> T {
    struct Restore(Option<bool>);
    impl Drop for Restore {
        fn drop(&mut self) {
            THREAD.with(|t| t.set(self.0));
        }
    }
    let _restore = Restore(THREAD.with(|t| t.replace(Some(on))));
    body()
}

/// A position the product never reaches. Panics in strict mode; otherwise returns and the caller keeps its counted bail.
///
/// # Panics
/// In strict mode, always.
#[track_caller]
pub fn unreachable_here(family: &str, what: &str) {
    assert!(!strict(), "product assertion {family}: {what} cannot arise in the product (docs/todo/lent-bodies.md, W17)");
}

/// XF-6: the typed state could not hold the position a lend was asked for.
#[track_caller]
pub fn refused(why: &Unmapped) {
    if strict() {
        unreachable_here("XF-6 (codec refusal)", &why.to_string());
    }
}

/// XF-7: whether the dice of a roll are the state's streams, so that the core can draw the same faces. The three conditions are
/// the ones every dual call site spelled out: no test faces, no pending restore of the streams, the seed the state carries.
#[track_caller]
#[must_use]
pub fn reproducible(preloaded: bool, sync_pending: bool, seed: u64, state_seed: u64) -> bool {
    let reproducible = !preloaded && !sync_pending && seed == state_seed;
    if !reproducible {
        unreachable_here("XF-7 (irreproducible dice)", "a roll with test faces, a pending stream restore or a foreign seed");
    }
    reproducible
}

/// The factions the lobby seats and the neutral owner of the units no seat plays. Anything else (the Keleres, Deepwrought, Firmament,
/// Obsidian and Ral Nel, the dropped Last Bastion and Crimson Rebellion, the fixture faction) is a dev-tool table.
#[must_use]
pub fn is_lobby_faction(faction: &str) -> bool {
    faction == "neutral" || crate::seating::LOBBY_FACTIONS.contains(&faction)
}

/// L21: a seat plays `faction`; a faction the lobby never seats is an assertion of the product.
#[track_caller]
pub fn lobby_faction(faction: &str) {
    if !is_lobby_faction(faction) && strict() {
        unreachable_here("L21 (non-lobby faction)", &format!("a seat of the faction {faction:?}"));
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_assertions_are_off_unless_asked_for() {
        with_strict(false, || {
            unreachable_here("X", "anything");
            lobby_faction("keleresa");
            assert!(!reproducible(true, false, 1, 1));
        });
    }

    #[test]
    #[should_panic(expected = "product assertion L21")]
    fn a_non_lobby_faction_panics_in_strict_mode() {
        with_strict(true, || lobby_faction("ralnel"));
    }

    #[test]
    fn the_lobby_factions_and_the_neutral_owner_are_the_product_tables() {
        assert!(crate::seating::LOBBY_FACTIONS.iter().all(|f| is_lobby_faction(f)));
        assert!(is_lobby_faction("neutral"));
        for f in ["keleresa", "keleresm", "keleresx", "deepwrought", "firmament", "obsidian", "ralnel", "bastion", "crimson", "generic"] {
            assert!(!is_lobby_faction(f), "{f}");
        }
    }

    #[test]
    fn the_override_is_restored_also_after_a_panic() {
        with_strict(false, || {
            let caught = std::panic::catch_unwind(|| with_strict(true, || unreachable_here("X", "y")));
            assert!(caught.is_err());
            assert!(!strict());
        });
    }

    #[test]
    fn irreproducible_dice_are_each_condition_alone() {
        with_strict(false, || {
            assert!(reproducible(false, false, 7, 7));
            assert!(!reproducible(true, false, 7, 7));
            assert!(!reproducible(false, true, 7, 7));
            assert!(!reproducible(false, false, 7, 8));
        });
    }

    #[test]
    fn a_codec_refusal_panics_only_in_strict_mode() {
        let why = crate::core_sync::Unmapped { slice: ti4_engine_core::state::Slice::Seats, what: "x", text: "y".to_owned() };
        with_strict(false, || refused(&why));
        let caught = std::panic::catch_unwind(|| with_strict(true, || refused(&why)));
        assert!(caught.is_err());
    }

    #[test]
    fn a_roll_that_cannot_be_reproduced_panics_in_strict_mode() {
        let caught = std::panic::catch_unwind(|| with_strict(true, || reproducible(false, false, 1, 2)));
        assert!(caught.is_err());
        assert!(with_strict(true, || reproducible(false, false, 3, 3)));
    }
}

// ---- lane D: the runtime gap counter (docs/todo/D2-8-wrapper.md section 3) ----

static GAPS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// A live call the core could not run (`Stop::Live(Unknown)`): a defect report with its cause, never a fallback. The wrapper counts it and
/// returns `GameError::Core`; the server exposes [`gap_count`] and a full nightly must end with 0.
pub fn note_gap(arm: &str, cause: &str) {
    GAPS.fetch_add(1, Ordering::Relaxed);
    if std::env::var_os("TI4_GAP_TRACE").is_some() {
        eprintln!("core gap in {arm}: {cause}");
    }
}

/// How many gaps this process has met.
#[must_use]
pub fn gap_count() -> u64 {
    GAPS.load(Ordering::Relaxed)
}
