//! The one constructor of an adapter's window driver (package D2-8a, docs/todo/D2-remaining.md 3.4).
//!
//! Every adapter that runs a core window (`dual_window.rs`, `*_core.rs`, `*_dual.rs`) used to build its driver with
//! `Driver::new(window, Resolver::new(&[seat], Some(seat)), AbilityTable::default(), 1)`: an empty ability registry, trace ids
//! from 1 and no spent frequency keys. So no ability could open a reaction window inside an adapter's window (the origin of the
//! counted `nested (unported ability reaction: ..)` bails), and the state's own resolver (`Pending::resolver`, D2-4b) was
//! ignored. [`driver_for`] is the single place those drivers are made now, and [`armed`] the single switch for what they get.
//!
//! * **Flag off (the default, and what every product binary runs):** [`driver_for`] is `Driver::new(window, resolver, table,
//!   next_id)` and [`arm`] does nothing. Behaviour, state, events and hashes are what they were before this module existed.
//! * **Flag on (`TI4_DRIVER_ARMED=1`, [`set_armed`], [`with_armed`]):** [`arm`], which the shared driving loop
//!   (`dual_window::drive_windowed`) calls once the core state the window runs on is at hand and the string run followed no
//!   nested activity, gives the window the resolver the flip will run it with: the ability table of the core
//!   (`reactions_core::table`) with the registry of every seat derived from the position (`Follow::arm`), the spent frequency
//!   keys and the trace counter of the state (`Pending::resolver`). A window that follows nested activity already got that
//!   from `reactions_core::follow` and is left alone. The armed dual turns nested bails into compared windows or into
//!   differences: that run is END-OF-PHASE ONLY (the first half of G-flip), and each difference it shows gets a regression test
//!   before its fix.
//!
//! Drivers that run their own loop instead of `drive_windowed` (the pay step of the landing windows, `strategy_cards_core`, the
//! production window) are constructed here too but armed only when they call [`arm`] themselves; none does yet.

use std::cell::Cell;
use std::sync::atomic::{AtomicU8, Ordering};

use ti4_engine_core::driver::Driver;
use ti4_engine_core::kernel::Resolver;
use ti4_engine_core::kernel::stage::{AbilityTable, Window};
use ti4_engine_core::state::{CoreState, Slice};
use ti4_model::state::GameState;

use crate::render::Render;

/// 0: not read yet, 1: off, 2: on.
static ARMED: AtomicU8 = AtomicU8::new(0);

thread_local! {
    static OVERRIDE: Cell<Option<bool>> = const { Cell::new(None) };
}

/// What `TI4_DRIVER_ARMED` says: only `1` arms.
fn flag_of(value: Option<&str>) -> bool {
    value == Some("1")
}

/// Whether the drivers are armed: [`with_armed`] on this thread, else the process flag ([`set_armed`], else
/// `TI4_DRIVER_ARMED=1`, else off).
#[must_use]
// mutants:skip (the first read caches the environment in a process-wide cell; the decision is `flag_of`, tested)
pub fn armed() -> bool {
    if let Some(on) = OVERRIDE.with(Cell::get) {
        return on;
    }
    match ARMED.load(Ordering::Relaxed) {
        1 => false,
        2 => true,
        _ => {
            #[cfg(not(target_arch = "wasm32"))]
            let on = flag_of(std::env::var("TI4_DRIVER_ARMED").ok().as_deref());
            #[cfg(target_arch = "wasm32")]
            let on = false;
            ARMED.store(if on { 2 } else { 1 }, Ordering::Relaxed);
            on
        }
    }
}

/// Set the process flag (a gate binary's switch).
// mutants:skip (a process-wide switch; `armed` is tested through `with_armed` and `driver_for`)
pub fn set_armed(on: bool) {
    ARMED.store(if on { 2 } else { 1 }, Ordering::Relaxed);
}

