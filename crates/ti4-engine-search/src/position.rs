//! The forkable position and its step machine.
//!
//! The step is `ti4_engine::native::native_step` (the product), cut at every question like `live::pull` (lane C5), on the core alone:
//!
//! | `native_step` | here |
//! |---|---|
//! | `env_with_plays` at the start of the step | [`Cursor::step_env`], built when the step begins (`Rc`: shared by the forks, never changed) |
//! | up to 8 reconcile runs (`live_reconcile`), unless the glue flag is set; stop after a run that finished without a job left open | [`Phase::Reconcile`] |
//! | the arm (`step_arm`, `live_step`) | [`Phase::Arm`] |
//! | a run (`live_step_inner`): combat texts minted, `ask`, else `advance(None)`; answers; a question asked from inside the answer belongs to the run | [`Position::begin_run`], [`Position::resume`], [`Cursor::run_env`], [`Cursor::fresh`] |
//! | `run_live`: the staged events dropped at a table without a faction module | [`Position::after_run`] |
//! | the lone draft card answered without the decider (`LoneCard` / the table's `auto_resolve`), counted as asked | [`Position::stop_at`] |
//! | the hand-overs (`after_arm`), the code table, the glue flag of the next step | [`Position::commit`] |
//!
//! The wrapper's presentation (the string mirror, events, rolls, journal, observers, the auto-paid and skipped-seat notes) is not made:
//! none of it feeds the game.

use std::rc::Rc;

use ti4_chance::{Faces, Request};
use ti4_engine::live::{self, Cx};
use ti4_engine::render::Render;
use ti4_engine::wire::choice::Choice as WireChoice;
use ti4_engine_chance::{Mode, chance_asked};
use ti4_engine_core::choice::{Ask, Choice, Pick, strategy};
use ti4_engine_core::driver::live::{AskFn, Asking, Env, Live, LiveError, LiveFn, Out, ask_arm_fn, is_open, live_arm};
use ti4_engine_core::driver::session::{Arm, AskArm, Facts, live_ask_arm, step_arm};
use ti4_engine_core::driver::turn::{self, TurnFlowWin};
use ti4_engine_core::seen::Seen;
use ti4_engine_core::state::{CoreState, DriverFlags, WindowKind};

use crate::agent::{Answer, Decision};
use crate::table::Table;
use crate::{BotMemory, Outcome};

/// The most reconcile runs a step makes before its arm (`native_step`'s loop).
const RECONCILE_ROUNDS: u8 = 8;

/// What the game waits for.
#[derive(Debug, Clone, Copy)]
pub enum Node<'p> {
    /// A question of `choice.actor`.
    Decide(&'p Choice),
    /// A chance request (only in [`Mode::Search`]): answer it with [`Position::answer_chance`].
    Chance(Request),
    /// The game is over.
    End,
    /// The position failed (a core error or a panic); it cannot go on.
    Failed(&'p str),
}

/// Why an answer was not applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchError {
    /// The answer is no option of the question; the position is unchanged.
    Illegal,
    /// The position does not wait for this kind of answer (a pick at a chance request, faces at a question, anything at the end).
    NotAsking,
    /// The core failed while running on (the position is now [`Node::Failed`]).
    Failed(String),
}

impl std::fmt::Display for SearchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Illegal => f.write_str("the answer is no option of the question"),
            Self::NotAsking => f.write_str("the position does not wait for this answer"),
            Self::Failed(why) => write!(f, "the game failed: {why}"),
        }
    }
}

impl std::error::Error for SearchError {}

/// Where the step is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// Between two steps.
    Boundary,
    /// Reconcile run `round` of the step; `left_open`: a turn job was open when it began.
    Reconcile { round: u8, left_open: bool },
    /// The run of the step's arm.
    Arm(Arm),
}

/// The step's own data between two calls (all of it immutable or `Copy`: a fork shares the `Env`s).
#[derive(Clone)]
struct Cursor {
    phase: Phase,
    /// The step's `Env` (built from the position the step began in).
    step_env: Option<Rc<Env<'static>>>,
    /// The run's `Env`: the step's with the combat texts of the position the run began in.
    run_env: Option<Rc<Env<'static>>>,
    /// The run's current segment began without an answer (the run's first call, and the chance answers after it): its question is the
    /// run's whatever its nesting. After an answer, only a question from inside the answer belongs to the run.
    fresh: bool,
    /// The questions the arm's run put (`StepOut::asked` of the arm; the glue flag reads it).
    arm_asked: usize,
}

