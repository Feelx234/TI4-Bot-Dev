//! The native `Game` (D2-8 lane D): the typed core owns the loop; there is no string engine behind it.
//!
//! One `Game::step()` is one step of the native loop (`docs/todo/D2-8-wrapper.md` 2.1, the loop of the bridge's `NativeGame::step_inner`):
//! the reconcile hooks, the arm, the questions put to the [`Table`], the commit. Everything a caller may read of the position is `state`
//! (the materialisation of the core after the last step, over the opening position: the fields no loaded slice carries stay as they were).
//!
//! # The two states
//!
//! * Before the first step the game is a position: `state` is the caller's (a preset or a scenario may write to it; `with_galaxy`
//!   places the frontier tokens). The core is loaded **lazily**, at the first step or [`Game::try_load`], from `state`, the galaxy and the
//!   random positions.
//! * After the load the core is the authority and `state` is a **mirror** refreshed after every step. A caller must not write to it any more
//!   (there is no way back from the typed core to a hand-edited string state); the host's fork-and-edit paths use [`Game::fork`].
//!
//! # Errors
//!
//! A step that fails restores the position (core, code table, decision log) exactly; the decider's own error is kept
//! ([`GameError::IllegalChoice`]), a defect of the core is [`GameError::Core`], a position the typed core does not carry is
//! [`GameError::Unmapped`] / [`GameError::Disabled`] and blocks the game.

use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;

use ti4_content::ContentStore;
use ti4_content::galaxy::Galaxy;
use ti4_engine_core::driver::live::{Asking, ask_arm_fn, is_open, live_arm};
use ti4_engine_core::driver::session::{Arm, AskArm, Facts, live_ask_arm, step_arm};
use ti4_engine_core::driver::turn::{self, TurnFlowWin};
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::state::{CoreState, Slice, SliceSet, WindowKind};
use ti4_model::content_types::{FULL, POK, SourceSet};
use ti4_model::id::{PlayerId, StrategyCardId, SystemId};
use ti4_model::state::{GameState, Phase};

use crate::core_sync::codec::rng::TextSeeds;
use crate::core_sync::{Codec, Ctx, Sources, materialize};
use crate::live::{self, arms};
use crate::render::Render;
use crate::wire::choice::{Choice, IllegalChoice, SeededRandom, Table, TableState};
use crate::wire::dice::Roll;
use crate::wire::observation::ExecutionObservation;
use crate::wire::rng::{GameRng, RngPositions};

mod asker;
mod rolls;

use asker::TableAsker;

/// Metadata returned after one attempted game step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepResult {
    /// The phase after this step.
    pub phase: Phase,
    /// The player whose action is next, if the phase has one.
    pub active: Option<PlayerId>,
    /// Whether the game has reached its terminal state.
    pub finished: bool,
    /// A precise reason this driver cannot safely continue.
    pub error: Option<GameError>,
    /// Whether this step resolved one generated player choice.
    pub resolved_choice: bool,
}

/// A game-level failure that is safe to expose to a caller.
///
/// The consumers read `step().error` as an opaque value; the variants are what is observable.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GameError {
    /// The decider's answer was refused, or the decider itself failed (the wasm host changed its seats, the server took a move back,
    /// a replay diverged): the position is as it was before the step.
    #[error(transparent)]
    IllegalChoice(#[from] IllegalChoice),
    /// The typed core cannot carry the position (a value without a typed form, a rules version it does not play).
    #[error("the position cannot be played: {0}")]
    Unmapped(String),
    /// The position uses a switched-off feature (`ti4_host::disabled`; docs/DISABLE_XF123.md).
    #[error("{0} is switched off")]
    Disabled(String),
    /// A defect of the core: a window that could not run. Never a fallback; counted by [`crate::product`]'s callers.
    #[error("the core stopped: {0}")]
    Core(String),
    /// The game cannot go on (it was refused at the load, or an earlier step blocked it).
    #[error("{0}")]
    Blocked(String),
    #[error("cannot prepare a hypothetical turn: {0}")]
    InvalidHypotheticalTurn(&'static str),
}

/// A bounded `run` stopped instead of silently looping forever.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RunError {
    #[error(transparent)]
    Step(#[from] GameError),
    #[error(
        "game did not progress within {max_steps} steps (round {round}, phase {phase:?});          the last {repeats} steps all asked {recent:?}"
    )]
    StepLimit {
        max_steps: usize,
        round: u32,
        phase: Phase,
        recent: String,
        repeats: usize,
    },
}