/// Run `body` with the flag set to `on` on this thread only; restored afterwards, also on panic.
pub fn with_armed<T>(on: bool, body: impl FnOnce() -> T) -> T {
    struct Restore(Option<bool>);
    impl Drop for Restore {
        fn drop(&mut self) {
            OVERRIDE.with(|o| o.set(self.0));
        }
    }
    let _restore = Restore(OVERRIDE.with(|o| o.replace(Some(on))));
    body()
}

/// The driver of one adapter window: `Driver::new(window, resolver, table, next_id)`. The arguments are the adapter's own
/// (its seats, its table, `1` or the recorded first event id); [`arm`] changes them later, when the flag is on.
#[must_use]
pub(crate) fn driver_for<W: Window>(window: W, resolver: Resolver, table: AbilityTable, next_id: u64) -> Driver<W> {
    Driver::new(window, resolver, table, next_id)
}

/// The driver of a window the core RUNS (package KF6): a live window is never replayed against a recording, so it is always the driver of
/// the state (`Driver::from_state`: the spent keys and the trace counter of `Pending::resolver`), armed with the registry and the table
/// of the game's live `Env` (`live::env::env_for`) and seeded from the game's streams. The adapters' `driver_for` stays the constructor of
/// the replay path; a site is CONVERTED when its adapter's live opener builds its driver here (or runs through `live::advance`, which does).
#[must_use]
pub fn driver_live<W: Window>(window: W, core: &CoreState, env: &ti4_engine_core::driver::live::Env<'_>) -> Driver<W> {
    Driver::from_state(window, core, env.registry.clone(), env.table.clone()).with_seeds(env.seeds.clone())
}

/// With the flag on, give a driver that follows no nested activity the resolver of the flip: the core table, the registry of
/// every seat, the spent keys and the trace counter of the state. Does nothing with the flag off.
pub(crate) fn arm<W: Window>(driver: &mut Driver<W>, r: &Render, gs: &GameState, core: &CoreState) {
    if !armed() {
        return;
    }
    let seats = std::mem::replace(&mut driver.resolver, Resolver::new(&[], None));
    let mut follow = crate::reactions_core::Follow::armed(r, gs, seats);
    follow.arm(r, gs, core);
    // the window's own links and facts stay: the armed table is the core table plus what the adapter set
    let own = std::mem::take(&mut driver.table);
    driver.table = merge_tables(follow.table, own);
    driver.resolver = follow.resolver;
    if core.loaded.has(Slice::Pending) {
        let carried = &core.pending().resolver;
        for (ability, scope, counter) in carried.used.iter() {
            driver.resolver.spend(*ability, *scope, *counter);
        }
        driver.next_id = driver.next_id.max(carried.next_id);
    }
}

/// The armed table with the window's own table's hooks where the armed one has none (the adapter set `bars`, `on_close`,
/// `slot_cards` ... for its window; the core table carries the ability rules).
fn merge_tables(mut armed: AbilityTable, own: AbilityTable) -> AbilityTable {
    armed.on_close = armed.on_close.or(own.on_close);
    armed.slot_cards = armed.slot_cards.or(own.slot_cards);
    armed.after_play = armed.after_play.or(own.after_play);
    armed.bars = armed.bars.or(own.bars);
    armed.card_name = armed.card_name.or(own.card_name);
    keep_exploration(&mut armed, &own);
    armed
}

/// What an exploration child of a window needs (the words of a log row, the event texts and the unit forms) is the adapter's, not the core
/// table's: a window whose table is replaced by the core table (it follows nested activity, or is armed) keeps what it was built with.
pub(crate) fn keep_exploration(table: &mut AbilityTable, own: &AbilityTable) {
    if own.facts.explore_words.is_some() {
        table.facts.explore_words = own.facts.explore_words;
        table.facts.roster = own.facts.roster;
        table.codes.explore = own.codes.explore;
    }
}

