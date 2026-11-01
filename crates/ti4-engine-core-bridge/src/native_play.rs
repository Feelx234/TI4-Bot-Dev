//! The native playthrough (lane M7, `docs/M7_RESUME.md`): a whole game played by the CORE ALONE.
//!
//! The rule of the project: ONE MUST BE ABLE TO PLAY AN ENTIRE GAME IN THE NEW ENGINE WITHOUT ANY BAIL on the agreed content. This module is
//! that sentence as a loop (Architecture A: the core owns the loop). A game is created from a lobby spec by the string setup (the only place
//! the legacy `Game` is used to START a game: bridge only, never in a production build), its position is loaded ONCE into a `CoreState` and
//! from then on every step is
//!
//! ```text
//! live_reconcile                      the per-step hooks every step starts with
//! facts = Facts::of_core(core)        the position's own facts (the prepared turn, the closing menu, the starting picks)
//! arm   = step_arm(facts, asks)       `asks` is the core's own: the draft, the starting technology, the menu, an open job
//! live_step(arm)                      the arm's native window, put to a seat-bound asker (the bot) with the rendered question
//! ```
//!
//! No string `Game` steps the game. A step that cannot be run natively STOPS the game with a counted [`Gap`]: an arm without a live function,
//! a source of questions no lane has made native (`AskArm`), a position a live window refuses (`LiveError::Unknown`, the bail of the live
//! path), or a close the core does not own yet ([`lent_close`]). Nothing is lent and nothing is skipped; the game never loops (a step budget
//! ends it). The gaps are classified by island ([`crate::islands`]) and printed as the coverage matrix ([`Matrix::render`]).
//!
//! Two bridge-only cross-checks live next to it: [`shadow_play`] runs the legacy `Game` in parallel with the same answers and compares every
//! step (questions, events, position) and reports the first difference; [`survey`] lets the legacy game lead a whole game and counts every
//! step by the arm and the question source the native loop would need (the volume of each gap).

use std::collections::BTreeMap;
use std::rc::Rc;

use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use ti4_engine::core_sync::codec::rng::TextSeeds;
use ti4_engine::core_sync::{materialize, Codec, Ctx, Sources};
use ti4_engine::game::Game;
use ti4_engine::live::{self, arms, Asker, Question, Stop};
use ti4_engine::render::Render;
use ti4_engine_core::driver::live::{is_open, live_arm, LiveError};
use ti4_engine_core::driver::session::{live_ask_arm, step_arm, Arm, AskArm, Facts};
use ti4_engine_core::driver::turn::{self, TurnFlowWin};
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::state::{CoreState, Slice, SliceSet, WindowKind};
use ti4_model::content_types::SourceSet;
use ti4_selfplay::spec::GameSpec;

use crate::islands::{island_of, Island};
use crate::oracle::{LegacyMask, MaskSubject, Pilot, Subject};

// ---------------------------------------------------------------------------------------------------------------------------------
// The lobby specs
// ---------------------------------------------------------------------------------------------------------------------------------

/// The maps a table of `players` plays (the golden configs' names; the seeded random board for the counts they have no template for).
#[must_use]
pub fn maps_for(players: usize) -> &'static [&'static str] {
    match players {
        4 => &["4pOuterRim", "4pHyperlanes"],
        6 => &["6pStandard", "6pOuterRim"],
        _ => &["random"],
    }
}

/// Seat a plays `faction`; the others are the next lobby factions in a rotation fixed by the seed, never the same faction twice.
#[must_use]
pub fn table(faction: &str, players: usize, seed: u64) -> Vec<String> {
    let lobby = ti4_engine::seating::LOBBY_FACTIONS;
    let mut out = vec![faction.to_owned()];
    let mut at = usize::try_from(seed).unwrap_or(0) % lobby.len();  // mutants:skip (the fallback of a seed that does not fit usize)
    while out.len() < players {
        at = (at + 1) % lobby.len();
        let candidate = lobby[at];
        if !out.iter().any(|f| f == candidate) {
            out.push(candidate.to_owned());
        }
    }
    out
}

/// The lobby game the server would create: `card_set te`, `starting_tech`, no Thunder's Edge content, the speaker drawn from the seed.
#[must_use]
pub fn lobby_spec(faction: &str, players: usize, seed: u64) -> GameSpec {
    let maps = maps_for(players);
    GameSpec {
        setup: format!("native-{faction}"),
        seed,
        players,
        map: maps[usize::try_from(seed).unwrap_or(0) % maps.len()].to_owned(),  // mutants:skip (the same fallback)
        factions: table(faction, players, seed),
        speaker: "random".to_owned(),
        card_set: Some("te".to_owned()),
        rotation: None,
        starting_tech: true,
        thunders_edge: false,
        rules_version: None,
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Gaps
// ---------------------------------------------------------------------------------------------------------------------------------

/// Why a native step could not be run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum GapKind {
    /// The arm the position needs has no live function (`driver::live::live_arm`).
    ArmNotNative,
    /// The arm is `Ask` and its source of questions is not native (`driver::turn::ask`).
    AskSource,
    /// A live window refused the position (`LiveError::Unknown`: the documented bail of the rule it reads).
    Bail,
    /// The arm ran, but the part of its close the string still does is not in the core ([`lent_close`]).
    LentClose,
}

impl GapKind {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::ArmNotNative => "arm not native",
            Self::AskSource => "ask source not native",
            Self::Bail => "bail of a live window",
            Self::LentClose => "close lent",
        }
    }
}

/// A step the core cannot run yet: the work list of an island lane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gap {
    /// The arm, or `ask.<source>` for the question sources of the `Ask` arm.
    pub arm: String,
    pub kind: GapKind,
    /// The words of the cause (the `Unknown` that a window raised, the source, the part of the close).
    pub cause: String,
}

impl Gap {
    /// The key the matrix counts it under: `<arm> [gap: <kind>: <cause>]`, the shape `islands::island_of` sorts.
    #[must_use]
    pub fn key(&self) -> String {
        format!("{} [gap: {}: {}]", self.arm, self.kind.name(), self.cause)
    }

    #[must_use]
    pub fn island(&self) -> Island {
        island_of(&self.key())
    }
}

/// A step the harness or the engine got wrong (not a missing feature): a window broke its contract, the bot's answer was refused, a panic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Defect {
    pub arm: String,
    pub what: String,
}

/// The part of an arm's close the string still does after the native window ended (`None`: the close is the core's).
///
/// `Tokens`: the status phase's steps 81.6-81.8 (`finish_status_phase`) run after the last token and are lane C's (KS5). The arms of the action
/// (`Tactical`, `Aftermath`) hand the turn on through [`turn::open_finish`], which the loop opens itself ([`after_arm`]).
#[must_use]
pub const fn lent_close(arm: Arm) -> Option<&'static str> {
    match arm {
        Arm::Tokens => Some("the status steps 81.6-81.8 after the last token (KS5, lane C)"),
        _ => None,
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The bot
// ---------------------------------------------------------------------------------------------------------------------------------

/// One question as the table saw it, with the answer given.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asked {
    pub player: String,
    pub prompt: String,
    pub options: Vec<String>,
    pub chosen: String,
}

/// The seat-bound answerer: a deterministic seeded random chooser over the offered options (the random pick under the rule bot, `ti4_rule_bot::
/// random`: seeded by game seed, the seat's place and the decision's index), with the weights of the shipped bot (opening a transaction weighs 0.1).
///
/// The typed rule bot (`ti4-rule-bot-typed`) cannot answer yet: its only entry point is the self-check that calls the string bot with a string
/// observation, and the live path has none. Until its `Seen` view answers, this is the bot of the native playthrough.
struct Bot<'a> {
    seed: u64,
    /// The seat texts in seat order (the place of a seat in the game is the index of its text).
    seats: &'a [String],
    /// Decisions answered so far (the index of the next one); committed by the caller only when the step succeeded.
    index: usize,
    asked: Vec<Asked>,
}

impl Asker for Bot<'_> {
    fn ask(&mut self, q: &Question<'_>) -> Result<String, Stop> {
        let rendered = &q.rendered;
        if rendered.options.is_empty() {
            return Err(Stop::Blocked);
        }
        let place = self.seats.iter().position(|s| s == rendered.player.as_str()).unwrap_or(0);  // mutants:skip (a seat text always found; the fallback place)
        let (picked, _, _) = ti4_rule_bot::choose_explained(rendered, None, self.seed, place, self.index);
        self.index += 1;
        let chosen = rendered.options[picked.min(rendered.options.len() - 1)].id.clone();  // mutants:skip (picked is always below the length: the clamp is defensive)
        self.asked.push(Asked {
            player: rendered.player.as_str().to_owned(),
            prompt: rendered.prompt.clone(),
            options: rendered.options.iter().map(|o| o.id.clone()).collect(),
            chosen: chosen.clone(),
        });
        Ok(chosen)
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The native game
// ---------------------------------------------------------------------------------------------------------------------------------

/// What one native step did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Played {
    /// The arm the step took (`ask` for the sources of the `Ask` arm: see [`Self::source`]).
    pub arm: Arm,
    /// The source of questions when the arm is `Ask`.
    pub source: Option<AskArm>,
    pub asked: Vec<Asked>,
    /// The plain events the step pushed, in order.
    pub events: Vec<String>,
    /// The arm's window ended.
    pub finished: bool,
}

