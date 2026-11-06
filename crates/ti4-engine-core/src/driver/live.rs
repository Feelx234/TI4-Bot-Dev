//! The live loop's core half (package M0/KF2, `docs/D2_LF_RESUME.md`): a window the core OWNS between two steps.
//!
//! A *held* window ([`Held`]) lives on the `Pending` stack as a typed, text-free payload and nowhere else. The machine object
//! (with its children) is rebuilt from `(payload, position)` at every call ([`Held::from_pending`]), run by a fresh
//! [`Driver`] ([`advance`]), and written back ([`Held::to_pending`]) when it asks; it is removed from the stack when it ends. So
//! a checkpoint is `CoreState::clone`, a fork is a clone, and nothing the string engine holds is needed.
//!
//! A question asked from INSIDE an emission (a reaction window, an ability) suspends the step with the driver's own
//! replay-from-the-start device ([`Driver::restore_nested`]); the suspension (the operation and the answers so far) is
//! `Pending::nested`, written here and nowhere else. A blocking table never needs it (the caller answers inside the same
//! `advance` loop); it is what makes a step resumable by a caller that returns between questions (D3, the oracle's pull style). The
//! suspension remembers the question it was put ([`Asked`], package nestq-1): in the same position an answer is checked against it and
//! resumes at once, and [`ask`] reads it, where both replayed the step to rebuild it.
//!
//! The arm registry ([`live_arm`]) maps a `session::Arm` to the function that runs it; an arm that is not in it stays `Lend`.
//!
//! L5: reads and writes the position through the windows it runs.

use std::rc::Rc;

use crate::choice::{Choice, Pick};
use crate::containers::Cow;
use crate::driver::session::Arm;
use crate::driver::{Driver, DriverError, Status};
use crate::kernel::ability::{NestedFacts, Unknown};
use crate::kernel::stage::{AbilityTable, Window};
use crate::kernel::{Kernel, Registry, Resolver};
use crate::state::{CoreState, EventRec, SeedSource, Window as OpenWindow};

pub use crate::kernel::stage::Held;

/// What a live window needs besides the position: built once per game by the wrapper, never per step.
#[derive(Clone)]
pub struct Env<'a> {
    pub kernel: &'a Kernel,
    /// The abilities of every seat, derived from the position (`AbilityTable::registry`).
    pub registry: Cow<Registry>,
    pub table: AbilityTable,
    /// The game's random streams (`Sx::die`). A live driver is ALWAYS seeded.
    pub seeds: Rc<dyn SeedSource>,
}

/// Why a live call did not return a question or the end.
#[derive(Debug, Clone, PartialEq)]
pub enum LiveError {
    /// No window of the kind is open.
    NotOpen,
    /// The window asks nothing (or no nested question is pending) but an answer was given.
    NotAsking,
    /// The pick is not an option of the question. The position is untouched.
    Illegal,
    /// The window broke its contract, or the resolver refused.
    Driver(DriverError),
    /// An ability condition the core cannot answer (a documented bail of the rule it reads): in the product this is the bail.
    Unknown(Unknown),
}

impl From<DriverError> for LiveError {
    fn from(e: DriverError) -> Self {
        match e {
            DriverError::Illegal => Self::Illegal,
            DriverError::NotAsking => Self::NotAsking,
            other => Self::Driver(other),
        }
    }
}

/// A question for the table, with what the renderer needs of the nested question it may be.
#[derive(Debug, Clone, PartialEq)]
pub struct Asking {
    pub choice: Choice,
    /// A question asked from inside an emission: the event of its window and the position it was asked in (the driver rolled
    /// the step back; the display facts read this state).
    pub nested: Option<(NestedFacts, Box<CoreState>)>,
    /// The position the question was put in, when it is a question asked from inside a step (the driver rolled the step back; the display
    /// facts of the question read this state). `None` for a stage question of the window.
    pub at: Option<Box<CoreState>>,
}

/// The result of running a held window to its next stop.
#[derive(Debug, Clone, PartialEq)]
pub enum Live {
    Asks(Box<Asking>),
    /// The window ended and left the stack.
    Done,
}

/// What the call produced besides the stop.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Out {
    /// The events the window emitted itself, in order.
    pub emitted: Vec<EventRec>,
    /// Every event the call's resolver emitted, in the order of its trace: the window's own ones AND the ones the abilities answering them emitted inside
    /// (the card a reaction plays: `ACTION_CARD_PLAYED` inside `PRODUCTION_USED`). The string's `mirror_timing_log` mirrors all of them; the aftermath
    /// chain does the same from this list.
    pub journal: Vec<EventRec>,
    /// The windows the rules asked the driver to open (`Next::Open`), in order: the arm opens them.
    pub opened: Vec<OpenWindow>,
    /// With `Live::Done`: the window ended its CALL, not itself ([`Held::holds`]): it is still on the stack, nothing is asked, and the string's next
    /// step (which asks nothing either) closes it.
    pub held: bool,
}

/// Open `window`: write its payload. The first [`advance`] (`None`) then runs it to its first question.
pub fn open<M: Held>(core: &mut CoreState, window: &M) {
    window.to_pending(core);
}

/// Whether a window of kind `M::KIND` is open.
#[must_use]
pub fn is_open<M: Held>(core: &CoreState) -> bool {
    core.pending().window(M::KIND).is_some()
}

/// The resolver's clock is the position's own (phase, round, turn, seating order, active seat, speaker).
fn sync_clock(r: &mut Resolver, core: &CoreState) {
    let m = core.meta();
    r.set_phase(m.phase);
    r.set_active_player(m.active);
    r.set_speaker(Some(m.speaker));
    r.set_seating_order(core.seats().seating_order());
    r.set_counters(u64::from(m.round), u64::from(m.turn_seq));
    // The windows of an emission go to the seats in the position's initiative order; a resolver rebuilt from a position that never set
    // it (the first window of a game) would offer them to the active seat alone (`chain::sync_initiative`, lane M5).
    r.set_initiative_order(crate::query::seats::initiative_order(core).as_slice());
}

