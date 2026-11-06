//! The MCTS lab (docs/MCTS_BOT_PLAN.md lanes M0, M1, later L2): whole 4p games on the typed core's live loop, every question
//! handed to a [`Decide`] together with the core `Choice`, the wire `Choice` and the position it is asked in.
//!
//! The loop is the product's (`ti4_engine::native::Game::step_native` / `native_step`, the body of the bridge's
//! `NativeGame::step_inner`) without the wrapper's per-step extras the search never needs: no string mirror
//! (`materialized`), no event text, no table, no journal, no window observers. What stays is exactly what C6's
//! `Position` will hold: the core, the renderer (the code table, the env cache) and the glue flag ([`Pos`]).
//!
//! The rule bot answers on the typed view ([`rule_bot`]: `Seen` -> `SeenView` -> `ti4_rule_bot::choose_explained_view`),
//! not on the string `Observed` the product's table builds.
//!
//! A dev tool: no production crate depends on this one (tools/engine_core/check_production_graph.sh).

pub mod agents;
pub mod eval;

use std::rc::Rc;
use std::time::{Duration, Instant};

use ti4_content::galaxy::Galaxy;
use ti4_content::ContentStore;
use ti4_engine::choice::Choice as WireChoice;
use ti4_engine::core_sync::codec::rng::TextSeeds;
use ti4_engine::core_sync::Ctx;
use ti4_engine::live::{self, Asker, Question, Stop};
use ti4_engine::render::Render;
use ti4_engine_core::choice::Choice;
use ti4_engine_core::driver::live::{is_open, live_arm};
use ti4_engine_core::driver::session::{live_ask_arm, step_arm, Arm, AskArm, Facts};
use ti4_engine_core::driver::turn::{self, TurnFlowWin};
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::seen::Seen;
use ti4_engine_core::state::{CoreState, WindowKind};
use ti4_model::content_types::SourceSet;
use ti4_model::id::PlayerId;
use ti4_rule_bot_typed::seen_view::{Owners, SeenView};
use ti4_selfplay::spec::GameSpec;

/// The four factions of the first MCTS setup (memory `project_mcts_bot_decisions`).
pub const FACTIONS: [&str; 4] = ["sol", "letnev", "xxcha", "jolnar"];
/// The map of the first setup.
pub const MAP: &str = "4pLearnToPlayPoK";

/// The game of `seed`: the four factions in the seating `seed % 24` (all 24 permutations cycle over the seeds), the speaker drawn
/// from the seed.
#[must_use]
pub fn spec(seed: u64) -> GameSpec {
    GameSpec {
        setup: "mcts-4p-ltp".to_owned(),
        seed,
        players: 4,
        map: MAP.to_owned(),
        factions: permutation(usize::try_from(seed % 24).unwrap_or(0)).iter().map(|f| (*f).to_owned()).collect(),
        speaker: "random".to_owned(),
        card_set: None,
        rotation: None,
        starting_tech: true,
        thunders_edge: false,
        rules_version: None,
    }
}

/// Permutation `k` (0..24) of [`FACTIONS`] (factorial number system).
#[must_use]
pub fn permutation(k: usize) -> [&'static str; 4] {
    let mut pool: Vec<&str> = FACTIONS.to_vec();
    let mut out = [""; 4];
    let mut k = k % 24;
    for (i, slot) in out.iter_mut().enumerate() {
        let f = [6, 2, 1, 1][i];
        *slot = pool.remove(k / f);
        k %= f;
    }
    out
}

/// What does not change in a game: the tables, the seats, the kernel.
pub struct Table {
    pub cx: Ctx,
    pub seeds: Rc<TextSeeds>,
    pub kernel: &'static Kernel,
    pub sources: SourceSet,
    pub galaxy: Galaxy,
    pub owners: Owners,
    /// The seat texts in seat order.
    pub seats: Vec<String>,
    pub factions: Vec<String>,
    pub seed: u64,
    pub content: &'static ContentStore,
}

/// A position the search can fork: the core, the renderer and the glue flag (what C6's `search::Position` holds).
pub struct Pos {
    pub core: CoreState,
    pub render: Render,
    pub after_glue: bool,
}

