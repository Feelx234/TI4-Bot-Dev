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
/// No arm lends a close any more (lane E2): the token gain hands the status phase's steps 81.5-81.8 to `TurnJob::StatusFinish`, which the same call
/// runs (`driver::windows::status::advance_tokens`, KS5), and the games of the sweep play on into the agenda phase. The arms of the action
/// (`Tactical`, `Aftermath`) hand the turn on through [`turn::open_finish`], which the loop opens itself ([`after_arm`]).
#[must_use]
pub const fn lent_close(_arm: Arm) -> Option<&'static str> {
    None
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
    /// The last step was a glued one that finished (`glued`: the turn prelude, the strategy announcement; a finished aftermath and the tail of the
    /// tactical action after it): the string runs it together with the next step, whose reconcile hooks are not run (the events a prelude question or
    /// a retreat staged are announced at the start of the string's NEXT step).
    after_glue: bool,
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
        // The string applies the laws and the printed lanes (rules version 22) to its galaxy at every step's reconcile (`laws::apply_to_galaxy`), which a
        // game that was not stepped yet has not done: the core's map would follow no lane (a lane tile a neighbour, no system joined across it).
        let galaxy = game.galaxy().cloned().map(|mut galaxy| {
            ti4_engine::laws::apply_to_galaxy(&game.state, &mut galaxy);
            galaxy
        });
        let core = Codec::load(&mut cx, &game.state, Sources { galaxy: galaxy.as_ref(), rng: Some(&positions) }, slices_for(true))
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
            after_glue: false,
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
            after_glue: self.after_glue,
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
        let after_glue = self.after_glue;
        let (codes, factions) = (self.render.codes_snapshot(), self.render.factions.clone());
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.step_inner()));
        let result = result.unwrap_or_else(|p| {
            let text = p.downcast_ref::<String>().cloned().or_else(|| p.downcast_ref::<&str>().map(|s| (*s).to_owned())).unwrap_or_default();
            Step::Defect(Defect { arm: "?".to_owned(), what: format!("PANIC: {text}") })
        });
        if matches!(result, Step::Gap(_) | Step::Defect(_)) {
            self.after_glue = after_glue;
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
        if !self.quiet && std::env::var_os("TI4_NATIVE_PLAY_TRACE").is_some() && !self.core.pending().staged.is_empty() {
            eprintln!("  staged at the start of the step (after glue {}): {:?}", self.after_glue, self.core.pending().staged.iter().map(|e| e.kind).collect::<Vec<_>>());
        }
        // 1. The reconcile hooks every step starts with (not the second half of a glued pair: the string's step runs on from the prelude).
        for _ in 0..8 {  // mutants:skip (the reconcile job is one window; the bound is defensive)
            if self.after_glue {
                break;
            }
            // A job the step before left open (the end of a turn a hook ended) runs first: `advance_job` runs the job that is open instead of opening the
            // reconcile. The reconcile follows it, and the events it announces come behind that job's (the string's next step starts after the turn ended).
            let left_open = is_open::<TurnFlowWin>(&self.core);
            let mut lone = live::LoneCard(&mut bot);
            match live::live_reconcile(&mut self.core, &lcx, &mut lone) {
                Ok(step) if step.finished && !left_open => break,  // mutants:skip (the reconcile job is one window; the loop ends on its first call)
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
        if !self.quiet && std::env::var_os("TI4_NATIVE_PLAY_TRACE").is_some() && !self.core.pending().staged.is_empty() {
            eprintln!("  staged at the end of the step: {:?}", self.core.pending().staged.iter().map(|e| e.kind).collect::<Vec<_>>());
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
        let one = self.render.sync_codes(&mut self.core.texts.codes);
        debug_assert!(one, "the renderer's code table continues the position's (a text minted into one and not the other would name two things)");
        self.steps += 1;
        // The string's step goes on past a finished aftermath into the tail of the tactical action (and the end of the action) up to its next question.
        // (And past the hand-off of a tactical action to its aftermath, when no movement question stands between them: a Ceasefire denies the movement and the
        // string's step runs from the activation through the aftermath to the closing menu; the events the activation staged wait for the step after.)
        let handed_off = arm == Arm::Tactical && self.core.pending().window(ti4_engine_core::state::WindowKind::Aftermath).is_some();
        // (Not past a scoring pause: the string's step ends at the pause, and the scoring step after it announces what was staged first.)
        let parked_for_scoring = self.core.pending().window(ti4_engine_core::state::WindowKind::EventScoring).is_some();
        // (And past a question of the aftermath that the string's step does not stop at: the removals over the capacity after a retreat are native steps of their
        // own, the string's step goes on through them; what the retreat staged is announced by the step after the whole action, not by the next removal.)
        let mid_aftermath = arm == Arm::Aftermath && !out.finished && !asked.is_empty();
        self.after_glue = !parked_for_scoring && (mid_aftermath || (out.finished && (glued(arm) || arm == Arm::Aftermath || handed_off || (self.after_glue && arm == Arm::Tactical && asked.is_empty()))));
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
    /// The difference is a string bug the core does not copy, recorded in `docs/STRING_ENGINE_BUGS.md` (`Some("known string bug S1")`): the
    /// game cannot go on past it (the script of the core's answers meets the string's other question), so it is reported on its own and does not
    /// fail the run as a difference of the core.
    pub known: Option<&'static str>,
}

impl std::fmt::Display for Difference {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "step {} [{}] {} {}: native {} | legacy {}", self.step, self.arms.join("+"), self.what, self.path, self.native, self.legacy)?;
        if let Some(known) = self.known {
            write!(f, " ({known})")?;
        }
        Ok(())
    }
}

/// The label of a difference that is the string's S1.
pub const KNOWN_S1: &str = "known string bug S1";

/// The label of a difference that is the string's S2 (the windows of the flip into a strategy phase are ordered by the stale context).
pub const KNOWN_S2: &str = "known string bug S2";

/// S1 (`docs/STRING_ENGINE_BUGS.md`): the order the string offers an action-phase reaction window in (its seating order, rotated from the active seat,
/// fixed at `Game::new`) against the order of the ruling (the active seat first, then the others in the initiative order of the moment).
fn s1_orders_differ(state: &ti4_model::state::GameState) -> bool {
    s1_orders(state).is_some_and(|(string, rule)| string != rule)
}

/// The two orders of S1 (the string's, the ruling's); `None` where both engines order from the speaker.
fn s1_orders(state: &ti4_model::state::GameState) -> Option<(Vec<ti4_model::id::PlayerId>, Vec<ti4_model::id::PlayerId>)> {
    // Strategy and agenda windows are ordered from the speaker, clockwise, by both engines.
    if state.phase.uses_speaker_order() {
        return None;
    }
    // Every other window (the action phase, and the status phase, where nobody is active: LRR 1.19, initiative order): the string's list is the seating order
    // of `Game::new`, rotated to start at the active seat when there is one; the ruling is the active seat first, then the initiative order of the moment.
    Some(match &state.active {
        Some(active) => (state.clockwise_from(active), std::iter::once(active.clone()).chain(state.initiative_order().into_iter().filter(|p| p != active)).collect()),
        None => (state.seating_order.clone(), state.initiative_order()),
    })
}

/// The id of an ability a window can offer (`reaction:..`, `leader:..`, `ability:..`, `technology:..`, `unit:..`, `promissory:..`, `breakthrough:..`).
fn is_reaction_id(id: &str) -> bool {
    ["reaction:", "leader:", "ability:", "technology:", "unit:", "promissory:", "breakthrough:"].iter().any(|p| id.starts_with(p))
}

/// S1 shows as a legacy error when the string offers a reaction of another seat than the script wanted: `script wanted "<reaction>" but <seat> was
/// offered ["<reaction>", ..]`.
fn s1_in_error(state: &ti4_model::state::GameState, error: &str) -> bool {
    order_in_error(s1_orders_differ(state), error)
}

fn order_in_error(differ: bool, error: &str) -> bool {
    let Some(wanted) = error.split("script wanted \"").nth(1).and_then(|r| r.split('"').next()) else { return false };
    let Some(offered) = error.split(" was offered ").nth(1) else { return false };
    differ && is_reaction_id(wanted) && offered.trim_start_matches('[').trim_start_matches('"').split('"').next().is_some_and(is_reaction_id)
}

/// S1 shows in a question of the same prompt asked of another seat, or with the same options in another order.
fn s1_in_questions(state: &ti4_model::state::GameState, path: &str, native: &[Value], legacy: &[Value]) -> bool {
    order_in_questions(s1_orders_differ(state), path, native, legacy) || s1_other_seat_first(state, path, native, legacy)
}

/// S1 shows as a question of ANOTHER prompt too: the window of one event holds an ability of seat X and an ability of seat Y, the core asks X first (the
/// ruling) and the string Y first (its seating order), and the string's question of Y is the ability's own prompt (a single optional ability asks its
/// own question, not the window's offer). The core's question is an ability offer of X; the string's question at the same place is a question of Y; the
/// string's order puts Y before X and the ruling's X before Y.
fn s1_other_seat_first(state: &ti4_model::state::GameState, path: &str, native: &[Value], legacy: &[Value]) -> bool {
    let Some(at) = path.strip_prefix('[').and_then(|p| p.split(']').next()).and_then(|i| i.parse::<usize>().ok()) else { return false };
    let (Some(n), Some(l)) = (native.get(at), legacy.get(at)) else { return false };
    let (Some(x), Some(y)) = (n["player"].as_str(), l["player"].as_str()) else { return false };
    if x == y || !n["options"].as_array().is_some_and(|o| o.iter().filter_map(Value::as_str).any(is_reaction_id)) {
        return false;
    }
    let Some((string, rule)) = s1_orders(state) else { return false };
    let place = |order: &[ti4_model::id::PlayerId], who: &str| order.iter().position(|p| p.as_str() == who);
    matches!((place(&string, x), place(&string, y), place(&rule, x), place(&rule, y)), (Some(sx), Some(sy), Some(rx), Some(ry)) if sy < sx && rx < ry)
}

fn order_in_questions(differ: bool, path: &str, native: &[Value], legacy: &[Value]) -> bool {
    let Some(at) = path.strip_prefix('[').and_then(|p| p.split(']').next()).and_then(|i| i.parse::<usize>().ok()) else { return false };
    let (Some(n), Some(l)) = (native.get(at), legacy.get(at)) else { return false };
    if !differ || n["prompt"] != l["prompt"] || !l["options"].as_array().is_some_and(|o| o.iter().filter_map(Value::as_str).any(is_reaction_id)) {
        return false;
    }
    let sorted = |v: &Value| -> Vec<String> {
        let mut o: Vec<String> = v.as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect()).unwrap_or_default();
        o.sort();
        o
    };
    n["player"] != l["player"] || sorted(&n["options"]) == sorted(&l["options"])
}

