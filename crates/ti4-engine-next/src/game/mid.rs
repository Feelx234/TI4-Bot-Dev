//! Lane C5 (`docs/C5_RESUME.md`, FORK_RNG_THEORY V1-V5): the step of the native loop cut at its questions, so a [`Game`] can be copied
//! at EVERY decision, not only between two steps.
//!
//! [`Game::step`] puts each question of a step to the table and waits for the answer inside the call (a blocking decider). [`Game::pull`]
//! runs the same step up to the first question the table's decider would be asked and returns it; [`Game::answer`] applies the answer and
//! runs on to the next question or to the end of the step. Between the two calls the step is plain data in the game ([`Mid`]): what the
//! blocking step keeps in its stack frames is written down here, so [`Game::fork`] / [`Game::snapshot`] copy a game that stands at a
//! question, and each copy answers on its own.
//!
//! What a step keeps between its questions, and where the blocking step keeps it:
//!
//! | what | `native_step` | here |
//! |---|---|---|
//! | the step's `Env` | built once from the position the step began with | rebuilt per call from [`Begun::start`] (the same value; the render's cache makes it a copy) |
//! | where the step stands | the reconcile loop's counter and `left_open`, the arm, the run's loop | [`Stage`], the run's question [`Mid::asking`] |
//! | the run's environment | `prepared` (the combat texts of the run's start) | [`Mid::combat`] |
//! | the logs of the ability table (skipped seats, single-way payments) | two shared vectors | [`Mid::skipped`], [`Mid::auto_paid`] |
//! | the asker's `hidden`, the arm's question count | the asker, `StepOut::asked` | [`Mid::hidden`], [`Mid::arm_asked`] |
//! | the rollback of a failed step | `before`, the code table, the journal length | [`Begun`] |
//! | the events' mark (V3) | `from` | [`Begun::from`] |
//! | the staged-event drop at a table without a faction module (V2) | the end of `run_live` | the end of a run here, in the same place: a copy cut inside a run drops them when that run ends |
//! | the glue flag (V1) | was `Native::after_glue` | in the position (`DriverFlags::AFTER_GLUE`), for both |
//!
//! The answers go through the same table: [`Table::present`] counts the question, puts it in its position and tells the session (what
//! `Table::ask_seeing` does before it calls the decider), and [`Table::settle_pulled`] validates and journals the answer. The questions the
//! asker settles without the table (a lone draft card, a declined reaction of a planning fork) are settled here the same way, without a stop.
//!
//! Off in the product (feature `search`): the blocking step is unchanged. The equality of the two is the test
//! `crates/ti4-engine-core-bridge/tests/fork_mid.rs`.

use ti4_engine_core::containers::Interner;
use ti4_engine_core::driver::live::{ask_arm_fn, live_arm, AskFn, Asking, LiveFn};
use ti4_engine_core::driver::turn::{self as core_turn, TurnFlowWin};
use ti4_engine_core::kernel::ability::{AutoPaid, CombatTexts};
use ti4_engine_core::state::{CoreState, EventRec, Memo};

use super::{
    after_glue, choose_arm, commit, is_open, publish, step_env, Done, Game, GameError, Native, Observed, StepLogs, StepResult, TableAsker,
    RECONCILE_ROUNDS,
};
use crate::live::pull::{self, RunStop};
use crate::live::{self, arms, Question, Stop};
use crate::observed::Observed as Seen;
use crate::wire::choice::Choice;

/// What [`Game::pull`] and [`Game::answer`] came to.
#[derive(Debug, Clone, PartialEq)]
pub enum Pulled {
    /// The step stands at this question (as the table's decider would be shown it): answer it with [`Game::answer`].
    Asks(Box<Choice>),
    /// The step ended, or failed, as [`Game::step`] reports it (a finished game reports a quiet step).
    Stepped(StepResult),
}

/// Where a step of the native loop stands at a question the table has not been asked yet (see the module text).
#[derive(Clone)]
pub(super) struct Mid {
    begun: Begun,
    stage: Stage,
    combat: CombatTexts,
    arm_asked: usize,
    hidden: bool,
    skipped: Vec<(ti4_engine_core::Seat, EventRec, ti4_engine_core::choice::reaction::Relation)>,
    auto_paid: Vec<AutoPaid>,
    /// The run's question, as the core asked it.
    asking: Asking,
    /// The same, as the decider is shown it.
    shown: Choice,
}