impl Pos {
    /// An independent copy (the core clone plus a renderer rebuilt from the code table, as `NativeGame::fork`).
    #[must_use]
    pub fn fork(&self, t: &Table) -> Self {
        let mut render = Render::new(t.cx.owners.ids().to_vec()).with_codes(self.render.codes_snapshot());
        render.factions.clone_from(&self.render.factions);
        Self { core: self.core.clone(), render, after_glue: self.after_glue }
    }

    #[must_use]
    pub fn round(&self) -> u32 {
        self.core.meta().round
    }

    #[must_use]
    pub fn finished(&self) -> bool {
        self.core.meta().finished
    }

    /// Victory points in seat order.
    #[must_use]
    pub fn vp(&self) -> Vec<i16> {
        self.core.players().iter().map(|p| p.econ.victory_points).collect()
    }
}

/// Set up the game of `spec` on the native setup.
///
/// # Errors
/// A spec the native setup does not build.
pub fn start(spec: &GameSpec) -> Result<(Table, Pos), String> {
    let Some(start) = spec.start()? else { return Err("not a native table".to_owned()) };
    let parts = start.into_parts().map_err(|e| e.to_string())?;
    let ti4_engine::core_setup::StartParts { core, cx, state, galaxy, seed } = parts;
    let factions: Vec<String> = cx.owners.ids().iter().map(|id| state.players.iter().find(|p| p.id.as_str() == id).map_or_else(String::new, |p| p.faction.as_str().to_owned())).collect();
    let render = Render::new(cx.owners.ids().to_vec()).with_factions(factions.clone()).with_codes(core.texts.codes.clone());
    let sources = ti4_model::content_types::POK;
    let seats = cx.owners.ids().to_vec();
    let owners = Owners::new(seats.iter().map(|s| PlayerId::new(s.as_str())).collect());
    let table = Table {
        seeds: Rc::new(TextSeeds::of(&cx)),
        kernel: live::kernel::kernel_for(sources),
        sources,
        galaxy,
        owners,
        seats,
        factions,
        seed,
        content: ContentStore::embedded(),
        cx,
    };
    Ok((table, Pos { core, render, after_glue: false }))
}

/// One question as the decider sees it.
pub struct Asked<'a> {
    /// The core's question (actor, ask, context, options).
    pub choice: &'a Choice,
    /// The wire form (option `i` of `rendered.options` is `choice.opts[i]`).
    pub rendered: &'a WireChoice,
    /// The position the question is asked in (a nested question's own).
    pub at: &'a CoreState,
    /// The decision's number in the game (the rule bot's stream index).
    pub index: usize,
    /// The seat's place in the seat order.
    pub place: usize,
}

/// Whoever answers the questions of a game.
pub trait Decide {
    /// The index of the picked option.
    ///
    /// # Errors
    /// A message that stops the game.
    fn decide(&mut self, t: &Table, a: &Asked<'_>) -> Result<usize, String>;
}

/// The rule bot's answer on the typed view: `(picked, smart, view time, bot time)`.
#[must_use]
pub fn rule_bot(t: &Table, a: &Asked<'_>) -> (usize, bool, Duration, Duration) {
    let t0 = Instant::now();
    let seen = Seen::new(a.at, t.kernel, a.choice.actor);
    let view = SeenView::new(&seen, &t.owners, t.content, t.sources, Some(&t.galaxy));
    let t1 = Instant::now();
    let (picked, smart, _) = ti4_rule_bot::choose_explained_view(a.rendered, Some(&view), t.seed, a.place, a.index);
    (picked, smart, t1 - t0, t1.elapsed())
}

/// The position a question is asked in (as the product's `TableAsker::position_of`).
fn position_of<'q>(q: &'q Question<'_>) -> &'q CoreState {
    q.asking.nested.as_ref().map(|(_, at)| &**at).or(q.asking.at.as_deref()).unwrap_or(q.core)
}

struct LabAsker<'a> {
    t: &'a Table,
    dec: &'a mut dyn Decide,
    index: usize,
    asked: usize,
    error: Option<String>,
}