/// Observes the automatic handoff after movement.
type AftermathObserver = Arc<dyn Fn(&GameState, &ContentStore, SourceSet) + Send + Sync>;
/// Told which card a strategic action will resolve.
type StrategicObserver = Arc<dyn Fn(&PlayerId, &StrategyCardId) + Send + Sync>;

/// The slices a position loads: all, and the map only when the game has a galaxy.
fn slices_for(has_galaxy: bool) -> SliceSet {
    if has_galaxy { SliceSet::ALL } else { SliceSet(SliceSet::ALL.0 & !(1 << Slice::Map as u32)) }
}

/// The loaded core and what it is read with.
struct Native {
    core: CoreState,
    cx: Ctx,
    render: Render,
    seeds: Rc<TextSeeds>,
    kernel: &'static Kernel,
    /// The position the core was loaded from: the fields no slice carries.
    base: GameState,
    /// The last step was a glued one that finished (the turn prelude, the strategy announcement, a finished aftermath, the tail of a tactical
    /// action): the next step does not run its reconcile hooks first (`native_play::NativeGame::after_glue`).
    after_glue: bool,
}

impl Native {
    fn load(state: &GameState, galaxy: Option<&Galaxy>, positions: &RngPositions, sources: SourceSet) -> Result<Self, GameError> {
        if state.rules_version != ti4_model::state::RULES_VERSION {
            return Err(GameError::Unmapped(format!(
                "rules version {} is not the latest ({}): the core supports the latest only",
                state.rules_version,
                ti4_model::state::RULES_VERSION
            )));
        }
        if let Some(why) = crate::core_sync::unsupported::find(state) {
            return Err(GameError::Disabled(format!("{why:?}")));
        }
        let mut cx = Ctx::of(state).map_err(|e| GameError::Unmapped(format!("no owners table: {e}")))?;
        let core = Codec::load(&mut cx, state, Sources { galaxy, rng: Some(positions) }, slices_for(galaxy.is_some()))
            .map_err(|e| GameError::Unmapped(e.to_string()))?;
        // (The options that name a seat by faction -- Politics' speaker, Trade's refreshed seats -- need the faction texts.)
        let factions = cx
            .owners
            .ids()
            .iter()
            .map(|id| state.players.iter().find(|p| p.id.as_str() == id).map_or_else(String::new, |p| p.faction.as_str().to_owned()))
            .collect();
        let render = Render::new(cx.owners.ids().to_vec()).with_factions(factions).with_codes(core.texts.codes.clone());
        Ok(Self { seeds: Rc::new(TextSeeds::of(&cx)), kernel: live::kernel::kernel_for(sources), base: state.clone(), core, cx, render, after_glue: false })
    }

    /// The position as the string's `GameState`.
    fn materialized(&self) -> Result<GameState, GameError> {
        materialize::materialize(&self.core, &self.cx, &self.base).map_err(|e| GameError::Core(format!("the position cannot be shown: {e}")))
    }

    fn rebuild_render(&mut self, codes: ti4_engine_core::containers::Interner, factions: Vec<String>) {
        self.render = Render::new(self.cx.owners.ids().to_vec()).with_factions(factions).with_codes(codes);
    }

    /// The hand-overs between the arms, after `arm`'s window ended: the end of a tactical action opens the end of the action.
    fn after_arm(&mut self, arm: Arm) {
        if arm == Arm::Tactical && self.core.pending().window(WindowKind::Tactical).is_none() && self.core.pending().window(WindowKind::Aftermath).is_none() && turn::open_job_of(&self.core).is_none() {
            turn::open_finish(&mut self.core);
        }
    }
}

/// The arms the string runs together with the next step (the prelude and the announcement go on into the question of the same step).
const fn glued(arm: Arm) -> bool {
    matches!(arm, Arm::TurnPrelude | Arm::StrategyAnnounce)
}