/// The ability table of one call: the game's table with the play facts of THIS position (a card played inside the window reads what a
/// seat places, the research it opens, the Mahact pools; they change every step, so they are computed here, never kept in the `Env`).
fn table_at(core: &CoreState, env: &Env<'_>) -> AbilityTable {
    let mut table = env.table.clone();
    table.facts.arm_plays(core, env.kernel);
    table
}

fn driver_of<M: Held>(window: M, core: &CoreState, env: &Env<'_>) -> Driver<M> {
    let mut d = Driver::from_state(window, core, env.registry.clone(), table_at(core, env)).with_seeds(env.seeds.clone());
    sync_clock(&mut d.resolver, core);
    d
}

/// Whether `pick` answers `choice`: one of its options, or a number to a count question.
fn is_option(choice: &Choice, pick: &Pick) -> bool {
    match pick {
        Pick::Opt(o) => choice.opts.iter().any(|x| x == o),
        Pick::Number(_) => matches!(choice.ask, crate::choice::Ask::Generic(crate::choice::generic::Ask::Count { .. })),
    }
}

/// The question the open window of kind `M::KIND` waits on, if any: a pure read for a stage question, a replay of the suspended
/// step for a nested one. `Ok(None)`: it waits on nothing (fresh, or about to end): [`advance`] with `None` runs it.
///
/// # Errors
/// The window is not open, or the replay of a suspended step failed.
pub fn ask<M: Held>(core: &CoreState, env: &Env<'_>) -> Result<Option<Asking>, LiveError> {
    let window = M::from_pending(core, env.kernel).ok_or(LiveError::NotOpen)?;
    let asked = recall::<M>(core);
    #[cfg(feature = "probe-counters")]
    if core.pending().nested.is_some() {
        memo_census::note(asked.is_some(), true);
    }
    if let Some(asked) = asked {
        let asking = as_replayed::<M>(core, env, &asked);
        if let Some(n) = &core.pending().nested {
            verify::<M>(core, env, n, &asking, true);
        }
        return Ok(Some(asking));
    }
    if let Some(n) = core.pending().nested.clone() {
        let mut scratch = core.clone();
        let mut d = driver_of(window, core, env);
        d.restore_nested(&n);
        return match d.reask(&mut scratch, env.kernel)? {
            Status::Asking(choice) => Ok(Some(asking_of(&d, choice))),
            Status::Done => Ok(None),
        };
    }
    Ok(window.ask_of(core, env.kernel).map(|choice| Asking { choice, nested: None, at: None }))
}

pub(crate) fn asking_of<M: Window>(d: &Driver<M>, choice: Choice) -> Asking {
    let at = d.nested_state().cloned().map(Box::new);
    let nested = d.nested_facts().cloned().zip(at.clone());
    Asking { choice, nested, at }
}

/// Run the open window of kind `M::KIND` to its next question or its end. With `answer`, apply it first: to the pending nested
/// question when there is one, else to the window's own. Without, run (a fresh window; or a repeat of a question).
///
/// On any error the position is exactly what it was before the call (a clone is restored: the generic rollback of a step).
///
/// # Errors
/// See [`LiveError`].
pub fn advance<M: Held>(core: &mut CoreState, env: &Env<'_>, answer: Option<Pick>) -> Result<(Live, Out), LiveError> {
    let before = core.clone();
    let result = advance_inner::<M>(core, env, answer);
    if result.is_err() {
        *core = before;
    }
    result
}

fn advance_inner<M: Held>(core: &mut CoreState, env: &Env<'_>, answer: Option<Pick>) -> Result<(Live, Out), LiveError> {
    let window = M::from_pending(core, env.kernel).ok_or(LiveError::NotOpen)?;
    let mut d = driver_of(window, core, env);
    let asked = recall::<M>(core);
    let nested = core.pending_mut().nested.take();
    let status = match (nested, answer) {
        (Some(n), Some(pick)) => answer_nested(&mut d, core, env, &n, asked.as_deref(), pick)?,
        (Some(n), None) => {
            d.restore_nested(&n);
            d.reask(core, env.kernel)?
        }
        (None, Some(pick)) => {
            if !d.restore_stage(core, env.kernel) {
                return Err(LiveError::NotAsking);
            }
            d.resume(core, env.kernel, pick)?
        }
        (None, None) => d.run(core, env.kernel)?,
    };
    if let Some(why) = d.unknown() {
        return Err(LiveError::Unknown(why));
    }
    let mut out = Out { journal: crate::kernel::fx::journal_recs(&d.resolver, 0), emitted: std::mem::take(&mut d.emitted), opened: std::mem::take(&mut d.opened), held: false };
    d.store(core);
    match status {
        Status::Done if d.window.holds() => {
            d.window.to_pending(core);
            core.pending_mut().nested = None;
            out.held = true;
            Ok((Live::Done, out))
        }
        Status::Done => {
            d.window.closed(core, env.kernel);
            core.pending_mut().close(M::KIND);
            core.pending_mut().nested = None;
            Ok((Live::Done, out))
        }
        Status::Asking(choice) => {
            d.window.to_pending(core);
            let asking = asking_of(&d, choice);
            suspend(core, &d, &asking);
            Ok((Live::Asks(Box::new(asking)), out))
        }
    }
}

// -- the remembered nested question (package nestq-1, `docs/NESTQ1_RESUME.md`) -----------------------------------------------------