impl Asker for LabAsker<'_> {
    fn ask(&mut self, q: &Question<'_>) -> Result<String, Stop> {
        let rendered = &q.rendered;
        if rendered.options.is_empty() {
            self.error = Some(format!("a question with no option: {}", rendered.prompt));
            return Err(Stop::Blocked);
        }
        let place = self.t.seats.iter().position(|s| s == rendered.player.as_str()).unwrap_or(0);
        let asked = Asked { choice: &q.asking.choice, rendered, at: position_of(q), index: self.index, place };
        match self.dec.decide(self.t, &asked) {
            Ok(i) if i < rendered.options.len() => {
                self.index += 1;
                self.asked += 1;
                Ok(rendered.options[i].id.clone())
            }
            Ok(i) => {
                self.error = Some(format!("pick {i} of {} options", rendered.options.len()));
                Err(Stop::Blocked)
            }
            Err(e) => {
                self.error = Some(e);
                Err(Stop::Blocked)
            }
        }
    }
}

/// What one step did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stepped {
    /// The step ran; `asked` questions were answered.
    Played { arm: Arm, asked: usize },
    /// The game is over.
    Finished,
}

fn glued(arm: Arm) -> bool {
    matches!(arm, Arm::TurnPrelude | Arm::StrategyAnnounce)
}

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

fn stop_text(t: &str, stop: &Stop, asker: &LabAsker<'_>) -> String {
    asker.error.clone().unwrap_or_else(|| format!("{t}: {stop:?}"))
}

thread_local! {
    static ENV_NS: std::cell::Cell<u128> = const { std::cell::Cell::new(0) };
}

/// Reset the env-build timer of this thread ([`env_ns_take`]).
pub fn env_ns_reset() {
    ENV_NS.with(|c| c.set(0));
}

/// Nanoseconds spent building the `Env` in [`step`] on this thread since the last reset.
#[must_use]
pub fn env_ns_take() -> u128 {
    ENV_NS.with(|c| c.replace(0))
}

/// One step of the loop, the questions put to `dec`. `index` is the decision number of the next question (advanced by the
/// questions answered). On an error the position may be half-stepped: the caller drops the game.
///
/// # Errors
/// A stopped live call or a decider error.
pub fn step(t: &Table, pos: &mut Pos, dec: &mut dyn Decide, index: &mut usize) -> Result<Stepped, String> {
    let te = Instant::now();
    let env = live::env::env_with_plays(&pos.render, &pos.core, t.kernel, t.seeds.clone(), t.sources);
    ENV_NS.with(|c| c.set(c.get() + te.elapsed().as_nanos()));
    let lcx = live::Cx::new(&pos.render, &env);
    let mut asker = LabAsker { t, dec, index: *index, asked: 0, error: None };
    // 1. The reconcile hooks (not the second half of a glued pair).
    for _ in 0..8 {
        if pos.after_glue {
            break;
        }
        let left_open = is_open::<TurnFlowWin>(&pos.core);
        let mut lone = live::LoneCard(&mut asker);
        match live::live_reconcile(&mut pos.core, &lcx, &mut lone) {
            Ok(s) if s.finished && !left_open => break,
            Ok(_) => {}
            Err(stop) => return Err(stop_text("reconcile", &stop, &asker)),
        }
    }
    // 2. The arm.
    let facts = Facts::of_core(&pos.core, false);
    let arm = step_arm(&facts, asks_of(&pos.core));
    if arm == Arm::Finished {
        return Ok(Stepped::Finished);
    }
    if live_arm(arm).is_none() {
        return Err(format!("the arm {} has no live function", live::arms::name(arm)));
    }
    // 3. The native step.
    let out = {
        let mut lone = live::LoneCard(&mut asker);
        match live::live_step(&mut pos.core, arm, &lcx, &mut lone) {
            Ok(out) => out,
            Err(stop) => return Err(stop_text(live::arms::name(arm), &stop, &asker)),
        }
    };
    let asked = out.asked.len();
    *index = asker.index;
    drop(lcx);
    drop(env);
    // 4. The hand-overs between the arms.
    if out.finished && arm == Arm::Tactical && pos.core.pending().window(WindowKind::Tactical).is_none() && pos.core.pending().window(WindowKind::Aftermath).is_none() && turn::open_job_of(&pos.core).is_none() {
        turn::open_finish(&mut pos.core);
    }
    // 5. Commit: the code table.
    let _ = pos.render.sync_codes(&mut pos.core.texts.codes);
    let handed_off = arm == Arm::Tactical && pos.core.pending().window(WindowKind::Aftermath).is_some();
    pos.after_glue = out.finished && (glued(arm) || arm == Arm::Aftermath || handed_off || (pos.after_glue && arm == Arm::Tactical && asked == 0));
    Ok(Stepped::Played { arm, asked })
}