/// What one native step did.
struct Done {
    finished_game: bool,
    asked: usize,
}

/// The stateful owner of generated choices, their decision log, and observable events; the typed core plays the game.
pub struct Game<'a> {
    /// The position: the caller's before the first step, the mirror of the core after it (see the module text).
    pub state: GameState,
    pub table: Table,
    pub events: Vec<String>,
    content: &'a ContentStore,
    sources: SourceSet,
    galaxy: Option<Galaxy>,
    /// The random streams before the load (the core owns them after it).
    rng: GameRng,
    native: Option<Native>,
    /// Every die rolled, rendered from the core's ledger after each step.
    rolls: Vec<Roll>,
    blocked: Option<GameError>,
    observation: Option<ExecutionObservation>,
    aftermath_observer: Option<AftermathObserver>,
    strategic_observer: Option<StrategicObserver>,
    /// Steps played (for tests and for the host's counters).
    steps: usize,
}

/// A copy of everything a [`Game`] needs to continue identically, taken at a step boundary. Plain data: a session worker can publish one
/// for other threads and turn it back into a game with [`GameSnapshot::instantiate`] or [`GameSnapshot::into_game`].
///
/// FORK RULE (AGENTS.md): `Game::snapshot`, `instantiate` and `into_game` destructure `Game` / `GameSnapshot` WITHOUT `..`: a new field is a
/// compile error until it is copied or listed as deliberately not copied.
#[derive(Clone)]
pub struct GameSnapshot<'a> {
    state: GameState,
    table: TableState,
    events: Vec<String>,
    content: &'a ContentStore,
    sources: SourceSet,
    galaxy: Option<Galaxy>,
    rng: RngPositions,
    seed: u64,
    native: Option<NativeSnapshot>,
    rolls: Vec<Roll>,
    blocked: Option<GameError>,
    steps: usize,
}

#[derive(Clone)]
struct NativeSnapshot {
    core: CoreState,
    cx: Ctx,
    codes: ti4_engine_core::containers::Interner,
    factions: Vec<String>,
    base: GameState,
    after_glue: bool,
}

impl NativeSnapshot {
    fn of(n: &Native) -> Self {
        Self { core: n.core.clone(), cx: n.cx.clone(), codes: n.render.codes_snapshot(), factions: n.render.factions.clone(), base: n.base.clone(), after_glue: n.after_glue }
    }

    fn open(&self, sources: SourceSet) -> Native {
        let mut render = Render::new(self.cx.owners.ids().to_vec()).with_codes(self.codes.clone());
        render.factions.clone_from(&self.factions);
        Native { core: self.core.clone(), cx: self.cx.clone(), render, seeds: Rc::new(TextSeeds::of(&self.cx)), kernel: live::kernel::kernel_for(sources), base: self.base.clone(), after_glue: self.after_glue }
    }
}

impl<'a> GameSnapshot<'a> {
    /// A new game that continues from here, with a fresh first-option table.
    #[must_use]
    pub fn instantiate(&self) -> Game<'a> {
        let mut game = self.clone().into_game();
        game.table = Table::from_state(self.table.clone());
        game
    }

    /// The same, consuming the snapshot (no copy of the position).
    #[must_use]
    pub fn into_game(self) -> Game<'a> {
        let Self { state, table, events, content, sources, galaxy, rng, seed, native, rolls, blocked, steps } = self;
        let mut game_rng = GameRng::new(seed);
        game_rng.set_positions(&rng);
        Game {
            native: native.as_ref().map(|n| n.open(sources)),
            state,
            table: Table::from_state(table),
            events,
            content,
            sources,
            galaxy,
            rng: game_rng,
            rolls,
            blocked,
            observation: None,
            aftermath_observer: None,
            strategic_observer: None,
            steps,
        }
    }

    /// The decisions journaled up to the snapshot.
    #[must_use]
    pub fn decisions(&self) -> &[crate::wire::choice::DecisionRecord] {
        &self.table.log.records
    }
}

