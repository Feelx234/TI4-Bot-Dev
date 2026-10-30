//! Package D2-2: the switched-off features (docs/DISABLE_XF123.md) at the entry points of the typed core, in both
//! policies (entries: `Codec::load`, `Lend::{with_core, region, view}`, `CoreView::of`, the window tables of the dual). The policy is set per thread (`with_disabled_policy`): the process-wide switch would trip parallel tests.

#![cfg(test)]

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

use ti4_engine_core::state::{Slice, SliceSet};
use ti4_model::id::FactionId;
use ti4_model::state::GameState;

use super::unmapped;
use super::unsupported::{Disabled, Policy, check, find, with_disabled_policy};
use super::*;

/// One string position per family.
fn families() -> Vec<(Disabled, &'static str, GameState)> {
    let base = || crate::fixtures::real_game(&["a", "b", "c"]);
    let mut out = Vec::new();
    let mut diplomacy = base();
    diplomacy.diplomacy.enabled = true;
    out.push((Disabled::Diplomacy, "diplomacy", diplomacy));
    let mut reserve = base();
    reserve.deck_reserve = Some(ti4_model::deck_reserve::DeckReserve::new(Arc::new(AtomicUsize::new(0)), Vec::new()));
    out.push((Disabled::DeckReserve, "deck reserve", reserve));
    for (d, ids) in [
        (Disabled::LastBastion, &["bastion"][..]),
        (Disabled::Crimson, &["crimson"]),
        (Disabled::Keleres, &["keleresm", "keleresx", "keleresa"]),
        (Disabled::Deepwrought, &["deepwrought"]),
        (Disabled::Firmament, &["firmament"]),
        (Disabled::Obsidian, &["obsidian"]),
        (Disabled::RalNel, &["ralnel"]),
    ] {
        for id in ids {
            let mut gs = base();
            gs.players[1].faction = FactionId::new(*id);
            out.push((d, *id, gs));
        }
    }
    out
}

fn panic_text(f: impl FnOnce()) -> Option<String> {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let r = catch_unwind(AssertUnwindSafe(f));
    std::panic::set_hook(prev);
    r.err().map(|e| e.downcast_ref::<String>().cloned().unwrap_or_default())
}

fn all_but_map() -> SliceSet {
    SliceSet(SliceSet::ALL.0 & !(1 << Slice::Map as u32))
}

#[test]
fn find_names_each_family_on_a_string_position_and_nothing_on_an_ordinary_table() {
    for (d, label, gs) in families() {
        assert_eq!(find(&gs), Some(d), "{label}");
    }
    assert_eq!(find(&crate::fixtures::real_game(&["a", "b", "c"])), None);
    // every lobby faction is playable: none of them is a switched-off one
    for f in crate::seating::LOBBY_FACTIONS {
        let mut gs = crate::fixtures::real_game(&["a", "b"]);
        gs.players[0].faction = FactionId::new(f);
        assert_eq!(find(&gs), None, "{f}");
    }
}

#[test]
fn off_is_the_default_and_checks_nothing() {
    assert_eq!(unsupported::disabled_policy(), Policy::Off);
    for (_, label, gs) in families() {
        assert!(check(&gs).is_ok(), "{label}");
    }
}

#[test]
fn bail_refuses_every_entry_with_a_counted_unmapped_and_leaves_ordinary_tables_alone() {
    for (d, label, gs) in families() {
        // the tables are built before the policy flips: the lend entries are what is under test
        let mut cx = Ctx::of(&gs).unwrap();
        let _ = unmapped::take_thread();
        with_disabled_policy(Policy::Bail, || {
            let reason = |e: Unmapped| {
                assert_eq!((e.slice, e.what), (Slice::Meta, "disabled feature"), "{label}");
                assert_eq!(e.text, format!("{d:?}"), "{label}");
            };
            reason(check(&gs).unwrap_err());
            reason(Codec::load(&mut cx, &gs, Sources::default(), all_but_map()).unwrap_err());
            let mut copy = gs.clone();
            reason(
                with_core(&mut copy, &mut cx, Sources::default(), Footprint::new(&[Slice::Players], &[]), |_, _| ()).unwrap_err(),
            );
            let mut lend = Lend::new(&mut copy, &mut cx, Sources::default());
            reason(lend.region(Footprint::new(&[Slice::Players], &[]), |_| ()).unwrap_err());
            reason(lend.view(SliceSet::EMPTY).unwrap_err());
            reason(CoreView::of(&gs, &mut cx, Sources::default(), SliceSet::EMPTY).unwrap_err());
            // the window tables of the dual bail the same way (the string result stands)
            assert!(crate::dual_window::tables(&gs).is_none(), "{label}");
        });
        let counts = unmapped::take_thread();
        assert_eq!(counts.by_reason.get(&(Slice::Meta as u8, "disabled feature")), Some(&7), "{label}: {counts:?}");
    }
    let ok = crate::fixtures::real_game(&["a", "b", "c"]);
    with_disabled_policy(Policy::Bail, || {
        let mut cx = Ctx::of(&ok).unwrap();
        Codec::load(&mut cx, &ok, Sources::default(), all_but_map()).unwrap();
        assert!(crate::dual_window::tables(&ok).is_some());
    });
}

#[test]
fn panic_names_the_family_at_every_entry_and_leaves_ordinary_tables_alone() {
    for (d, label, gs) in families() {
        let mut cx = Ctx::of(&gs).unwrap();
        let named = |text: Option<String>, entry: &str| {
            let text = text.unwrap_or_else(|| panic!("{label}: {entry} did not panic"));
            assert!(text.contains(&format!("{d:?}")) && text.contains("DISABLE_XF123"), "{label}: {entry}: {text}");
        };
        with_disabled_policy(Policy::Panic, || {
            named(panic_text(|| drop(Codec::load(&mut cx, &gs, Sources::default(), all_but_map()))), "Codec::load");
            let mut copy = gs.clone();
            named(
                panic_text(|| drop(with_core(&mut copy, &mut cx, Sources::default(), Footprint::new(&[Slice::Players], &[]), |_, _| ()))),
                "with_core",
            );
            named(panic_text(|| drop(CoreView::of(&gs, &mut cx, Sources::default(), SliceSet::EMPTY))), "CoreView::of");
            named(panic_text(|| drop(crate::dual_window::tables(&gs))), "dual_window::tables");
        });
    }
    let ok = crate::fixtures::real_game(&["a", "b", "c"]);
    with_disabled_policy(Policy::Panic, || {
        let mut cx = Ctx::of(&ok).unwrap();
        Codec::load(&mut cx, &ok, Sources::default(), all_but_map()).unwrap();
    });
    // the thread's policy ends with the closure
    assert_eq!(unsupported::disabled_policy(), Policy::Off);
}

/// The lend of a ported call at a table that seats a switched-off faction: a counted bail with `Bail`, the string engine
/// answering as before.
#[test]
fn a_ported_write_at_a_switched_off_table_bails_under_bail_and_runs_under_off() {
    let mut gs = crate::fixtures::real_game(&["a", "b", "c"]);
    gs.players[1].faction = FactionId::new("bastion");
    let ran = |gs: &mut GameState| crate::dual_window::lend(gs, Footprint::new(&[Slice::Players], &[]), None, |_, _| ());
    assert!(with_disabled_policy(Policy::Off, || ran(&mut gs.clone())).is_some());
    assert!(with_disabled_policy(Policy::Bail, || ran(&mut gs.clone())).is_none());
    assert!(panic_text(|| with_disabled_policy(Policy::Panic, || drop(ran(&mut gs.clone())))).is_some_and(|t| t.contains("LastBastion")));
}