/// Whether the rendered option `i` names the core option `i` where the wire id is the core's name (activations, the draft,
/// Thunder's Edge Warfare's systems); `true` for the other options (their ids are not names). With the count check of
/// [`aligned`] this is the 1:1 option-order assertion of the plan (2.1, risk R4).
#[must_use]
pub fn option_named(rendered: &str, o: &ti4_engine_core::choice::Opt) -> bool {
    use ti4_engine::generated::IdStr;
    use ti4_engine_core::choice::{strategy, tactical, Opt};
    match o {
        Opt::Tactical(tactical::Opt::Activate(s)) | Opt::Strategy(strategy::Opt::System(s)) => rendered == s.as_str(),
        Opt::Strategy(strategy::Opt::Card(c)) => rendered == c.as_str(),
        _ => true,
    }
}

/// The wire options are the core options in the same order (count, and the names where the ids are names).
#[must_use]
pub fn aligned(a: &Asked<'_>) -> bool {
    a.rendered.options.len() == a.choice.opts.len() && a.choice.opts.iter().zip(&a.rendered.options).all(|(o, r)| option_named(&r.id, o))
}

/// SplitMix64: the lab's deterministic noise (exploration, subsampling) from a seed and a counter.
#[must_use]
pub const fn mix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

/// A uniform number in `[0, 1)` from 53 bits of [`mix`].
#[must_use]
pub fn unit(x: u64) -> f64 {
    (mix(x) >> 11) as f64 / (1u64 << 53) as f64
}

/// The outcome of an ended game by absolute seat: `z` (1 for the most victory points, a tie split) and the victory points.
#[must_use]
pub fn outcome(vp: &[i16]) -> (Vec<f32>, Vec<f32>) {
    let vp: Vec<f32> = vp.iter().map(|v| f32::from(*v)).collect();
    let best = vp.iter().copied().fold(f32::MIN, f32::max);
    let winners = vp.iter().filter(|v| **v == best).count() as f32;
    (vp.iter().map(|v| if *v == best { 1.0 / winners } else { 0.0 }).collect(), vp)
}

/// The rule bot at every seat.
pub struct RuleBot;

impl Decide for RuleBot {
    fn decide(&mut self, t: &Table, a: &Asked<'_>) -> Result<usize, String> {
        Ok(rule_bot(t, a).0)
    }
}

/// How a game ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ended {
    pub finished: bool,
    pub round: u32,
    pub steps: usize,
    pub decisions: usize,
    pub vp: Vec<i16>,
    pub error: Option<String>,
}

/// Play a whole game with `dec` (at most `max_steps` steps, `max_rounds` rounds). `on_step` is called after every step.
pub fn play(t: &Table, pos: &mut Pos, dec: &mut dyn Decide, max_steps: usize, max_rounds: u32, mut on_step: impl FnMut(&Pos, Stepped)) -> Ended {
    ti4_rule_bot::backward::set_memory(ti4_rule_bot::backward::Memory::default());
    let (mut steps, mut index) = (0, 0);
    let mut error = None;
    while steps < max_steps && pos.round() <= max_rounds {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| step(t, pos, dec, &mut index))) {
            Ok(Ok(Stepped::Finished)) => break,
            Ok(Ok(s)) => {
                steps += 1;
                on_step(pos, s);
            }
            Ok(Err(e)) => {
                error = Some(e);
                break;
            }
            Err(p) => {
                error = Some(format!("PANIC: {}", p.downcast_ref::<String>().cloned().or_else(|| p.downcast_ref::<&str>().map(|s| (*s).to_owned())).unwrap_or_default()));
                break;
            }
        }
    }
    Ended { finished: pos.finished(), round: pos.round(), steps, decisions: index, vp: pos.vp(), error }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permutations_cover_all_seatings() {
        let all: std::collections::BTreeSet<[&str; 4]> = (0..24).map(permutation).collect();
        assert_eq!(all.len(), 24);
    }
}