impl<'a> Game<'a> {
    /// Create a game with the default first-option table.
    #[must_use]
    pub fn new(state: GameState, content: &'a ContentStore) -> Self {
        Self::with_table(state, content, Table::new())
    }

    /// Create a game whose every unseated player draws uniformly from one seeded legal stream; the dice share the seed.
    #[must_use]
    pub fn with_seeded_random(state: GameState, content: &'a ContentStore, seed: u64) -> Self {
        let mut game = Self::with_table(state, content, Table::with_default(Box::new(SeededRandom::new(seed))));
        game.rng = GameRng::new(seed);
        game.state.rng_seed = seed;
        game
    }

    /// Create a game with explicit deciders for generated choices.
    #[must_use]
    pub fn with_table(mut state: GameState, content: &'a ContentStore, table: Table) -> Self {
        let seed = state.rng_seed;
        // Loaded cards are not newly gained. Preserve a saved baseline so gains pending in an in-progress state still receive their window
        // after reconstruction.
        if crate::supply::staging_enabled(&state) {
            for seat in &state.players {
                let breakthrough = seat.breakthrough.as_ref().map(ToString::to_string).unwrap_or_default();
                let relics = seat.relics.iter().map(ToString::to_string).collect::<Vec<_>>().join(",");
                state.faction_marks.entry(format!("private:#seen:breakthrough:{}", seat.id)).or_insert(breakthrough);
                state.faction_marks.entry(format!("private:#seen:relics:{}", seat.id)).or_insert(relics);
            }
        }
        let thunders_edge = state.thunders_edge;
        Self {
            state,
            table,
            events: Vec::new(),
            content,
            sources: if thunders_edge { FULL } else { POK },
            galaxy: None,
            rng: GameRng::new(seed),
            native: None,
            rolls: Vec::new(),
            blocked: None,
            observation: None,
            aftermath_observer: None,
            strategic_observer: None,
            steps: 0,
        }
    }

    /// [`Game::with_table`] that refuses a position the core does not play, instead of failing at the first step: an older rules version,
    /// a switched-off feature, a position the codecs cannot carry. The core is loaded at once when the game has no map to wait for; with
    /// a map, call [`Game::with_galaxy`] and then [`Game::try_load`].
    ///
    /// # Errors
    /// [`GameError::Unmapped`] / [`GameError::Disabled`].
    pub fn try_with_table(state: GameState, content: &'a ContentStore, table: Table) -> Result<Self, GameError> {
        let game = Self::with_table(state, content, table);
        game.check_playable()?;
        Ok(game)
    }

    /// [`Game::with_seeded_random`] with the guards of [`Game::try_with_table`].
    ///
    /// # Errors
    /// As [`Game::try_with_table`].
    pub fn try_with_seeded_random(state: GameState, content: &'a ContentStore, seed: u64) -> Result<Self, GameError> {
        let game = Self::with_seeded_random(state, content, seed);
        game.check_playable()?;
        Ok(game)
    }

    /// [`Game::new`] with the guards of [`Game::try_with_table`].
    ///
    /// # Errors
    /// As [`Game::try_with_table`].
    pub fn try_new(state: GameState, content: &'a ContentStore) -> Result<Self, GameError> {
        Self::try_with_table(state, content, Table::new())
    }

    /// What `ti4_host::disabled::check_playable` refuses, as the engine sees it: the version and the switched-off features. (The creation
    /// guards of the lobby -- `ids_supported`, `factions_known` -- stay in `ti4_host::setup`.)
    ///
    /// # Errors
    /// [`GameError::Unmapped`] for a rules version below the latest, [`GameError::Disabled`] for a switched-off feature.
    pub fn check_playable(&self) -> Result<(), GameError> {
        if self.state.rules_version != ti4_model::state::RULES_VERSION {
            return Err(GameError::Unmapped(format!(
                "rules version {} is not the latest ({}): the core supports the latest only",
                self.state.rules_version,
                ti4_model::state::RULES_VERSION
            )));
        }
        if let Some(why) = crate::core_sync::unsupported::find(&self.state) {
            return Err(GameError::Disabled(format!("{why:?}")));
        }
        Ok(())
    }

