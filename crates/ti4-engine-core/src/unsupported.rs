//! The switched-off features (docs/DISABLE_XF123.md; package D2-2).
//!
//! Structured diplomacy, the turn-redo deck reserve and the factions Last Bastion, the Crimson Rebellion, the Council
//! Keleres, the Deepwrought Scholarate, the Firmament, the Obsidian and the Ral Nel Consortium are not played any more:
//! the product refuses to create such a game and the legacy engine keeps playing them for the tools. Their rules stay in
//! this crate (tested, gated; deleting them is a clean-up of package D4), but are **dead code after the flip**.
//!
//! Until then this module is the switch: [`Policy::Off`] (the default) changes nothing, so the core crates' own tests of
//! those rules and the dual run of the string engine behave as before; [`Policy::Panic`] makes a position that uses one
//! of them a panic naming the family, at the entry points ([`guard`], called by the driver and the adapters) and at the
//! sites that would play the rule ([`hit`]). The gate binaries and the flip (D2-8) set `Panic`.

use std::cell::Cell;
use std::sync::atomic::{AtomicU8, Ordering};

use crate::generated::Faction;
use crate::state::CoreState;

/// One switched-off feature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Disabled {
    Diplomacy,
    DeckReserve,
    LastBastion,
    Crimson,
    Keleres,
    Deepwrought,
    Firmament,
    Obsidian,
    RalNel,
}

/// What a switched-off feature does when the core meets it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Policy {
    /// Nothing: the rules run as before (default).
    Off,
    /// Panic with the family named.
    Panic,
}

static POLICY: AtomicU8 = AtomicU8::new(0);

thread_local! {
    /// A policy for this thread only (the tests of the panics: the process-wide switch would make every parallel test panic).
    static LOCAL: Cell<Option<Policy>> = const { Cell::new(None) };
}

/// Run `f` with `p` as the policy of this thread.
pub fn with_policy<T>(p: Policy, f: impl FnOnce() -> T) -> T {
    struct Restore(Option<Policy>);
    impl Drop for Restore {
        fn drop(&mut self) {
            LOCAL.with(|l| l.set(self.0));
        }
    }
    let _restore = Restore(LOCAL.with(|l| l.replace(Some(p))));
    f()
}

const fn encode(p: Policy) -> u8 {
    match p {
        Policy::Off => 0,
        Policy::Panic => 1,
    }
}

const fn decode(n: u8) -> Policy {
    if n == 0 { Policy::Off } else { Policy::Panic }
}

/// Set the policy of this process.
// mutants:skip (the process-wide switch cannot be flipped in a test process: every parallel test would see it; `encode`/`decode` are tested)
pub fn set_policy(p: Policy) {
    POLICY.store(encode(p), Ordering::Relaxed);
}

/// The policy of this process.
#[must_use]
pub fn policy() -> Policy {
    if let Some(p) = LOCAL.with(Cell::get) {
        return p;
    }
    decode(POLICY.load(Ordering::Relaxed))
}

/// The feature a faction belongs to, if it is switched off.
#[must_use]
pub const fn of_faction(f: Faction) -> Option<Disabled> {
    match f {
        Faction::LastBastion => Some(Disabled::LastBastion),
        Faction::TheCrimsonRebellion => Some(Disabled::Crimson),
        Faction::TheCouncilKeleresArgent | Faction::TheCouncilKeleresMentak | Faction::TheCouncilKeleresXxcha => {
            Some(Disabled::Keleres)
        }
        Faction::TheDeepwroughtScholarate => Some(Disabled::Deepwrought),
        Faction::TheFirmament => Some(Disabled::Firmament),
        Faction::TheObsidian => Some(Disabled::Obsidian),
        Faction::TheRalNelConsortium => Some(Disabled::RalNel),
        _ => None,
    }
}

/// The first switched-off feature the position uses.
#[must_use]
pub fn find(s: &CoreState) -> Option<Disabled> {
    if s.trade().diplomacy.enabled {
        return Some(Disabled::Diplomacy);
    }
    if s.cards().reserve.is_some() {
        return Some(Disabled::DeckReserve);
    }
    s.players().iter().find_map(|p| of_faction(p.faction))
}