/// What [`NativeGame::step`] came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    Played(Played),
    /// The game is over (`Arm::Finished`).
    Finished,
    /// The step cannot be run natively. The position is as before the step.
    Gap(Gap),
    /// A defect. The position is as before the step.
    Defect(Defect),
}

/// A game played by the core alone.
pub struct NativeGame {
    pub core: CoreState,
    cx: Ctx,
    render: Render,
    sources: SourceSet,
    seeds: Rc<TextSeeds>,
    kernel: &'static Kernel,
    /// The position the game began from (the fields no loaded slice carries stay as they were).
    base: ti4_model::state::GameState,
    /// The seat texts in seat order.
    seats: Vec<String>,
    seed: u64,
    /// Decisions answered (the bot's index).
    decided: usize,
    /// Steps played.
    pub steps: usize,
    /// Plain events pushed so far.
    pub events: Vec<String>,
    /// The chain of the answers given: `sha256(previous || player || prompt || chosen)`, hex.
    chain: String,
    /// A fork (the shadow's lookahead): no trace.
    quiet: bool,
}

/// The slices a position loads: all, and the map only when the game has a string galaxy.
fn slices_for(has_galaxy: bool) -> SliceSet {
    if has_galaxy { SliceSet::ALL } else { SliceSet(SliceSet::ALL.0 & !(1 << Slice::Map as u32)) }
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

impl NativeGame {
    /// Load the position of a freshly created `game` into a core and own it from now on. The game is not stepped and not kept.
    ///
    /// # Errors
    /// A position the codecs cannot carry (a dropped feature, more than 16 owners), or a game without a galaxy.
    pub fn from_game(game: &Game<'static>, seed: u64) -> Result<Self, String> {
        if game.galaxy().is_none() {
            return Err("the game has no galaxy".to_owned());
        }
        // Only the latest rules version is supported by the core (user decision): an older game is refused, never played and never counted as a gap.
        if game.state.rules_version != ti4_model::state::RULES_VERSION {
            return Err(format!("rules version {} is not the latest ({}): the core supports the latest only", game.state.rules_version, ti4_model::state::RULES_VERSION));
        }
        let mut cx = Ctx::of(&game.state).map_err(|e| format!("no owners table: {e}"))?;
        let positions = game.rng_positions();
        let core = Codec::load(&mut cx, &game.state, Sources { galaxy: game.galaxy(), rng: Some(&positions) }, slices_for(true))
            .map_err(|e| format!("the position does not load: {e}"))?;
        // (The options that name a seat by faction -- Politics' speaker, Trade's refreshed seats -- need the faction texts.)
        let factions = cx.owners.ids().iter().map(|id| game.state.players.iter().find(|p| p.id.as_str() == id).map_or_else(String::new, |p| p.faction.as_str().to_owned())).collect();
        let render = Render::new(cx.owners.ids().to_vec()).with_factions(factions).with_codes(core.texts.codes.clone());
        let sources = game.content_sources();
        let seats = cx.owners.ids().to_vec();
        Ok(Self {
            seeds: Rc::new(TextSeeds::of(&cx)),
            kernel: live::kernel::kernel_for(sources),
            base: game.state.clone(),
            core,
            cx,
            render,
            sources,
            seats,
            seed,
            decided: 0,
            steps: 0,
            events: Vec::new(),
            chain: String::new(),
            quiet: false,
        })
    }

    /// Create the game of `spec` (the string setup) and load it.
    ///
    /// # Errors
    /// A spec that does not build, or [`Self::from_game`].
    pub fn from_spec(spec: &GameSpec) -> Result<Self, String> {
        let game = spec.build()?;
        Self::from_game(&game, spec.seed)
    }

    /// An independent copy that plays on identically (the checkpoint is a `CoreState` clone; the render is rebuilt from its code table).
    #[must_use]
    pub fn fork(&self) -> Self {
        let mut render = Render::new(self.cx.owners.ids().to_vec()).with_codes(self.render.codes_snapshot());
        render.factions.clone_from(&self.render.factions);
        Self {
            core: self.core.clone(),
            cx: self.cx.clone(),
            render,
            sources: self.sources,
            seeds: Rc::clone(&self.seeds),
            kernel: self.kernel,
            base: self.base.clone(),
            seats: self.seats.clone(),
            seed: self.seed,
            decided: self.decided,
            steps: self.steps,
            events: self.events.clone(),
            chain: self.chain.clone(),
            quiet: true,
        }
    }

    #[must_use]
    pub fn round(&self) -> u32 {
        self.core.meta().round
    }

    #[must_use]
    pub fn finished(&self) -> bool {
        self.core.meta().finished
    }

    /// The question source the next `Ask` step would serve (`live_ask_arm`).
    fn source(&self) -> AskArm {
        live_ask_arm(&Facts::of_core(&self.core, false))
    }

    /// `Game::legal_options().is_some()` from the core's own facts: whether the table is asked something (the last two arms of the precedence
    /// differ by it).
    #[must_use]
    pub fn asks(&self) -> bool {
        if is_open::<TurnFlowWin>(&self.core) {
            return true;
        }
        let facts = Facts::of_core(&self.core, false);
        match live_ask_arm(&facts) {
            AskArm::None | AskArm::Nothing => false,
            AskArm::Draft => turn::draft::question(&self.core).is_some(),
            AskArm::StartingTech | AskArm::LeaderFollowers | AskArm::EventScoring | AskArm::Scoring | AskArm::Tokens | AskArm::Voting | AskArm::Diplomacy | AskArm::Aftermath | AskArm::AgendaTalks | AskArm::Tactical => true,
            // The menu, the closing menu, a follower window and an open transaction: the seat in turn is asked unless everyone has passed.
            AskArm::Secondary | AskArm::Closing | AskArm::Trade | AskArm::ActionMenu => {
                self.core.meta().active.is_some() && !ti4_engine_core::driver::turn_flow::all_passed(&self.core)
            }
        }
    }

    /// Whether the `Ask` arm's source is one the core runs (`driver::turn::ask`: an open job, the draft, the starting technology, the action menu and the closing menu).
    fn ask_is_native(&self) -> bool {
        is_open::<TurnFlowWin>(&self.core) || matches!(self.source(), AskArm::Draft | AskArm::StartingTech | AskArm::Closing | AskArm::ActionMenu)
    }

    /// One step of the loop. The position is what it was before the call unless the result is [`Step::Played`].
    pub fn step(&mut self) -> Step {
        let before = self.core.clone();
        let (codes, factions) = (self.render.codes_snapshot(), self.render.factions.clone());
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.step_inner()));
        let result = result.unwrap_or_else(|p| {
            let text = p.downcast_ref::<String>().cloned().or_else(|| p.downcast_ref::<&str>().map(|s| (*s).to_owned())).unwrap_or_default();
            Step::Defect(Defect { arm: "?".to_owned(), what: format!("PANIC: {text}") })
        });
        if matches!(result, Step::Gap(_) | Step::Defect(_)) {
            self.core = before;
            self.render = Render::new(self.cx.owners.ids().to_vec()).with_factions(factions).with_codes(codes);
        }
        result
    }

    fn step_inner(&mut self) -> Step {
        let env = live::env::env_with_plays(&self.render, &self.core, self.kernel, self.seeds.clone(), self.sources);
        let lcx = live::Cx::new(&self.render, &env);
        let mut bot = Bot { seed: self.seed, seats: &self.seats, index: self.decided, asked: Vec::new() };
        let from = self.core.ledger.events.len();
        // 1. The reconcile hooks every step starts with.
        for _ in 0..8 {  // mutants:skip (the reconcile job is one window; the bound is defensive)
            let mut lone = live::LoneCard(&mut bot);
            match live::live_reconcile(&mut self.core, &lcx, &mut lone) {
                Ok(step) if step.finished => break,  // mutants:skip (the reconcile job is one window; the loop ends on its first call)
                Ok(_) => {}
                Err(stop) => return stop_to_step("reconcile", stop),
            }
        }
        // 2. The arm.
        let facts = Facts::of_core(&self.core, false);
        let asks = self.asks();
        let arm = step_arm(&facts, asks);
        if arm == Arm::Finished {
            return Step::Finished;
        }
        let name = arms::name(arm);
        if live_arm(arm).is_none() {
            return Step::Gap(Gap { arm: name.to_owned(), kind: GapKind::ArmNotNative, cause: "no live function".to_owned() });
        }
        let source = (arm == Arm::Ask).then(|| self.source());
        if arm == Arm::Ask && !self.ask_is_native() {
            let source = source.map_or("?", ask_name);
            return Step::Gap(Gap { arm: format!("ask.{source}"), kind: GapKind::AskSource, cause: format!("question source {source}") });
        }
        let arm_label = source.map_or_else(|| name.to_owned(), |s| format!("ask.{}", ask_name(s)));
        // 3. The native step.
        let mut lone = live::LoneCard(&mut bot);
        let out = match live::live_step(&mut self.core, arm, &lcx, &mut lone) {
            Ok(out) => out,
            Err(stop) => return stop_to_step(&arm_label, stop),
        };
        if !self.quiet && std::env::var_os("TI4_NATIVE_PLAY_TRACE").is_some() {
            eprintln!("step {arm_label}: asked {:?}", out.asked.iter().map(|c| (c.player.to_string(), c.prompt.clone(), c.options.iter().map(|o| o.id.clone()).collect::<Vec<_>>())).collect::<Vec<_>>());
        }
        if out.finished
            && let Some(part) = lent_close(arm)
        {
            return Step::Gap(Gap { arm: arm_label, kind: GapKind::LentClose, cause: part.to_owned() });
        }
        // 4. The hand-overs between the arms.
        if out.finished {  // mutants:skip (after_arm does nothing for an unfinished window of another arm)
            after_arm(&mut self.core, arm);
        }
        // 5. Commit: the decisions, the events, the code table.
        let asked = std::mem::take(&mut bot.asked);
        self.decided = bot.index;
        for a in &asked {
            let mut h = Sha256::new();
            h.update(self.chain.as_bytes());
            h.update(format!("{}\u{1}{}\u{1}{}", a.player, a.prompt, a.chosen).as_bytes());
            self.chain = hex(&h.finalize());
        }
        let events = ti4_engine::render::events::pushed_since(&self.render, &self.cx, &self.core, from)
            .map(|lines| lines.into_iter().map(|p| p.text().to_owned()).collect::<Vec<_>>())
            .unwrap_or_else(|| vec!["<a line the tables cannot name>".to_owned()]);
        self.events.extend(events.iter().cloned());
        self.core.texts.codes = self.render.codes_snapshot();
        self.steps += 1;
        Step::Played(Played { arm, source, asked, events, finished: out.finished })
    }

    /// The final scores `(faction, victory points)` in seat order.
    #[must_use]
    pub fn scores(&self) -> Vec<(String, i16)> {
        self.core.players().iter().map(|p| (<ti4_engine_core::generated::Faction as ti4_engine::generated::IdStr>::as_str(p.faction).to_owned(), p.econ.victory_points)).collect()
    }

    /// The decision chain only (hex): equal for two games that were asked the same questions and answered them the same way.
    #[must_use]
    pub fn decisions_hash(&self) -> &str {
        &self.chain
    }

    /// A fingerprint of the game so far: the whole position (materialized as the string engine's `GameState`, as JSON with sorted keys, or the
    /// debug text of the core when a value has no text) and the decision chain. Sixteen hex digits.
    #[must_use]
    pub fn fingerprint(&self) -> String {
        let mut h = Sha256::new();
        match self.materialized() {
            Ok(state) => h.update(state.to_string().as_bytes()),
            Err(why) => h.update(format!("unmaterializable:{why}:{:?}", self.core).as_bytes()),
        }
        h.update(self.chain.as_bytes());
        hex(&h.finalize())[..16].to_owned()
    }

    /// The position as the string engine's `GameState` JSON (over the game's opening position: every field the loaded slices carry is overwritten).
    fn materialized(&self) -> Result<Value, String> {
        self.materialized_over(&self.base)
    }
}

