//! The stage probe: a seam for the coverage census of the test crate (`tools/engine_core/d4/coverage.py`), nothing the rules read.
//!
//! A test installs a [`Probe`] on its thread ([`set`]); from then on every window that runs (a [`Driver`](super::Driver) step, or a window
//! run inline inside a job of the turn flow, `driver::turn::inline::run`) reports its type name and its stage, before a step and when
//! the run stops. The census joins them with the inventory of `impl Window for X` to say which windows and which stage variants a test
//! visited. Without a probe (every product thread) the cost is one read of a thread-local `Cell` that holds `None`.

use std::cell::Cell;
use std::fmt::Debug;

use crate::kernel::stage::Window;

/// What a test hands in: the type name of the window and its stage (`Debug`).
pub type Probe = fn(&'static str, &dyn Debug); // lint-allow(core): the type name of a window is a static text; a test seam, no game path reads it

thread_local! {
    static PROBE: Cell<Option<Probe>> = const { Cell::new(None) };
}

/// Install `probe` for this thread (`None` removes it).
pub fn set(probe: Option<Probe>) {
    PROBE.with(|c| c.set(probe));
}

/// Report the stage `w` is in, if a probe is installed on this thread.
#[inline]
pub(crate) fn seen<W: Window>(w: &W) {
    PROBE.with(|c| {
        if let Some(probe) = c.get() {
            probe(std::any::type_name::<W>(), &w.stage()); // lint-allow(core): the type name is a static text; a test seam
        }
    });
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;
    use crate::choice::Choice;
    use crate::kernel::stage::{Illegal, Sx};
    use crate::kernel::{Kernel, Next};
    use crate::choice::Pick;
    use crate::state::CoreState;

    #[derive(Clone)]
    struct Two(bool);

    impl Window for Two {
        type Stage = bool;
        fn stage(&self) -> bool {
            self.0
        }
        fn step(&mut self, _: &mut Sx<'_>) -> Next {
            self.0 = true;
            Next::Done
        }
        fn ask_of(&self, _: &CoreState, _: &Kernel) -> Option<Choice> {
            None
        }
        fn answer(&mut self, _: &mut Sx<'_>, _: Pick) -> Result<(), Illegal> {
            Ok(())
        }
    }

    thread_local! {
        static SEEN: RefCell<Vec<(bool, bool)>> = const { RefCell::new(Vec::new()) };
    }

    fn record(name: &'static str, stage: &dyn Debug) {
        // lint-allow is not needed in a test module
        let named = name.ends_with("Two");
        SEEN.with(|s| s.borrow_mut().push((named, format!("{stage:?}") == "true")));
    }

    #[test]
    fn a_probe_sees_the_window_before_a_step_and_when_the_run_stops_and_only_on_its_own_thread() {
        let w = Two(false);
        seen(&w); // no probe: nothing
        assert!(SEEN.with(|s| s.borrow().is_empty()));
        set(Some(record));
        seen(&w);
        seen(&Two(true));
        set(None);
        seen(&w);
        assert_eq!(SEEN.with(|s| s.borrow().clone()), [(true, false), (true, true)], "the type name and the stage, nothing after removal");
        set(Some(record));
        std::thread::spawn(|| seen(&Two(true))).join().unwrap();
        set(None);
        assert_eq!(SEEN.with(|s| s.borrow().len()), 2, "another thread has no probe");
    }
}