/// S2 (`docs/STRING_ENGINE_BUGS.md`): the string opens the windows of the flip into a strategy phase of round 2 and later (`STRATEGY_PHASE_BEGAN`, and a
/// card played in it) before it syncs its timing context: the resolver still orders by the phase before (the seating list of `Game::new`, nobody active),
/// not from the speaker. The ruling's order is the speaker's, clockwise.
fn s2_orders_differ(state: &ti4_model::state::GameState) -> bool {
    state.seating_order != state.clockwise_from(&state.speaker)
}

/// Whether the group of arms is a phase flip (the `phase` step, alone or with the draft's first picks behind it).
fn flips_a_phase(arms: &[String]) -> bool {
    arms.iter().any(|a| a == "phase")
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
    // The step in `carry` was held back once so the group before it is compared first.
    let mut flush_first = false;
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
                if std::env::var_os("TI4_NATIVE_PLAY_TRACE").is_some_and(|v| v == "3") {
                    eprintln!("  PHASE after native step {}: native {:?} legacy {:?}", native.steps, native.materialized_over(&base).ok().map(|v| v["combat_presentation"]["phase"].clone()), base.combat_presentation.phase);
                }
                // A step that asks nothing and flips the phase (`Arm::Phase`) never joins the questions of a group the legacy step has used up: the string
                // ends its step at the close of the status phase (after its last answer) and flips the next phase in a step of its own.
                if opens_a_step_of_its_own(&p, &group, flush_first) {
                    carry = Some(Step::Played(p));
                    flush_first = true; // mutants:skip (a defensive bound: a held-back step is never held back twice)
                } else {
                    flush_first = false;
                    matrix.note_played(&p);
                    let arm = p.arm;
                    group.push(p);
                    if glued(arm) {
                        continue;
                    }
                }
                // A native step that asks nothing while the legacy game already stands at a question: the string ran that stretch in an earlier step
                // (its events were owed to this one); there is no legacy step to compare it with, its events are owed to the next.
                if group.iter().all(|p| p.asked.is_empty()) && !owed.legacy.is_empty() && legacy.options().is_some() {
                    let arms: Vec<String> = group.iter().map(|p| p.source.map_or_else(|| arms::name(p.arm).to_owned(), |s| format!("ask.{}", ask_name(s)))).collect();
                    let events: Vec<String> = group.iter().flat_map(|p| p.events.iter().cloned()).collect();
                    if std::env::var_os("TI4_NATIVE_PLAY_TRACE").is_some() {
                        eprintln!("  native steps with no legacy step of their own: {arms:?} events {events:?} against the owed legacy events {:?}", owed.legacy);
                    }
                    if let Some((at, n, l)) = owed.settle(events, Vec::new()) {
                        first_difference = Some(Difference { step: native.steps, arms, what: "events", path: format!("[{at}]"), native: n, legacy: l, known: None });
                        break End::Finished;
                    }
                    group.clear();
                    continue;
                }
                compared += 1;
                // One legacy step can settle questions the native loop asks in later steps (a question the string puts INSIDE a step: an
                // exploration's reward after a first claim, the rewards of a card played in a window): the legacy game gets the answers of
                // the next native steps too (played on a fork), and the group grows by the native steps whose answers the legacy used.
                let ahead = answers_ahead(&native, ahead_for(&group));
                let mut run = run_legacy(&mut legacy, &group, &ahead);
                // The native steps ran on where the string takes a step of its own with no question (the close of an action): that step uses no
                // answer; its events meet what the native pushed ahead, and the legacy game is stepped again with the same answers.
                let mut catch_up = 0; // mutants:skip (the counter only feeds the defensive bound)
                // (Or the string took a step that did and said nothing -- the scoring window of an event nobody scores, ahead of the phase flip --
                // where the native loop has a step of its own: the answers are still unused.)
                // (Or the string stopped after the events that open an arm and asked nothing, where the native step puts the question right behind them: the
                // retreat announcement after `RETREAT_STEP_STARTED`. Those events are the head of what the native steps pushed; the legacy game goes on.)
                while run.records.is_empty() && (!owed.native.is_empty() || run.events.is_empty() || events_lead(&owed.native, &group, &run.events)) && run.info.error.is_none() && (!group.iter().all(|p| p.asked.is_empty()) || steps_idle(&run, &group)) && catch_up_allowed(catch_up) {
                    let arms: Vec<String> = group.iter().map(|p| p.source.map_or_else(|| arms::name(p.arm).to_owned(), |s| format!("ask.{}", ask_name(s)))).collect();
                    let _ = (legacy.decisions_since(), legacy.events_since());
                    if let Some((at, n, l)) = owed.settle(Vec::new(), run.events.clone()) {
                        first_difference = Some(Difference { step: native.steps, arms, what: "events", path: format!("[{at}]"), native: n, legacy: l, known: None });
                        break;
                    }
                    run = run_legacy(&mut legacy, &group, &ahead);
                    catch_up += 1; // mutants:skip (the counter only feeds the defensive bound)
                }
                if first_difference.is_some() {
                    break End::Finished;
                }
                let mut used = run.records.len();
                let mut have: usize = group.iter().map(|p| p.asked.len()).sum();
                while have < used {
                    match carry.take().unwrap_or_else(|| native.step()) {
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
                // The native steps asked more than the legacy step did, and the extra questions are the last ones of the group: a glued native step (the
                // string ends its step after a seat's last question of the action and asks the first follower in the NEXT step). They open the next group,
                // which the next legacy step must open with the same questions (a question the string never asks is a difference there, at the first
                // question of that step). A step cut in two keeps its events with the later half (the events a native step pushes after its last answer): the head is compared as a prefix, the surplus of the legacy events is owed to the later half.
                if std::env::var_os("TI4_NATIVE_PLAY_TRACE").is_some() {
                    eprintln!("  group has {have} questions, the legacy step used {used}, carry {}", carry.is_some());
                }
                let mut tail = Vec::new();
                if have > used && used > 0 && run.info.error.is_none() {
                    let mut before = 0;
                    let cut = group.iter().enumerate().find_map(|(k, p)| {
                        let at = before;
                        before += p.asked.len();
                        (!p.asked.is_empty() && at <= used && used < before).then_some((k, used - at))
                    });
                    if let Some((k, m)) = cut {
                        tail = group.split_off(k);
                        if m > 0 {
                            let mut head = tail[0].clone();
                            head.asked.truncate(m);
                            head.events.clear();
                            tail[0].asked.drain(..m);
                            group.push(head);
                        }
                    }
                }
                if let Some(d) = compare_run(&mut legacy, &native, &base, &group, &run, &mut owed, !tail.is_empty() || carry.is_some()) {
                    first_difference = Some(d);
                    break End::Finished;
                }
                group = tail;
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

/// Whether the native step `p` (just played) must not join `group`: it flips the phase (or makes the forced last pick of the draft) without a
/// question while the group still holds answers the legacy step takes (the string's step ends at the close of the status phase / after the
/// pick that was asked, the next phase begins / the forced pick is made in the following step). Held back
/// once (`held`), so the group is compared first and `p` opens the next.
fn opens_a_step_of_its_own(p: &Played, group: &[Played], held: bool) -> bool {
    !held && (p.arm == Arm::Phase || p.source == Some(AskArm::Draft)) && p.asked.is_empty() && group.iter().any(|q| !q.asked.is_empty())
}

/// The events of a legacy step that asked nothing are the first events of what the native steps of `group` pushed (after the ones owed): the string
/// ended its step early, and the question the native step asks follows in the string's next one.
fn events_lead(owed_native: &[String], group: &[Played], legacy: &[String]) -> bool {
    let native: Vec<&String> = owed_native.iter().chain(group.iter().flat_map(|p| p.events.iter())).collect();
    !legacy.is_empty() && legacy.len() <= native.len() && legacy.iter().zip(native).all(|(l, n)| l == n)
}

/// The legacy step did and said nothing (no question, no event, no error) where the native steps of `group` pushed events and asked nothing: the
/// string's idle step ahead of the phase flip (the scoring window of an event nobody scores); the next legacy step is the one to compare.
fn steps_idle(run: &LegacyRun, group: &[Played]) -> bool {
    run.events.is_empty() && group.iter().any(|p| !p.events.is_empty())
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

/// The defensive bound of the catch-up loop of [`shadow_play`] (the string never takes more than a handful of steps that ask and say nothing in a row).
const fn catch_up_allowed(done: usize) -> bool { // mutants:skip (a bound that only guards a loop)
    done < 4
}

/// How many native steps the legacy game is given answers for beyond the group it is stepped with.
const AHEAD: usize = 4;

/// The same for a group that ends in a movement or aftermath step: the string's step runs on from a movement's `done_moving` through the removals of
/// the units over the fleet supply or the capacity (each one a native step of its own), the rest of the action and, in a Warfare action, the question after it.
/// The string refuses a step whose script runs dry and cannot be stepped again, so the answers are given up front.
const AHEAD_LONG: usize = 12;

fn ahead_for(group: &[Played]) -> usize {
    if group.last().is_some_and(|p| matches!(p.arm, Arm::Tactical | Arm::Aftermath)) { AHEAD_LONG } else { AHEAD }
}

/// The answers of the next `n` native steps, played on a fork (the native game itself does not move).
fn answers_ahead(native: &NativeGame, n: usize) -> Vec<String> {
    let mut fork = native.fork();
    let mut out = Vec::new();
    for _ in 0..n {
        match fork.step() {
            Step::Played(p) => out.extend(p.asked.iter().map(|a| a.chosen.clone())),
            other => {
                if std::env::var_os("TI4_NATIVE_PLAY_TRACE").is_some() {
                    eprintln!("  answers ahead stop at a step that is not a played one: {}", match &other { Step::Gap(g) => format!("gap {} {}", g.arm, g.cause), Step::Defect(d) => format!("defect {} {}", d.arm, d.what), _ => "finished".to_owned() });
                }
                break;
            }
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
    if std::env::var_os("TI4_NATIVE_PLAY_TRACE").is_some() {
        eprintln!("  legacy script: {:?} + ahead {:?}", group.iter().flat_map(|p| p.asked.iter().map(|a| a.chosen.as_str())).collect::<Vec<_>>(), ahead);
    }
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
#[cfg(test)]
fn compare_group(legacy: &mut MaskSubject<LegacyMask>, native: &NativeGame, base: &ti4_model::state::GameState, group: &[Played]) -> Option<Difference> {
    let run = run_legacy(legacy, group, &[]);
    compare_run(legacy, native, base, group, &run, &mut Owed::default(), false)
}

/// Compare the native steps of `group` with the legacy step that ran them.
/// `ahead`: native steps beyond this group already ran (they open the next group), so the native position is ahead of the legacy one and the state is
/// compared with the next group's, which holds them.
fn compare_run(legacy: &mut MaskSubject<LegacyMask>, native: &NativeGame, base: &ti4_model::state::GameState, group: &[Played], run: &LegacyRun, owed: &mut Owed, ahead: bool) -> Option<Difference> {
    let arms: Vec<String> = group.iter().map(|p| p.source.map_or_else(|| arms::name(p.arm).to_owned(), |s| format!("ask.{}", ask_name(s)))).collect();
    let diff = |what: &'static str, path: String, n: String, l: String| Difference { step: native.steps, arms: arms.clone(), what, path, native: n, legacy: l, known: None };
    let asked: Vec<&Asked> = group.iter().flat_map(|p| p.asked.iter()).collect();
    let (info, records, events) = (&run.info, &run.records, &run.events);
    if let Some(error) = &info.error {
        if std::env::var_os("TI4_NATIVE_PLAY_TRACE").is_some() {
            eprintln!("  legacy ERROR after answering {:?}: asked {:?}", asked.iter().map(|a| a.chosen.as_str()).collect::<Vec<_>>(), records.iter().map(|r| (r.player.as_str().to_owned(), r.prompt.clone(), r.chosen.clone())).collect::<Vec<_>>());
            eprintln!("  legacy offers: {:?}", records.iter().map(|r| &r.offered).collect::<Vec<_>>());
            eprintln!("  legacy events: {events:?}");
        }
        // For a closer look at a step the string refuses (`TI4_NATIVE_PLAY_DUMP=<dir>`): the legacy position before it and the core's after it.
        if let Some(dir) = std::env::var_os("TI4_NATIVE_PLAY_DUMP") {
            let dir = std::path::Path::new(&dir);
            if let Ok(json) = serde_json::to_string(base) {
                let _ = std::fs::write(dir.join(format!("base-{}.json", native.steps)), json);
            }
            if let Ok(after) = native.materialized_over(base) {
                let _ = std::fs::write(dir.join(format!("native-{}.json", native.steps)), after.to_string());
            }
            let _ = std::fs::write(dir.join(format!("legacy-{}.json", native.steps)), legacy.state_json().to_string());
        }
        if std::env::var_os("TI4_NATIVE_PLAY_TRACE").is_some() {
            let mut fork = native.fork();
            for _ in 0..3 {
                match fork.step() {
                    Step::Played(p) => eprintln!("  the native step after the group: {} asked {:?} events {:?}", p.source.map_or_else(|| arms::name(p.arm).to_owned(), |s| format!("ask.{}", ask_name(s))), p.asked.iter().map(|a| (&a.player, &a.prompt, &a.options)).collect::<Vec<_>>(), p.events),
                    other => {
                        eprintln!("  the native step after the group: {}", match &other { Step::Gap(g) => format!("gap {} {}", g.arm, g.cause), Step::Defect(d) => format!("defect {} {}", d.arm, d.what), _ => "finished".to_owned() });
                        break;
                    }
                }
            }
        }
        let mut d = diff("legacy", "step.error".to_owned(), "no error".to_owned(), error.clone());
        d.known = if s1_in_error(&legacy.game().state, error) {
            Some(KNOWN_S1)
        } else {
            (flips_a_phase(&d.arms) && order_in_error(s2_orders_differ(&legacy.game().state), error)).then_some(KNOWN_S2)
        };
        return Some(d);
    }
    if std::env::var_os("TI4_NATIVE_PLAY_TRACE").is_some() {
        eprintln!("  legacy: answered {:?} -> asked {:?}", asked.iter().map(|a| a.chosen.as_str()).collect::<Vec<_>>(), records.iter().map(|r| (r.player.as_str().to_owned(), r.prompt.clone(), r.chosen.clone())).collect::<Vec<_>>());
        eprintln!("  legacy offers: {:?}", records.iter().map(|r| &r.offered).collect::<Vec<_>>());
        eprintln!("  legacy events: {events:?}");
        eprintln!("  native events: {:?}", group.iter().flat_map(|p| p.events.iter()).collect::<Vec<_>>());
        eprintln!("  native group: {}", group.iter().map(|p| format!("{}[asked {} events {:?}]", p.source.map_or_else(|| arms::name(p.arm).to_owned(), |s| format!("ask.{}", ask_name(s))), p.asked.len(), p.events)).collect::<Vec<_>>().join(" + "));
        // The legacy position BEFORE the compared step, for a closer look (`TI4_NATIVE_PLAY_DUMP=<dir>`).
        if let Some(dir) = std::env::var_os("TI4_NATIVE_PLAY_DUMP") && let Ok(json) = serde_json::to_string(base) {
            let _ = std::fs::write(std::path::Path::new(&dir).join(format!("base-{}.json", native.steps)), json);
            if let Ok(after) = serde_json::to_string(&legacy.state_json()) {
                let _ = std::fs::write(std::path::Path::new(&dir).join(format!("after-{}.json", native.steps)), after);
            }
            if let Ok(after) = native.materialized_over(base) {
                let _ = std::fs::write(std::path::Path::new(&dir).join(format!("native-{}.json", native.steps)), after.to_string());
            }
        }
    }
    let _ = (legacy.decisions_since(), legacy.events_since());
    let last = group.last()?;
    let strict = strict(last.arm);
    // Questions: player, prompt and the option ids, in order. A group that ENDS an arm of the string's close compares the prefix.
    let n: Vec<Value> = asked.iter().map(|a| json!({ "player": a.player, "prompt": a.prompt, "options": a.options })).collect();
    let l: Vec<Value> = records.iter().map(|r| json!({ "player": r.player.as_str(), "prompt": r.prompt, "options": r.offered })).collect();
    let l = legacy_prefix(strict, l, n.len());
    if let Some(d) = crate::native_subject::first_leaf(&Value::Array(l.clone()), &Value::Array(n.clone())) {
        let known = if s1_in_questions(&legacy.game().state, &d.path, &n, &l) {
            Some(KNOWN_S1)
        } else {
            (flips_a_phase(&arms) && order_in_questions(s2_orders_differ(&legacy.game().state), &d.path, &n, &l)).then_some(KNOWN_S2)
        };
        let mut found = diff("questions", d.path, d.right, d.left);
        found.known = known;
        return Some(found);
    }
    // The events are two streams: what one side pushed beyond the other's end of step is matched by the other's next step. A step that ends an
    // arm the string's close goes on after (non-strict) may leave either side ahead; a strict step must end level, unless the group was cut in two
    // (`ahead`): the string's events beyond the head are the later half's (a reaction asked inside the step, the question after it in the same native step).
    let native_stream: Vec<String> = owed.native.drain(..).chain(group.iter().flat_map(|p| p.events.iter().cloned())).collect();
    let legacy_stream: Vec<String> = owed.legacy.drain(..).chain(events.iter().cloned()).collect();
    let common = native_stream.len().min(legacy_stream.len());
    let at = (0..common).find(|i| native_stream[*i] != legacy_stream[*i]).or_else(|| (strict && !ahead && native_stream.len() != legacy_stream.len()).then_some(common));
    if let Some(at) = at {
        let show = |v: &[String]| v.get(at).map_or_else(|| format!("<end after {}>", v.len()), Clone::clone);
        return Some(diff("events", format!("[{at}]"), show(&native_stream), show(&legacy_stream)));
    }
    owed.native = native_stream[common..].to_vec();
    owed.legacy = legacy_stream[common..].to_vec();
    if strict && !ahead {
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
                if let Some(dir) = std::env::var_os("TI4_NATIVE_PLAY_DUMP") {
                    let _ = std::fs::write(std::path::Path::new(&dir).join(format!("native-{}.json", native.steps)), after.to_string());
                }
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

    /// Integration 14: the core's map of a hyperlane game follows the printed lanes from the first step. The string applies the laws and the lanes to its
    /// galaxy at every reconcile; a game that was loaded before its first step had a map where a lane tile was a neighbour and no system was joined across
    /// it (the Peace Accords question of an Xxcha seat listed the planets of the lane tile's far end short: found by `--quick --shadow`).
    #[test]
    fn the_core_map_of_a_hyperlane_game_follows_the_printed_lanes_from_the_first_step() {
        let g = game("sol", 4, 1);
        assert_eq!(lobby_spec("sol", 4, 1).map, "4pHyperlanes");
        let galaxy = g.core.galaxy().expect("the map is loaded");
        assert!(galaxy.hyperlane_lanes, "rules version 22: adjacency follows the printed lanes");
        let mut joined_across_a_lane = 0;
        for system in ti4_engine_core::generated::System::ALL {
            if galaxy.is_hyperlane(system) {
                assert!(galaxy.adjacent(system).is_empty(), "a lane tile has no neighbours of its own");
                continue;
            }
            joined_across_a_lane += galaxy.adjacent(system).iter().filter(|n| galaxy.is_hyperlane(ti4_engine_core::ids::system_at(*n))).count();
        }
        assert_eq!(joined_across_a_lane, 0, "a lane tile is nobody's neighbour");
    }

    /// The E-to-M6 join in a whole game (integration 12): a Thunder's Edge Warfare primary begins its free tactical action inside the strategic job
    /// (`strategic::begin` calls `tactical::begin_free`, no `TurnGap::FreeTactical`), the tactical tail opens the job after it (`strategic::after::open_after`,
    /// no `TacticalGap::{LeaderStrategy, WarfareSecondary}`), and the follower window and the end of the action follow, in the string's order. Seed 4 of the
    /// 4-player sol table plays the card on its 47th step; before the join the same game stopped there with the counted gap `FreeTactical`.
    #[test]
    fn a_thunders_edge_warfare_primary_plays_its_free_tactical_action_inside_a_whole_native_game() {
        let mut g = game("sol", 4, 4);
        let p = play_game(&mut g, 600, 50);
        let at = g.events.iter().position(|e| e == "FREE_TACTICAL_ACTION").expect("the Warfare primary of this game begins its free tactical action natively");
        let order = ["FREE_TACTICAL_ACTION", "STRATEGIC_ACTION_BEGAN", "MOVEMENT_FINISHED", "PRODUCTION_RESOLVED", "TACTICAL_ACTION_COMPLETE", "STRATEGY_SECONDARY_DECLINED", "STRATEGIC_ACTION_COMPLETE", "ACTION_COMPLETED", "TURN_PASSED"];
        let mut from = at;
        for name in order {
            let found = g.events[from..].iter().position(|e| e == name).unwrap_or_else(|| panic!("{name} after the free tactical action: {:?}", &g.events[at..(at + 30).min(g.events.len())]));
            from += found + 1;
        }
        assert!(p.steps > 50, "the game goes on past the card: {} steps", p.steps);
        // No game of the table stops AT the card any more.
        for seed in 1..=12 {
            let mut game = game("sol", 4, seed);
            if let End::Gap(gap) = play_game(&mut game, 600, 50).end {
                assert!(!gap.cause.contains("FreeTactical") && !gap.cause.contains("WarfareSecondary") && !gap.cause.contains("LeaderStrategy"), "seed {seed}: {gap:?}");
            }
        }
    }

    /// The string applies the rules version to its galaxy at the start of its steps (`laws::apply_to_galaxy`); the position of a game it has not stepped
    /// yet carries the printed lanes of the hyperlane tiles off, and a core loaded from it found no route through a lane (J: the movement options of
    /// the 4-player hyperlane map were `done_moving` alone).
    #[test]
    fn a_core_loaded_from_an_unstepped_game_knows_the_printed_lanes_of_the_hyperlane_map() {
        let spec = lobby_spec("xxcha", 4, 1);
        assert_eq!(spec.map, "4pHyperlanes");
        let legacy = spec.build().expect("builds");
        assert!(!legacy.galaxy().expect("a galaxy").hyperlane_lanes, "the string has not applied its laws to the galaxy yet");
        let g = game("xxcha", 4, 1);
        assert!(g.core.galaxy().expect("a map").hyperlane_lanes, "the core's map holds the lanes");
    }

    /// The string takes a step that asks and says nothing ahead of the phase flip (the scoring window of an event nobody scores); the native loop has a step of its
    /// own with no question there, and the shadow compared Arborec's Mitosis question of the status phase with that empty step (5-player letnev table, seed 1,
    /// native step 98).
    #[test]
    fn the_shadow_steps_the_string_again_over_a_step_that_said_nothing_instead_of_calling_the_status_phase_question_a_difference() {
        let s = shadow_play(&lobby_spec("letnev", 5, 1), 400, 50).expect("builds");
        assert!(s.first_difference.as_ref().is_none_or(|d| d.step > 98), "{:?}", s.first_difference);
        assert!(s.played.steps > 98, "the game plays past the status phase question: {}", s.played.steps);
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

    /// The native game plays the setup, the draft, the announcement, the phase flip, the turn prelude, the action menu and nine rounds of actions (an Orbital
    /// Drop, tactical actions, a first claim with its exploration, strategic actions, Ggrocuto Rinn, the Technology primary, the status phases, action cards
    /// played from the menu) on the core alone, to the end of the game: ZERO gaps. (Integration 12: round one, `TurnGap::StrategyAbility`; lane E2: the Technology
    /// primary, the status close and the secrets; integration 14: the core's map follows the printed lanes of this hyperlane table, eight rounds, 967 steps, then
    /// `TurnGap::Menu(ActionCard)`: the menu could not list Signal Jamming, which a seat held; lane K lists it, and the game plays on to its end.)
    #[test]
    fn a_native_game_plays_a_whole_game_with_zero_gaps() {
        let mut g = game("sol", 4, 1);
        let played = play_game(&mut g, 2_000, 50);
        assert_eq!(played.end, End::Finished, "a whole game finishes natively");
        let native = &played.matrix.native;
        assert!(native.get("ask.draft").copied().unwrap_or(0) >= 3, "the draft was asked natively: {native:?}");
        assert_eq!(native.get("strategy_announce"), Some(&1), "{native:?}");
        assert_eq!(native.get("phase"), Some(&26), "the phases flipped: strategy to action, action to status, status to strategy, eight times over, and the last into the action phase: {native:?}");
        assert!(native.get("turn_prelude").copied().unwrap_or(0) >= 1, "{native:?}");
        assert_eq!(played.matrix.total_gaps(), 0, "one game, no gap");
        // (1,174 steps in lane K; the follower windows lane J holds open close in a step of their own: 24 more steps, the same position and fingerprint. Lane P: the
        // ruling on S1 changed the order of the questions: 1,269. Lane R: the Technology secondary is no longer offered to a seat that can pay only with War
        // Machine's 5, the marks are abilities of a window: another game, 1,165.)
        assert_eq!((played.rounds, played.steps), (9, 1165), "nine rounds of the table");
        assert!(native.get("tokens").copied().unwrap_or(0) >= 8 && native.get("status").copied().unwrap_or(0) >= 8, "the status phases ran natively, their close included: {native:?}");
        assert_eq!(played.fingerprint, "74d5675ac754ac81");
        assert_eq!(g.step(), Step::Finished, "a finished game stops the loop");
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
    fn the_last_token_of_the_status_phase_runs_the_close_natively_in_the_same_step() {
        let mut g = game("sol", 4, 1);
        open(&mut g.core, &TokenGain::new(Tokens { pending: SmallList::from_slice(&[Seat(0)]), placed: SmallList::new(), report: StatusReport::default() }));
        let held = g.core.clone();
        let step = g.step();
        let Step::Played(played) = step else { panic!("the last token ends the window and its close is the core's now (StatusFinish): {step:?}") };
        assert_eq!(played.arm, Arm::Tokens);
        assert!(played.finished);
        assert_ne!(g.core, held, "the step moved the position");
        assert!(Arm::ALL.iter().all(|a| lent_close(*a).is_none()), "no arm closes in the string any more");
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
        assert_eq!(play_game(&mut g, 2_000, 1).end, End::RoundLimit, "the game plays on past round one (lane E2): a budget of one round ends it");
        let mut g = game("sol", 4, 1);
        let p = play_game(&mut g, 100, 50);
        assert_eq!(p.steps, 100);
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
            (legacy, native, base, group)
        };
        group.last_mut().unwrap().arm = Arm::Tokens;
        group.last_mut().unwrap().events.push("INVENTED".to_owned());
        let run = run_legacy(&mut legacy, &group, &[]);
        let mut owed = Owed::default();
        assert_eq!(compare_run(&mut legacy, &native, &base, &group, &run, &mut owed, false), None, "the native step is ahead: not a difference yet");
        assert_eq!((owed.native.as_slice(), owed.legacy.len()), (["INVENTED".to_owned()].as_slice(), 0));
        // The next group's legacy events must open with it.
        let mut next = Owed { native: vec!["INVENTED".to_owned()], legacy: Vec::new() };
        let nothing: Vec<Played> = vec![Played { arm: Arm::Tokens, source: None, asked: Vec::new(), events: Vec::new(), finished: false }];
        let run = LegacyRun { info: run.info, records: Vec::new(), events: vec!["OTHER".to_owned()] };
        let d = compare_run(&mut legacy, &native, &base, &nothing, &run, &mut next, false).expect("the string pushed another line than the one owed");
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
        assert_eq!((s.compared, s.played.steps), (61, 100), "a hundred native steps (the budget of the call: the shadow of the whole game ends at the production-order difference, see the next test); the glued ones, the ones that ask nothing while the legacy game stands at a question and the ones a lookahead groups with the step before are not legacy steps of their own");
        let one_round = shadow_play(&lobby_spec("sol", 4, 1), 100, 1).unwrap();
        assert!(matches!(one_round.played.end, End::StepLimit) && one_round.first_difference.is_none(), "round 1 is within a budget of one, and the hundred steps run out before any gap or difference (integration 13: the gap of round one was Peace Accords): {:?}", one_round.played.end);
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

    /// Lane R: the quick game's shadow plays the WHOLE game with no difference (808 legacy steps). Lane P pinned its follower window of the Technology card
    /// (step 448: native asked seat `c`, the string `d`; the string gates the secondary on `payment::affordable`, which does not count War Machine's 5 of an
    /// activation); the pin said it would fail as written the day that was fixed.
    #[test]
    fn the_shadow_run_agrees_with_the_legacy_game_for_the_whole_quick_game() {
        let shadowed = shadow_play(&lobby_spec("sol", 4, 1), 2_000, 50).expect("builds");
        assert!(shadowed.first_difference.is_none(), "{:?}", shadowed.first_difference);
        assert_eq!(shadowed.compared, 808, "every legacy step of the game was compared");
        assert!(matches!(shadowed.played.end, End::Finished), "{:?}", shadowed.played.end);
    }

    /// Lane R, the marks (`hooks::marks` of the activation chain became the string's abilities `leader:*:sardakkagent_mark:SYSTEM_ACTIVATED:after` and
    /// `ability:*:reclamation_tactical:SYSTEM_ACTIVATED:after`): a window with another eligible reaction offers the mark as a choice. Sardakk (3 players, step
    /// 230) and Winnu (3 players, step 500) differed there.
    #[test]
    fn the_activation_marks_of_the_sardakk_agent_and_the_winnu_are_offered_in_the_window_the_string_offers_them() {
        for (faction, steps) in [("sardakk", 236), ("winnu", 505)] {
            let shadowed = shadow_play(&lobby_spec(faction, 3, 1), steps, 50).expect("builds");
            assert!(shadowed.first_difference.is_none(), "{faction}: {:?}", shadowed.first_difference);
            assert!(shadowed.compared > 100, "{faction}: {}", shadowed.compared);
        }
    }

    /// Lane R, the staged events (a trade good the prelude's Psychoarchaeology staged waits for the next step; a retreat's command token waits for the next step
    /// behind the hit flush of the invasion; the extra die of the Letnev agent is no barrage die): Muaat (3 players, steps 514 and 550) and Jol-Nar (5 players,
    /// steps 369 and 371, where the string also stops its step after `RETREAT_STEP_STARTED`).
    #[test]
    fn staged_events_wait_for_the_next_step_and_the_agents_extra_die_is_no_barrage_die() {
        for (faction, players, steps) in [("muaat", 3, 560), ("jolnar", 5, 375)] {
            let shadowed = shadow_play(&lobby_spec(faction, players, 1), steps, 50).expect("builds");
            assert!(shadowed.first_difference.is_none(), "{faction}: {:?}", shadowed.first_difference);
        }
    }

    /// Lane R, the step starts (the string announces what a step staged at the start of its NEXT step): a Ceasefire denies the movement and the string's step runs
    /// from the activation through the aftermath to the closing menu (Yssaril, 4 players, seed 3, step 874: the reconcile of the native aftermath step announced the
    /// activation's trade good ahead of the owed `PRODUCTION_RESOLVED`); an end of turn a hook left open (Nullification Field) runs instead of the reconcile, and
    /// the strategy token it staged was never announced (Cabal, 3 players, seed 1, step 766).
    #[test]
    fn staged_events_wait_behind_a_hand_off_to_the_aftermath_and_behind_a_turn_a_hook_ended() {
        for (faction, players, seed, steps) in [("yssaril", 4, 3, 880), ("cabal", 3, 1, 772)] {
            let shadowed = shadow_play(&lobby_spec(faction, players, seed), steps, 50).expect("builds");
            assert!(shadowed.first_difference.is_none(), "{faction} {players}p seed {seed}: {:?}", shadowed.first_difference);
            assert!(shadowed.compared > 400, "{faction}: {}", shadowed.compared);
        }
    }

    /// Lane S, Skilled Retreat in a live game: the card names the destinations the position's own map gives (the core was lent none, so the played card did
    /// nothing: the string withdrew the fleet and asked "withdraw to which system", the core asked nothing and went on to the combat the card ended). Xxcha
    /// (4 players, seed 2, step 392), Winnu (4 players, seed 4, step 416: the question has three destinations, and the answer the table gave -- the destination
    /// id alone -- was refused because it carried no count of the ships already there), Sardakk (4 players, seed 1, step 919: the Reflective Shielding played in
    /// the same window of the use of SUSTAIN DAMAGE left its two hits staged, the live machine never drained them).
    #[test]
    fn the_cards_of_a_combat_round_window_play_in_a_live_game_as_the_string_plays_them() {
        for (faction, players, seed, steps) in [("xxcha", 4, 2, 398), ("winnu", 4, 4, 422), ("sardakk", 4, 1, 925)] {
            let shadowed = shadow_play(&lobby_spec(faction, players, seed), steps, 50).expect("builds");
            assert!(shadowed.first_difference.is_none(), "{faction} {players}p seed {seed}: {:?}", shadowed.first_difference);
            assert!(shadowed.compared > 150, "{faction}: {}", shadowed.compared);
        }
    }

    /// Lane S, the map of a live game: the wormhole tokens (Gamma Wormhole, the open Nexus, the ion storm) the game places join the core's map at the next
    /// reconcile, as the string rebuilds them every step. Sol (5 players, seed 1, step 867): Signal Jamming's systems lacked `82a`, which a gamma token in `74`
    /// makes a neighbour of the Mallice tile; Ghosts (4 players, seed 2, step 414) found no route into `82a`; Yin (4 players, seed 4, step 943).
    #[test]
    fn a_wormhole_token_the_game_places_joins_the_map_of_the_core() {
        for (faction, players, seed, steps) in [("sol", 5, 1, 872), ("ghost", 4, 2, 420), ("yin", 4, 4, 950)] {
            let shadowed = shadow_play(&lobby_spec(faction, players, seed), steps, 50).expect("builds");
            assert!(shadowed.first_difference.is_none(), "{faction} {players}p seed {seed}: {:?}", shadowed.first_difference);
        }
    }

    /// Lane S, the printing of the Naalu mech: the sheet of the generated data names the Thunder's Edge one; a game without that source plays the Prophecy of
    /// Kings one (the unit the Stillness of Stars offered to capture was the plain `mech`). Cabal (6 players, seed 4, step 943). And the Yssaril seat's copy of the
    /// Mentak agent against the Suffi An question of the Mentak seat (6 players, seed 2, step 1294): the two seats react to one event, the core asks them in the
    /// ruling's order and the string in its seating order (S1), whose question of the second seat is the ability's own prompt.
    #[test]
    fn the_naalu_mech_is_the_printing_of_the_box_and_two_abilities_of_one_event_are_the_known_s1() {
        let shadowed = shadow_play(&lobby_spec("cabal", 6, 4), 950, 50).expect("builds");
        assert!(shadowed.first_difference.is_none(), "{:?}", shadowed.first_difference);
        let shadowed = shadow_play(&lobby_spec("yssaril", 6, 2), 1300, 50).expect("builds");
        let difference = shadowed.first_difference.expect("the string orders the two abilities by its seating");
        assert_eq!(difference.known, Some(KNOWN_S1), "{difference}");
    }

    /// Lane S, found by the seeds 5 to 8 of the Z-gate command: (a) the flip into the strategy phase of a later round told the resolver the new phase before it
    /// announced `STRATEGY_PHASE_BEGAN`, as the status phase's flip does not forget to: the windows went to the seats in the initiative order of the status phase
    /// (the Naalu seat first) instead of from the speaker (L1Z1X, 6 players, seed 5, step 812); (b) the retreat's command token an aftermath step staged is
    /// announced by the scoring step that follows a scoring pause, which is the string's next step (Sol, 6 players, seed 8, step 495).
    #[test]
    fn the_strategy_phase_of_a_later_round_orders_from_the_speaker_and_a_scoring_pause_announces_what_was_staged() {
        for (faction, seed, steps) in [("l1z1x", 5, 818), ("sol", 8, 500)] {
            let shadowed = shadow_play(&lobby_spec(faction, 6, seed), steps, 50).expect("builds");
            assert!(shadowed.first_difference.is_none(), "{faction} seed {seed}: {:?}", shadowed.first_difference);
        }
    }

    /// Lane S: the Nekro agent's discard is announced after the gain it pays for AND after the Pillage of that gain (the string emits the Pillage's gain inside the
    /// event of the first, and the card moves follow the outer event): Xxcha (6 players, seed 6, step 382) and Arborec (6 players, seed 5, step 962) had the
    /// discard between the two gains.
    #[test]
    fn the_nekro_agents_discard_follows_the_gain_and_the_pillage_of_the_gain() {
        for (faction, seed, steps) in [("xxcha", 6, 388), ("arborec", 5, 968)] {
            let shadowed = shadow_play(&lobby_spec(faction, 6, seed), steps, 50).expect("builds");
            assert!(shadowed.first_difference.is_none(), "{faction} seed {seed}: {:?}", shadowed.first_difference);
        }
    }

    /// Lane S: the removals over the capacity after a retreat are native steps of their own, the string's step goes on through them: the command token the retreat staged
    /// is announced by the step after the whole action, not at the start of the next removal (Yin 5 players seed 7 step 850, Yin 4 players seed 6 step 750, Sardakk 4 players seed 8 step 816).
    #[test]
    fn the_removals_after_a_retreat_do_not_announce_what_the_retreat_staged() {
        for (faction, players, seed, steps) in [("yin", 5, 7, 856), ("yin", 4, 6, 756), ("sardakk", 4, 8, 822)] {
            let shadowed = shadow_play(&lobby_spec(faction, players, seed), steps, 50).expect("builds");
            assert!(shadowed.first_difference.is_none(), "{faction} {players}p seed {seed}: {:?}", shadowed.first_difference);
        }
    }

    /// Lane R: one string step runs through many native steps (every removal over the fleet supply or the capacity is a native step of its own, and a Warfare
    /// action asks its second redistribution after them): the legacy game is given the answers of the steps ahead. Letnev (3 players, step 684; 4 players, step 289).
    #[test]
    fn a_string_step_that_runs_through_the_removals_and_the_warfare_question_is_given_the_answers_ahead() {
        for (players, steps) in [(3, 692), (4, 298)] {
            let shadowed = shadow_play(&lobby_spec("letnev", players, 1), steps, 50).expect("builds");
            assert!(shadowed.first_difference.is_none(), "letnev {players}: {:?}", shadowed.first_difference);
        }
    }

    /// Lane P (PB, 65 of the 96 games): the string ends its step at the close of the status phase and flips the next phase in a step of its own; the
    /// native `phase` step joined the answers of the last tokens group and was compared with the legacy step that ended before it. And when the string
    /// takes a step that asks and says nothing ahead of the flip (the scoring window of an event nobody scores), the group that asks nothing is
    /// compared with the legacy step after it. Sol, 3 players: the first difference was the flip of the first round, native step 153.
    #[test]
    fn the_phase_flip_after_the_last_tokens_answer_is_a_legacy_step_of_its_own() {
        let shadowed = shadow_play(&lobby_spec("sol", 3, 1), 170, 50).expect("builds");
        assert!(shadowed.first_difference.is_none(), "{:?}", shadowed.first_difference);
        assert!(shadowed.compared > 101, "the flip at step 153 was compared: {}", shadowed.compared);
    }

    /// Lane P (PB, the draft): the last card of the draft is taken without a question, in a native step of its own where the string's step of the pick
    /// before it ended. Jol-Nar, 4 players: the first difference was the strategy phase of round 5, native step 380.
    #[test]
    fn the_forced_last_pick_of_the_draft_is_a_legacy_step_of_its_own() {
        let shadowed = shadow_play(&lobby_spec("jolnar", 4, 1), 400, 50).expect("builds");
        assert!(shadowed.first_difference.is_none(), "{:?}", shadowed.first_difference);
    }

    /// Lane P (AC): the string asks the reaction card at `after ACTION_COMPLETED` inside the step that answers `done_moving`; the native step that asks
    /// it also asks the next menu: the group is cut there and the string's events beyond the head are owed to the later half. Xxcha, 4 players, step 282.
    #[test]
    fn a_group_cut_between_two_questions_of_a_native_step_owes_the_legacy_surplus_to_the_later_half() {
        let shadowed = shadow_play(&lobby_spec("xxcha", 4, 1), 290, 50).expect("builds");
        assert!(shadowed.first_difference.is_none(), "{:?}", shadowed.first_difference);
    }

    /// Lane P (TG): the string announces the events a prelude question staged (Psychoarchaeology's trade good) at the start of its NEXT step; the native
    /// step after the prelude runs no reconcile. And a component action flushes the staged events like the string's `announce_staged_ground_events`.
    /// Sardakk (3 players, step 166) and Naaz (3 players, step 110).
    #[test]
    fn the_events_a_prelude_question_staged_are_announced_by_the_next_legacy_step_and_a_component_action_flushes_them() {
        for (faction, steps) in [("sardakk", 170), ("naaz", 115)] {
            let shadowed = shadow_play(&lobby_spec(faction, 3, 1), steps, 50).expect("builds");
            assert!(shadowed.first_difference.is_none(), "{faction}: {:?}", shadowed.first_difference);
        }
    }

    /// Lane P (SC): a gunner's roll is staged under the type id of its unit (the string's `type_id`), so a reroll question of a space cannon offense names
    /// `pds`, not the roll kind (Fire Team, Scramble Frequency). Mentak, 5 players (the native step 190 of the game as it plays now; 178 before the ruling on S1): the question's prompt was native `reroll die 1 of
    /// space_cannon`, legacy `reroll die 1 of pds`; 8 of the 96 games.
    #[test]
    fn a_reroll_question_of_the_space_cannon_offense_names_the_unit_type_of_the_gun() {
        let shadowed = shadow_play(&lobby_spec("mentak", 5, 1), 195, 50).expect("builds");
        assert!(shadowed.first_difference.is_none(), "{:?}", shadowed.first_difference);
    }

    /// Lane P (AIDA): AI Development Algorithm's question is a tape question of the production use inside the string's step: `PRODUCTION_USED` follows
    /// it before the aftermath's own line. Nomad, 4 players, step 285.
    #[test]
    fn a_tape_question_of_the_production_use_keeps_the_aftermath_line_behind_the_use() {
        let shadowed = shadow_play(&lobby_spec("nomad", 4, 1), 290, 50).expect("builds");
        assert!(shadowed.first_difference.is_none(), "{:?}", shadowed.first_difference);
    }

    /// Lane P (the retreat token): the string announces the command token a retreat staged at the start of its NEXT step, after the tail of the
    /// tactical action and the end of the action; the tail step after a finished aftermath runs no reconcile. Ghosts, 4 players, step 517.
    #[test]
    fn the_tail_of_a_tactical_action_after_a_finished_aftermath_announces_no_staged_event_of_its_own() {
        let shadowed = shadow_play(&lobby_spec("ghost", 4, 1), 520, 50).expect("builds");
        assert!(shadowed.first_difference.is_none(), "{:?}", shadowed.first_difference);
    }

    /// Lane P (S1): the user ruled that an action-phase window offers the ACTIVE player first and then the others in INITIATIVE order. The string
    /// offers its seating order rotated from the active seat; the shadow reports that difference as a known string bug and does not copy it.
    /// Saar, 6 players: the script wanted the L1Z1X agent's reaction and the string offered the Saar agent's first (step 38).
    #[test]
    fn a_reaction_window_the_string_orders_by_its_seating_is_reported_as_the_known_string_bug_s1() {
        let shadowed = shadow_play(&lobby_spec("saar", 6, 1), 60, 50).expect("builds");
        let d = shadowed.first_difference.expect("the string offers the other seat's reaction first");
        assert_eq!(d.known, Some(KNOWN_S1), "{d}");
        assert_eq!(d.what, "legacy", "{d}");
        assert!(d.legacy.contains("script wanted"), "{d}");
        assert!(d.to_string().contains("known string bug S1"), "{d}");
    }

    /// S1 is recognised only for a reaction, and only when the string's order differs from the ruling's.
    #[test]
    fn s1_is_recognised_only_for_a_reaction_and_only_when_the_two_orders_differ() {
        let legacy = MaskSubject::<LegacyMask>::new(&lobby_spec("sol", 4, 1), Pilot::Script).expect("builds");
        let mut state = legacy.game().state.clone();
        let ids: Vec<_> = state.seating_order.clone();
        // The strategy and agenda windows are ordered from the speaker by both engines: nothing to recognise there.
        assert!(!s1_orders_differ(&state), "the game starts in the strategy phase");
        state.phase = ti4_model::state::Phase::Action;
        let wanted_reaction = "legacy step failed: script wanted \"leader:a:b:SYSTEM_ACTIVATED:after\" but a was offered [\"leader:c:d:SYSTEM_ACTIVATED:after\", \"decline\"]";
        let wanted_move = "legacy step failed: script wanted \"done_moving\" but a was offered [\"leader:c:d:SYSTEM_ACTIVATED:after\", \"decline\"]";
        // The first seat is active: the ruling's order (the active seat, then the others by initiative with ties by seating) is the clockwise order.
        state.active = Some(ids[0].clone());
        assert!(!s1_orders_differ(&state));
        assert!(!s1_in_error(&state, wanted_reaction), "the same order is not S1");
        // The second seat is active: the seat before it sits last clockwise but first by initiative.
        state.active = Some(ids[1].clone());
        assert!(s1_orders_differ(&state));
        assert!(s1_in_error(&state, wanted_reaction));
        assert!(!s1_in_error(&state, wanted_move), "a script that wanted a move met a reaction: not S1");
        assert!(!s1_in_error(&state, "legacy step failed: script wanted \"leader:a:b:SYSTEM_ACTIVATED:after\" but a was offered [\"0|2|4\"]"), "the offer is no reaction");
        assert!(!s1_in_error(&state, "something else"));
        assert!(!s1_in_error(&state, "legacy step failed: script wanted \"leader:a:b:SYSTEM_ACTIVATED:after\""), "no offer named");
        // Nobody active and nobody holds a card: the initiative order is the seating order.
        state.active = None;
        assert!(!s1_orders_differ(&state));
        // The ids of the other abilities a window offers are reaction ids too (an Instinct Training offer is `technology:..`).
        let wanted_technology = "legacy step failed: script wanted \"technology:a:it:ACTION_CARD_PLAYED:when\" but a was offered [\"reaction:c:ACTION_CARD_PLAYED:when\", \"decline\"]";
        state.active = Some(ids[1].clone());
        assert!(s1_in_error(&state, wanted_technology));
        // The status phase has no active seat: the string's list is the seating order of `Game::new`, the ruling's the initiative order of the moment (LRR 1.19).
        state.phase = ti4_model::state::Phase::Status;
        state.active = None;
        let last = state.players.last().expect("a player").id.clone();
        let card = ti4_model::id::StrategyCardId::new("pok1leadership");
        state.card_initiative.insert(card.clone(), 1);
        state.player_mut(&last).expect("seated").strategy_cards = vec![card];
        assert!(s1_orders_differ(&state), "the seat with the first card is first by initiative and last by seating");
        assert!(s1_in_error(&state, wanted_reaction));
        state.phase = ti4_model::state::Phase::Strategy;
        assert!(!s1_orders_differ(&state), "the strategy phase orders from the speaker in both engines");
        state.phase = ti4_model::state::Phase::Action;
        state.player_mut(&last).expect("seated").strategy_cards = Vec::new();
        // The questions: the same prompt asked of another seat, or the same options in another order.
        state.active = Some(ids[1].clone());
        let q = |player: &str, options: &[&str]| json!({ "player": player, "prompt": "after SYSTEM_ACTIVATED", "options": options });
        let reaction = ["reaction:a:SYSTEM_ACTIVATED:after", "reaction:b:SYSTEM_ACTIVATED:after", "decline"];
        let flipped = ["reaction:b:SYSTEM_ACTIVATED:after", "reaction:a:SYSTEM_ACTIVATED:after", "decline"];
        assert!(s1_in_questions(&state, "[0].options[0]", &[q("a", &flipped)], &[q("a", &reaction)]));
        assert!(s1_in_questions(&state, "[0].player", &[q("b", &reaction)], &[q("a", &reaction)]));
        assert!(!s1_in_questions(&state, "[0].options[1]", &[q("a", &["reaction:a:SYSTEM_ACTIVATED:after", "decline"])], &[q("a", &reaction)]), "an offer the core does not make is not an order");
        assert!(!s1_in_questions(&state, "[0].options[0]", &[q("a", &flipped)], &[json!({ "player": "a", "prompt": "another", "options": reaction })]), "another prompt");
        assert!(!s1_in_questions(&state, "[0].options[0]", &[q("a", &["x", "y"])], &[q("a", &["y", "x"])]), "no reaction in the string's options");
        assert!(!s1_in_questions(&state, "nonsense", &[q("a", &flipped)], &[q("a", &reaction)]));
        assert!(!s1_in_questions(&state, "[3].options[0]", &[q("a", &flipped)], &[q("a", &reaction)]), "no such question");
        // The ability of the seat the string asks first has its own prompt (a single optional ability asks its own question): the core asks the
        // ruling's first seat (a), the string the seating order's (d, behind b: b c d a).
        let own = |player: &str| json!({ "player": player, "prompt": "Suffi An: exhaust to draw 1 action card", "options": ["exhaust", "decline"] });
        assert!(s1_in_questions(&state, "[0].options[0]", &[q("a", &reaction)], &[own("d")]), "the string asked d first, the ruling asks a first");
        assert!(!s1_in_questions(&state, "[0].options[0]", &[q("d", &reaction)], &[own("a")]), "the other way round is no S1: the string would ask a last");
        assert!(!s1_in_questions(&state, "[0].options[0]", &[q("a", &reaction)], &[own("a")]), "the same seat");
        assert!(!s1_in_questions(&state, "[0].options[0]", &[q("a", &["0|2|4"])], &[own("d")]), "no ability offer of the core");
        assert!(!s1_in_questions(&state, "[2].options[0]", &[q("a", &reaction)], &[own("d")]), "no such question");
        assert!(!s1_in_questions(&state, "[0].options[0]", &[json!({ "prompt": "x", "options": reaction })], &[own("d")]), "a question with no seat");
        assert!(!s1_in_questions(&state, "[0].options[0]", &[q("a", &reaction)], &[json!({ "prompt": "x", "options": ["exhaust"] })]), "a string question with no seat");
        state.phase = ti4_model::state::Phase::Strategy;
        assert!(!s1_in_questions(&state, "[0].options[0]", &[q("a", &reaction)], &[own("d")]), "the strategy phase orders from the speaker in both engines");
        state.phase = ti4_model::state::Phase::Action;
        state.active = Some(ids[0].clone());
        assert!(!s1_in_questions(&state, "[0].options[0]", &[q("a", &flipped)], &[q("a", &reaction)]), "the same order");
        assert!(!s1_in_questions(&state, "[0].options[0]", &[q("a", &reaction)], &[own("d")]), "the same order");
    }

    /// S2: the windows of the flip into a strategy phase are ordered by the string's stale context (the seating list), the ruling's order is the speaker's.
    #[test]
    fn s2_is_recognised_for_a_reaction_in_a_phase_flip_only_when_the_speaker_is_not_the_first_seat() {
        let legacy = MaskSubject::<LegacyMask>::new(&lobby_spec("sol", 4, 1), Pilot::Script).expect("builds");
        let mut state = legacy.game().state.clone();
        let ids: Vec<_> = state.seating_order.clone();
        let wanted = "legacy step failed: script wanted \"reaction:a:STRATEGY_PHASE_BEGAN:after\" but c was offered [\"reaction:c:STRATEGY_PHASE_BEGAN:after\", \"decline\"]";
        state.speaker = ids[0].clone();
        assert!(!s2_orders_differ(&state), "the speaker sits first: the two orders are the same");
        state.speaker = ids[2].clone();
        assert!(s2_orders_differ(&state));
        assert!(order_in_error(s2_orders_differ(&state), wanted));
        assert!(!order_in_error(false, wanted));
        assert!(flips_a_phase(&["phase".to_owned(), "ask.draft".to_owned()]) && !flips_a_phase(&["tactical".to_owned()]));
    }

    /// A legacy step that asked nothing but pushed the first events of what the native steps pushed is the string stopping early (`RETREAT_STEP_STARTED` ahead
    /// of the retreat announcement): its events are the head of the group's, never more, and never different.
    #[test]
    fn a_legacy_step_that_asked_nothing_and_pushed_the_head_of_the_groups_events_stopped_early() {
        let played = |events: &[&str]| Played { arm: Arm::Aftermath, source: None, asked: Vec::new(), events: events.iter().map(|e| (*e).to_owned()).collect(), finished: false };
        let group = [played(&["RETREAT_STEP_STARTED", "HITS_TO_ASSIGN"])];
        let text = |events: &[&str]| events.iter().map(|e| (*e).to_owned()).collect::<Vec<_>>();
        assert!(events_lead(&[], &group, &text(&["RETREAT_STEP_STARTED"])));
        assert!(events_lead(&[], &group, &text(&["RETREAT_STEP_STARTED", "HITS_TO_ASSIGN"])));
        assert!(!events_lead(&[], &group, &text(&["RETREAT_STEP_STARTED", "SHIP_DESTROYED"])), "a different event is a difference");
        assert!(!events_lead(&[], &group, &text(&["RETREAT_STEP_STARTED", "HITS_TO_ASSIGN", "SHIP_DESTROYED"])), "more than the native pushed is a difference");
        assert!(!events_lead(&[], &group, &[]), "no events: the idle step is the other case");
        assert!(events_lead(&text(&["X"]), &group, &text(&["X", "RETREAT_STEP_STARTED"])), "the owed native events come first");
    }

    /// The string's idle step: no question, no event, no error, where the native group pushed events.
    #[test]
    fn a_legacy_step_that_did_and_said_nothing_is_idle_against_a_group_that_pushed_events() {
        let info = crate::oracle::StepInfo { phase: "action".to_owned(), active: None, finished: false, error: None, resolved_choice: false };
        let run = |events: &[&str]| LegacyRun { info: info.clone(), records: Vec::new(), events: events.iter().map(|e| (*e).to_owned()).collect() };
        let pushed = Played { arm: Arm::Phase, source: None, asked: Vec::new(), events: vec!["STATUS_PHASE_BEGAN".to_owned()], finished: true };
        let silent = Played { events: Vec::new(), ..pushed.clone() };
        assert!(steps_idle(&run(&[]), std::slice::from_ref(&pushed)));
        assert!(!steps_idle(&run(&["X"]), &[pushed]), "the string said something");
        assert!(!steps_idle(&run(&[]), &[silent]), "the group pushed nothing either");
    }

    /// The helper of the shadow's grouping: a native step that flips the phase, or makes the forced last pick of the draft, asks nothing and does not
    /// join a group that still holds answers (once).
    #[test]
    fn a_phase_flip_or_a_forced_pick_opens_its_own_group_once() {
        let played = |arm: Arm, source: Option<AskArm>, asks: usize| Played {
            arm,
            source,
            asked: (0..asks).map(|_| Asked { player: "a".to_owned(), prompt: "p".to_owned(), options: vec![], chosen: "c".to_owned() }).collect(),
            events: Vec::new(),
            finished: true,
        };
        let holding = [played(Arm::Ask, Some(AskArm::Tokens), 1)];
        let flip = played(Arm::Phase, None, 0);
        let pick = played(Arm::Ask, Some(AskArm::Draft), 0);
        assert!(opens_a_step_of_its_own(&flip, &holding, false));
        assert!(opens_a_step_of_its_own(&pick, &holding, false));
        assert!(!opens_a_step_of_its_own(&flip, &holding, true), "held back once");
        assert!(!opens_a_step_of_its_own(&flip, &[], false), "nothing to be held apart from");
        assert!(!opens_a_step_of_its_own(&flip, &[played(Arm::Ask, Some(AskArm::Tokens), 0)], false), "a group without answers");
        assert!(!opens_a_step_of_its_own(&played(Arm::Phase, None, 1), &holding, false), "a flip that asks joins");
        assert!(!opens_a_step_of_its_own(&played(Arm::Secondary, None, 0), &holding, false), "another arm joins");
        assert!(!opens_a_step_of_its_own(&played(Arm::Ask, Some(AskArm::Tokens), 0), &holding, false), "another source joins");
    }

    /// Integration 15 (M2): the Xxcha seat of the 6-player sol table plays Exploration Probe from the menu at native step 270; the frontier token it
    /// explores is NEXT to its ships, not under them. The card's facts did not say the position holds a map, so the core looked at the systems of
    /// its ships alone, found no token, drew nothing and still discarded the card: the shadow's first difference was the top of the frontier deck
    /// (`exploration_decks.FRONTIER[0]`, native `minent` against legacy `lc2`) in 12 of the 96 games (every 6-player table).
    #[test]
    fn an_exploration_probe_played_from_the_menu_explores_the_frontier_token_next_to_the_seats_ships() {
        let shadowed = shadow_play(&lobby_spec("sol", 6, 1), 2_000, 50).expect("builds");
        assert!(shadowed.compared > 189, "the legacy game agrees past the probe: {}", shadowed.compared);
        assert!(shadowed.first_difference.as_ref().is_none_or(|d| d.step > 270 && !d.path.starts_with("exploration_decks")), "{:?}", shadowed.first_difference);
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