    /// Load the core now (the first step does it otherwise).
    ///
    /// # Errors
    /// [`GameError::Unmapped`] / [`GameError::Disabled`]; the game is then blocked.
    pub fn try_load(&mut self) -> Result<(), GameError> {
        if self.native.is_some() {
            return Ok(());
        }
        if let Some(error) = &self.blocked {
            return Err(error.clone());
        }
        let loaded = Native::load(&self.state, self.galaxy.as_ref(), &self.rng.positions(), self.sources).inspect_err(|e| self.blocked = Some(e.clone()))?;
        self.native = Some(loaded);
        Ok(())
    }

    /// Give the game its map, which is what makes a tactical action possible. Call it before the first step.
    #[must_use]
    pub fn with_galaxy(mut self, mut galaxy: Galaxy) -> Self {
        // The Thunder's Edge Mecatol Rex tile: registered on the board so state-only readers find it without the map.
        if crate::seating::mecatol_in_galaxy(&galaxy) == crate::seating::MECATOL_TE {
            self.state.system_mut(&SystemId::new(crate::seating::MECATOL_TE));
        }
        // 35.5: a frontier token sits on every planetless system from the start.
        let systems = crate::exploration::frontier_systems(self.content, self.sources, &galaxy);
        if !systems.is_empty() {
            self.state.frontier_tokens = systems.into_iter().collect();
        }
        // The laws and the printed lanes onto the map (the string did it at every step; the core follows them from its own state).
        crate::laws::apply_to_galaxy(&self.state, &mut galaxy);
        debug_assert!(self.native.is_none(), "the map is attached before the core is loaded");
        self.galaxy = Some(galaxy);
        self
    }

    /// Play under a different source scope than the `PoK` default.
    #[must_use]
    pub const fn with_sources(mut self, sources: SourceSet) -> Self {
        self.sources = sources;
        self
    }

    /// The source scope this game is played under.
    #[must_use]
    pub const fn sources(&self) -> SourceSet {
        self.sources
    }