#[cfg(test)]
mod tests {
    use ti4_engine_core::Seat;
    use ti4_engine_core::choice::reaction::{AbilityId, AbilityKind};
    use ti4_engine_core::choice::{Choice as CoreChoice, Pick};
    use ti4_engine_core::kernel::stage::{Illegal, Sx};
    use ti4_engine_core::kernel::{Kernel, Next};
    use ti4_engine_core::state::{Code, UsedScope};

    use super::*;
    use crate::core_sync::{Footprint, Sources, with_core};

    /// A window that ends at once: the driver and its resolver are all there is to look at.
    #[derive(Clone)]
    struct Idle;

    impl Window for Idle {
        type Stage = ();
        fn stage(&self) {}
        fn step(&mut self, _: &mut Sx<'_>) -> Next {
            Next::Done
        }
        fn ask_of(&self, _: &CoreState, _: &Kernel) -> Option<CoreChoice> {
            None
        }
        fn answer(&mut self, _: &mut Sx<'_>, _: Pick) -> Result<(), Illegal> {
            Err(Illegal)
        }
    }

    fn idle(next_id: u64) -> Driver<Idle> {
        driver_for(Idle, Resolver::new(&[Seat(0), Seat(1)], Some(Seat(0))), AbilityTable::default(), next_id)
    }

    /// Run `body` on the core state, the render tables and the string position of a two-seat game (Mentak and Sol have abilities).
    fn on_a_position<T>(body: impl FnOnce(&Render, &GameState, &mut CoreState) -> T) -> T {
        let mut gs = crate::fixtures::seated_dealt(&[("a", "mentak"), ("b", "sol")], ti4_model::content_types::POK);
        let (mut cx, r) = crate::dual_window::tables_for_tests(&gs).expect("a position the codecs carry");
        let at = gs.clone();
        // every slice but the map (which needs the string galaxy); the pending slice is written
        let reads: Vec<Slice> = Slice::ALL.iter().copied().filter(|s| *s != Slice::Map).collect();
        with_core(&mut gs, &mut cx, Sources::default(), Footprint::new(&reads, &[Slice::Pending]), |core, _| body(&r, &at, core)).expect("lendable")
    }

    #[test]
    fn a_live_driver_continues_the_trace_ids_of_the_state_and_is_seeded() {
        on_a_position(|_, _, core| {
            core.pending_mut().resolver.next_id = 41;
            let kernel = Kernel::plain();
            let env = ti4_engine_core::driver::live::Env {
                kernel: &kernel,
                registry: ti4_engine_core::containers::Cow::new(ti4_engine_core::kernel::Registry::default()),
                table: AbilityTable::default(),
                seeds: std::rc::Rc::new(ti4_engine_core::driver::NoSeeds),
            };
            let d = driver_live(Idle, core, &env);
            assert_eq!(d.next_id, 41, "the counter is the state's");
            assert!(d.is_seeded(), "a live driver always draws from the game's streams");
        });
    }

    #[test]
    fn with_armed_sets_the_flag_on_this_thread_only_and_restores_it_also_after_a_panic() {
        let outside = armed();
        with_armed(true, || {
            assert!(armed());
            with_armed(false, || assert!(!armed()));
            assert!(armed(), "the inner override is undone");
            std::thread::spawn(|| assert_eq!(armed(), super::ARMED.load(Ordering::Relaxed) == 2)).join().expect("another thread sees the process flag");
        });
        assert_eq!(armed(), outside);
        let result = std::panic::catch_unwind(|| with_armed(!outside, || panic!("boom")));
        assert!(result.is_err());
        assert_eq!(armed(), outside, "restored by the guard");
    }

    #[test]
    fn the_constructor_is_driver_new_and_keeps_the_adapters_arguments() {
        let d = idle(7);
        let plain = Driver::new(Idle, Resolver::new(&[Seat(0), Seat(1)], Some(Seat(0))), AbilityTable::default(), 7);
        assert_eq!(d.next_id, plain.next_id);
        assert_eq!(d.next_id, 7);
        assert_eq!(d.resolver.registry().len(), plain.resolver.registry().len());
        assert_eq!(d.resolver.registry().len(), 0, "an adapter's window starts with an empty registry");
        assert!(d.resolver.used_keys().is_empty());
    }