impl Mid {
    /// A copy of the game whose journal holds `len` records (a copy without the log holds none of this step's).
    pub(super) fn clamp_log(&mut self, len: usize) {
        self.begun.log_len = self.begun.log_len.min(len);
    }

    fn differences(&self, other: &Self) -> Vec<&'static str> {
        let mut out = Vec::new();
        let b = (&self.begun, &other.begun);
        if b.0.start != b.1.start {
            out.push("begun.start");
        }
        if b.0.log_len != b.1.log_len {
            out.push("begun.log_len");
        }
        if b.0.from != b.1.from {
            out.push("begun.from");
        }
        if format!("{:?}", b.0.observed) != format!("{:?}", b.1.observed) {
            out.push("begun.observed");
        }
        if self.stage != other.stage {
            out.push("stage");
        }
        if format!("{:?}", self.combat) != format!("{:?}", other.combat) {
            out.push("combat");
        }
        if (self.arm_asked, self.hidden) != (other.arm_asked, other.hidden) {
            out.push("arm_asked/hidden");
        }
        if format!("{:?}", self.skipped) != format!("{:?}", other.skipped) {
            out.push("skipped");
        }
        if format!("{:?}", self.auto_paid) != format!("{:?}", other.auto_paid) {
            out.push("auto_paid");
        }
        if self.asking != other.asking {
            out.push("asking");
        }
        if self.shown != other.shown {
            out.push("shown");
        }
        out
    }
}

/// What a step keeps from where it began: the position (the step's `Env` is built from it; a failure restores it), the render's code table
/// and faction texts and the journal's length (restored with it), the ledger's event count (the events of the step are rendered from it), and
/// what the observation compares the step with.
#[derive(Clone)]
struct Begun {
    start: CoreState,
    codes: Interner,
    factions: Vec<String>,
    log_len: usize,
    from: usize,
    observed: Observed,
}

/// The run a step stands in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    /// Reconcile run `round` of the step (`left_open`: the turn flow had a job open when the run began).
    Reconcile { round: u8, left_open: bool },
    /// The arm's run.
    Arm(ti4_engine_core::driver::session::Arm),
}

impl Stage {
    fn label(self) -> &'static str {
        match self {
            Self::Reconcile { .. } => "reconcile",
            Self::Arm(arm) => arms::name(arm),
        }
    }

    fn fns(self) -> Result<(LiveFn, AskFn), Stop> {
        match self {
            Self::Reconcile { .. } => Ok((core_turn::advance_reconcile, core_turn::ask_job)),
            Self::Arm(arm) => match (live_arm(arm), ask_arm_fn(arm)) {
                (Some(advance), Some(ask)) => Ok((advance, ask)),
                _ => Err(Stop::NoLiveFn(arm)),
            },
        }
    }
}

/// Where [`run`] goes on from.
enum Next {
    /// Reconcile run `round` (or the arm, past the last round or after a glued step).
    Round(u8),
    Arm,
    /// The answer `id` to the run's question; `told`: the decider gave it (the observer of the strategic actions is told).
    Answer { stage: Stage, combat: Box<CombatTexts>, asking: Box<Asking>, id: String, told: Option<ti4_model::id::PlayerId> },
}

/// What [`run`] came to.
enum Ran {
    Asks(Box<Mid>),
    Done(Done),
}

/// The step's carried state between its questions.
#[derive(Default)]
struct Carried {
    arm_asked: usize,
    hidden: bool,
}

impl<'a> Game<'a> {
    /// The question the step stands at, as the decider is shown it (`None` between two steps).
    #[must_use]
    pub fn pending_question(&self) -> Option<&Choice> {
        self.native.as_ref()?.mid.as_ref().map(|m| &m.shown)
    }