    /// The content store the game plays on.
    #[must_use]
    pub const fn content(&self) -> &'a ContentStore {
        self.content
    }

    /// The map, when the game has one.
    #[must_use]
    pub const fn galaxy(&self) -> Option<&Galaxy> {
        self.galaxy.as_ref()
    }

    /// Replace the table (the server hands a worker's deciders to a game it built elsewhere).
    pub fn install_table(&mut self, table: Table) {
        self.table = table;
    }

    /// Where every random stream of this game stands.
    #[must_use]
    pub fn rng_positions(&self) -> RngPositions {
        self.native.as_ref().and_then(|n| materialize::rng_positions(&n.core).ok()).unwrap_or_else(|| self.rng.positions())
    }

    /// Put the game's random streams at recorded positions (before the first step).
    pub fn set_rng_positions(&mut self, positions: &RngPositions) {
        debug_assert!(self.native.is_none(), "the streams are positioned before the core is loaded");
        self.rng.set_positions(positions);
    }

    /// Latch what a step used that an undo must not repeat ([`ExecutionObservation`]): randomness (any stream moved) and hidden information
    /// (a private mark changed).
    pub fn bind_observation(&mut self, observation: ExecutionObservation) {
        self.observation = Some(observation);
    }

    /// Observe movement's handoff to combat, invasion and production. Not forked.
    pub fn on_aftermath(&mut self, observer: impl Fn(&GameState, &ContentStore, SourceSet) + Send + Sync + 'static) {
        self.aftermath_observer = Some(Arc::new(observer));
    }

    /// Learn which card a strategic action resolves as soon as it is certain to resolve. Not forked.
    pub fn on_strategic_action_chosen(&mut self, observer: impl Fn(&PlayerId, &StrategyCardId) + Send + Sync + 'static) {
        self.strategic_observer = Some(Arc::new(observer));
    }

    /// Every die this game has rolled, in order. Read-only: a viewer shows them, nothing may change them.
    #[must_use]
    pub fn rolls(&self) -> &[Roll] {
        &self.rolls
    }

    /// Steps played.
    #[must_use]
    pub const fn steps(&self) -> usize {
        self.steps
    }

    /// The names of the windows that are open now. Empty at a step boundary means the state, the dice and the counters are all that a stored
    /// copy of this game would need.
    #[must_use]
    pub fn open_windows(&self) -> Vec<&'static str> {
        self.native.as_ref().map_or_else(Vec::new, |n| n.core.pending().stack.iter().map(|w| window_name(w.kind())).collect())
    }

    /// Copy the complete rules state, including open windows and current RNG positions. Both decision tables start fresh.
    #[must_use]
    pub fn fork(&self) -> Self {
        let mut game = self.snapshot_unlogged().into_game();
        game.table = Table::new();
        game
    }

    /// [`Game::fork`] that keeps the decision log and the counters.
    #[must_use]
    pub fn fork_keeping_log(&self) -> Self {
        self.snapshot().into_game()
    }

    /// A plain-data copy that continues identically (see [`GameSnapshot`]).
    #[must_use]
    pub fn snapshot(&self) -> GameSnapshot<'a> {
        self.snapshot_with(self.table.state())
    }

    /// [`Game::snapshot`] without the decision log (a worker publishing a checkpoint does not need the journal).
    #[must_use]
    pub fn snapshot_unlogged(&self) -> GameSnapshot<'a> {
        self.snapshot_with(self.table.state_unlogged())
    }

    fn snapshot_with(&self, table: TableState) -> GameSnapshot<'a> {
        let Self { state, table: _, events, content, sources, galaxy, rng: _, native, rolls, blocked, observation: _, aftermath_observer: _, strategic_observer: _, steps } = self;
        GameSnapshot {
            state: state.clone(),
            table,
            events: events.clone(),
            content,
            sources: *sources,
            galaxy: galaxy.clone(),
            rng: self.rng_positions(),
            seed: state.rng_seed,
            native: native.as_ref().map(NativeSnapshot::of),
            rolls: rolls.clone(),
            blocked: blocked.clone(),
            steps: *steps,
        }
    }

    /// A closure that builds the fork on another thread (the snapshot is plain data; the game is built there).
    #[must_use]
    pub fn fork_for_worker(&self) -> impl FnOnce() -> Self + Send + use<'a> {
        let snapshot = self.snapshot_unlogged();
        move || {
            let mut game = snapshot.into_game();
            game.table = Table::new();
            game
        }
    }

    /// The choice currently offered, without resolving automatic followers or phase work.
    #[must_use]
    pub fn legal_options(&self) -> Option<Choice> {
        if self.state.finished || self.blocked.is_some() {
            return None;
        }
        let temporary;
        let native = if let Some(n) = &self.native {
            n
        } else {
            temporary = Native::load(&self.state, self.galaxy.as_ref(), &self.rng.positions(), self.sources).ok()?;
            &temporary
        };
        let asks = asks_of(&native.core);
        let facts = Facts::of_core(&native.core, false);
        let arm = step_arm(&facts, asks);
        if arm == Arm::Finished {
            return None;
        }
        let env = live::env::env_with_plays(&native.render, &native.core, native.kernel, native.seeds.clone(), self.sources);
        let cx = live::Cx::new(&native.render, &env);
        let ask = ask_arm_fn(arm)?;
        let asking: Asking = ask(&native.core, &env).ok()??;
        Some(live::render_asking(&cx, &native.core, &asking))
    }

    /// Resolve one generated decision, or one choice-free phase/window transition.
    #[must_use]
    pub fn step(&mut self) -> StepResult {
        self.table.begin_step();
        if let Err(error) = self.try_load() {
            return self.result(false, Some(error));
        }
        let before_rng = self.observation.as_ref().and_then(|_| self.native.as_ref().and_then(|n| materialize::rng_positions(&n.core).ok()));
        let before_private = self.observation.as_ref().map(|_| private_marks(&self.state));
        let outcome = self.step_native();
        match outcome {
            Ok(done) => {
                if done.finished_game {
                    return self.result(false, None);
                }
                if let Some(observation) = &self.observation {
                    if self.rng_positions_differ(before_rng.as_ref()) {
                        observation.randomness();
                    }
                    if before_private.is_some_and(|b| b != private_marks(&self.state)) {
                        observation.hidden_information();
                    }
                }
                self.result(done.asked > 0, None)
            }
            Err(error) => {
                if matches!(error, GameError::Core(_) | GameError::Unmapped(_) | GameError::Disabled(_)) {
                    self.blocked = Some(error.clone());
                }
                self.result(false, Some(error))
            }
        }
    }

    fn rng_positions_differ(&self, before: Option<&RngPositions>) -> bool {
        before.is_some_and(|b| self.native.as_ref().and_then(|n| materialize::rng_positions(&n.core).ok()).is_some_and(|after| &after != b))
    }

    fn result(&self, resolved_choice: bool, error: Option<GameError>) -> StepResult {
        StepResult { phase: self.state.phase, active: self.state.active.clone(), finished: self.state.finished, error, resolved_choice }
    }

    /// Play whole rounds, bounded so an incomplete or stalled model fails loudly.
    ///
    /// # Errors
    /// [`RunError::StepLimit`] when `max_steps` is reached, or [`RunError::Step`] when a step failed.
    pub fn run(&mut self, rounds: u32, max_steps: usize) -> Result<&GameState, RunError> {
        let target = self.state.round.saturating_add(rounds);
        let mut steps = 0;
        let mut last_prompt = String::new();
        let mut repeats = 0usize;
        while self.state.round < target && !self.state.finished {
            if steps >= max_steps {
                return Err(RunError::StepLimit { max_steps, round: self.state.round, phase: self.state.phase, recent: last_prompt, repeats });
            }
            let asked = self.legal_options().map_or_else(String::new, |choice| choice.prompt);
            if asked == last_prompt {
                repeats += 1;
            } else {
                last_prompt = asked;
                repeats = 1;
            }
            let result = self.step();
            if let Some(error) = result.error {
                return Err(error.into());
            }
            steps += 1;
        }
        Ok(&self.state)
    }

    /// One step of the native loop. On an error the core, the code table and the decision log are what they were before the call.
    fn step_native(&mut self) -> Result<Done, GameError> {
        let Self { state, table, events, content, sources, galaxy, native, rolls, blocked, aftermath_observer, strategic_observer, steps, .. } = self;
        let Some(native) = native.as_mut() else { return Err(GameError::Blocked("the core is not loaded".to_owned())) };
        let before = native.core.clone();
        let (codes, factions) = (native.render.codes_snapshot(), native.render.factions.clone());
        let after_glue = native.after_glue;
        let log_len = table.log.records.len();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            native_step(native, table, events, content, *sources, galaxy.as_ref(), strategic_observer.as_ref(), aftermath_observer.as_ref(), state)
        }));
        let result = match result {
            Ok(r) => r,
            Err(p) => {
                let text = p.downcast_ref::<String>().cloned().or_else(|| p.downcast_ref::<&str>().map(|s| (*s).to_owned())).unwrap_or_default();
                Err(GameError::Core(format!("PANIC: {text}")))
            }
        };
        match result {
            Ok(done) if done.finished_game => Ok(done),
            Ok(done) => {
                *steps += 1;
                let mirror = native.materialized()?;
                *state = mirror;
                *rolls = rolls::rendered(&native.core, &native.cx, &native.render, rolls.len());
                if let Some(galaxy) = galaxy.as_mut() {
                    crate::laws::apply_to_galaxy(state, galaxy);
                }
                if let Some(error) = blocked.clone() {
                    return Err(error);
                }
                Ok(done)
            }
            Err(error) => {
                native.core = before;
                native.after_glue = after_glue;
                native.rebuild_render(codes, factions);
                table.log.records.truncate(log_len);
                Err(error)
            }
        }
    }
}