    #[test]
    fn with_the_flag_off_arming_changes_nothing() {
        on_a_position(|r, gs, core| {
            core.pending_mut().resolver.next_id = 9;
            let mut d = idle(1);
            with_armed(false, || arm(&mut d, r, gs, core));
            assert_eq!(d.resolver.registry().len(), 0);
            assert_eq!(d.next_id, 1);
        });
    }

    #[test]
    fn with_the_flag_on_the_window_gets_the_registry_of_every_seat_and_the_resolver_of_the_state() {
        on_a_position(|r, gs, core| {
            let spent = AbilityId { kind: AbilityKind::Ability, key: Code(3) };
            let mut carried = Resolver::new(&[Seat(0), Seat(1)], Some(Seat(0)));
            carried.spend(spent, UsedScope::Round, 1);
            core.pending_mut().resolver = carried.into_state(9);
            let mut d = idle(1);
            with_armed(true, || arm(&mut d, r, gs, core));
            assert!(d.resolver.registry().len() > 0, "Mentak's and Sol's abilities are registered");
            assert_eq!(d.next_id, 9, "the trace counter of the state");
            assert_eq!(d.resolver.used_keys(), vec![(spent, UsedScope::Round, 1)], "the spent keys of the state");
            let again = d.resolver.registry().len();
            let mut twice = idle(1);
            with_armed(true, || arm(&mut twice, r, gs, core));
            assert_eq!(twice.resolver.registry().len(), again, "the registry is derived from the position, not accumulated");
        });
    }

    /// The point of the module: no adapter builds its driver by hand any more. A `Driver::new(` in the production part of a file of this crate
    /// (text before its first `#[cfg(test)]` / `#[cfg(all(test` line; test files are skipped) is a driver the armed flag cannot reach.
    #[test]
    fn no_adapter_builds_a_driver_without_the_constructor() {
        fn walk(dir: &std::path::Path, hits: &mut Vec<String>) {
            for entry in std::fs::read_dir(dir).expect("the source directory") {
                let path = entry.expect("an entry").path();
                if path.is_dir() {
                    walk(&path, hits);
                    continue;
                }
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
                if !name.ends_with(".rs") || name.ends_with("_tests.rs") || name == "tests.rs" || name == "window_driver.rs" {
                    continue;
                }
                let text = std::fs::read_to_string(&path).expect("a source file");
                for (n, line) in text.lines().enumerate() {
                    let trimmed = line.trim_start();
                    if trimmed.starts_with("#[cfg(test)]") || trimmed.starts_with("#[cfg(all(test") {
                        break;
                    }
                    if !trimmed.starts_with("//") && line.contains("Driver::new(") {
                        hits.push(format!("{}:{}", path.display(), n + 1));
                    }
                }
            }
        }
        let mut hits = Vec::new();
        walk(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut hits);
        assert!(hits.is_empty(), "build the driver with window_driver::driver_for: {hits:?}");
    }

    #[test]
    fn only_a_one_arms_the_environment_flag() {
        assert!(flag_of(Some("1")));
        for off in [None, Some(""), Some("0"), Some("true"), Some("11")] {
            assert!(!flag_of(off), "{off:?}");
        }
    }

    #[test]
    fn the_armed_table_keeps_the_core_rules_and_takes_the_hooks_the_window_set() {
        fn bars(_: &CoreState, _: Seat) -> bool {
            true
        }
        let mut core_table = AbilityTable::default();
        core_table.card_name = Some(|_| 7);
        let mut own = AbilityTable::default();
        own.bars = Some(bars);
        own.card_name = Some(|_| 9);
        let merged = merge_tables(core_table, own);
        assert!(merged.bars.is_some(), "the window's own hook fills the gap");
        assert_eq!(merged.card_name.map(|f| f(ti4_engine_core::generated::ActionCard::ALL[0])), Some(7), "the core table's own hook wins");
    }
}
