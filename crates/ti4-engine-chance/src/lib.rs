//! The chance resolver (docs/FORK_RNG_THEORY.md 4.4, lane C3).
//!
//! The core never draws: every random event (a roll group, a random card, the Neuraloop's objective) is a question,
//! `Ask::Generic(generic::Ask::Chance(request))`, put like any nested question and answered with `Pick::Chance(faces)`
//! (decision D1: there is no inline fast path). The streams stay in the position (`CoreState::rng`, `ti4_chance::Streams`)
//! as data. This crate is the one place that turns the two together:
//!
//! * [`Mode::Play`] (the product, the benches, selfplay, the browser): [`advance_resolved`] answers every chance question from
//!   the position's streams with `ti4_chance::answer_mut`, writing the advanced streams into the position BEFORE the resumed
//!   run, and calls the arm again, until the arm stops at a question that is not chance or ends. A caller in play never sees a
//!   chance question.
//! * [`Mode::Search`] (decision D2): nothing is answered and the streams are never read or moved; every chance question goes
//!   up to the caller, which answers it with faces of its own (`advance(core, env, Some(Pick::Chance(faces)))`).
//!
//! Exactness: the core's run of an operation snapshots the position at the start of each iteration, which is after the write of
//! the streams; a rollback inside or around the resumed run restores a position that holds them. So the sequence of answered
//! requests equals the draws the core made before this lane, in order (T3: the quick-game fingerprint and the perf suite hash do
//! not move). On an error the streams are put back to what they were before the answer: the position then holds the question and
//! the streams that match it again.
//!
//! Layer: over `ti4-engine-core` and `ti4-chance` only (tools/engine_core/check_layers.py `resolver`).

use ti4_chance::{Faces, Request, answer_mut};
use ti4_engine_core::choice::{Ask, Choice, Pick, generic};
use ti4_engine_core::driver::live::{Asking, Env, Live, LiveError, LiveFn, Out};
use ti4_engine_core::driver::{Driver, DriverError, Status};
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::kernel::stage::Window;
use ti4_engine_core::state::CoreState;

/// Who answers the chance questions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// The position's own streams answer them (the product).
    Play,
    /// Nobody here: they go up to the caller (a tree search), and the streams are left alone.
    Search,
}

/// The chance request a question is, if it is one.
#[must_use]
pub fn chance_of(choice: &Choice) -> Option<Request> {
    match choice.ask {
        Ask::Generic(generic::Ask::Chance(req)) => Some(req),
        _ => None,
    }
}

/// [`chance_of`] of a live question.
#[must_use]
pub fn chance_asked(asking: &Asking) -> Option<Request> {
    chance_of(&asking.choice)
}

/// `f` (an arm's live function, or any function of its shape) with `answer`, then every chance question it stops at answered from
/// the position's streams, until it stops at another question or ends. What the calls produced is merged in order.
///
/// # Errors
/// What `f` refuses with; the position is then what `f` left it (its own rollback) with the streams put back.
pub fn advance_resolved(core: &mut CoreState, env: &Env<'_>, f: LiveFn, answer: Option<Pick>) -> Result<(Live, Out), LiveError> {
    advance_observed(core, env, f, answer, &mut |_, _| {})
}

/// `f` as a function that answers its chance questions ([`advance_resolved`]): `resolved(advance::<W>)(core, env, answer)`.
pub fn resolved(f: LiveFn) -> impl Fn(&mut CoreState, &Env<'_>, Option<Pick>) -> Result<(Live, Out), LiveError> {
    move |core, env, answer| advance_resolved(core, env, f, answer)
}

/// [`advance_resolved`] in `mode`: in [`Mode::Search`] `f` is called once and its chance question (if any) is returned as it is.
///
/// # Errors
/// As [`advance_resolved`].
pub fn advance_in(mode: Mode, core: &mut CoreState, env: &Env<'_>, f: LiveFn, answer: Option<Pick>) -> Result<(Live, Out), LiveError> {
    match mode {
        Mode::Play => advance_resolved(core, env, f, answer),
        Mode::Search => f(core, env, answer),
    }
}

/// [`advance_resolved`] that shows each answered request and its faces to `seen` (the census of the benches, the draw-sequence tests).
///
/// # Errors
/// As [`advance_resolved`].
pub fn advance_observed(core: &mut CoreState, env: &Env<'_>, f: LiveFn, answer: Option<Pick>, seen: &mut dyn FnMut(&Request, &Faces)) -> Result<(Live, Out), LiveError> {
    let (mut live, mut out) = f(core, env, answer)?;
    loop {
        let Live::Asks(asking) = &live else { return Ok((live, out)) };
        let Some(req) = chance_asked(asking) else { return Ok((live, out)) };
        let saved = core.rng.clone();
        let faces = answer_mut(&req, &mut core.rng);
        seen(&req, &faces);
        match f(core, env, Some(Pick::Chance(faces))) {
            Ok((next, more)) => {
                merge(&mut out, more);
                live = next;
            }
            Err(e) => {
                core.rng = saved;
                return Err(e);
            }
        }
    }
}

/// The output of one more call appended to what the calls before it produced.
fn merge(out: &mut Out, more: Out) {
    out.emitted.extend(more.emitted);
    out.journal.extend(more.journal);
    out.opened.extend(more.opened);
    out.held = more.held;
}

/// The driver-level form (tests, the oracle's window duals): `status` (what `run` or `resume` returned), then every chance question
/// answered from `s`'s streams by `resume`, until another question or the end.
///
/// # Errors
/// What the driver refuses with (the streams are put back).
pub fn settle<W: Window>(d: &mut Driver<W>, s: &mut CoreState, k: &Kernel, status: Result<Status, DriverError>) -> Result<Status, DriverError> {
    settle_observed(d, s, k, status, &mut |_, _| {})
}

/// [`settle`] showing each answered request to `seen`.
///
/// # Errors
/// As [`settle`].
pub fn settle_observed<W: Window>(d: &mut Driver<W>, s: &mut CoreState, k: &Kernel, status: Result<Status, DriverError>, seen: &mut dyn FnMut(&Request, &Faces)) -> Result<Status, DriverError> {
    let mut status = status?;
    loop {
        let Status::Asking(choice) = &status else { return Ok(status) };
        let Some(req) = chance_of(choice) else { return Ok(status) };
        let saved = s.rng.clone();
        let faces = answer_mut(&req, &mut s.rng);
        seen(&req, &faces);
        match d.resume(s, k, Pick::Chance(faces)) {
            Ok(next) => status = next,
            Err(e) => {
                s.rng = saved;
                return Err(e);
            }
        }
    }
}

/// [`settle`] answering from `streams` instead of the position's (a test rig's own randomness, or a search that answers with a
/// sampler of its own): the position's streams are not read or moved.
///
/// # Errors
/// As [`settle`].
pub fn settle_from<W: Window>(d: &mut Driver<W>, s: &mut CoreState, k: &Kernel, status: Result<Status, DriverError>, streams: &mut ti4_chance::Streams) -> Result<Status, DriverError> {
    let mut status = status?;
    loop {
        let Status::Asking(choice) = &status else { return Ok(status) };
        let Some(req) = chance_of(choice) else { return Ok(status) };
        let saved = streams.clone();
        let faces = answer_mut(&req, streams);
        match d.resume(s, k, Pick::Chance(faces)) {
            Ok(next) => status = next,
            Err(e) => {
                *streams = saved;
                return Err(e);
            }
        }
    }
}