const fn window_name(kind: WindowKind) -> &'static str {
    match kind {
        WindowKind::StrategySecondary => "secondary",
        WindowKind::StrategySecondaryAfterTactical => "secondary_after_tactical",
        WindowKind::LeaderStrategy => "leader_strategy",
        WindowKind::LeaderFollowers => "leader_followers",
        WindowKind::Scoring => "scoring",
        WindowKind::EventScoring => "event_scoring",
        WindowKind::Tokens => "tokens",
        WindowKind::Voting => "voting",
        WindowKind::ExecutiveOrder => "executive_order",
        WindowKind::AgendaQueueAfterEventScoring => "agenda_queue_after_event_scoring",
        WindowKind::Tactical => "tactical",
        WindowKind::Aftermath => "aftermath",
        WindowKind::Trade => "trade",
        WindowKind::Diplomacy => "diplomacy",
        WindowKind::AgendaTalks => "agenda_talks",
        WindowKind::Combat => "combat",
        WindowKind::Invasion => "invasion",
        WindowKind::Production => "production",
        WindowKind::TurnFlow => "turn_flow",
    }
}

/// The private marks of a position (what only one seat may see): a change is a step that showed something.
fn private_marks(state: &GameState) -> BTreeMap<String, String> {
    state.faction_marks.iter().filter(|(k, _)| k.starts_with("private:") && !k.starts_with("private:#seen:")).map(|(k, v)| (k.clone(), v.clone())).collect()
}