/// The names of the question sources (snake case, the words the island sort reads).
#[must_use]
pub const fn ask_name(source: AskArm) -> &'static str {
    match source {
        AskArm::None => "none",
        AskArm::LeaderFollowers => "leader_followers",
        AskArm::EventScoring => "event_scoring",
        AskArm::Scoring => "scoring",
        AskArm::Tokens => "tokens",
        AskArm::Voting => "voting",
        AskArm::Diplomacy => "diplomacy",
        AskArm::Aftermath => "aftermath",
        AskArm::AgendaTalks => "agenda_talks",
        AskArm::Tactical => "tactical",
        AskArm::StartingTech => "starting_tech",
        AskArm::Draft => "draft",
        AskArm::Secondary => "secondary",
        AskArm::Closing => "closing",
        AskArm::Trade => "trade",
        AskArm::ActionMenu => "action_menu",
        AskArm::Nothing => "nothing",
    }
}

/// How a live call that stopped is reported: a bail of the live path is a gap, everything else is a defect.
fn stop_to_step(arm: &str, stop: Stop) -> Step {
    match stop {
        Stop::Live(LiveError::Unknown(why)) => {
            let mut cause = format!("{why:?}");
            cause.truncate(120);  // mutants:skip (a cause is a short Debug text; the truncation is defensive)
            Step::Gap(Gap { arm: arm.to_owned(), kind: GapKind::Bail, cause })
        }
        Stop::NoLiveFn(a) => Step::Gap(Gap { arm: arms::name(a).to_owned(), kind: GapKind::ArmNotNative, cause: "no live function".to_owned() }),
        other => Step::Defect(Defect { arm: arm.to_owned(), what: format!("{other:?}") }),
    }
}

/// The hand-overs between the arms, after `arm`'s window ended: the end of a tactical action opens the end of the action (`ACTION_COMPLETED`, then
/// the closing menu or the end of the turn: `turn::open_finish`), which the next `Ask` step runs. The string does both in one step
/// (`close_tactical` ends in `finish_action`); the live loop splits them at the question.
pub fn after_arm(core: &mut CoreState, arm: Arm) {
    // (A strategy card's free action ends in the strategic action's own job, `strategic::after`: its end opens the follower window, whose close
    // finishes the action.)
    if arm == Arm::Tactical && core.pending().window(WindowKind::Tactical).is_none() && core.pending().window(WindowKind::Aftermath).is_none() && turn::open_job_of(core).is_none() {
        turn::open_finish(core);
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Whole games
// ---------------------------------------------------------------------------------------------------------------------------------

/// What the coverage of a set of games came to.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Matrix {
    /// Native steps by arm (`ask.<source>` for the question sources of the `Ask` arm).
    pub native: BTreeMap<String, u64>,
    /// Gaps by key ([`Gap::key`]).
    pub gaps: BTreeMap<String, u64>,
    pub defects: BTreeMap<String, u64>,
}

impl Matrix {
    pub fn note_played(&mut self, p: &Played) {
        let key = p.source.map_or_else(|| arms::name(p.arm).to_owned(), |s| format!("ask.{}", ask_name(s)));
        *self.native.entry(key).or_default() += 1;
    }

    pub fn note_gap(&mut self, g: &Gap) {
        *self.gaps.entry(g.key()).or_default() += 1;
    }

    pub fn note_defect(&mut self, d: &Defect) {
        *self.defects.entry(format!("{}: {}", d.arm, d.what)).or_default() += 1;
    }

    pub fn merge(&mut self, other: &Self) {
        for (k, n) in &other.native {
            *self.native.entry(k.clone()).or_default() += n;
        }
        for (k, n) in &other.gaps {
            *self.gaps.entry(k.clone()).or_default() += n;
        }
        for (k, n) in &other.defects {
            *self.defects.entry(k.clone()).or_default() += n;
        }
    }

    #[must_use]
    pub fn total_gaps(&self) -> u64 {
        self.gaps.values().sum()
    }

    /// The island coverage matrix: for every island its native steps and its gaps by cause (biggest first), then the arms with their native
    /// steps. A green island with zero native steps is NOT proven (trap T-Z1) and is marked `not reached`.
    #[must_use]
    pub fn render(&self, top: usize) -> Vec<String> {
        let mut native_by_island: BTreeMap<Island, u64> = BTreeMap::new();
        for (arm, n) in &self.native {
            *native_by_island.entry(island_of(arm)).or_default() += n;
        }
        let rows = crate::islands::by_island(self.gaps.iter().map(|(k, n)| (k.as_str(), *n)));
        let mut out = vec![format!("{:>9}  {:>9}  {}", "native", "gaps", "island")];
        for island in Island::ALL {
            let native = native_by_island.get(&island).copied().unwrap_or(0);
            let row = rows.get(&island).cloned().unwrap_or_default();
            let state = match (row.count, native) {
                (0, 0) => "green, not reached".to_owned(),
                (0, _) => "green".to_owned(),
                (_, _) => format!("{} cause(s)", row.causes.len()),
            };
            out.push(format!("{native:>9}  {:>9}  {:<20} {state}", row.count, island.name()));
            for (cause, n) in row.causes.iter().take(top) {
                out.push(format!("{:>9}  {:>9}      {cause}", "", n));
            }
        }
        out.push("---- native steps by arm ----".to_owned());
        for (arm, n) in &self.native {
            out.push(format!("{n:>9}  {arm}"));
        }
        if !self.defects.is_empty() {
            out.push("---- DEFECTS (not missing features) ----".to_owned());
            for (d, n) in &self.defects {
                out.push(format!("{n:>9}  {d}"));
            }
        }
        out
    }
}

/// How a game ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum End {
    /// The core played the game to its end with no gap.
    Finished,
    /// The first gap; the game stopped there.
    Gap(Gap),
    Defect(Defect),
    /// The step budget ran out before the game ended or gapped (never a loop: the budget ends it).
    StepLimit,
    /// The round budget ran out.
    RoundLimit,
}

/// One game played natively.
#[derive(Debug, Clone)]
pub struct Played1 {
    pub end: End,
    pub rounds: u32,
    pub steps: usize,
    pub scores: Vec<(String, i16)>,
    pub fingerprint: String,
    pub matrix: Matrix,
}