    /// Run a step up to the first question the table's decider would be asked, and return it (the step stays open: answer it with
    /// [`Game::answer`]); or the whole step when it asks the decider nothing. A step that stands at a question returns that question again.
    /// The same step as [`Game::step`]: the same calls of the core in the same order, the same journal, the same events, rolls and mirror.
    pub fn pull(&mut self) -> Pulled {
        if let Some(q) = self.pending_question() {
            return Pulled::Asks(Box::new(q.clone()));
        }
        self.table.begin_step();
        if let Err(error) = self.try_load() {
            return Pulled::Stepped(self.result(false, Some(error)));
        }
        let observed = self.observed_before();
        let Some(n) = self.native.as_ref() else { return Pulled::Stepped(self.result(false, Some(GameError::Blocked("the core is not loaded".to_owned())))) };
        let begun = Begun { start: n.core.clone(), codes: n.render.codes_snapshot(), factions: n.render.factions.clone(), log_len: self.table.log.records.len(), from: n.core.ledger.events.len(), observed };
        self.drive(begun, Next::Round(0), Carried::default(), Vec::new(), Vec::new())
    }

    /// Answer the question the step stands at with the option `option_id`, and run on to the step's next question or its end.
    ///
    /// # Errors
    /// [`GameError::IllegalChoice`] when the answer is no option of the question, [`GameError::Blocked`] when no question is pending: the game
    /// is then unchanged. A failure of the step itself is reported as [`Game::step`] reports it (`Pulled::Stepped` with the error; the
    /// position is the one the step began with).
    pub fn answer(&mut self, option_id: &str) -> Result<Pulled, GameError> {
        let Some(mid) = self.native.as_mut().and_then(|n| n.mid.take()) else {
            return Err(GameError::Blocked("no question is pending".to_owned()));
        };
        let option = match self.table.settle_pulled(&mid.shown, option_id) {
            Ok(option) => option,
            Err(error) => {
                if let Some(n) = self.native.as_mut() {
                    n.mid = Some(mid);
                }
                return Err(GameError::IllegalChoice(error));
            }
        };
        let Mid { begun, stage, combat, arm_asked, hidden, skipped, auto_paid, asking, shown } = mid;
        let next = Next::Answer { stage, combat: Box::new(combat), asking: Box::new(asking), id: option.id, told: Some(shown.player) };
        Ok(self.drive(begun, next, Carried { arm_asked, hidden }, skipped, auto_paid))
    }

    /// Replay `answers` (option ids, in the order the journal holds them) on `game` (a game where the journal began: the opening, or a step
    /// boundary, with whatever table and observation the caller wired) through [`Game::pull`] / [`Game::answer`], up to the first question
    /// after the last answer (or the end of the game): the load of a game saved at a question. Every part of the position a
    /// question stands in (the window stack, the suspended nested question, the remembered question and the parked step of the core, the
    /// step's own state here) is rebuilt by the replay; none of it is in the codec (`docs/C5_RESUME.md`, V5).
    ///
    /// # Errors
    /// The first answer that is refused, or the first step that fails.
    pub fn replayed(mut game: Self, answers: &[String]) -> Result<Self, GameError> {
        let mut left = answers.iter();
        let mut pulled = game.pull();
        loop {
            match pulled {
                Pulled::Asks(_) => match left.next() {
                    Some(id) => pulled = game.answer(id)?,
                    None => return Ok(game),
                },
                Pulled::Stepped(result) => {
                    if let Some(error) = result.error {
                        return Err(error);
                    }
                    if result.finished {
                        return Ok(game);
                    }
                    pulled = game.pull();
                }
            }
        }
    }

    /// Forget what the core remembers of the question it stands at (the nestq memo and the parked step of a strategy card, both in
    /// `Nested::asked`): the answer then rebuilds them by a replay of the operation. A test of their being derived only.
    pub fn forget_memos(&mut self) {
        let Some(n) = self.native.as_mut() else { return };
        forget(&mut n.core);
        if let Some(mid) = n.mid.as_mut() {
            forget(&mut mid.begun.start);
        }
    }