/// What a suspension remembers of its question (`Nested::asked`): the question as the call that suspended the step returned it, the kind of
/// the window that asked, and the position the call left (the suspension included). A later call trusts it only in that very position
/// ([`recall`]): an answer is then checked against it and resumes the step at once, where it re-ran the step to rebuild the question
/// first (`Driver::reask`: one run of the operation per answer), and [`ask`] reads it instead of a replay. A position changed in between (a
/// caller that wrote to it, a code table synced into it) has no question remembered: the replay rebuilds it as before.
#[derive(Debug)]
pub(crate) struct Asked {
    kind: crate::state::WindowKind,
    pub(crate) asking: Asking,
    at: CoreState,
    /// The operation as it stood at the question, when a converted path parked it (stepmachine-1): the answer resumes it there.
    pub(crate) park: Option<std::sync::Arc<crate::driver::Park>>,
}

/// Write the suspension of `d` (none: the question is a stage of the window, or there is none) with the question `asking` it was put.
pub(crate) fn suspend<M: Held>(core: &mut CoreState, d: &Driver<M>, asking: &Asking) {
    core.pending_mut().nested = d.nested_journal();
    if core.pending().nested.is_some() {
        let at = core.clone();
        if let Some(n) = core.pending_mut().nested.as_mut() {
            n.asked = crate::state::Memo::of(Asked { kind: M::KIND, asking: asking.clone(), at, park: d.park().cloned() });
        }
    }
}

/// The remembered question as a replay in `core` puts it. The positions it was put in are those of the suspending call's run, whose stack
/// still held the payload and the resolver the call BEGAN with; a replay starts from the position the call left, which holds what the call
/// wrote at its end (`Held::to_pending`, `Driver::store`). Those two are written into the copies here (on a read only: an answer needs the
/// question alone).
fn as_replayed<M: Held>(core: &CoreState, env: &Env<'_>, asked: &Asked) -> Asking {
    let mut asking = asked.asking.clone();
    let window = M::from_pending(core, env.kernel);
    for at in [asking.nested.as_mut().map(|(_, at)| &mut **at), asking.at.as_deref_mut()].into_iter().flatten() {
        if let Some(w) = &window {
            w.to_pending(at);
        }
        at.pending_mut().resolver = core.pending().resolver.clone();
    }
    asking
}

/// The question the suspension of `core` remembers, if it was put by a window of kind `M::KIND` in this very position.
pub(crate) fn recall<M: Held>(core: &CoreState) -> Option<std::sync::Arc<Asked>> {
    let asked = core.pending().nested.as_ref()?.asked.shared::<Asked>()?;
    (asked.kind == M::KIND && asked.at == *core).then_some(asked)
}

/// Answer the suspended question `n` with `pick`: checked against the remembered question `asked` when there is one, else against the
/// question a re-run of the step rebuilds. A pick that is no option is refused (`Illegal`); the caller rolls the position back.
///
/// # Errors
/// As [`advance`].
pub(crate) fn answer_nested<M: Held>(d: &mut Driver<M>, core: &mut CoreState, env: &Env<'_>, n: &crate::state::Nested, asked: Option<&Asked>, pick: Pick) -> Result<Status, LiveError> {
    // The driver replays a nested answer without judging it (the dual feeds it recorded ones): a live answer comes from outside, so it
    // must be one of the options of the question.
    d.restore_nested(n);
    #[cfg(feature = "probe-counters")]
    memo_census::note(asked.is_some(), false);
    let park = asked.and_then(|a| a.park.as_ref()).filter(|_| crate::driver::parking());
    let legal = match asked {
        Some(asked) => {
            verify::<M>(core, env, n, &asked.asking, false);
            crate::driver::note_answer(|| path_of::<M>(park.map(|p| &p.at), &asked.asking.choice));
            is_option(&asked.asking.choice, &pick)
        }
        None => {
            let Status::Asking(choice) = d.reask(core, env.kernel)? else { return Err(LiveError::NotAsking) };
            crate::driver::note_answer(|| path_of::<M>(None, &choice));
            is_option(&choice, &pick)
        }
    };
    if !legal {
        return Err(LiveError::Illegal);
    }
    if let Some(park) = park {
        let replayed = verifying_park().then(|| replay_answer::<M>(core, env, n, pick));
        let window = M::from_pending(core, env.kernel);
        let status = d.resume_parked(core, env.kernel, park, pick, |call, at| {
            if let Some(w) = &window {
                w.to_pending(at);
            }
            at.pending_mut().resolver = call.pending().resolver.clone();
        })?;
        if let Some(replayed) = replayed {
            check_parked(&replayed, &Answered::of(d, core, &status));
        }
        return Ok(status);
    }
    Ok(d.resume(core, env.kernel, pick)?)
}

/// The counter key of a nested answer (`driver::park_detail`).
fn path_of<M: Held>(park: Option<&crate::driver::Resume>, choice: &Choice) -> crate::driver::PathKey {
    crate::driver::PathKey { parked: park.map(crate::driver::Resume::key), kind: M::KIND, subtype: choice.ctx.subtype }
}

// -- the check of a parked answer (stepmachine-1) -------------------------------------------------------------------------------------

thread_local! {
    static VERIFY_PARK: std::cell::Cell<bool> = const { std::cell::Cell::new(cfg!(debug_assertions)) };
}

/// The check of the parked answers (on by default in debug builds, off in release; any build can switch it on): every answer that resumes a
/// parked operation is also answered by the replay, on a scratch copy with a fresh driver, and the two must leave the same position, the same
/// events, journal, opened windows, trace counter and the same next question (or end). A difference panics.
pub fn verify_parked(on: bool) {
    VERIFY_PARK.with(|v| v.set(on));
}

fn verifying_park() -> bool {
    VERIFY_PARK.with(std::cell::Cell::get)
}

