//! The core-level properties P1-P6 (docs/todo/D4-core-tests.md 5.2) as plain functions over a window factory.
//!
//! A *factory* builds the window from a scenario (and may add to the scenario what the window needs): `Fn(Scenario) -> (Scenario, W)`.
//! A property test draws a scenario and a pick script (`generators`), and calls [`check_all`] (or one property). Every property panics with
//! the transcript of the run on a violation, so a proptest failure shrinks to a readable case.

use ti4_engine_core::choice::{Opt, Pick};
use ti4_engine_core::driver::DriverError;
use ti4_engine_core::kernel::stage::Window;

use crate::inv;
use crate::rig::Rig;
use crate::scenario::Scenario;

/// Run `rig` to its first question, then answer `script[i] % options` at each question; `each` sees the rig after every step (also the last).
fn walk<W: Window>(rig: &mut Rig<W>, script: &[u8], mut each: impl FnMut(&Rig<W>)) {
    rig.run();
    each(rig);
    for b in script {
        if rig.is_done() {
            break;
        }
        let len = rig.asked().opts.len().max(1);
        let i = usize::from(*b) % len;
        let pick = rig.asked().pick(i).unwrap_or(Pick::Opt(Opt::Decline));
        if rig.try_answer(pick).is_err() {
            panic!("the answer {pick:?} was refused\n{}", rig.transcript());
        }
        each(rig);
    }
}

/// P1 `clone_equals_and_diverges_independently`: a clone equals its original; running the window on the original leaves the clone as it was
/// (copy-on-write never writes through a shared chunk); a resolver checkpoint restored after the run reproduces the pre-run resolver.
///
/// # Panics
/// On a violation.
pub fn p1_clone_independence<W: Window>(make: &impl Fn(Scenario) -> (Scenario, W), sc: Scenario, script: &[u8]) {
    let (sc, w) = make(sc);
    let mut rig = Rig::new(sc, w);
    let before = rig.state.clone();
    assert!(before == rig.state, "a clone differs from its original");
    let checkpoint = format!("{:?}", rig.driver.resolver.checkpoint());
    let cp = rig.driver.resolver.checkpoint();
    walk(&mut rig, script, |_| {});
    assert!(before == *rig.start_state(), "running the window changed a clone taken before the run:\n{}", crate::delta::delta(&before, rig.start_state()));
    rig.driver.resolver.restore(cp);
    assert_eq!(format!("{:?}", rig.driver.resolver.checkpoint()), checkpoint, "restoring a resolver checkpoint did not reproduce the resolver");
}

/// P2 `driver_rollback_is_exact`: at a question of the window itself, `ask_of` is pure (twice the same, and the question the driver reported);
/// an illegal pick leaves state and question as they were; after `Done`, an answer is `NotAsking`.
///
/// # Panics
/// On a violation.
pub fn p2_rollback_is_exact<W: Window>(make: &impl Fn(Scenario) -> (Scenario, W), sc: Scenario, script: &[u8]) {
    let (sc, w) = make(sc);
    let mut rig = Rig::new(sc, w);
    rig.run();
    let mut i = 0;
    loop {
        if rig.is_done() {
            let r = rig.try_answer(Pick::Opt(Opt::Decline));
            assert_eq!(r.map(|_| ()), Err(DriverError::NotAsking), "an answer after Done must be NotAsking");
            return;
        }
        if rig.driver.nested_facts().is_none() {
            let state = rig.state.clone();
            let a = rig.driver.window.ask_of(&rig.state, &rig.kernel);
            let b = rig.driver.window.ask_of(&rig.state, &rig.kernel);
            assert_eq!(a, b, "ask_of is not pure\n{}", rig.transcript());
            assert!(state == rig.state, "ask_of changed the state");
            assert_eq!(a.as_ref(), Some(rig.asked()), "ask_of differs from the question the driver reported\n{}", rig.transcript());
        }
        let foreign = Pick::Opt(Opt::Number(-9_999_001));
        if !rig.asked().opts.iter().any(|o| *o == Opt::Number(-9_999_001)) {
            rig.expect_illegal(foreign);
        }
        let Some(b) = script.get(i) else { return };
        i += 1;
        let len = rig.asked().opts.len().max(1);
        let pick = rig.asked().pick(usize::from(*b) % len).unwrap_or(Pick::Opt(Opt::Decline));
        rig.try_answer(pick).unwrap_or_else(|e| panic!("{pick:?} refused: {e:?}"));
    }
}