/// The body of `NativeGame::step_inner` with the table instead of the bot.
#[allow(clippy::too_many_arguments)]
fn native_step(
    n: &mut Native,
    table: &mut Table,
    events: &mut Vec<String>,
    content: &ContentStore,
    sources: SourceSet,
    galaxy: Option<&Galaxy>,
    strategic: Option<&StrategicObserver>,
    aftermath: Option<&AftermathObserver>,
    state: &GameState,
) -> Result<Done, GameError> {
    let env = live::env::env_with_plays(&n.render, &n.core, n.kernel, n.seeds.clone(), sources);
    let lcx = live::Cx::new(&n.render, &env);
    let from = n.core.ledger.events.len();
    let mut asker = TableAsker::new(table, content, sources, galaxy, &n.cx, &n.base, strategic, aftermath);
    // 1. The reconcile hooks every step starts with (not the second half of a glued pair: the string's step runs on from the prelude).
    for _ in 0..8 {
        if n.after_glue {
            break;
        }
        let left_open = is_open::<TurnFlowWin>(&n.core);
        match live::live_reconcile(&mut n.core, &lcx, &mut asker) {
            Ok(step) if step.finished && !left_open => break,
            Ok(_) => {}
            Err(stop) => return Err(asker.error_of(stop, "reconcile")),
        }
    }
    // 2. The arm.
    let facts = Facts::of_core(&n.core, false);
    let asks = asks_of(&n.core);
    let arm = step_arm(&facts, asks);
    if arm == Arm::Finished {
        return Ok(Done { finished_game: true, asked: 0 });
    }
    if live_arm(arm).is_none() {
        return Err(GameError::Core(format!("the arm {} has no live function", arms::name(arm))));
    }
    // 3. The native step.
    let out = match live::live_step(&mut n.core, arm, &lcx, &mut asker) {
        Ok(out) => out,
        Err(stop) => return Err(asker.error_of(stop, arms::name(arm))),
    };
    asker.arm_done(arm, &out, &n.core);
    let asked = out.asked.len();
    drop(asker);
    drop(lcx);
    // 4. The hand-overs between the arms.
    if out.finished {
        n.after_arm(arm);
    }
    // 5. Commit: the events, the code table.
    let lines = crate::render::events::pushed_since(&n.render, &n.cx, &n.core, from).map(|lines| lines.into_iter().map(|p| p.text().to_owned()).collect::<Vec<_>>());
    events.extend(lines.unwrap_or_else(|| vec!["<a line the tables cannot name>".to_owned()]));
    let one = n.render.sync_codes(&mut n.core.texts.codes);
    debug_assert!(one, "the renderer's code table continues the position's");
    let handed_off = arm == Arm::Tactical && n.core.pending().window(WindowKind::Aftermath).is_some();
    n.after_glue = out.finished && (glued(arm) || arm == Arm::Aftermath || handed_off || (n.after_glue && arm == Arm::Tactical && asked == 0));
    let _ = state;
    Ok(Done { finished_game: false, asked })
}

/// [`Native::asks`] on a bare core.
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