/// Play `game` until it ends, gaps, or the budgets run out.
pub fn play_game(game: &mut NativeGame, max_steps: usize, max_rounds: u32) -> Played1 {
    let mut matrix = Matrix::default();
    let end = loop {
        if game.steps >= max_steps {
            break End::StepLimit;
        }
        if game.round() > max_rounds {
            break End::RoundLimit;
        }
        match game.step() {
            Step::Played(p) => matrix.note_played(&p),
            Step::Finished => break End::Finished,
            Step::Gap(g) => {
                matrix.note_gap(&g);
                break End::Gap(g);
            }
            Step::Defect(d) => {
                matrix.note_defect(&d);
                break End::Defect(d);
            }
        }
    };
    Played1 { end, rounds: game.round(), steps: game.steps, scores: game.scores(), fingerprint: game.fingerprint(), matrix }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The shadow: the legacy game in parallel (bridge only)
// ---------------------------------------------------------------------------------------------------------------------------------

/// The first difference between the core's game and the legacy game that followed it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Difference {
    /// The native step (1-based) at the end of which the two disagreed, and the arms of the group of native steps it covers.
    pub step: usize,
    pub arms: Vec<String>,
    /// `questions`, `events`, `state` or `legacy`.
    pub what: &'static str,
    pub path: String,
    pub native: String,
    pub legacy: String,
}

impl std::fmt::Display for Difference {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "step {} [{}] {} {}: native {} | legacy {}", self.step, self.arms.join("+"), self.what, self.path, self.native, self.legacy)
    }
}

/// The arms whose steps the legacy engine runs together with the NEXT native step (`Game::step_flow` goes on from the prelude and the
/// announcement into the question of the same step).
const fn glued(arm: Arm) -> bool {
    matches!(arm, Arm::TurnPrelude | Arm::StrategyAnnounce)
}

/// Whether the group of native steps ending in `arm` ends where the legacy step does, so the position can be compared (the close of the other
/// arms is the string's, `native_subject::close_is_native`).
const fn strict(arm: Arm) -> bool {
    matches!(arm, Arm::Ask | Arm::Phase)
}

/// What a shadowed game came to.
#[derive(Debug, Clone)]
pub struct Shadowed {
    pub played: Played1,
    /// Groups of native steps compared (legacy steps).
    pub compared: usize,
    pub first_difference: Option<Difference>,
}

/// Play `spec` natively with the legacy game in parallel: the legacy game is fed the answers the native bot gave and stepped once per group of
/// native steps; questions, plain events and the position are compared after every group. Stops at the first difference.
///
/// # Errors
/// A spec that does not build.
pub fn shadow_play(spec: &GameSpec, max_steps: usize, max_rounds: u32) -> Result<Shadowed, String> {
    let mut legacy = MaskSubject::<LegacyMask>::new(spec, Pilot::Script)?;
    let mut native = NativeGame::from_game(legacy.game(), spec.seed)?;
    let mut matrix = Matrix::default();
    let mut group: Vec<Played> = Vec::new();
    let mut compared = 0usize;
    let mut first_difference = None;
    // A step the group's extension ran ahead of the loop and that is not a played one (a gap, a defect, the end).
    let mut carry: Option<Step> = None;
    // The events one side is ahead of the other (the string's step goes on into the next arm's first run where the native steps split, and the
    // other way round): they open the events of the next group.
    let mut owed = Owed::default();
    let end = loop {
        if native.steps >= max_steps {
            break End::StepLimit;
        }
        if native.round() > max_rounds {
            break End::RoundLimit;
        }
        let base = legacy.game().state.clone();
        match carry.take().unwrap_or_else(|| native.step()) {
            Step::Played(p) => {
                matrix.note_played(&p);
                let arm = p.arm;
                group.push(p);
                if glued(arm) {
                    continue;
                }
                // A native step that asks nothing while the legacy game already stands at a question: the string ran that stretch in an earlier step
                // (its events were owed to this one); there is no legacy step to compare it with, its events are owed to the next.
                if group.iter().all(|p| p.asked.is_empty()) && !owed.legacy.is_empty() && legacy.options().is_some() {
                    let arms: Vec<String> = group.iter().map(|p| p.source.map_or_else(|| arms::name(p.arm).to_owned(), |s| format!("ask.{}", ask_name(s)))).collect();
                    let events: Vec<String> = group.iter().flat_map(|p| p.events.iter().cloned()).collect();
                    if let Some((at, n, l)) = owed.settle(events, Vec::new()) {
                        first_difference = Some(Difference { step: native.steps, arms, what: "events", path: format!("[{at}]"), native: n, legacy: l });
                        break End::Finished;
                    }
                    group.clear();
                    continue;
                }
                compared += 1;
                // One legacy step can settle questions the native loop asks in later steps (a question the string puts INSIDE a step: an
                // exploration's reward after a first claim, the rewards of a card played in a window): the legacy game gets the answers of
                // the next native steps too (played on a fork), and the group grows by the native steps whose answers the legacy used.
                let ahead = answers_ahead(&native, AHEAD);
                let mut run = run_legacy(&mut legacy, &group, &ahead);
                // The native steps ran on where the string takes a step of its own with no question (the close of an action): that step uses no
                // answer; its events meet what the native pushed ahead, and the legacy game is stepped again with the same answers.
                let mut catch_up = 0;
                while run.records.is_empty() && !owed.native.is_empty() && run.info.error.is_none() && !group.iter().all(|p| p.asked.is_empty()) && catch_up < 4 {
                    let arms: Vec<String> = group.iter().map(|p| p.source.map_or_else(|| arms::name(p.arm).to_owned(), |s| format!("ask.{}", ask_name(s)))).collect();
                    let _ = (legacy.decisions_since(), legacy.events_since());
                    if let Some((at, n, l)) = owed.settle(Vec::new(), run.events.clone()) {
                        first_difference = Some(Difference { step: native.steps, arms, what: "events", path: format!("[{at}]"), native: n, legacy: l });
                        break;
                    }
                    run = run_legacy(&mut legacy, &group, &ahead);
                    catch_up += 1;
                }
                if first_difference.is_some() {
                    break End::Finished;
                }
                let mut used = run.records.len();
                let mut have: usize = group.iter().map(|p| p.asked.len()).sum();
                while have < used {
                    match native.step() {
                        Step::Played(q) => {
                            matrix.note_played(&q);
                            have += q.asked.len();
                            group.push(q);
                        }
                        other => {
                            carry = Some(other);
                            used = have;
                        }
                    }
                }
                if let Some(d) = compare_run(&mut legacy, &native, &base, &group, &run, &mut owed) {
                    first_difference = Some(d);
                    break End::Finished;
                }
                group.clear();
            }
            Step::Finished => break End::Finished,
            Step::Gap(g) => {
                matrix.note_gap(&g);
                break End::Gap(g);
            }
            Step::Defect(d) => {
                matrix.note_defect(&d);
                break End::Defect(d);
            }
        }
    };
    let played = Played1 { end, rounds: native.round(), steps: native.steps, scores: native.scores(), fingerprint: native.fingerprint(), matrix };
    Ok(Shadowed { played, compared, first_difference })
}

/// The events one side has pushed that the other has not, yet (a native step and a legacy step end in different places inside one arm's close).
#[derive(Debug, Default)]
struct Owed {
    /// Pushed by the legacy steps, to be matched by the next native ones.
    legacy: Vec<String>,
    /// Pushed by the native steps, to be matched by the next legacy ones.
    native: Vec<String>,
}

impl Owed {
    /// New events of either side with no step of the other to meet: matched against what the other side pushed ahead. `Some((index, native, legacy))`
    /// at the first line that differs.
    fn settle(&mut self, native_new: Vec<String>, legacy_new: Vec<String>) -> Option<(usize, String, String)> {
        let native: Vec<String> = self.native.drain(..).chain(native_new).collect();
        let legacy: Vec<String> = self.legacy.drain(..).chain(legacy_new).collect();
        let common = native.len().min(legacy.len());
        if let Some(at) = (0..common).find(|i| native[*i] != legacy[*i]) {
            return Some((at, native[at].clone(), legacy[at].clone()));
        }
        self.native = native[common..].to_vec();
        self.legacy = legacy[common..].to_vec();
        None
    }
}

/// How many native steps the legacy game is given answers for beyond the group it is stepped with.
const AHEAD: usize = 4;

/// The answers of the next `n` native steps, played on a fork (the native game itself does not move).
fn answers_ahead(native: &NativeGame, n: usize) -> Vec<String> {
    let mut fork = native.fork();
    let mut out = Vec::new();
    for _ in 0..n {
        match fork.step() {
            Step::Played(p) => out.extend(p.asked.iter().map(|a| a.chosen.clone())),
            _ => break,
        }
    }
    out
}

/// What of the legacy step is compared: all of it for a step the core owns whole, the first `native_len` of it where the legacy step goes on with a close the
/// core does not own (`strict` false).
fn legacy_prefix<T: Clone>(strict: bool, legacy: Vec<T>, native_len: usize) -> Vec<T> {
    if !strict && legacy.len() > native_len { legacy[..native_len].to_vec() } else { legacy } // mutants:skip (equal lengths: the cut is the identity)
}

/// What one legacy step came to.
struct LegacyRun {
    info: crate::oracle::StepInfo,
    records: Vec<ti4_engine::choice::DecisionRecord>,
    events: Vec<String>,
}