/// The stop the position stands at.
#[derive(Clone)]
enum Stop {
    Decide(Rc<Asking>),
    Chance(Rc<Asking>, Request),
    End,
    Failed(Rc<str>),
}

/// A game cut at its next stop. See the crate doc.
pub struct Position {
    table: Rc<Table>,
    core: CoreState,
    render: Render,
    mode: Mode,
    cursor: Cursor,
    stop: Stop,
    steps: usize,
    decisions: usize,
    bot_memory: Rc<BotMemory>,
    /// The chance requests the last call answered in [`Mode::Play`] (from the streams), with their faces.
    rolled: Vec<(Request, Faces)>,
}

fn glued(arm: Arm) -> bool {
    matches!(arm, Arm::TurnPrelude | Arm::StrategyAnnounce)
}

fn after_glue(core: &CoreState) -> bool {
    core.pending().driver.has(DriverFlags::AFTER_GLUE)
}

fn set_after_glue(core: &mut CoreState, on: bool) {
    if after_glue(core) != on {
        core.pending_mut().driver.set(DriverFlags::AFTER_GLUE, on);
    }
}

/// `Game::legal_options().is_some()` on a bare core (`native_step`'s `asks_of`).
fn asks_of(core: &CoreState) -> bool {
    if is_open::<TurnFlowWin>(core) {
        return true;
    }
    let facts = Facts::of_core(core, false);
    match live_ask_arm(&facts) {
        AskArm::None | AskArm::Nothing => false,
        AskArm::Draft => turn::draft::question(core).is_some(),
        AskArm::StartingTech | AskArm::LeaderFollowers | AskArm::EventScoring | AskArm::Scoring | AskArm::Tokens | AskArm::Voting | AskArm::Diplomacy | AskArm::Aftermath | AskArm::AgendaTalks | AskArm::Tactical => true,
        AskArm::Secondary | AskArm::Closing | AskArm::Trade | AskArm::ActionMenu => core.meta().active.is_some() && !ti4_engine_core::driver::turn_flow::all_passed(core),
    }
}

/// The position a question is asked in (a nested question's own, else the one a question from inside a step carries, else the live core).
fn asked_in<'a>(asking: &'a Asking, core: &'a CoreState) -> &'a CoreState {
    asking.nested.as_ref().map(|(_, at)| &**at).or(asking.at.as_deref()).unwrap_or(core)
}

fn panic_text(p: &(dyn std::any::Any + Send)) -> String {
    p.downcast_ref::<String>().cloned().or_else(|| p.downcast_ref::<&str>().map(|s| (*s).to_owned())).unwrap_or_default()
}

/// What a run did after a call.
enum Flow {
    /// The position stands at a stop.
    Stopped,
    /// The run is over; `finished`: its window ended.
    RunEnded { finished: bool },
}

impl Position {
    /// The game at `core` (a step boundary), run on to its first stop, in [`Mode::Play`].
    pub(crate) fn open(table: Rc<Table>, core: CoreState, render: Render) -> Self {
        let mut pos = Self {
            table,
            core,
            render,
            mode: Mode::Play,
            cursor: Cursor { phase: Phase::Boundary, step_env: None, run_env: None, fresh: true, arm_asked: 0 },
            stop: Stop::End,
            steps: 0,
            decisions: 0,
            bot_memory: Rc::new(BotMemory::default()),
            rolled: Vec::new(),
        };
        pos.guarded(|p| {
            let flow = p.begin_step()?;
            p.run_on(flow)
        });
        pos
    }

    // ---------------------------------------------------------------------------------------------------------------------------------
    // Reading
    // ---------------------------------------------------------------------------------------------------------------------------------