/// What an answer left: compared field by field between the parked resume and the replay.
#[derive(Debug, PartialEq)]
struct Answered {
    core: CoreState,
    status: Status,
    emitted: Vec<EventRec>,
    journal: Vec<EventRec>,
    opened: Vec<OpenWindow>,
    resolver: crate::state::ResolverState,
    nested: Option<crate::state::Nested>,
    facts: Option<NestedFacts>,
    at: Option<CoreState>,
    window: crate::state::Pending,
}

impl Answered {
    fn of<M: Held>(d: &Driver<M>, core: &CoreState, status: &Status) -> Self {
        let mut written = core.clone();
        d.window.to_pending(&mut written);
        let window = written.pending().clone();
        Self {
            core: core.clone(),
            status: status.clone(),
            emitted: d.emitted.clone(),
            journal: crate::kernel::fx::journal_recs(&d.resolver, 0),
            opened: d.opened.clone(),
            resolver: d.resolver.into_state(d.next_id),
            nested: d.nested_journal(),
            facts: d.nested_facts().cloned(),
            at: d.nested_state().cloned(),
            window,
        }
    }
}

/// The answer `pick` to the suspended question `n` of `core`, by the replay (what [`answer_nested`] does without a park).
fn replay_answer<M: Held>(core: &CoreState, env: &Env<'_>, n: &crate::state::Nested, pick: Pick) -> Result<Answered, LiveError> {
    let window = M::from_pending(core, env.kernel).ok_or(LiveError::NotOpen)?;
    let mut scratch = core.clone();
    scratch.pending_mut().nested = None;
    let mut d = driver_of(window, &scratch, env);
    d.restore_nested(n);
    let status = d.resume(&mut scratch, env.kernel, pick)?;
    Ok(Answered::of(&d, &scratch, &status))
}

fn check_parked(replayed: &Result<Answered, LiveError>, parked: &Answered) {
    let Ok(replayed) = replayed else { panic!("the replay of a parked answer failed: {replayed:?}") };
    for (what, same) in [
        ("status", replayed.status == parked.status),
        ("position", replayed.core == parked.core),
        ("window", replayed.window == parked.window),
        ("emitted", replayed.emitted == parked.emitted),
        ("journal", replayed.journal == parked.journal),
        ("opened", replayed.opened == parked.opened),
        ("resolver", replayed.resolver == parked.resolver),
        ("suspension", replayed.nested == parked.nested),
        ("facts", replayed.facts == parked.facts),
        ("question position", replayed.at == parked.at),
    ] {
        if !same {
            // The first line of the two printouts that differs, with the lines before it (a failing check is read by a person).
            let (r, p) = (format!("{replayed:#?}"), format!("{parked:#?}")); // lint-allow(core): the message of a failed debug check
            let (r, p): (Vec<_>, Vec<_>) = (r.lines().collect(), p.lines().collect());
            let at = r.iter().zip(&p).position(|(a, b)| a != b).unwrap_or(0);
            let from = at.saturating_sub(12);
            panic!("a parked answer differs from its replay in the {what}:\nreplay:\n{}\nparked:\n{}", r[from..(at + 4).min(r.len())].join("\n"), p[from..(at + 4).min(p.len())].join("\n"));
        }
    }
}

thread_local! {
    static VERIFY: std::cell::Cell<bool> = const { std::cell::Cell::new(true) };
}

/// How often a nested question was served from its memo (feature `probe-counters` only, a test-build feature of the bridge; no product,
/// gate or wasm build has it): per thread, answers (`answer_nested`) and reads (`ask` with a nested question pending), each split into
/// served from the memo and re-asked (fallback). `checked` counts the memo uses the debug check (`verify_remembered`) rebuilt and compared.
#[cfg(feature = "probe-counters")]
pub mod memo_census {
    use std::cell::Cell;

    /// The counts since the last [`reset`].
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct Counts {
        /// Answers checked against the remembered question.
        pub answer_hits: u64,
        /// Answers that re-ran the step to rebuild the question (no memo, or a memo of another position).
        pub answer_fallbacks: u64,
        /// Reads (`ask`) served from the memo.
        pub read_hits: u64,
        /// Reads that replayed the step.
        pub read_fallbacks: u64,
        /// Memo uses the debug check rebuilt and compared (0 in builds without debug assertions or with the check off).
        pub checked: u64,
    }

    thread_local! {
        static COUNTS: Cell<Counts> = const { Cell::new(Counts { answer_hits: 0, answer_fallbacks: 0, read_hits: 0, read_fallbacks: 0, checked: 0 }) };
    }

    pub(super) fn note(hit: bool, read: bool) {
        COUNTS.with(|c| {
            let mut n = c.get();
            match (read, hit) {
                (false, true) => n.answer_hits += 1,
                (false, false) => n.answer_fallbacks += 1,
                (true, true) => n.read_hits += 1,
                (true, false) => n.read_fallbacks += 1,
            }
            c.set(n);
        });
    }

    pub(super) fn checked() {
        COUNTS.with(|c| {
            let mut n = c.get();
            n.checked += 1;
            c.set(n);
        });
    }

    /// Zero the counts of this thread.
    pub fn reset() {
        COUNTS.with(|c| c.set(Counts::default()));
    }

    /// The counts of this thread since the last [`reset`].
    #[must_use]
    pub fn counts() -> Counts {
        COUNTS.with(Cell::get)
    }
}

/// The check of the remembered questions (a test mode of debug builds, on by default there; release builds never check): every use of a
/// remembered question also rebuilds it with a fresh driver on a scratch copy and panics when the two differ. `false` turns it off on this
/// thread (a test that counts the runs).
pub fn verify_remembered(on: bool) {
    VERIFY.with(|v| v.set(on));
}

