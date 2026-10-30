//! `smoke`: the standard rule test of one window position. It runs the position four times, answering first / last / two mixed options, and at
//! every step checks the contract (P4 options sound, P6 conservation) and that a replay of the answers reaches the same state (P3). The run that
//! always takes the first option is snapshotted (`BLESS=<area> cargo test ...` writes it; review the file against the rules: it is the question
//! sequence and the delta of the state).

use ti4_engine_core::choice::{Opt, Pick};
use ti4_engine_core::kernel::stage::Window;

use crate::inv;
use crate::rig::{Policy, Rig};
use crate::snap::assert_snapshot;

/// Questions after which the option probe is no longer repeated (each probe replays the run).
const PROBED: usize = 8;

fn drive<W: Window>(mut rig: Rig<W>, policy: Policy) -> Rig<W> {
    rig.run();
    let mut n = 0u64;
    loop {
        if let Err(bad) = inv::conservation(rig.start_state(), &rig.state) {
            panic!("an invariant broke: {bad:?}\n{}", rig.transcript());
        }
        if rig.is_done() {
            break;
        }
        assert!(n < 120, "more than 120 questions\n{}", rig.transcript());
        if (n as usize) < PROBED {
            rig.assert_options_sound();
        }
        let c = rig.asked();
        let len = c.opts.len();
        let i = match policy {
            Policy::First => 0,
            Policy::Last => len.saturating_sub(1),
            Policy::Mix(seed) => ((seed + n * 7) % len.max(1) as u64) as usize,
        };
        let pick = c.pick(i).unwrap_or(Pick::Opt(Opt::Decline));
        if let Err(e) = rig.try_answer(pick) {
            panic!("{pick:?} refused: {e:?}\n{}", rig.transcript());
        }
        n += 1;
    }
    let again = rig.replay();
    assert!(again.state == rig.state, "a replay of the answers reached another state:\n{}", crate::delta::delta(&rig.state, &again.state));
    rig
}

/// Run the position `make` builds under four answer policies; snapshot the first-option run as `name` (`area/name`).
///
/// # Panics
/// On any violation, with the transcript.
pub fn smoke<W: Window>(name: &str, make: impl Fn() -> Rig<W>) -> Rig<W> {
    let first = drive(make(), Policy::First);
    assert_snapshot(name, &first.snapshot());
    drive(make(), Policy::Last);
    drive(make(), Policy::Mix(1));
    drive(make(), Policy::Mix(2));
    first
}

/// [`smoke`] without a snapshot (for positions whose text is not worth keeping, e.g. the variants of a table test).
///
/// # Panics
/// On any violation, with the transcript.
pub fn smoke_quiet<W: Window>(make: impl Fn() -> Rig<W>) -> Rig<W> {
    let first = drive(make(), Policy::First);
    drive(make(), Policy::Last);
    drive(make(), Policy::Mix(1));
    first
}