/// Panic if the policy is [`Policy::Panic`]: `d` is switched off.
#[track_caller]
pub fn hit(d: Disabled) {
    if policy() == Policy::Panic {
        refuse(d);
    }
}

/// [`hit`] for a faction that is being played.
#[track_caller]
pub fn faction_in_play(f: Faction) {
    if let Some(d) = of_faction(f) {
        hit(d);
    }
}

/// Entry check: the position must not use a switched-off feature (a no-op under [`Policy::Off`]).
#[track_caller]
pub fn guard(s: &CoreState) {
    if policy() == Policy::Panic
        && let Some(d) = find(s)
    {
        refuse(d);
    }
}

#[track_caller]
fn refuse(d: Disabled) -> ! {
    panic!("{d:?} is switched off (docs/DISABLE_XF123.md): the typed core does not play it; use ti4-engine-legacy")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::containers::Cow;
    use crate::ids::Seat;
    use crate::state::{Player, Players};
    use std::panic::{AssertUnwindSafe, catch_unwind};

    fn table(factions: &[Faction]) -> CoreState {
        let mut s = CoreState::all_loaded();
        s.players = Players::from_vec(factions.iter().enumerate().map(|(i, f)| Player::new(Seat(i as u8), *f)).collect());
        s
    }

    /// One position per family, with the family it must be named as.
    fn families() -> Vec<(Disabled, CoreState)> {
        let mut diplomacy = table(&[Faction::TheArborec, Faction::TheEmiratesOfHacan]);
        diplomacy.trade_mut().diplomacy.edit().enabled = true;
        let mut reserve = table(&[Faction::TheArborec, Faction::TheEmiratesOfHacan]);
        reserve.cards_mut().reserve = Some(Cow::new(Default::default()));
        let with = |f| table(&[Faction::TheArborec, f]);
        vec![
            (Disabled::Diplomacy, diplomacy),
            (Disabled::DeckReserve, reserve),
            (Disabled::LastBastion, with(Faction::LastBastion)),
            (Disabled::Crimson, with(Faction::TheCrimsonRebellion)),
            (Disabled::Keleres, with(Faction::TheCouncilKeleresArgent)),
            (Disabled::Keleres, with(Faction::TheCouncilKeleresMentak)),
            (Disabled::Keleres, with(Faction::TheCouncilKeleresXxcha)),
            (Disabled::Deepwrought, with(Faction::TheDeepwroughtScholarate)),
            (Disabled::Firmament, with(Faction::TheFirmament)),
            (Disabled::Obsidian, with(Faction::TheObsidian)),
            (Disabled::RalNel, with(Faction::TheRalNelConsortium)),
        ]
    }

    /// The panic message of `f`, if it panics.
    fn panic_text(f: impl FnOnce()) -> Option<String> {
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let r = catch_unwind(AssertUnwindSafe(f));
        std::panic::set_hook(prev);
        r.err().map(|e| e.downcast_ref::<String>().cloned().or_else(|| e.downcast_ref::<&str>().map(|s| (*s).to_owned())).unwrap_or_default())
    }

    #[test]
    fn find_names_every_switched_off_family_and_nothing_else() {
        for (d, s) in families() {
            assert_eq!(find(&s), Some(d));
        }
        assert_eq!(find(&table(&[Faction::TheArborec, Faction::TheEmiratesOfHacan, Faction::TheFederationOfSol])), None);
        // the neutral placeholder is not a switched-off seat
        assert_eq!(of_faction(Faction::Neutral), None);
    }

    #[test]
    fn off_changes_nothing_and_panic_names_the_family_at_the_entry() {
        for (d, s) in families() {
            with_policy(Policy::Off, || guard(&s));
            let text = panic_text(|| with_policy(Policy::Panic, || guard(&s))).unwrap_or_else(|| panic!("{d:?} did not panic"));
            assert!(text.contains(&format!("{d:?}")) && text.contains("DISABLE_XF123"), "{text}");
        }
        // an ordinary table passes under Panic
        with_policy(Policy::Panic, || guard(&table(&[Faction::TheArborec, Faction::TheEmiratesOfHacan])));
    }

    #[test]
    fn the_process_wide_policy_survives_its_encoding() {
        assert_eq!(decode(encode(Policy::Off)), Policy::Off);
        assert_eq!(decode(encode(Policy::Panic)), Policy::Panic);
        assert_ne!(encode(Policy::Off), encode(Policy::Panic));
    }

    #[test]
    fn the_policy_of_a_thread_does_not_leak() {
        with_policy(Policy::Panic, || assert_eq!(policy(), Policy::Panic));
        assert_eq!(policy(), Policy::Off);
    }

    #[test]
    fn the_faction_helper_of_the_modules_panics_for_a_played_faction_only() {
        for f in [
            Faction::LastBastion,
            Faction::TheCrimsonRebellion,
            Faction::TheCouncilKeleresXxcha,
            Faction::TheDeepwroughtScholarate,
            Faction::TheFirmament,
            Faction::TheObsidian,
            Faction::TheRalNelConsortium,
        ] {
            assert!(panic_text(|| with_policy(Policy::Panic, || faction_in_play(f))).is_some(), "{f:?}");
            with_policy(Policy::Off, || faction_in_play(f));
        }
        with_policy(Policy::Panic, || faction_in_play(Faction::TheArborec));
    }

    #[test]
    fn the_diplomacy_rules_panic_when_diplomacy_is_on_and_not_otherwise() {
        use crate::rules::diplomacy::{RelEvent, apply_relationship_event, decay_relationships};
        let mut on = table(&[Faction::TheArborec, Faction::TheEmiratesOfHacan]);
        on.trade_mut().diplomacy.edit().enabled = true;
        let ev = RelEvent::FairTransaction { a: Seat(0), b: Seat(1) };
        // Off: the rule runs as before
        with_policy(Policy::Off, || {
            let _ = apply_relationship_event(&mut on.clone(), ev);
            decay_relationships(&mut on.clone());
        });
        for site in 0..2 {
            let mut s = on.clone();
            let text = panic_text(|| {
                with_policy(Policy::Panic, || {
                    if site == 0 {
                        let _ = apply_relationship_event(&mut s, ev);
                    } else {
                        decay_relationships(&mut s);
                    }
                });
            });
            assert!(text.is_some_and(|t| t.contains("Diplomacy")), "site {site}");
        }
        // diplomacy off: no panic under Panic
        let mut off = table(&[Faction::TheArborec, Faction::TheEmiratesOfHacan]);
        with_policy(Policy::Panic, || {
            let _ = apply_relationship_event(&mut off, ev);
            decay_relationships(&mut off);
        });
    }

    #[test]
    fn the_diplomacy_book_panics_at_every_entry_when_diplomacy_is_on() {
        use crate::rules::diplomacy_book::{Moment, evaluate_event, settle_deadlines};
        let mut on = table(&[Faction::TheArborec, Faction::TheEmiratesOfHacan]);
        on.trade_mut().diplomacy.edit().enabled = true;
        let moment = Moment::HostileEngagement { attacker: Seat(0), victim: Seat(1), activation_seq: 1 };
        let mut s = on.clone();
        assert!(panic_text(|| with_policy(Policy::Panic, || { let _ = evaluate_event(&mut s, moment); })).is_some_and(|t| t.contains("Diplomacy")));
        let mut s = on.clone();
        assert!(panic_text(|| with_policy(Policy::Panic, || { let _ = settle_deadlines(&mut s, 1); })).is_some_and(|t| t.contains("Diplomacy")));
        let mut s = on.clone();
        with_policy(Policy::Off, || {
            let _ = evaluate_event(&mut s, moment);
            let _ = settle_deadlines(&mut s, 1);
        });
    }

    #[test]
    fn the_firmament_plot_helper_panics_for_a_firmament_seat() {
        let s = table(&[Faction::TheArborec, Faction::TheFirmament]);
        assert!(with_policy(Policy::Off, || crate::kernel::fx_plots::is_firmament(&s, Seat(1))));
        assert!(panic_text(|| with_policy(Policy::Panic, || { let _ = crate::kernel::fx_plots::is_firmament(&s, Seat(1)); })).is_some_and(|t| t.contains("Firmament")));
        with_policy(Policy::Panic, || assert!(!crate::kernel::fx_plots::is_firmament(&s, Seat(0))));
    }
}