/// See [`verify_remembered`]. `core`: the position of the call; `asked`: the remembered question as it is used (`whole`: with its display
/// positions, a read; else the question and its display facts, an answer).
#[allow(unused_variables, reason = "the check is compiled into debug builds only")]
fn verify<M: Held>(core: &CoreState, env: &Env<'_>, n: &crate::state::Nested, asked: &Asking, whole: bool) {
    #[cfg(debug_assertions)]
    if VERIFY.with(std::cell::Cell::get) {
        let window = M::from_pending(core, env.kernel).expect("the window of a remembered question is open");
        let mut scratch = core.clone();
        scratch.pending_mut().nested = None;
        let mut d = driver_of(window, &scratch, env);
        d.restore_nested(n);
        let rebuilt = match d.reask(&mut scratch, env.kernel) {
            Ok(Status::Asking(choice)) => asking_of(&d, choice),
            other => panic!("a remembered question that a re-run does not ask: {other:?}"),
        };
        assert_eq!(rebuilt.choice, asked.choice, "the remembered question differs from the rebuilt one");
        assert_eq!(rebuilt.nested.as_ref().map(|(f, _)| f), asked.nested.as_ref().map(|(f, _)| f), "the remembered display facts differ");
        if whole {
            assert_eq!(rebuilt, *asked, "the remembered positions of the question differ");
        }
        #[cfg(feature = "probe-counters")]
        memo_census::checked();
    }
}

// -- the arm registry --------------------------------------------------------------------------------