/// P3 `replay_is_deterministic`: the same scenario and script twice give the same questions, the same final state (events, dice, rng
/// positions included), and a replay of the recorded answers reaches that state again.
///
/// # Panics
/// On a violation.
pub fn p3_replay_is_deterministic<W: Window>(make: &impl Fn(Scenario) -> (Scenario, W), sc: Scenario, script: &[u8]) {
    let run = |sc: Scenario| {
        let (sc, w) = make(sc);
        let mut rig = Rig::new(sc, w).seeded(7);
        walk(&mut rig, script, |_| {});
        rig
    };
    let (a, b) = (run(sc.clone()), run(sc));
    assert_eq!(a.transcript(), b.transcript(), "two runs asked different questions");
    assert!(a.state == b.state, "two runs reached different states:\n{}", crate::delta::delta(&a.state, &b.state));
    let c = a.replay();
    assert!(c.state == a.state, "a replay of the recorded answers reached another state:\n{}", crate::delta::delta(&a.state, &c.state));
    assert_eq!(c.transcript(), a.transcript());
}

/// P4 `options_are_sound`: at every question, no option twice, the actor seated, every option accepted on a replayed copy, a foreign pick illegal.
///
/// # Panics
/// On a violation.
pub fn p4_options_are_sound<W: Window>(make: &impl Fn(Scenario) -> (Scenario, W), sc: Scenario, script: &[u8]) {
    let (sc, w) = make(sc);
    let mut rig = Rig::new(sc, w).seeded(3);
    walk(&mut rig, &script[..script.len().min(10)], |r| {
        if !r.is_done() {
            r.assert_options_sound();
        }
    });
}

/// P5 `option_order_is_stable`: the position built in another construction order asks the same questions with the same options in the same order.
///
/// # Panics
/// On a violation.
pub fn p5_option_order_is_stable<W: Window>(make: &impl Fn(Scenario) -> (Scenario, W), sc: Scenario, script: &[u8], shuffle: u64) {
    let run = |sc: Scenario| {
        let (sc, w) = make(sc);
        let mut rig = Rig::new(sc, w).seeded(5);
        walk(&mut rig, script, |_| {});
        rig
    };
    let a = run(sc.clone());
    let b = run(sc.permuted(shuffle));
    assert_eq!(a.transcript(), b.transcript(), "the construction order of the position changed the questions");
}

/// P6 `conservation`: after every step the invariants of [`inv`] hold.
///
/// # Panics
/// On a violation.
pub fn p6_conservation<W: Window>(make: &impl Fn(Scenario) -> (Scenario, W), sc: Scenario, script: &[u8]) {
    let (sc, w) = make(sc);
    let mut rig = Rig::new(sc, w).seeded(11);
    if let Err(bad) = inv::conservation(rig.start_state(), rig.start_state()) {
        panic!("the scenario itself breaks an invariant: {bad:?}");
    }
    walk(&mut rig, script, |r| {
        if let Err(bad) = inv::conservation(r.start_state(), &r.state) {
            panic!("an invariant broke: {bad:?}\n{}", r.transcript());
        }
    });
}

/// All of P1-P6 on one case.
///
/// # Panics
/// On the first violation.
pub fn check_all<W: Window>(make: &impl Fn(Scenario) -> (Scenario, W), sc: &Scenario, script: &[u8], shuffle: u64) {
    p1_clone_independence(make, sc.clone(), script);
    p2_rollback_is_exact(make, sc.clone(), script);
    p3_replay_is_deterministic(make, sc.clone(), script);
    p4_options_are_sound(make, sc.clone(), script);
    p5_option_order_is_stable(make, sc.clone(), script, shuffle);
    p6_conservation(make, sc.clone(), script);
}
