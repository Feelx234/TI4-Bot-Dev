//! The switched-off features at the entry points of the typed core (docs/DISABLE_XF123.md; package D2-2).
//!
//! One function decides whether a string position uses a feature that is switched off: structured diplomacy, the turn-redo
//! deck reserve, or a seat of Last Bastion, the Crimson Rebellion, the Council Keleres, the Deepwrought Scholarate, the
//! Firmament, the Obsidian or the Ral Nel Consortium ([`find`]). What the typed core does with such a position is a policy
//! ([`set_disabled_policy`]):
//!
//! * [`Policy::Off`] (the default until the flip, D2-8): nothing. The string engine keeps playing these games for the tools
//!   (`ti4-training --diplomacy`, `ti4-review`, `--roster wide`) and the lockstep tests of the ported rules keep running.
//! * [`Policy::Bail`]: the entry refuses with a counted [`Unmapped`](super::Unmapped) ("disabled feature"); the caller falls
//!   back to the string engine as it does for any value the core cannot hold.
//! * [`Policy::Panic`]: the entry panics naming the family; the typed core's own sites ([`ti4_engine_core::unsupported`])
//!   panic too. The gate binaries set it (`set_disabled_policy_for_gate`): no golden, bench, template or alpha position may
//!   trip it (docs/DISABLE_XF123.md 1.2, 1.3).
//!
//! Entries that call [`check`]: `Ctx::of` / `with_extra` / `refresh` (so every `Lend`, `Region`, `CoreView` and the window
//! tables of the dual), `Codec::load`.

use std::cell::Cell;
use std::sync::atomic::{AtomicU8, Ordering};

use ti4_engine_core::state::Slice;
use ti4_engine_core::unsupported as core;
use ti4_model::state::GameState;

pub use core::Disabled;

use super::error::{R, bad};
use crate::generated::IdStr;

/// What an entry does with a switched-off feature.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Policy {
    Off,
    Bail,
    Panic,
}

static POLICY: AtomicU8 = AtomicU8::new(0);

thread_local! {
    static LOCAL: Cell<Option<Policy>> = const { Cell::new(None) };
}

/// Run `f` with `p` as the policy of this thread only (the tests: the process-wide switch would trip every parallel test).
pub fn with_disabled_policy<T>(p: Policy, f: impl FnOnce() -> T) -> T {
    struct Restore(Option<Policy>);
    impl Drop for Restore {
        fn drop(&mut self) {
            LOCAL.with(|l| l.set(self.0));
        }
    }
    let _restore = Restore(LOCAL.with(|l| l.replace(Some(p))));
    core::with_policy(core_policy(p), f)
}

/// Make `p` the policy of this thread's entries until the guard drops: the form of [`with_disabled_policy`] for a test that holds it for its
/// whole body (the bail-as-failure tests of the heroes). The typed core's own sites keep their policy: they read the trade slice, which the
/// footprints of the windows do not lend.
#[must_use]
pub struct ThreadGuard(Option<Policy>);

impl Drop for ThreadGuard {
    fn drop(&mut self) {
        LOCAL.with(|l| l.set(self.0));
    }
}

/// See [`ThreadGuard`].
pub fn enter_disabled_policy(p: Policy) -> ThreadGuard {
    ThreadGuard(LOCAL.with(|l| l.replace(Some(p))))
}

/// The policy of the typed core's own sites that goes with `p`: `Panic` only for [`Policy::Panic`].
const fn core_policy(p: Policy) -> core::Policy {
    match p {
        Policy::Panic => core::Policy::Panic,
        Policy::Off | Policy::Bail => core::Policy::Off,
    }
}

/// Set the policy of this process (and, for [`Policy::Panic`], that of the typed core's own sites).
// mutants:skip (the process-wide switch cannot be flipped in a test process: every parallel test would see it; `core_policy` is tested)
pub fn set_disabled_policy(p: Policy) {
    POLICY.store(p as u8, Ordering::Relaxed);
    core::set_policy(core_policy(p));
}

/// The policy of this process.
#[must_use]
pub fn disabled_policy() -> Policy {
    if let Some(p) = LOCAL.with(Cell::get) {
        return p;
    }
    match POLICY.load(Ordering::Relaxed) {
        0 => Policy::Off,
        1 => Policy::Bail,
        _ => Policy::Panic,
    }
}

/// The gate binaries' first call: panic on a switched-off feature, unless `TI4_DISABLED=off` says otherwise.
// mutants:skip (sets the process-wide switch; the decision is `gate_policy`, tested)
pub fn set_disabled_policy_for_gate() {
    #[cfg(not(target_arch = "wasm32"))]
    set_disabled_policy(gate_policy(std::env::var("TI4_DISABLED").ok().as_deref()));
    #[cfg(target_arch = "wasm32")]
    set_disabled_policy(Policy::Panic);
}

/// The policy a gate binary runs with, given `TI4_DISABLED`: `Panic`, unless it says `off`.
fn gate_policy(env: Option<&str>) -> Policy {
    if env == Some("off") { Policy::Off } else { Policy::Panic }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_gate_runs_panic_unless_the_environment_says_off() {
        assert_eq!(gate_policy(None), Policy::Panic);
        assert_eq!(gate_policy(Some("panic")), Policy::Panic);
        assert_eq!(gate_policy(Some("off")), Policy::Off);
    }

    #[test]
    fn only_panic_reaches_the_sites_of_the_typed_core() {
        assert_eq!(core_policy(Policy::Panic), core::Policy::Panic);
        assert_eq!(core_policy(Policy::Bail), core::Policy::Off);
        assert_eq!(core_policy(Policy::Off), core::Policy::Off);
    }

    #[test]
    fn a_thread_policy_reaches_the_core_and_ends_with_its_closure() {
        with_disabled_policy(Policy::Panic, || assert_eq!(core::policy(), core::Policy::Panic));
        with_disabled_policy(Policy::Bail, || {
            assert_eq!(disabled_policy(), Policy::Bail);
            assert_eq!(core::policy(), core::Policy::Off);
        });
        assert_eq!(core::policy(), core::Policy::Off);
        assert_eq!(disabled_policy(), Policy::Off);
    }
}

/// The first switched-off feature the position uses.
#[must_use]
pub fn find(gs: &GameState) -> Option<Disabled> {
    if gs.diplomacy.enabled {
        return Some(Disabled::Diplomacy);
    }
    if gs.deck_reserve.is_some() {
        return Some(Disabled::DeckReserve);
    }
    gs.players
        .iter()
        .find_map(|p| ti4_engine_core::generated::Faction::from_id(p.faction.as_str()).and_then(core::of_faction))
}

/// The entry check: nothing under [`Policy::Off`]; a counted refusal under [`Policy::Bail`]; a panic under [`Policy::Panic`].
///
/// # Errors
/// The position uses a switched-off feature and the policy is [`Policy::Bail`].
#[track_caller]
pub fn check(gs: &GameState) -> R<()> {
    let policy = disabled_policy();
    if policy == Policy::Off {
        return Ok(());
    }
    let Some(d) = find(gs) else { return Ok(()) };
    if policy == Policy::Panic {
        panic!("{d:?} is switched off (docs/DISABLE_XF123.md): the typed core does not play it; use ti4-engine-legacy");
    }
    Err(bad(Slice::Meta, "disabled feature", format!("{d:?}")))
}