/// The function that runs one arm live: `(position, env, the answer to the arm's pending question)`.
pub type LiveFn = fn(&mut CoreState, &Env<'_>, Option<Pick>) -> Result<(Live, Out), LiveError>;

/// The function that reads the question the arm's open window waits on.
pub type AskFn = fn(&CoreState, &Env<'_>) -> Result<Option<Asking>, LiveError>;

/// The arm registry (package KF1/M0): the live functions of `arm`, `None` while the arm is not native (`Step::Lend`). A lane adds its
/// arm HERE, one line in each of the two matches (the only edit this file takes from the lanes), and removes its `Lent::Arm` use.
#[must_use]
pub fn live_arm(arm: Arm) -> Option<LiveFn> {
    match arm {
        Arm::Tokens => Some(super::windows::status::advance_tokens),
        Arm::Status => Some(super::windows::status::advance_status),
        Arm::Scoring => Some(super::windows::status::advance_scoring),
        Arm::EventScoring => Some(super::windows::status::advance_event_scoring),
        Arm::Agenda => Some(super::agenda::advance_agenda),
        Arm::Voting => Some(super::agenda::advance_voting),
        Arm::Tactical => Some(crate::driver::windows::tactical::advance_tactical),
        Arm::Invasion => Some(crate::driver::windows::invasion::advance_invasion),
        Arm::Production => Some(crate::driver::windows::production::advance_production),
        Arm::Aftermath => Some(crate::driver::windows::chain::advance_aftermath),
        Arm::TurnPrelude => Some(super::turn::advance_prelude),
        Arm::StrategyAnnounce => Some(super::turn::advance_announce),
        Arm::Phase => Some(super::turn::advance_phase),
        Arm::Secondary => Some(super::windows::strategic::advance_secondary),
        Arm::LeaderFollowers => Some(super::windows::strategic::advance_leader_followers),
        Arm::Finished => Some(super::windows::strategic::advance_finished),
        Arm::Ask => Some(super::turn::ask::advance_ask),
        Arm::Trade => Some(super::windows::trade::advance_trade),
        _ => None,
    }
}

/// The ask counterpart of [`live_arm`] (the two entries of an arm go together).
#[must_use]
pub fn ask_arm_fn(arm: Arm) -> Option<AskFn> {
    match arm {
        Arm::Tokens => Some(super::windows::status::ask_tokens),
        Arm::Scoring => Some(super::windows::status::ask_scoring),
        Arm::EventScoring => Some(super::windows::status::ask_event_scoring),
        Arm::Voting => Some(super::agenda::ask_voting),
        Arm::Tactical => Some(crate::driver::windows::tactical::ask_tactical),
        Arm::Invasion => Some(crate::driver::windows::invasion::ask_invasion),
        Arm::Production => Some(crate::driver::windows::production::ask_production),
        Arm::Aftermath => Some(crate::driver::windows::chain::ask_aftermath),
        Arm::TurnPrelude | Arm::StrategyAnnounce | Arm::Phase | Arm::Status | Arm::Agenda => Some(super::turn::ask_job),
        Arm::Ask => Some(super::turn::ask::ask_ask),
        Arm::Trade => Some(super::windows::trade::ask_trade),
        Arm::Secondary => Some(super::windows::strategic::ask_secondary),
        Arm::LeaderFollowers => Some(super::windows::strategic::ask_leader_followers),
        Arm::Finished => Some(super::windows::strategic::ask_finished),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::choice::reaction::{AbilityId, AbilityKind, Ask as RAsk, Opt as ROpt, Relation};
    use crate::choice::{Ask, Opt};
    use crate::driver::NoSeeds;
    use crate::generated::Faction;
    use crate::ids::Seat;
    use crate::kernel::resolver::AbilityDef;
    use crate::kernel::stage::{Illegal, Slot, Spec, Sx};
    use crate::kernel::{fx, Next};
    use crate::state::{Code, EventKind, Payload, Player, Players, ProductionUse, ProductionWindow, UseMode, UsePhase, WindowKind};

    /// A toy machine held as the production window (only its phase is used: `Using` is whether its event was emitted). It emits one event and ends;
    /// the event has two optional abilities, so the emission asks from inside the step.
    #[derive(Clone)]
    struct Toy {
        emitted: bool,
    }

    impl Window for Toy {
        type Stage = bool;
        fn stage(&self) -> bool {
            self.emitted
        }
        fn step(&mut self, sx: &mut Sx<'_>) -> Next {
            if !self.emitted {
                self.emitted = true;
                if sx.emit(fx::event(EventKind::TradeGoodsGained, Payload::new())).is_err() {
                    return Next::Done;
                }
            }
            Next::Done
        }
        fn ask_of(&self, _: &CoreState, _: &Kernel) -> Option<Choice> {
            None
        }
        fn answer(&mut self, _: &mut Sx<'_>, _: Pick) -> Result<(), Illegal> {
            Err(Illegal)
        }
    }

    impl Held for Toy {
        const KIND: WindowKind = WindowKind::Production;
        fn from_pending(core: &CoreState, _: &Kernel) -> Option<Self> {
            match core.pending().window(WindowKind::Production)? {
                OpenWindow::Production(p) => Some(Self { emitted: p.phase == UsePhase::Using }),
                _ => None,
            }
        }
        fn to_pending(&self, core: &mut CoreState) {
            let (who, system) = (Seat(0), crate::generated::System::ALL[0]);
            let phase = if self.emitted { UsePhase::Using } else { UsePhase::Start };
            core.pending_mut().open(OpenWindow::Production(Box::new(ProductionUse { who, system, mode: UseMode::Home, phase, window: ProductionWindow::blank(who, system), answered: false })));
        }
    }

    fn ability(key: u16) -> AbilityId {
        AbilityId { kind: AbilityKind::Ability, key: Code(key) }
    }

    fn position() -> CoreState {
        let mut s = CoreState::all_loaded();
        s.players = Players::from_vec(vec![Player::new(Seat(0), Faction::from_index(0).unwrap())]);
        s
    }

    fn env(k: &Kernel) -> Env<'_> {
        let mut resolver = Resolver::new(&[Seat(0)], Some(Seat(0)));
        resolver.register([
            AbilityDef::new(ability(1), Seat(0), EventKind::TradeGoodsGained, Relation::After).with_optional(true).with_arg(1),
            AbilityDef::new(ability(2), Seat(0), EventKind::TradeGoodsGained, Relation::After).with_optional(true),
        ]);
        let mut table = AbilityTable::default();
        // The effect gains a trade good: a replayed step that applied it twice would show.
        table.add(
            Spec::new(Slot(1), AbilityKind::Ability, EventKind::TradeGoodsGained, Relation::After, |eff| {
                fx::gain_trade_goods(eff.state(), Seat(0), 1);
                Ok(())
            })
            .optional(),
        );
        Env { kernel: k, registry: Cow::new(resolver.registry().clone()), table, seeds: Rc::new(NoSeeds) }
    }

    fn goods(s: &CoreState) -> i16 {
        s.players().by_seat(Seat(0)).unwrap().econ.trade_goods
    }

    fn pick(id: u16) -> Pick {
        Pick::Opt(Opt::Reaction(ROpt::Ability(ability(id))))
    }

    /// Ask, drop the driver, rebuild from the position alone, answer: the same position as the run that was never interrupted.
    #[test]
    fn a_nested_question_survives_a_dropped_driver_and_continues_to_the_same_position() {
        let k = Kernel::plain();
        let env = env(&k);
        // The uninterrupted run, in one `Driver`.
        let mut straight = position();
        open(&mut straight, &Toy { emitted: false });
        let (live, _) = advance::<Toy>(&mut straight, &env, None).unwrap();
        let Live::Asks(first) = live else { panic!("the reaction window asks") };
        assert_eq!(first.choice.ask, Ask::Reaction(RAsk::Window { event: EventKind::TradeGoodsGained, relation: Relation::After }));
        assert!(first.nested.is_some(), "it is a question from inside an emission");
        assert!(straight.pending().nested.is_some(), "the suspension is in the position");
        assert_eq!(goods(&straight), 0, "the suspended run was rolled back");
        // A clone of the position stands for the checkpoint: the driver of the first call is long gone.
        let mut resumed = straight.clone();
        let asked = ask::<Toy>(&resumed, &env).unwrap().expect("the suspended question is asked again");
        assert_eq!(asked.choice, first.choice, "a replay of the suspension asks the same question");
        let (live, _) = advance::<Toy>(&mut resumed, &env, Some(pick(1))).unwrap();
        let Live::Asks(second) = live else { panic!("the second ability may still be used") };
        assert!(!second.choice.opts.iter().any(|o| *o == Opt::Reaction(ROpt::Ability(ability(1)))), "already resolved in this window");
        let (live, out) = advance::<Toy>(&mut resumed, &env, Some(Pick::Opt(Opt::Decline))).unwrap();
        assert_eq!(live, Live::Done);
        assert_eq!(goods(&resumed), 1, "the effect ran once");
        assert_eq!(out.emitted.len(), 1);
        assert!(!is_open::<Toy>(&resumed) && resumed.pending().nested.is_none(), "the window left the stack");
        // The same answers in one go on the position that was never cloned.
        advance::<Toy>(&mut straight, &env, Some(pick(1))).unwrap();
        advance::<Toy>(&mut straight, &env, Some(Pick::Opt(Opt::Decline))).unwrap();
        assert_eq!(straight, resumed, "a clone mid-window continues identically");
    }

    #[test]
    fn an_illegal_answer_leaves_the_position_exactly_as_it_was() {
        let k = Kernel::plain();
        let env = env(&k);
        let mut s = position();
        open(&mut s, &Toy { emitted: false });
        advance::<Toy>(&mut s, &env, None).unwrap();
        let held = s.clone();
        assert_eq!(advance::<Toy>(&mut s, &env, Some(Pick::Number(3))), Err(LiveError::Illegal));
        assert_eq!(s, held, "the rollback restores the position, the suspension included");
        let mut closed = position();
        assert_eq!(advance::<Toy>(&mut closed, &env, None), Err(LiveError::NotOpen));
    }

    #[test]
    fn a_nested_pick_that_is_no_option_is_refused_and_the_registry_names_the_native_arms() {
        let k = Kernel::plain();
        let env = env(&k);
        let mut s = position();
        open(&mut s, &Toy { emitted: false });
        assert!(is_open::<Toy>(&s), "an opened window is on the stack");
        advance::<Toy>(&mut s, &env, None).unwrap();
        let held = s.clone();
        assert_eq!(advance::<Toy>(&mut s, &env, Some(pick(9))), Err(LiveError::Illegal), "an ability the window does not offer");
        assert_eq!(s, held);
        assert!(live_arm(Arm::Tokens).is_some() && ask_arm_fn(Arm::Tokens).is_some());
        assert!(live_arm(Arm::Diplomacy).is_none() && ask_arm_fn(Arm::Diplomacy).is_none(), "an arm no lane has ported stays lent");
    }

    /// The play facts of a card played inside the window are those of the position the call starts from (`table_at`): the provider is
    /// asked at every call, never once per game. The ability reads the facts it finds and gains that many trade goods.
    #[test]
    fn the_play_facts_an_ability_finds_are_computed_from_the_position_of_the_call() {
        use crate::kernel::ability::{PlayFacts, PlaysProvider};
        let k = Kernel::plain();
        let mut env = env(&k);
        let mut table = AbilityTable::default();
        table.add(
            Spec::new(Slot(1), AbilityKind::Ability, EventKind::TradeGoodsGained, Relation::After, |eff| {
                // The facts of the position: `rules_version` stands for any fact that changes during a game.
                let seen = eff.adapter_facts().plays.as_ref().map_or(0, |p| i32::try_from(p.rules_version).unwrap_or(i32::MAX));
                fx::gain_trade_goods(eff.state(), Seat(0), seen);
                Ok(())
            })
            .optional(),
        );
        table.facts.plays_provider = Some(PlaysProvider(std::sync::Arc::new(|core, _| {
            Some(PlayFacts {
                scope: 0,
                blanked: false,
                action_card: Code(0),
                supply_base: Code(0),
                note_order: |a, b| a.cmp(&b),
                seats: Vec::new(),
                name_group: Vec::new(),
                has_effect: Vec::new(),
                skimmer: false,
                roster: None,
                rules_version: core.meta().rules_version,
                tech_order: |a, b| a.cmp(&b),
                shared: crate::kernel::ability::SharedPlay::default(),
            })
        })));
        env.table = table;
        // Two positions that differ in the one fact: the same Env, the same window, the facts follow the position.
        for version in [5u32, 9] {
            let mut s = position();
            s.meta_mut().rules_version = version;
            assert_eq!(table_at(&s, &env).facts.plays.as_ref().map(|p| p.rules_version), Some(version));
            open(&mut s, &Toy { emitted: false });
            advance::<Toy>(&mut s, &env, None).unwrap();
            advance::<Toy>(&mut s, &env, Some(pick(1))).unwrap();
            advance::<Toy>(&mut s, &env, Some(Pick::Opt(Opt::Decline))).unwrap();
            assert_eq!(goods(&s), i16::try_from(version).unwrap(), "the ability found the facts of THIS position");
        }
        // No provider: the table's own facts stand (none), and the ability sees no play facts.
        let mut plain = env.clone();
        plain.table.facts.plays_provider = None;
        assert!(table_at(&position(), &plain).facts.plays.is_none());
        let mut s = position();
        open(&mut s, &Toy { emitted: false });
        advance::<Toy>(&mut s, &plain, None).unwrap();
        advance::<Toy>(&mut s, &plain, Some(pick(1))).unwrap();
        advance::<Toy>(&mut s, &plain, Some(Pick::Opt(Opt::Decline))).unwrap();
        assert_eq!(goods(&s), 0, "without a provider a card finds no play facts");
    }

    #[test]
    fn a_window_that_asks_nothing_runs_to_its_end_and_refuses_an_answer() {
        let k = Kernel::plain();
        let mut env = env(&k);
        env.registry = Cow::new(Registry::default());
        let mut s = position();
        open(&mut s, &Toy { emitted: false });
        assert_eq!(advance::<Toy>(&mut s, &env, Some(pick(1))), Err(LiveError::NotAsking), "nothing is asked on a fresh window");
        let (live, out) = advance::<Toy>(&mut s, &env, None).unwrap();
        assert_eq!((live, out.emitted.len()), (Live::Done, 1));
        assert!(!is_open::<Toy>(&s));
    }
    // -- the remembered nested question (nestq-1) --------------------------------------------------------------------------------

    /// The operations this thread's drivers ran during `f`, without the check of the remembered questions (its own re-runs are not counted).
    fn runs<T>(f: impl FnOnce() -> T) -> (T, u32) {
        verify_remembered(false);
        let before = crate::driver::RUNS.with(std::cell::Cell::get);
        let out = f();
        let n = crate::driver::RUNS.with(std::cell::Cell::get) - before;
        verify_remembered(true);
        (out, n)
    }

    fn asked_open() -> (Kernel, CoreState) {
        let mut s = position();
        open(&mut s, &Toy { emitted: false });
        (Kernel::plain(), s)
    }

    /// An answer to a remembered nested question runs the operation once (the resume), where it ran it twice (the re-run that rebuilt the
    /// question, then the resume); a read of the question runs nothing. Without the memo, or in a position changed since, the re-run is back.
    #[test]
    fn a_remembered_nested_question_is_answered_with_one_run_and_read_with_none() {
        let (k, mut s) = asked_open();
        let env = env(&k);
        let (first, discover) = runs(|| advance::<Toy>(&mut s, &env, None).unwrap());
        assert_eq!(discover, 1, "the run that finds the question");
        let Live::Asks(first) = first.0 else { panic!("the reaction window asks") };
        assert!(recall::<Toy>(&s).is_some(), "the suspension remembers its question");
        let (asked, read) = runs(|| ask::<Toy>(&s, &env).unwrap().expect("asks"));
        assert_eq!((read, &asked.choice), (0, &first.choice), "the question is read, not replayed");
        // (What a replay reads, the display position included: the check of the remembered questions compares it in every other test.)
        verify::<Toy>(&s, &env, s.pending().nested.as_ref().unwrap(), &asked, true);
        // Another kind of window does not take the memo of this one.
        assert!(recall::<crate::driver::windows::tactical::TacticalAction>(&s).is_none());
        let mut plain = s.clone();
        let (_, answered) = runs(|| advance::<Toy>(&mut s, &env, Some(pick(1))).unwrap());
        assert_eq!(answered, 1, "the resume alone (it stops at the next question of the window)");
        // The same answer without the memo (a suspension written by an older build): the question is rebuilt first.
        plain.pending_mut().nested.as_mut().unwrap().asked = crate::state::Memo::default();
        assert!(recall::<Toy>(&plain).is_none());
        let (_, rebuilt) = runs(|| advance::<Toy>(&mut plain, &env, Some(pick(1))).unwrap());
        assert_eq!(rebuilt, 2, "the re-run that rebuilds the question, then the resume");
        assert_eq!(s, plain, "the same position either way");
        // A position changed after the call is not trusted: the question is rebuilt.
        let mut changed = s.clone();
        changed.meta_mut().rules_version += 1;
        assert!(recall::<Toy>(&changed).is_none(), "the memo belongs to the position it was made in");
        let (_, rebuilt) = runs(|| advance::<Toy>(&mut changed, &env, Some(Pick::Opt(Opt::Decline))).unwrap());
        assert_eq!(rebuilt, 2);
        let (live, _) = advance::<Toy>(&mut s, &env, Some(Pick::Opt(Opt::Decline))).unwrap();
        assert_eq!((live, goods(&s)), (Live::Done, 1));
        assert!(s.pending().nested.is_none(), "the suspension and its memo are gone with the window");
    }

    /// A pick the remembered question does not offer is refused as before, and the position (its memo included) is what it was.
    #[test]
    fn an_illegal_pick_to_a_remembered_question_is_refused_and_the_memo_stays_usable() {
        let (k, mut s) = asked_open();
        let env = env(&k);
        advance::<Toy>(&mut s, &env, None).unwrap();
        let held = s.clone();
        for wrong in [pick(9), Pick::Number(3), Pick::Opt(Opt::Done)] {
            assert_eq!(advance::<Toy>(&mut s, &env, Some(wrong)), Err(LiveError::Illegal), "{wrong:?}");
            assert_eq!(s, held);
            assert!(recall::<Toy>(&s).is_some(), "the rollback restores the memo with the position");
        }
        let ((live, _), n) = runs(|| advance::<Toy>(&mut s, &env, Some(pick(1))).unwrap());
        assert!(matches!(live, Live::Asks(_)) && n == 1, "a legal pick after the refusals resumes at once");
    }

    /// A clone of a suspended position shares the memo; each copy answers on its own: two different picks give the positions two straight runs
    /// with those picks give, and the clone answered gives what the original answered gives.
    #[test]
    fn copies_of_a_suspended_position_answer_independently() {
        let (k, mut s) = asked_open();
        let env = env(&k);
        advance::<Toy>(&mut s, &env, None).unwrap();
        let straight = |answers: &[Pick]| {
            let (_, mut t) = asked_open();
            advance::<Toy>(&mut t, &env, None).unwrap();
            for a in answers {
                t.pending_mut().nested.as_mut().unwrap().asked = crate::state::Memo::default();
                advance::<Toy>(&mut t, &env, Some(*a)).unwrap();
            }
            t
        };
        let mut fork = s.clone();
        let mut same = s.clone();
        advance::<Toy>(&mut s, &env, Some(pick(1))).unwrap();
        advance::<Toy>(&mut s, &env, Some(Pick::Opt(Opt::Decline))).unwrap();
        advance::<Toy>(&mut fork, &env, Some(Pick::Opt(Opt::Decline))).unwrap();
        assert_eq!((goods(&s), goods(&fork)), (1, 0), "the ability on one copy, the decline on the other");
        assert_eq!(s, straight(&[pick(1), Pick::Opt(Opt::Decline)]));
        assert_eq!(fork, straight(&[Pick::Opt(Opt::Decline)]));
        advance::<Toy>(&mut same, &env, Some(pick(1))).unwrap();
        advance::<Toy>(&mut same, &env, Some(Pick::Opt(Opt::Decline))).unwrap();
        assert_eq!(same, s, "ask, clone, answer is ask, answer");
    }

    /// The live resolver is built from the position, initiative order included: in the action phase a window is offered to the active seat first
    /// and then to the others in initiative order. A resolver that kept the order of the stored `Pending` (empty at the first window of a game)
    /// offered the window to the active seat alone, so a reaction of any other seat was never asked (found by the Letnev agent in lane M5).
    #[test]
    fn a_live_driver_offers_its_windows_in_the_initiative_order_of_the_position() {
        use crate::state::Phase;
        let k = Kernel::plain();
        let env = env(&k);
        let mut s = position();
        s.players = Players::from_vec((0..3u8).map(|i| Player::new(Seat(i), Faction::from_index(usize::from(i)).unwrap())).collect());
        s.meta_mut().phase = Phase::Action;
        s.meta_mut().active = Some(Seat(2));
        for (seat, initiative) in [(Seat(2), 1), (Seat(0), 2), (Seat(1), 3)] {
            s.strategy_mut().initiative_overrides.insert(seat, initiative);
        }
        assert!(s.pending().resolver.initiative.is_empty(), "nothing set the order before");
        let d = driver_of(Toy { emitted: false }, &s, &env);
        assert_eq!(d.resolver.player_order().as_slice(), &[Seat(2), Seat(0), Seat(1)]);
    }
}