/// Step the legacy game once with the answers of `group` (and then the `ahead` ones, which a question the string asks inside the step
/// takes); what the step did not use is dropped.
fn run_legacy(legacy: &mut MaskSubject<LegacyMask>, group: &[Played], ahead: &[String]) -> LegacyRun {
    for a in group.iter().flat_map(|p| p.asked.iter()) {
        legacy.answer(&a.chosen);
    }
    for a in ahead {
        legacy.answer(a);
    }
    let info = legacy.step();
    legacy.clear_script();
    let records = legacy.peek_records().to_vec();
    let events = legacy.peek_events().to_vec();
    LegacyRun { info, records, events }
}

/// Step the legacy game once with the answers of `group` and compare.
fn compare_group(legacy: &mut MaskSubject<LegacyMask>, native: &NativeGame, base: &ti4_model::state::GameState, group: &[Played]) -> Option<Difference> {
    let run = run_legacy(legacy, group, &[]);
    compare_run(legacy, native, base, group, &run, &mut Owed::default())
}

/// Compare the native steps of `group` with the legacy step that ran them.
fn compare_run(legacy: &mut MaskSubject<LegacyMask>, native: &NativeGame, base: &ti4_model::state::GameState, group: &[Played], run: &LegacyRun, owed: &mut Owed) -> Option<Difference> {
    let arms: Vec<String> = group.iter().map(|p| p.source.map_or_else(|| arms::name(p.arm).to_owned(), |s| format!("ask.{}", ask_name(s)))).collect();
    let diff = |what: &'static str, path: String, n: String, l: String| Difference { step: native.steps, arms: arms.clone(), what, path, native: n, legacy: l };
    let asked: Vec<&Asked> = group.iter().flat_map(|p| p.asked.iter()).collect();
    let (info, records, events) = (&run.info, &run.records, &run.events);
    if let Some(error) = &info.error {
        return Some(diff("legacy", "step.error".to_owned(), "no error".to_owned(), error.clone()));
    }
    if std::env::var_os("TI4_NATIVE_PLAY_TRACE").is_some() {
        eprintln!("  legacy: answered {:?} -> asked {:?}", asked.iter().map(|a| a.chosen.as_str()).collect::<Vec<_>>(), records.iter().map(|r| (r.player.as_str().to_owned(), r.prompt.clone(), r.chosen.clone())).collect::<Vec<_>>());
        eprintln!("  legacy events: {events:?}");
        eprintln!("  native events: {:?}", group.iter().flat_map(|p| p.events.iter()).collect::<Vec<_>>());
    }
    let _ = (legacy.decisions_since(), legacy.events_since());
    let last = group.last()?;
    let strict = strict(last.arm);
    // Questions: player, prompt and the option ids, in order. A group that ENDS an arm of the string's close compares the prefix.
    let n: Vec<Value> = asked.iter().map(|a| json!({ "player": a.player, "prompt": a.prompt, "options": a.options })).collect();
    let l: Vec<Value> = records.iter().map(|r| json!({ "player": r.player.as_str(), "prompt": r.prompt, "options": r.offered })).collect();
    let l = legacy_prefix(strict, l, n.len());
    if let Some(d) = crate::native_subject::first_leaf(&Value::Array(l), &Value::Array(n)) {
        return Some(diff("questions", d.path, d.right, d.left));
    }
    // The events are two streams: what one side pushed beyond the other's end of step is matched by the other's next step. A step that ends an
    // arm the string's close goes on after (non-strict) may leave either side ahead; a strict step must end level.
    let native_stream: Vec<String> = owed.native.drain(..).chain(group.iter().flat_map(|p| p.events.iter().cloned())).collect();
    let legacy_stream: Vec<String> = owed.legacy.drain(..).chain(events.iter().cloned()).collect();
    let common = native_stream.len().min(legacy_stream.len());
    let at = (0..common).find(|i| native_stream[*i] != legacy_stream[*i]).or_else(|| (strict && native_stream.len() != legacy_stream.len()).then_some(common));
    if let Some(at) = at {
        let show = |v: &[String]| v.get(at).map_or_else(|| format!("<end after {}>", v.len()), Clone::clone);
        return Some(diff("events", format!("[{at}]"), show(&native_stream), show(&legacy_stream)));
    }
    owed.native = native_stream[common..].to_vec();
    owed.legacy = legacy_stream[common..].to_vec();
    if strict {
        // The events effects staged for the next step's announcement are marks of JSON in the legacy position and `Pending::staged` in the core's:
        // compared by name, in order, and kept out of the position (the position carries only what the codec carries).
        let mut legacy_state = legacy.state_json();
        let legacy_staged = crate::native_subject::strip_staged(&mut legacy_state);
        let native_staged: Vec<String> = native.core.pending().staged.iter().map(|e| ti4_engine::kernel_sync::text::event_text(&native.render, e.kind)).collect();
        if legacy_staged != native_staged {
            return Some(diff("staged", "pending.staged".to_owned(), format!("{native_staged:?}"), format!("{legacy_staged:?}")));
        }
        match native.materialized_over(base) {
            Ok(after) => {
                if let Some(d) = crate::native_subject::first_leaf(&legacy_state, &crate::native_subject::without_staged_marks(after)) {
                    return Some(diff("state", d.path, d.right, d.left));
                }
            }
            Err(why) => return Some(diff("state", "materialize".to_owned(), why, "a position".to_owned())),
        }
    }
    None
}