    /// The parts of the two positions that differ, beyond what is public (the core, and the step that stands at a question).
    #[must_use]
    pub fn position_differences(&self, other: &Self) -> Vec<&'static str> {
        let mut out = Vec::new();
        match (&self.native, &other.native) {
            (None, None) => {}
            (Some(a), Some(b)) => {
                if a.core != b.core {
                    out.push("core");
                }
                match (&a.mid, &b.mid) {
                    (None, None) => {}
                    (Some(x), Some(y)) => out.extend(x.differences(y)),
                    _ => out.push("mid"),
                }
            }
            _ => out.push("native"),
        }
        out
    }

    /// The step from `next` on, with the game's error handling: a failure restores the position the step began with; a finished step is
    /// published as [`Game::step`] publishes it.
    fn drive(&mut self, begun: Begun, next: Next, carried: Carried, skipped: Vec<SkippedRow>, auto_paid: Vec<AutoPaid>) -> Pulled {
        let observed = begun.observed.clone();
        if self.native.is_none() {
            return Pulled::Stepped(self.result(false, Some(GameError::Blocked("the core is not loaded".to_owned()))));
        }
        let Self { state, table, events, content, sources, galaxy, native, rolls, blocked, aftermath_observer, strategic_observer, steps, secondary, participation, .. } = self;
        let n = native.as_mut().expect("loaded (checked above)");
        let rollback = (begun.start.clone(), begun.codes.clone(), begun.factions.clone(), begun.log_len);
        let ctx = Ctx { content, sources: *sources, galaxy: galaxy.as_ref(), strategic: strategic_observer.as_ref(), aftermath: aftermath_observer.as_ref(), participation: participation.as_ref() };
        let ran = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(n, table, events, &ctx, begun, next, carried, skipped, auto_paid)));
        let ran = ran.unwrap_or_else(|p| {
            let text = p.downcast_ref::<String>().cloned().or_else(|| p.downcast_ref::<&str>().map(|s| (*s).to_owned())).unwrap_or_default();
            Err(GameError::Core(format!("PANIC: {text}")))
        });
        let outcome = match ran {
            Ok(Ran::Asks(mid)) => {
                let q = mid.shown.clone();
                n.mid = Some(*mid);
                return Pulled::Asks(Box::new(q));
            }
            Ok(Ran::Done(done)) if done.finished_game => Ok(done),
            Ok(Ran::Done(done)) => publish(n, state, rolls, secondary, galaxy.as_mut(), steps).and_then(|()| blocked.clone().map_or(Ok(done), Err)),
            Err(error) => {
                let (start, codes, factions, log_len) = rollback;
                n.core = start;
                n.rebuild_render(codes, factions);
                table.log.records.truncate(log_len);
                Err(error)
            }
        };
        Pulled::Stepped(self.step_outcome(outcome, &observed))
    }
}

type SkippedRow = (ti4_engine_core::Seat, EventRec, ti4_engine_core::choice::reaction::Relation);

/// The game's parts a step reads besides the position and the table.
struct Ctx<'g, 'a> {
    content: &'a ti4_content::ContentStore,
    sources: ti4_model::content_types::SourceSet,
    galaxy: Option<&'g ti4_content::galaxy::Galaxy>,
    strategic: Option<&'g super::StrategicObserver>,
    aftermath: Option<&'g super::AftermathObserver>,
    participation: Option<&'g super::Participation>,
}

fn forget(core: &mut CoreState) {
    if core.pending().nested.is_some()
        && let Some(nested) = core.pending_mut().nested.as_mut()
    {
        nested.asked = Memo::default();
    }
}