    /// What the game waits for.
    #[must_use]
    pub fn node(&self) -> Node<'_> {
        match &self.stop {
            Stop::Decide(a) => Node::Decide(&a.choice),
            Stop::Chance(_, req) => Node::Chance(*req),
            Stop::End => Node::End,
            Stop::Failed(why) => Node::Failed(why),
        }
    }

    /// The question waiting for an answer, if any.
    #[must_use]
    pub fn choice(&self) -> Option<&Choice> {
        match &self.stop {
            Stop::Decide(a) => Some(&a.choice),
            _ => None,
        }
    }

    /// The legal answers of the question, one per option in option order (`Choice::pick`: for a multi-pick the set that pays with that atom
    /// first; option `i` of the wire form is answer `i`). Empty when nothing is asked.
    #[must_use]
    pub fn legal(&self) -> Vec<Pick> {
        self.choice().map_or_else(Vec::new, |c| (0..c.opts.len()).filter_map(|i| c.pick(i)).collect())
    }

    /// Up to `limit` legal answers, a multi-pick's whole sets included (`Choice::answers`).
    #[must_use]
    pub fn answers(&self, limit: usize) -> Vec<Pick> {
        self.choice().map_or_else(Vec::new, |c| c.answers(limit))
    }

    /// The question as the product's table shows it (the rule bot's input; option `i` is `choice().opts[i]`). Rendering mints texts into
    /// the code table, as the product does for every question it puts: a caller that renders some questions and not others plays the same
    /// game with other code numbers.
    #[must_use]
    pub fn rendered(&self) -> Option<WireChoice> {
        let Stop::Decide(asking) = &self.stop else { return None };
        let env = self.cursor.step_env.as_ref()?;
        let cx = Cx::new(&self.render, env);
        Some(live::render_asking(&cx, &self.core, asking))
    }

    /// What the actor of the question may see, in the position the question is asked in (for a nested question: the position the core
    /// rolled back to). `None` when nothing is asked.
    #[must_use]
    pub fn seen(&self) -> Option<Seen<'_>> {
        let Stop::Decide(asking) = &self.stop else { return None };
        Some(Seen::new(asked_in(asking, &self.core), self.table.kernel, asking.choice.actor))
    }

    /// What `seat` may see in the live position.
    #[must_use]
    pub fn seen_for(&self, seat: ti4_engine_core::Seat) -> Seen<'_> {
        Seen::new(&self.core, self.table.kernel, seat)
    }

    /// The game's tables.
    #[must_use]
    pub fn table(&self) -> &Table {
        &self.table
    }

    /// The table, shared.
    #[must_use]
    pub fn table_rc(&self) -> Rc<Table> {
        Rc::clone(&self.table)
    }

    #[must_use]
    pub const fn mode(&self) -> Mode {
        self.mode
    }

    /// Steps committed (the product's `Game::steps`).
    #[must_use]
    pub const fn steps(&self) -> usize {
        self.steps
    }

    /// Questions answered (a chance answer and the lone draft card are not counted): the rule bot's decision index of the next question.
    #[must_use]
    pub const fn decisions(&self) -> usize {
        self.decisions
    }

    #[must_use]
    pub fn round(&self) -> u32 {
        self.core.meta().round
    }

    /// Victory points in seat order.
    #[must_use]
    pub fn vp(&self) -> Vec<i16> {
        self.core.players().iter().map(|p| p.econ.victory_points).collect()
    }

    /// Whether the game is over (or failed).
    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        matches!(self.stop, Stop::End | Stop::Failed(_))
    }

    /// The points, and the winner once the game is over.
    #[must_use]
    pub fn outcome(&self) -> Outcome {
        let vp = self.vp();
        let finished = self.core.meta().finished;
        let winner = finished
            .then(|| {
                let best = vp.iter().copied().max()?;
                let order = ti4_engine_core::query::seats::initiative_order(&self.core);
                order.iter().map(|s| usize::from(s.0)).find(|&i| vp.get(i) == Some(&best)).or_else(|| vp.iter().position(|&v| v == best))
            })
            .flatten();
        Outcome { finished, round: self.round(), vp, winner }
    }

    /// The rule bot's movement memory as this position's line of play left it.
    #[must_use]
    pub fn bot_memory(&self) -> &BotMemory {
        &self.bot_memory
    }

    /// The chance requests the last answer resolved from the streams ([`Mode::Play`] only), with their faces.
    #[must_use]
    pub fn rolled(&self) -> &[(Request, Faces)] {
        &self.rolled
    }

    /// How far each random stream was drawn (positions, never keys: nothing of the future can be read from it). For checks that a search
    /// leaves the game's streams alone.
    #[must_use]
    pub fn stream_positions(&self) -> Vec<(ti4_chance::Domain, u128)> {
        self.core.rng.positions().collect()
    }

    /// A hash of the position (the core's `Debug` text, the streams' positions included, and the step's place): equal positions hash equal.
    /// O(game) (the ledgers are printed): for tests and replay checks, not for a transposition table.
    #[must_use]
    pub fn state_hash(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        format!("{:?}", self.core).hash(&mut h);
        format!("{:?}", self.cursor.phase).hash(&mut h);
        (self.cursor.fresh, self.cursor.arm_asked, self.steps).hash(&mut h);
        if let Stop::Decide(a) | Stop::Chance(a, _) = &self.stop {
            format!("{:?}", a.choice).hash(&mut h);
        }
        h.finish()
    }

    /// A hash of the core's `Debug` text alone (the MCTS lab's position compares with it).
    #[must_use]
    pub fn core_hash(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        format!("{:?}", self.core).hash(&mut h);
        h.finish()
    }

    /// The core (this crate's tests: the streams are read there, never by the API).
    #[cfg(test)]
    pub(crate) const fn core_for_tests(&self) -> &CoreState {
        &self.core
    }

    /// The position as the string `GameState` JSON (what the product's fingerprint hashes: `NativeGame::fingerprint`).
    ///
    /// # Errors
    /// A position the materialiser cannot show.
    pub fn materialized_json(&self) -> Result<String, String> {
        let state = ti4_engine::core_sync::materialize::materialize(&self.core, &self.table.cx, &self.table.base).map_err(|e| e.to_string())?;
        serde_json::to_value(&state).map(|v| v.to_string()).map_err(|e| e.to_string())
    }

    /// Whether `other` is the same position (the core, the step's place, the stop, the counters). The core is compared with `==`, which
    /// reads the code numbers: two lines of play that rendered different questions are equal games with unequal positions.
    #[must_use]
    pub fn same_as(&self, other: &Self) -> bool {
        self.differences(other).is_empty()
    }

    /// The parts in which `other` differs (empty: the same position).
    #[must_use]
    pub fn differences(&self, other: &Self) -> Vec<&'static str> {
        let mut out = Vec::new();
        if self.core != other.core {
            out.push("core");
        }
        if self.cursor.phase != other.cursor.phase || self.cursor.fresh != other.cursor.fresh || self.cursor.arm_asked != other.cursor.arm_asked {
            out.push("step");
        }
        let stop = |s: &Stop| match s {
            Stop::Decide(a) => format!("decide {:?} {:?}", a.choice, a.nested.as_ref().map(|(f, at)| (format!("{f:?}"), at.ledger.events.len()))),
            Stop::Chance(a, r) => format!("chance {r:?} {:?}", a.choice),
            Stop::End => "end".to_owned(),
            Stop::Failed(w) => format!("failed {w}"),
        };
        if stop(&self.stop) != stop(&other.stop) {
            out.push("stop");
        }
        if (self.steps, self.decisions) != (other.steps, other.decisions) {
            out.push("counters");
        }
        if *self.bot_memory != *other.bot_memory {
            out.push("bot memory");
        }
        if self.mode != other.mode {
            out.push("mode");
        }
        out
    }

    // ---------------------------------------------------------------------------------------------------------------------------------
    // Forking
    // ---------------------------------------------------------------------------------------------------------------------------------

    /// An independent copy in the same mode: the core cloned (shared slices, `Cow`), the renderer copied warm, the step's data and the
    /// question shared (`Rc`, immutable).
    #[must_use]
    pub fn fork(&self) -> Self {
        Self {
            table: Rc::clone(&self.table),
            core: self.core.clone(),
            render: self.render.fork_warm(),
            mode: self.mode,
            cursor: self.cursor.clone(),
            stop: self.stop.clone(),
            steps: self.steps,
            decisions: self.decisions,
            bot_memory: Rc::clone(&self.bot_memory),
            rolled: Vec::new(),
        }
    }

    /// The parts of a fork one by one, cloned and dropped (the bench's bytes per part): 0 the core, 1 the renderer's warm copy, 2 a cold
    /// renderer (`Render::new` + the code table), 3 the step's data and the question.
    #[doc(hidden)]
    pub fn clone_part_for_bench(&self, part: u8) {
        match part {
            0 => drop(self.core.clone()),
            1 => drop(self.render.fork_warm()),
            2 => drop(Render::new(self.table.seats.clone()).with_factions(self.table.factions.clone()).with_codes(self.render.codes_snapshot())),
            _ => drop((self.cursor.clone(), self.stop.clone())),
        }
    }

    /// [`Self::fork`] in [`Mode::Search`]: from here on every chance request stops the run and is answered by the caller; the streams are
    /// never read or moved.
    #[must_use]
    pub fn fork_search(&self) -> Self {
        let mut f = self.fork();
        f.mode = Mode::Search;
        f
    }

    /// Lane C7: drop what no rule reads of the ledgers (a search position only). Kept: the last event (`turn::inline::last_plain_is`
    /// reads it) and the dice rows (the barrage and Space Cannon windows read the rows after a mark they took, the mark is an absolute
    /// count). Dropped: the older events and the victory-point and relic rows (output only). The game goes on identically; the
    /// materialised position and the hash do not (they print the ledgers).
    pub fn trim_ledger(&mut self) {
        let last = self.core.ledger.events.last().cloned();
        self.core.ledger.events = last.into_iter().collect();
        self.core.ledger.vp = ti4_engine_core::containers::Log::new();
        self.core.ledger.relics = ti4_engine_core::containers::Log::new();
    }

    /// The bytes the ledgers of this position hold (the records' inline size; shared chunks counted in full).
    #[must_use]
    pub fn ledger_bytes(&self) -> [usize; 4] {
        let l = &self.core.ledger;
        [
            l.events.len() * std::mem::size_of::<ti4_engine_core::state::Emit>(),
            l.dice.len() * std::mem::size_of::<ti4_engine_core::state::RollRec>(),
            l.vp.len() * std::mem::size_of::<ti4_engine_core::state::VpRec>(),
            l.relics.len() * std::mem::size_of::<ti4_engine_core::state::RelicRec>(),
        ]
    }

    /// The records of each ledger (events, dice, victory points, relics).
    #[must_use]
    pub fn ledger_lens(&self) -> [usize; 4] {
        let l = &self.core.ledger;
        [l.events.len(), l.dice.len(), l.vp.len(), l.relics.len()]
    }

    // ---------------------------------------------------------------------------------------------------------------------------------
    // Answering
    // ---------------------------------------------------------------------------------------------------------------------------------

    /// Answer the question with a core pick and run on to the next stop.
    ///
    /// # Errors
    /// [`SearchError::Illegal`] (the position is unchanged), [`SearchError::NotAsking`], or [`SearchError::Failed`].
    pub fn answer(&mut self, pick: Pick) -> Result<(), SearchError> {
        if !matches!(self.stop, Stop::Decide(_)) || matches!(pick, Pick::Chance(_)) {
            return Err(SearchError::NotAsking);
        }
        self.decided(pick)
    }

    /// Answer with the legal answer `i` ([`Self::legal`]).
    ///
    /// # Errors
    /// As [`Self::answer`].
    pub fn answer_index(&mut self, i: usize) -> Result<(), SearchError> {
        let pick = self.choice().ok_or(SearchError::NotAsking)?.pick(i).ok_or(SearchError::Illegal)?;
        self.answer(pick)
    }

    /// Answer with a wire option id, as the product's live loop reads it (`live::trade::pick` for a free-form deal, else `live::pick_of`).
    ///
    /// # Errors
    /// As [`Self::answer`].
    pub fn answer_id(&mut self, id: &str) -> Result<(), SearchError> {
        let Stop::Decide(asking) = &self.stop else { return Err(SearchError::NotAsking) };
        let asking = Rc::clone(asking);
        // (A deal writes its terms into the window before the pick names them: kept aside for an answer that is no option.)
        let before = matches!(asking.choice.ask, Ask::Trade(_)).then(|| self.core.clone());
        let pick = match live::trade::pick(&mut self.core, &asking.choice, id) {
            Some(Ok(pick)) => Some(pick),
            Some(Err(why)) => {
                self.fail(format!("the deal {id} cannot be read: {why:?}"));
                return Err(SearchError::Failed(format!("{why:?}")));
            }
            None => live::pick_of(&self.render, &asking.choice, id),
        };
        let Some(pick) = pick else {
            if let Some(before) = before {
                self.core = before;
            }
            return Err(SearchError::Illegal);
        };
        match self.decided(pick) {
            Err(SearchError::Illegal) => {
                if let Some(before) = before {
                    self.core = before;
                }
                Err(SearchError::Illegal)
            }
            other => other,
        }
    }

    /// The core pick an answer names at the question (without answering it): a pick as it is, a legal index's pick, a wire id read as
    /// [`Self::answer_id`] reads it. `None` for a free-form deal (its id writes terms into the window) or an id that is no option. For an
    /// agent that follows the game's answers through a tree (lane L6's tree reuse).
    #[must_use]
    pub fn resolve(&self, answer: &Answer) -> Option<Pick> {
        let Stop::Decide(asking) = &self.stop else { return None };
        match answer {
            Answer::Pick(p) => Some(*p),
            Answer::Index(i) => asking.choice.pick(*i),
            Answer::Id(_) if matches!(asking.choice.ask, Ask::Trade(_)) => None,
            Answer::Id(id) => live::pick_of(&self.render, &asking.choice, id),
        }
    }

    /// Apply an agent's decision: the answer, then the bot memory it left.
    ///
    /// # Errors
    /// As [`Self::answer`].
    pub fn apply(&mut self, decision: Decision) -> Result<(), SearchError> {
        let Decision { answer, memory } = decision;
        let result = match answer {
            Answer::Pick(pick) => self.answer(pick),
            Answer::Index(i) => self.answer_index(i),
            Answer::Id(id) => self.answer_id(&id),
        };
        if result.is_ok()
            && let Some(memory) = memory
        {
            self.bot_memory = Rc::new(memory);
        }
        result
    }

    /// Set the rule bot's movement memory of this line of play.
    pub fn set_bot_memory(&mut self, memory: BotMemory) {
        self.bot_memory = Rc::new(memory);
    }

    /// Answer the chance request with `faces` ([`Mode::Search`]) and run on.
    ///
    /// # Errors
    /// [`SearchError::Illegal`] for faces the request does not accept (the position is unchanged), [`SearchError::NotAsking`],
    /// [`SearchError::Failed`].
    pub fn answer_chance(&mut self, faces: Faces) -> Result<(), SearchError> {
        let Stop::Chance(_, req) = &self.stop else { return Err(SearchError::NotAsking) };
        if !req.accepts(&faces) {
            return Err(SearchError::Illegal);
        }
        self.resume(Pick::Chance(faces), true)
    }

    fn decided(&mut self, pick: Pick) -> Result<(), SearchError> {
        let result = self.resume(pick, false);
        if result.is_ok() {
            self.decisions += 1;
        }
        result
    }

    // ---------------------------------------------------------------------------------------------------------------------------------
    // The step machine
    // ---------------------------------------------------------------------------------------------------------------------------------

    fn fail(&mut self, why: String) {
        self.stop = Stop::Failed(why.into());
    }

    /// Run `f` (a part of the step machine that ends at a stop), a panic or an error failing the position.
    fn guarded(&mut self, f: impl FnOnce(&mut Self) -> Result<(), String>) {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(self))) {
            Ok(Ok(())) => {}
            Ok(Err(why)) => self.fail(why),
            Err(p) => self.fail(format!("PANIC: {}", panic_text(&*p))),
        }
    }

    /// The live functions of the run the step is in.
    fn fns(&self) -> Result<(LiveFn, AskFn), String> {
        match self.cursor.phase {
            Phase::Reconcile { .. } => Ok((turn::advance_reconcile, turn::ask_job)),
            Phase::Arm(arm) => match (live_arm(arm), ask_arm_fn(arm)) {
                (Some(a), Some(q)) => Ok((a, q)),
                _ => Err(format!("the arm {} has no live function", live::arms::name(arm))),
            },
            Phase::Boundary => Err("no run at a step boundary".to_owned()),
        }
    }

    /// One call of the run's live function with `answer`, in the position's mode.
    fn call(&mut self, f: LiveFn, answer: Option<Pick>) -> Result<(Live, Out), LiveError> {
        let env = Rc::clone(self.cursor.run_env.as_ref().expect("a run is open"));
        let result = match self.mode {
            Mode::Play => {
                let rolled = &mut self.rolled;
                ti4_engine_chance::advance_observed(&mut self.core, &env, f, answer, &mut |r, faces| rolled.push((*r, *faces)))
            }
            Mode::Search => f(&mut self.core, &env, answer),
        };
        self.render.sync_codes(&mut self.core.texts.codes);
        result
    }

    /// Stop at `asking` (a question or, in Search mode, a chance request). The lone draft card is taken at once (the product's table
    /// answers it without its decider and counts it as asked).
    fn stop_at(&mut self, asking: Asking) -> Result<Flow, String> {
        if let Some(req) = chance_asked(&asking) {
            self.stop = Stop::Chance(Rc::new(asking), req);
            return Ok(Flow::Stopped);
        }
        if matches!(self.cursor.phase, Phase::Arm(_)) {
            self.cursor.arm_asked += 1;
        }
        if asking.choice.ask == Ask::Strategy(strategy::Ask::Draft) && asking.choice.opts.len() == 1 {
            // (Rendered as the product renders every question it puts: the render may mint texts into the code table.)
            if let Some(env) = self.cursor.step_env.as_ref() {
                let _ = live::render_asking(&Cx::new(&self.render, env), &self.core, &asking);
            }
            let pick = asking.choice.pick(0).ok_or("the lone draft card has no pick")?;
            let (f, _) = self.fns()?;
            self.cursor.fresh = false;
            let r = self.call(f, Some(pick)).map_err(|e| format!("the lone draft card: {e:?}"))?;
            return self.handle(r);
        }
        self.stop = Stop::Decide(Rc::new(asking));
        Ok(Flow::Stopped)
    }

    /// What a call's result means for the run.
    fn handle(&mut self, (live, out): (Live, Out)) -> Result<Flow, String> {
        match live {
            Live::Done => Ok(Flow::RunEnded { finished: !out.held }),
            Live::Asks(a) => {
                let belongs = self.cursor.fresh || a.nested.is_some() || self.core.pending().nested.is_some() || chance_asked(&a).is_some();
                if belongs { self.stop_at(*a) } else { Ok(Flow::RunEnded { finished: false }) }
            }
        }
    }

    /// Follow `flow` to the next stop: a run that ended hands on to the next run, the arm, the commit and the next step.
    fn run_on(&mut self, mut flow: Flow) -> Result<(), String> {
        loop {
            match flow {
                Flow::Stopped => return Ok(()),
                Flow::RunEnded { finished } => flow = self.after_run(finished)?,
            }
        }
    }

    /// Resume the run at its stop with `pick`; `chance`: a chance answer (the run's segment keeps its kind).
    fn resume(&mut self, pick: Pick, chance: bool) -> Result<(), SearchError> {
        let (f, _) = self.fns().map_err(SearchError::Failed)?;
        let (fresh, rolled) = (self.cursor.fresh, std::mem::take(&mut self.rolled));
        if !chance {
            self.cursor.fresh = false;
        }
        let called = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.call(f, Some(pick))));
        let r = match called {
            Ok(Ok(r)) => r,
            Ok(Err(LiveError::Illegal)) => {
                // The core left the position as it was.
                self.cursor.fresh = fresh;
                self.rolled = rolled;
                return Err(SearchError::Illegal);
            }
            Ok(Err(e)) => {
                let why = format!("{e:?}");
                self.fail(why.clone());
                return Err(SearchError::Failed(why));
            }
            Err(p) => {
                let why = format!("PANIC: {}", panic_text(&*p));
                self.fail(why.clone());
                return Err(SearchError::Failed(why));
            }
        };
        self.guarded(|pos| {
            let flow = pos.handle(r)?;
            pos.run_on(flow)
        });
        match &self.stop {
            Stop::Failed(why) => Err(SearchError::Failed(why.to_string())),
            _ => Ok(()),
        }
    }

    /// Begin a step at a boundary: the game over, or the step's `Env` and its first reconcile run.
    fn begin_step(&mut self) -> Result<Flow, String> {
        self.cursor = Cursor { phase: Phase::Boundary, step_env: None, run_env: None, fresh: true, arm_asked: 0 };
        if self.core.meta().finished {
            self.stop = Stop::End;
            return Ok(Flow::Stopped);
        }
        let env = live::env::env_with_plays(&self.render, &self.core, self.table.kernel, self.table.sources);
        self.cursor.step_env = Some(Rc::new(env));
        self.begin_reconcile(0)
    }

    /// Reconcile run `round`, or the arm when the reconcile is over (the glue flag set, or the rounds used up).
    fn begin_reconcile(&mut self, round: u8) -> Result<Flow, String> {
        if round >= RECONCILE_ROUNDS || after_glue(&self.core) {
            return self.begin_arm();
        }
        self.cursor.phase = Phase::Reconcile { round, left_open: is_open::<TurnFlowWin>(&self.core) };
        self.begin_run()
    }

    /// The step's arm (or the end of the game).
    fn begin_arm(&mut self) -> Result<Flow, String> {
        let facts = Facts::of_core(&self.core, false);
        let arm = step_arm(&facts, asks_of(&self.core));
        if arm == Arm::Finished {
            self.stop = Stop::End;
            return Ok(Flow::Stopped);
        }
        self.cursor.phase = Phase::Arm(arm);
        self.cursor.arm_asked = 0;
        self.begin_run()
    }

    /// The first call of a run (`live_step_inner`): the combat texts of the position minted, the code table synced, the pending question
    /// (or, in Search mode, a pending chance request), else the window run to its first stop.
    fn begin_run(&mut self) -> Result<Flow, String> {
        let (advance, ask) = self.fns()?;
        let step_env = self.cursor.step_env.as_ref().ok_or("a run outside a step")?;
        let mut prepared = (**step_env).clone();
        prepared.table.facts.combat = ti4_engine::ties::combat_live::combat_texts(&mut self.core, &self.render);
        self.cursor.run_env = Some(Rc::new(prepared));
        self.cursor.fresh = true;
        self.render.sync_codes(&mut self.core.texts.codes);
        let env = Rc::clone(self.cursor.run_env.as_ref().expect("set above"));
        match ask(&self.core, &env).map_err(|e| format!("ask: {e:?}"))? {
            Some(a) if chance_asked(&a).is_none() => self.stop_at(a),
            Some(a) if self.mode == Mode::Search => self.stop_at(a),
            // (A chance request left pending by a search, in play: the resolver answers it like any other.)
            _ => {
                let r = self.call(advance, None).map_err(|e| format!("{e:?}"))?;
                self.handle(r)
            }
        }
    }

    /// A run ended (`run_live` after a run that went well: the staged events dropped at a table that announces none), then the next run,
    /// the arm, or the commit and the next step.
    fn after_run(&mut self, finished: bool) -> Result<Flow, String> {
        if !self.cursor.run_env.as_ref().is_some_and(|e| e.table.facts.staging) {
            self.core.pending_mut().staged.clear();
        }
        self.cursor.run_env = None;
        match self.cursor.phase {
            Phase::Reconcile { round, left_open } => {
                if finished && !left_open {
                    self.begin_arm()
                } else {
                    self.begin_reconcile(round + 1)
                }
            }
            Phase::Arm(arm) => {
                self.commit(arm, finished);
                self.begin_step()
            }
            Phase::Boundary => Err("a run ended at a step boundary".to_owned()),
        }
    }

    /// The end of a step whose arm ran (`native_step` 4-5): the hand-over of a finished tactical action, the code table, the glue flag.
    fn commit(&mut self, arm: Arm, finished: bool) {
        let core = &mut self.core;
        if finished && arm == Arm::Tactical && core.pending().window(WindowKind::Tactical).is_none() && core.pending().window(WindowKind::Aftermath).is_none() && turn::open_job_of(core).is_none() {
            turn::open_finish(core);
        }
        let _ = self.render.sync_codes(&mut core.texts.codes);
        let handed_off = arm == Arm::Tactical && core.pending().window(WindowKind::Aftermath).is_some();
        let glue = finished && (glued(arm) || arm == Arm::Aftermath || handed_off || (after_glue(core) && arm == Arm::Tactical && self.cursor.arm_asked == 0));
        set_after_glue(core, glue);
        self.steps += 1;
    }
}