impl NativeGame {
    /// The position over `base` (a string `GameState` the loaded slices overwrite).
    fn materialized_over(&self, base: &ti4_model::state::GameState) -> Result<Value, String> {
        let state = materialize::materialize(&self.core, &self.cx, base).map_err(|e| e.to_string())?;
        serde_json::to_value(&state).map_err(|e| e.to_string())
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The survey: the legacy game leads, the native loop is asked what it would need
// ---------------------------------------------------------------------------------------------------------------------------------

/// What a surveyed game came to.
#[derive(Debug, Clone, Default)]
pub struct Surveyed {
    pub rounds: u32,
    pub steps: usize,
    pub finished: bool,
    /// Steps by `(arm label, native?)`: how many steps of a whole legacy game the native loop would run (`true`) and how many it would stop at.
    pub steps_by_arm: BTreeMap<(String, bool), u64>,
}

/// Play `spec` with the legacy game (the shipped rule bot answers) and classify every step by the arm and the source of questions the native loop
/// would take: native when the arm has a live function (and, for `Ask`, a native source), else a gap of that arm. Bridge only.
///
/// # Errors
/// A spec that does not build.
pub fn survey(spec: &GameSpec, max_steps: usize, max_rounds: u32) -> Result<Surveyed, String> {
    let mut legacy = MaskSubject::<LegacyMask>::new(spec, Pilot::Bot)?;
    let mut out = Surveyed::default();
    while !legacy.finished() && legacy.round() <= max_rounds && out.steps < max_steps {
        let facts = legacy.game().step_facts();
        let asks = legacy.options().is_some();
        let arm = step_arm(&facts, asks);
        let source_native = |source: AskArm| matches!(source, AskArm::Draft | AskArm::StartingTech | AskArm::Closing);
        let (label, native) = if arm == Arm::Ask {
            let source = live_ask_arm(&facts);
            (format!("ask.{}", ask_name(source)), source_native(source))
        } else if glued(arm) && asks {
            // The legacy step goes on from the prelude and the announcement into the question of the same step: the native loop needs the source of that
            // question too (two native steps, one legacy step).
            let source = live_ask_arm(&facts);
            (format!("{}>ask.{}", arms::name(arm), ask_name(source)), live_arm(arm).is_some() && source_native(source))
        } else {
            (arms::name(arm).to_owned(), live_arm(arm).is_some())
        };
        *out.steps_by_arm.entry((label, native)).or_default() += 1;
        let info = legacy.step();
        out.steps += 1;
        if info.error.is_some() {
            break;
        }
    }
    out.rounds = legacy.round();
    out.finished = legacy.finished();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use ti4_engine::seating::LOBBY_FACTIONS;
    use ti4_engine_core::containers::SmallList;
    use ti4_engine_core::driver::live::open;
    use ti4_engine_core::ids::Seat;
    use ti4_engine_core::rules::token_gain::TokenGain;
    use ti4_engine_core::state::{StatusReport, Tokens, TurnJob};
    use ti4_engine::live::Question;

    use super::*;

    fn game(faction: &str, players: usize, seed: u64) -> NativeGame {
        NativeGame::from_spec(&lobby_spec(faction, players, seed)).expect("a lobby game builds and loads")
    }

    #[test]
    fn a_lobby_spec_is_the_game_the_server_creates() {
        for players in 3..=6 {
            let spec = lobby_spec("sol", players, 3);
            assert_eq!((spec.card_set.as_deref(), spec.starting_tech, spec.thunders_edge), (Some("te"), true, false));
            assert_eq!(spec.factions.len(), players);
            assert_eq!(spec.factions[0], "sol");
            let mut distinct = spec.factions.clone();
            distinct.sort();
            distinct.dedup();
            assert_eq!(distinct.len(), players, "never the same faction twice: {:?}", spec.factions);
            assert!(spec.factions.iter().all(|f| LOBBY_FACTIONS.contains(&f.as_str())));
        }
        assert_eq!(lobby_spec("sol", 4, 1).map, "4pHyperlanes");
        assert_eq!(lobby_spec("sol", 4, 2).map, "4pOuterRim", "the seed picks the map");
        assert_eq!(lobby_spec("hacan", 3, 1).map, "random");
        assert_ne!(lobby_spec("sol", 4, 1).factions, lobby_spec("sol", 4, 2).factions, "the seed rotates the table");
    }

    /// The native game plays the setup, the draft, the announcement, the phase flip, the turn prelude, the action menu and the first round of actions (an Orbital
    /// Drop, tactical actions, a first claim with its exploration, strategic actions, Ggrocuto Rinn) on the core alone, and stops with a COUNTED gap at the first
    /// body no lane has made native (Thunder's Edge Warfare's free tactical action: `TurnGap::StrategyAbility`), with the position as it was before the step.
    #[test]
    fn a_native_game_plays_the_first_turns_and_stops_with_a_counted_gap_at_an_unrouted_menu_option() {
        let mut g = game("sol", 4, 1);
        let played = play_game(&mut g, 2_000, 50);
        let End::Gap(gap) = &played.end else { panic!("a gap is the end of a game today, got {:?}", played.end) };
        assert_eq!((gap.arm.as_str(), gap.kind, gap.cause.as_str()), ("ask.action_menu", GapKind::Bail, "TurnGap(StrategyAbility)"), "{gap:?}");
        let native = &played.matrix.native;
        assert!(native.get("ask.draft").copied().unwrap_or(0) >= 3, "the draft was asked natively: {native:?}");
        assert_eq!(native.get("strategy_announce"), Some(&1), "{native:?}");
        assert_eq!(native.get("phase"), Some(&1), "the strategy phase flipped into the action phase: {native:?}");
        assert!(native.get("turn_prelude").copied().unwrap_or(0) >= 1, "{native:?}");
        assert_eq!(played.matrix.total_gaps(), 1, "one game, one gap");
        assert_eq!(played.rounds, 1);
        // The position is the one before the step that gapped: the same gap again, the same position.
        let held = g.core.clone();
        assert_eq!(g.step(), Step::Gap(gap.clone()));
        assert_eq!(g.core, held);
        assert_eq!(g.fingerprint(), played.fingerprint);
    }

    #[test]
    fn a_game_below_the_latest_rules_version_is_refused_not_played() {
        let mut spec = lobby_spec("sol", 4, 1);
        assert_eq!(spec.rules_version, None, "a lobby game is made at the newest version the build knows");
        assert_eq!(NativeGame::from_spec(&spec).map(|g| g.core.meta().rules_version), Ok(ti4_model::state::RULES_VERSION));
        spec.rules_version = Some(ti4_model::state::RULES_VERSION - 1);
        let refused = NativeGame::from_spec(&spec).err().expect("an old game is refused");
        assert!(refused.contains("not the latest"), "{refused}");
    }

    #[test]
    fn the_same_seed_twice_gives_the_same_fingerprint_and_another_seed_another() {
        let run = |faction: &str, seed: u64| {
            let mut g = game(faction, 4, seed);
            let p = play_game(&mut g, 2_000, 50);
            (p.fingerprint, g.decisions_hash().to_owned(), p.steps)
        };
        let (a, b) = (run("sol", 1), run("sol", 1));
        assert_eq!(a, b);
        assert!(!a.1.is_empty(), "the draft was answered: the decision chain is not empty");
        assert_ne!(a.0, run("sol", 2).0, "another seed, another table, another game");
        assert_ne!(a.0, run("hacan", 1).0, "another faction at seat a");
    }

    #[test]
    fn a_tokens_window_runs_natively_and_its_lent_close_is_a_counted_gap() {
        let mut g = game("sol", 4, 1);
        open(&mut g.core, &TokenGain::new(Tokens { pending: SmallList::from_slice(&[Seat(0)]), placed: SmallList::new(), report: StatusReport::default() }));
        let held = g.core.clone();
        let Step::Gap(gap) = g.step() else { panic!("the last token ends the window, whose close the string still does") };
        assert_eq!((gap.arm.as_str(), gap.kind), ("tokens", GapKind::LentClose), "{gap:?}");
        assert_eq!(gap.island(), Island::Status);
        assert_eq!(g.core, held, "a gap leaves the position as it was");
        assert_eq!(lent_close(Arm::Tokens).is_some(), true);
        assert!(Arm::ALL.iter().filter(|a| lent_close(**a).is_some()).count() == 1, "only the token gain closes in the string today");
    }

    #[test]
    fn the_end_of_the_action_is_opened_after_a_closed_tactical_window_only() {
        let mut g = game("sol", 4, 1);
        after_arm(&mut g.core, Arm::Ask);
        assert_eq!(turn::open_job_of(&g.core), None, "another arm opens nothing");
        after_arm(&mut g.core, Arm::Tactical);
        assert_eq!(turn::open_job_of(&g.core), Some(TurnJob::Finish), "the tactical window is gone: the action ends");
        let mut g = game("sol", 4, 1);
        g.core.pending_mut().open(ti4_engine_core::state::Window::Untyped { kind: WindowKind::Aftermath, actor: None, data: [0; 3] });
        after_arm(&mut g.core, Arm::Tactical);
        assert_eq!(turn::open_job_of(&g.core), None, "an aftermath still open: the action goes on");
    }

    #[test]
    fn the_table_rotates_from_the_seed_and_never_seats_a_faction_twice() {
        let lobby = LOBBY_FACTIONS;
        let t = table("sol", 3, 5);
        assert_eq!(t, ["sol".to_owned(), lobby[6].to_owned(), lobby[7].to_owned()], "the next lobby factions after the seed's place");
        let before_sol = (lobby.iter().position(|f| *f == "sol").unwrap() + lobby.len() - 1) % lobby.len();
        let from_sol = table("sol", 4, before_sol as u64);
        assert_eq!(from_sol.len(), 4);
        assert!(!from_sol[1..].contains(&"sol".to_owned()), "seat a's faction is skipped: {from_sol:?}");
    }

    #[test]
    fn a_new_game_has_played_nothing_and_loads_the_map() {
        let g = game("sol", 4, 1);
        assert_eq!((g.steps, g.decided, g.round(), g.finished()), (0, 0, 1, false));
        assert!(g.core.loaded.has(Slice::Map), "the board is the core's");
        assert!(slices_for(true).has(Slice::Map) && !slices_for(false).has(Slice::Map));
        assert!(slices_for(false).has(Slice::Players), "only the map is left out");
        assert_eq!(g.scores().len(), 4);
        assert!(g.scores().iter().all(|(faction, vp)| !faction.is_empty() && *vp == 0));
    }

    #[test]
    fn the_scores_are_the_victory_points_and_the_fingerprint_is_sixteen_digits_that_follow_the_position() {
        let mut g = game("sol", 4, 1);
        let before = g.fingerprint();
        assert_eq!(before.len(), 16);
        g.core.players_mut().edit(2).econ.victory_points = 7;
        assert_eq!(g.scores()[2].1, 7);
        assert_ne!(g.fingerprint(), before, "a point is in the fingerprint");
        g.core.meta_mut().finished = true;
        assert!(g.finished());
        assert_eq!(g.step(), Step::Finished, "a finished game stops the loop");
    }

    #[test]
    fn the_budgets_end_a_game_and_count_the_steps_played() {
        let mut g = game("sol", 4, 1);
        let p = play_game(&mut g, 3, 50);
        assert_eq!((p.end, p.steps), (End::StepLimit, 3));
        assert_eq!(p.matrix.native.values().sum::<u64>(), 3, "every step played is counted once");
        assert_eq!(g.decided, g.events.len().min(g.decided), "the decisions are counted");
        let mut g = game("sol", 4, 1);
        assert_eq!(play_game(&mut g, 100, 0).end, End::RoundLimit, "round 1 is beyond a budget of none");
        let mut g = game("sol", 4, 1);
        assert!(matches!(play_game(&mut g, 100, 1).end, End::Gap(_)), "round 1 is within a budget of one");
        let mut g = game("sol", 4, 1);
        let p = play_game(&mut g, 100, 50);
        assert_eq!(p.steps, 54);
        assert!(g.decided >= p.matrix.native.get("ask.draft").copied().unwrap_or(0) as usize - 1 + p.matrix.native.get("ask.starting_tech").copied().unwrap_or(0) as usize, "the last card of the draft is not asked; the menu and the actions add their answers");
        // The shadow and the survey keep the same budgets.
        let s = shadow_play(&lobby_spec("sol", 4, 1), 3, 50).unwrap();
        assert_eq!((s.played.end, s.played.steps), (End::StepLimit, 3));
        assert_eq!(shadow_play(&lobby_spec("sol", 4, 1), 100, 0).unwrap().played.end, End::RoundLimit);
        assert_eq!(survey(&lobby_spec("sol", 4, 1), 5, 50).unwrap().steps, 5);
        assert!(survey(&lobby_spec("sol", 4, 1), 100, 1).unwrap().steps > 5, "round 1 is within a budget of one");
        assert_eq!(survey(&lobby_spec("sol", 4, 1), 100, 0).unwrap().steps, 0);
    }

    #[test]
    fn the_bot_is_the_rule_bots_random_pick_of_the_seat_place_and_the_decision_index() {
        let g = game("sol", 4, 1);
        let render = Render::new(g.cx.owners.ids().to_vec());
        let core = turn::draft::question(&g.core).expect("the draft is asked");
        let asking = ti4_engine_core::driver::live::Asking { choice: core.clone(), nested: None, at: None };
        let rendered = live::render_asking(&live::Cx::new(&render, &live::env::env_with_plays(&render, &g.core, g.kernel, g.seeds.clone(), g.sources)), &g.core, &asking);
        let seats = g.seats.clone();
        for place in 0..seats.len() {
            let mut choice = rendered.clone();
            choice.player = ti4_model::id::PlayerId::new(&seats[place]);
            for index in 0..6 {
                let mut bot = Bot { seed: 9, seats: &seats, index, asked: Vec::new() };
                let id = bot.ask(&Question { asking: &asking, rendered: choice.clone() }).unwrap();
                let (picked, _, _) = ti4_rule_bot::choose_explained(&choice, None, 9, place, index);
                assert_eq!(id, choice.options[picked].id, "place {place}, index {index}");
                assert_eq!(bot.index, index + 1, "one decision more");
                assert_eq!(bot.asked.len(), 1);
                assert_eq!((bot.asked[0].chosen.as_str(), bot.asked[0].player.as_str()), (id.as_str(), seats[place].as_str()));
            }
        }
        let mut empty = rendered.clone();
        empty.options.clear();
        let mut bot = Bot { seed: 9, seats: &seats, index: 0, asked: Vec::new() };
        assert_eq!(bot.ask(&Question { asking: &asking, rendered: empty }), Err(Stop::Blocked), "a question without options is not answered");
    }

    /// Fleet Logistics' second action is a free option of the closing menu: since lane F the core assembles it (`driver::menu::closing_menu`), so the
    /// step is the question itself (it was the counted gap `MenuSources` before; the gap no longer exists).
    #[test]
    fn a_closing_menu_with_a_second_action_is_asked_natively_and_offers_the_end_of_the_turn_and_the_action() {
        use ti4_engine_core::generated::Technology;
        let mut g = game("sol", 4, 1);
        g.core.transient_mut().misc.starting_tech_picks.clear();
        let turn_seq = g.core.meta().turn_seq;
        g.core.meta_mut().phase = ti4_engine_core::state::Phase::Action;
        g.core.meta_mut().active = Some(Seat(0));
        g.core.players_mut().edit(0).tech.owned.insert(Technology::FleetLogistics);
        // A card to act with: the second action is offered ("end your turn" first), as lane F's closing menu assembles it.
        g.core.players_mut().edit(0).strat.push(ti4_engine_core::generated::StrategyCard::ALL[0]);
        g.core.pending_mut().prepared_turn_seq = Some(turn_seq);
        g.core.pending_mut().turn_closing = Some(Seat(0));
        g.core.pending_mut().actions_this_turn = 1;
        let Step::Played(played) = g.step() else { panic!("the closing menu with a second action is a native question") };
        assert_eq!(played.source, Some(AskArm::Closing));
        let menu = &played.asked[0];
        assert!(menu.options.len() >= 2, "the end of the turn and the second action: {menu:?}");
        assert!(menu.options.iter().any(|o| o == "end_turn" || o.contains("end")), "the end of the turn is offered: {menu:?}");
    }

    #[test]
    fn a_pending_window_of_any_kind_means_the_table_is_asked() {
        for kind in [WindowKind::LeaderFollowers, WindowKind::EventScoring, WindowKind::Scoring, WindowKind::Tokens, WindowKind::Voting, WindowKind::Diplomacy, WindowKind::Aftermath, WindowKind::AgendaTalks, WindowKind::Tactical] {
            let mut g = game("sol", 4, 1);
            g.core.transient_mut().misc.starting_tech_picks.clear();
            g.core.meta_mut().phase = ti4_engine_core::state::Phase::Status;
            g.core.pending_mut().open(ti4_engine_core::state::Window::Untyped { kind, actor: None, data: [0; 3] });
            assert!(g.asks(), "{kind:?}");
        }
        let mut g = game("sol", 4, 1);
        g.core.transient_mut().misc.starting_tech_picks.clear();
        g.core.meta_mut().phase = ti4_engine_core::state::Phase::Action;
        g.core.meta_mut().active = None;
        assert!(!g.asks(), "nobody in turn: the phase moves on");
        g.core.meta_mut().active = Some(Seat(0));
        assert!(g.asks());
        let g = game("sol", 4, 1);
        let picks = !g.core.transient().misc.starting_tech_picks.is_empty();
        assert_eq!(live_ask_arm(&Facts::of_core(&g.core, false)) == AskArm::StartingTech, picks);
        if picks {
            assert!(g.asks(), "a pick is asked");
        }
    }

    /// `compare_group` finds a perturbed event, question or position, and compares only the prefix where the arm's close is the string's.
    #[test]
    fn the_shadow_compare_finds_each_kind_of_difference_and_compares_a_prefix_where_the_close_is_the_strings() {
        let setup = || {
            let spec = lobby_spec("sol", 4, 1);
            let legacy = MaskSubject::<LegacyMask>::new(&spec, Pilot::Script).unwrap();
            let mut native = NativeGame::from_game(legacy.game(), spec.seed).unwrap();
            let base = legacy.game().state.clone();
            let mut group: Vec<Played> = Vec::new();
            loop {
                let Step::Played(played) = native.step() else { panic!("the first steps are played") };
                let is_glued = glued(played.arm);
                group.push(played);
                if !is_glued {
                    break;
                }
            }
            let last = group.last().unwrap();
            assert_eq!(last.arm, Arm::Ask);
            assert!(!last.events.is_empty() && last.asked.len() == 1, "{last:?}");
            (legacy, native, base, group)
        };
        let (mut legacy, native, base, group) = setup();
        assert_eq!(compare_group(&mut legacy, &native, &base, &group), None, "untouched, native and legacy agree");
        let (mut legacy, native, base, mut group) = setup();
        group.last_mut().unwrap().events.push("INVENTED".to_owned());
        let d = compare_group(&mut legacy, &native, &base, &group).expect("an extra event");
        assert_eq!(d.what, "events");
        let (mut legacy, native, base, mut group) = setup();
        group.last_mut().unwrap().events[0] = "OTHER".to_owned();
        assert_eq!(compare_group(&mut legacy, &native, &base, &group).map(|d| (d.what, d.native)), Some(("events", "OTHER".to_owned())));
        let (mut legacy, native, base, mut group) = setup();
        group.last_mut().unwrap().asked[0].prompt = "another prompt".to_owned();
        assert_eq!(compare_group(&mut legacy, &native, &base, &group).map(|d| d.what), Some("questions"));
        let (mut legacy, mut native, base, group) = setup();
        native.core.players_mut().edit(0).econ.trade_goods += 1;
        let d = compare_group(&mut legacy, &native, &base, &group).expect("a perturbed position");
        assert_eq!(d.what, "state");
        assert!(d.path.contains("trade_goods"), "{d}");
        // An arm whose close is the string's: the position is not compared, and a legacy step that went on asks and pushes more than the arm did.
        let (mut legacy, mut native, base, mut group) = setup();
        native.core.players_mut().edit(0).econ.trade_goods += 1;
        group.last_mut().unwrap().arm = Arm::Tokens;
        group.last_mut().unwrap().events.clear();
        assert_eq!(compare_group(&mut legacy, &native, &base, &group), None, "only the (empty) prefix of the events is compared, and no position");
        let (mut legacy, native, base, mut group) = setup();
        group.last_mut().unwrap().events.clear();
        assert_eq!(compare_group(&mut legacy, &native, &base, &group).map(|d| d.what), Some("events"), "a whole step must push what the legacy step pushed");
    }

    /// A step that ends where the string's close goes on may leave either side ahead: the events one side pushed beyond the other's end of step are
    /// matched by the other's next step (found by `native_play --shadow`: the native secondary arm closes the whole strategic action in the step the
    /// string takes two for; the string's tactical step runs the aftermath up to its first question where the native steps split there).
    #[test]
    fn events_one_side_is_ahead_by_are_owed_to_the_next_group_and_a_strict_step_must_end_level() {
        let (mut legacy, native, base, mut group) = {
            let spec = lobby_spec("sol", 4, 1);
            let mut legacy = MaskSubject::<LegacyMask>::new(&spec, Pilot::Script).unwrap();
            let mut native = NativeGame::from_game(legacy.game(), spec.seed).unwrap();
            let base = legacy.game().state.clone();
            let mut group: Vec<Played> = Vec::new();
            loop {
                let Step::Played(played) = native.step() else { panic!("the first steps are played") };
                let is_glued = glued(played.arm);
                group.push(played);
                if !is_glued {
                    break;
                }
            }
            (legacy, native, base, group)
        };
        group.last_mut().unwrap().arm = Arm::Tokens;
        group.last_mut().unwrap().events.push("INVENTED".to_owned());
        let run = run_legacy(&mut legacy, &group, &[]);
        let mut owed = Owed::default();
        assert_eq!(compare_run(&mut legacy, &native, &base, &group, &run, &mut owed), None, "the native step is ahead: not a difference yet");
        assert_eq!((owed.native.as_slice(), owed.legacy.len()), (["INVENTED".to_owned()].as_slice(), 0));
        // The next group's legacy events must open with it.
        let mut next = Owed { native: vec!["INVENTED".to_owned()], legacy: Vec::new() };
        let nothing: Vec<Played> = vec![Played { arm: Arm::Tokens, source: None, asked: Vec::new(), events: Vec::new(), finished: false }];
        let run = LegacyRun { info: run.info, records: Vec::new(), events: vec!["OTHER".to_owned()] };
        let d = compare_run(&mut legacy, &native, &base, &nothing, &run, &mut next).expect("the string pushed another line than the one owed");
        assert_eq!((d.what, d.native.as_str(), d.legacy.as_str()), ("events", "INVENTED", "OTHER"));
    }

    /// A question the string puts INSIDE a step (the reward of an exploration after a first claim) is answered in the legacy step that raises it: the
    /// shadow gives the legacy game the answers of the next native steps and the group grows by them. Before: "script wanted \"gain\"".
    #[test]
    fn a_question_the_string_asks_inside_a_step_is_answered_in_that_legacy_step() {
        let s = shadow_play(&lobby_spec("sol", 4, 1), 26, 50).unwrap();
        assert_eq!(s.first_difference, None, "{:?}", s.first_difference);
        assert!(s.played.steps >= 25, "the first claim of the quick game is played through the Local Fabricators question: {}", s.played.steps);
    }

    /// The words of an exploration's log row are minted straight into the state's code table; the live loop must not overwrite that table with the
    /// render's older copy at the end of the step (found by `native_play --shadow`: "native Explore: unknown text code" at the first strict compare after
    /// the first claim of the quick game).
    #[test]
    fn the_words_of_an_exploration_row_survive_the_step_and_the_position_materializes() {
        let mut g = game("sol", 4, 1);
        for _ in 0..26 {
            assert!(matches!(g.step(), Step::Played(_)));
        }
        assert!(!g.core.explore().log.is_empty(), "the first claim explored a planet");
        assert!(g.materialized().is_ok(), "{:?}", g.materialized().err());
    }

    #[test]
    fn only_a_step_the_core_does_not_own_whole_is_compared_on_a_prefix() {
        assert_eq!(legacy_prefix(true, vec![1, 2, 3], 1), [1, 2, 3], "a whole step: all of it");
        assert_eq!(legacy_prefix(false, vec![1, 2, 3], 1), [1], "the legacy step went on: the prefix");
        assert_eq!(legacy_prefix(false, vec![1, 2, 3], 3), [1, 2, 3]);
        assert_eq!(legacy_prefix(false, vec![1], 3), [1], "the legacy step did less than the arm: a difference to report, nothing to cut");
    }

    #[test]
    fn the_round_the_asks_of_a_job_and_the_shadow_count_are_the_positions() {
        let mut g = game("sol", 4, 1);
        g.core.meta_mut().round = 3;
        assert_eq!(g.round(), 3);
        let mut g = game("sol", 4, 1);
        g.core.transient_mut().misc.starting_tech_picks.clear();
        g.core.meta_mut().phase = ti4_engine_core::state::Phase::Status;
        assert!(!g.asks());
        turn::open_job(&mut g.core, TurnJob::Phase);
        assert!(g.asks(), "an open job is waiting on its step: the arm is Ask");
        let s = shadow_play(&lobby_spec("sol", 4, 1), 100, 50).unwrap();
        assert_eq!((s.compared, s.played.steps), (36, 54), "fifty-four native steps; the glued ones, the ones that ask nothing while the legacy game stands at a question and the ones a lookahead groups with the step before are not legacy steps of their own");
        assert!(matches!(shadow_play(&lobby_spec("sol", 4, 1), 100, 1).unwrap().played.end, End::Gap(_)), "round 1 is within a budget of one");
        let survey = survey(&lobby_spec("sol", 4, 1), 400, 50).unwrap();
        assert!(survey.steps_by_arm.keys().all(|(label, _)| !label.contains('>') || label.starts_with("turn_prelude>") || label.starts_with("strategy_announce>")), "{:?}", survey.steps_by_arm);
    }

    #[test]
    fn the_matrix_counts_defects_apart_and_prints_them() {
        let mut m = Matrix::default();
        let d = Defect { arm: "tactical".to_owned(), what: "Illegal".to_owned() };
        m.note_defect(&d);
        m.note_defect(&d);
        assert_eq!(m.defects["tactical: Illegal"], 2);
        let mut other = Matrix::default();
        other.merge(&m);
        other.merge(&m);
        assert_eq!(other.defects["tactical: Illegal"], 4);
        assert!(other.render(3).iter().any(|l| l.contains("DEFECTS")));
        assert!(!Matrix::default().render(3).iter().any(|l| l.contains("DEFECTS")), "no defect, no section");
    }

    #[test]
    fn the_table_is_asked_only_where_the_string_asks() {
        let mut g = game("sol", 4, 1);
        // Setup: sol has no starting pick but others at the table may; either way a question exists (a pick or the draft).
        assert!(g.asks());
        let facts = Facts::of_core(&g.core, false);
        assert!(matches!(live_ask_arm(&facts), AskArm::Draft | AskArm::StartingTech));
        // Everyone passed in the action phase: nothing to ask (the phase moves on).
        g.core.meta_mut().phase = ti4_engine_core::state::Phase::Action;
        g.core.transient_mut().misc.starting_tech_picks.clear();
        for i in 0..4 {
            g.core.players_mut().edit(i).econ.passed = true;
        }
        assert!(!g.asks(), "all passed: no question");
        g.core.players_mut().edit(1).econ.passed = false;
        g.core.meta_mut().active = Some(Seat(1));
        assert!(g.asks(), "a seat has not passed: the menu");
        g.core.meta_mut().phase = ti4_engine_core::state::Phase::Status;
        assert!(!g.asks(), "the status phase asks nothing of its own");
    }

    /// The native loop and the legacy game agree step by step up to the gap: the same questions, the same plain events, the same position. (Integration 11
    /// pinned a KNOWN difference here, the Local Fabricators reward of step 24; lane F2c showed it was the harness' (the string asks it inside the legacy step
    /// that answers the second commit) and fixed the shadow, so the pin fired as it was written to and is replaced by this test.)
    #[test]
    fn the_shadow_run_agrees_with_the_legacy_game_up_to_the_gap() {
        let shadowed = shadow_play(&lobby_spec("sol", 4, 1), 2_000, 50).expect("builds");
        assert_eq!(shadowed.first_difference, None);
        assert!(shadowed.compared >= 8, "the draft was compared step by step: {}", shadowed.compared);
        assert!(matches!(shadowed.played.end, End::Gap(_)));
    }

    #[test]
    fn the_survey_counts_every_step_of_a_whole_legacy_game_by_the_arm_the_native_loop_needs() {
        let s = survey(&lobby_spec("sol", 4, 1), 400, 50).expect("builds");
        let native = |n: bool| s.steps_by_arm.iter().filter(|((_, native), _)| *native == n).map(|(_, c)| c).sum::<u64>();
        assert_eq!(native(true) + native(false), s.steps as u64, "every step is classified once");
        assert!(s.steps_by_arm.contains_key(&("ask.draft".to_owned(), true)), "{:?}", s.steps_by_arm);
        assert!(s.steps_by_arm.keys().any(|(arm, native)| arm == "turn_prelude>ask.action_menu" && !native), "the prelude and the menu are one legacy step: {:?}", s.steps_by_arm);
        assert!(s.steps_by_arm.keys().any(|(arm, native)| arm == "strategy_announce>ask.draft" && *native), "{:?}", s.steps_by_arm);
    }

    #[test]
    fn the_matrix_names_the_islands_that_were_reached_and_the_ones_that_were_not() {
        let mut m = Matrix::default();
        m.note_gap(&Gap { arm: "ask.action_menu".to_owned(), kind: GapKind::AskSource, cause: "question source action_menu".to_owned() });
        m.note_gap(&Gap { arm: "ask.action_menu".to_owned(), kind: GapKind::AskSource, cause: "question source action_menu".to_owned() });
        *m.native.entry("ask.draft".to_owned()).or_default() += 5;
        let lines = m.render(3);
        let row = |island: Island| lines.iter().find(|l| l.contains(island.name())).cloned().unwrap_or_default();
        assert!(row(Island::Menu).contains("1 cause(s)") && row(Island::Menu).trim_start().starts_with('0'), "{lines:#?}");
        assert!(row(Island::TurnFlow).contains("green") && !row(Island::TurnFlow).contains("not reached"), "5 native steps: green and reached: {lines:#?}");
        assert!(row(Island::Trade).contains("green, not reached"), "{lines:#?}");
        assert_eq!(m.total_gaps(), 2);
        let mut other = Matrix::default();
        other.merge(&m);
        other.merge(&m);
        assert_eq!(other.total_gaps(), 4);
        assert_eq!(other.native["ask.draft"], 10);
    }

    #[test]
    fn a_live_bail_is_a_gap_and_anything_else_is_a_defect() {
        use ti4_engine_core::kernel::ability::Unknown;
        let gap = stop_to_step("tactical", Stop::Live(LiveError::Unknown(Unknown::PlayFacts)));
        assert!(matches!(&gap, Step::Gap(g) if g.kind == GapKind::Bail && g.cause == "PlayFacts" && g.arm == "tactical"), "{gap:?}");
        for stop in [Stop::Illegal, Stop::Exhausted, Stop::Blocked, Stop::Live(LiveError::NotOpen)] {
            assert!(matches!(stop_to_step("x", stop), Step::Defect(_)));
        }
        assert!(matches!(stop_to_step("x", Stop::NoLiveFn(Arm::Status)), Step::Gap(g) if g.kind == GapKind::ArmNotNative && g.arm == "status"));
    }
}