/// `native_step` from `next` on, up to the next question the table's decider is asked or the end of the step (see the module text: the same
/// calls in the same order).
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn run(
    n: &mut Native,
    table: &mut crate::wire::choice::Table,
    events: &mut Vec<String>,
    g: &Ctx<'_, '_>,
    begun: Begun,
    mut next: Next,
    mut carried: Carried,
    skipped: Vec<SkippedRow>,
    auto_paid: Vec<AutoPaid>,
) -> Result<Ran, GameError> {
    let logs = StepLogs::new();
    *logs.skipped.lock().expect("skipped log") = skipped;
    *logs.auto_paid.lock().expect("auto-paid log") = auto_paid;
    let (env, logs) = step_env(n, &begun.start, g.sources, logs);
    let lcx = live::Cx::new(&n.render, &env);
    let mut asker = TableAsker::new(table, g.content, g.sources, g.galaxy, &n.cx, &n.base, g.strategic, g.aftermath).with_participation(g.participation);
    asker.hidden = carried.hidden;
    loop {
        let (stage, combat, stop) = match next {
            Next::Round(round) => {
                if round >= RECONCILE_ROUNDS || after_glue(&n.core) {
                    next = Next::Arm;
                    continue;
                }
                let stage = Stage::Reconcile { round, left_open: is_open::<TurnFlowWin>(&n.core) };
                let (advance, ask) = stage.fns().map_err(|s| asker.error_of(s, stage.label()))?;
                let (combat, stop) = pull::begin(&mut n.core, advance, ask, &lcx).map_err(|s| asker.error_of(s, stage.label()))?;
                (stage, combat, stop)
            }
            Next::Arm => {
                let Some(arm) = choose_arm(&n.core)? else {
                    return Ok(Ran::Done(Done { finished_game: true, asked: 0, hidden: false }));
                };
                let stage = Stage::Arm(arm);
                let (advance, ask) = stage.fns().map_err(|s| asker.error_of(s, stage.label()))?;
                let (combat, stop) = pull::begin(&mut n.core, advance, ask, &lcx).map_err(|s| asker.error_of(s, stage.label()))?;
                (stage, combat, stop)
            }
            Next::Answer { stage, combat, asking, id, told } => {
                if let Some(player) = &told {
                    asker.answered(&n.core, &id, player);
                }
                let (advance, _) = stage.fns().map_err(|s| asker.error_of(s, stage.label()))?;
                let stop = pull::answer(&mut n.core, advance, &lcx, &combat, &asking, &id).map_err(|s| asker.error_of(s, stage.label()))?;
                (stage, *combat, stop)
            }
        };
        match stop {
            RunStop::Asks(asking) => {
                let rendered = live::render_asking(&lcx, &n.core, &asking);
                if matches!(stage, Stage::Arm(_)) {
                    carried.arm_asked += 1;
                }
                let q = Question { asking: &asking, rendered, core: &n.core };
                if let Some(id) = asker.screen(&q).map_err(|s| asker.error_of(s, stage.label()))? {
                    next = Next::Answer { stage, combat: Box::new(combat), asking, id, told: None };
                    continue;
                }
                let seen_state = asker.shown(&q).map_err(|s| asker.error_of(s, stage.label()))?;
                let seen = Seen::new(&seen_state, g.content, g.sources, g.galaxy);
                let shown = asker.table().present(&q.rendered, &seen).map_err(|e| asker.error_of(Stop::Asker(e), stage.label()))?;
                drop(seen);
                let hidden = asker.hidden;
                let skipped = logs.skipped.lock().expect("skipped log").clone();
                let auto_paid = logs.auto_paid.lock().expect("auto-paid log").clone();
                return Ok(Ran::Asks(Box::new(Mid { begun, stage, combat, arm_asked: carried.arm_asked, hidden, skipped, auto_paid, asking: *asking, shown })));
            }
            RunStop::Ended { finished } => {
                // The end of `run_live`: the staged events of a table without a faction module are dropped with the run (V2: a copy cut inside
                // the run drops them here too, never at the start of another step).
                if !env.table.facts.staging {
                    n.core.pending_mut().staged.clear();
                }
                live::coverage::note(stage.label());
                match stage {
                    Stage::Reconcile { round, left_open } => next = if finished && !left_open { Next::Arm } else { Next::Round(round + 1) },
                    Stage::Arm(arm) => {
                        asker.arm_done(arm, &live::StepOut { finished, asked: Vec::new(), out: ti4_engine_core::driver::live::Out::default(), events_from: begun.from }, &n.core);
                        let hidden = asker.hidden;
                        drop(asker);
                        drop(lcx);
                        commit(n, table, events, &env, &logs, arm, finished, carried.arm_asked, begun.from)?;
                        return Ok(Ran::Done(Done { finished_game: false, asked: carried.arm_asked, hidden }));
                    }
                }
            }
        }
    }
}
