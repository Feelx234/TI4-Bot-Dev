//! The oracle's `NativeSubject` (package KF5, `docs/D2_LF_RESUME.md` section 5): a legacy `Game` that runs the NATIVE arms of the live
//! loop in shadow, step by step, and reports every difference between what the core did and what the legacy step did.
//!
//! BRIDGE ONLY. The legacy game is the lender: it steps for real, answered by the oracle's script like any follower, so the game never
//! diverges from the legacy line and every step is compared from a true position (no cascade of consequences). Before each step the
//! subject
//!
//! 1. reads the arm the legacy step loop will take ([`Game::step_facts`] and `driver::session::step_arm`);
//! 2. when the arm is in the native set and has a live function, materializes a `CoreState` from the game (`Codec::load`, the same
//!    load the lend uses), and opens the arm's window on it from the windows the legacy game holds ([`Game::observed_windows`], the
//!    payloads `d1::pending` compares) with [`open_arm`];
//! 3. otherwise counts the step as LENT (`product::note_lent("oracle::<arm>")`, [`lent_counts`]): the scaffold of the plan, never a
//!    silent skip.
//!
//! After the legacy step, the native arm runs on the materialized position with the SAME answers (`live::live_step` with `Scripted`),
//! and the two are compared:
//!
//! | what | path of the difference | category |
//! |---|---|---|
//! | how many questions, and each question's player, prompt and option ids | `native.questions...` | `Decision` |
//! | the plain events the core queued against the legacy step's new events | `native.events...` | `Event` |
//! | the position: the core's, materialized, against the legacy `GameState` after the step | `native.state...` | `State` |
//! | the window ended exactly when the legacy window ended; the native run did not stop | `native.finished`, `native.stop` | `Value` |
//! | answers the native arm did not consume | `native.questions.unconsumed` | `Decision` |
//!
//! A step that ENDS the window is compared on the prefix of the questions and events only: the legacy step goes on with the close the
//! core does not own yet (the string's remainder, e.g. steps 81.6-81.8 after the last token), asks more, emits more and moves the very
//! pools the last token filled. The position at the end of a window is compared when the close is the core's too.
//!
//! They reach the harness through [`Subject::take_extra`], so an allow-list entry selects them by `area: "driver"`, label
//! `oracle::<subtype>` and the path, exactly like the rest of the oracle.
//!
//! # How a lane wires its arm
//!
//! Register the live function (`driver::live::live_arm`/`ask_arm_fn`, the recipe of `docs/D2_LF_RESUME.md` section 2) and add ONE
//! line to [`open_arm`]: how the observed window of the arm opens its core window on the materialized position. An arm without a line
//! there is counted lent with the reason `no opener`. The set of native arms is [`set_native_arms`] (the `oracle` bin's `--native`).

use std::collections::BTreeMap;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::Mutex;

use serde_json::{json, Value};
use ti4_engine::choice::DecisionRecord;
use ti4_engine::core_sync::codec::rng::TextSeeds;
use ti4_engine::core_sync::{materialize, Codec, Ctx, Sources};
use ti4_engine::live::{self, arms, Scripted};
use ti4_engine::render::Render;
use ti4_engine_core::driver::live::{live_arm, open, LiveError};
use ti4_engine_core::kernel::ability::Unknown;
use ti4_engine_core::kernel::turn_codes::TurnGap;
use ti4_engine_core::driver::session::{bit, live_ask_arm, step_arm, Arm, AskArm};
use ti4_engine_core::driver::step::NativeArms;
use ti4_engine_core::rules::token_gain::TokenGain;
use ti4_engine_core::state::{CoreState, Slice, SliceSet, Window, WindowKind};
use ti4_selfplay::spec::GameSpec;

use crate::oracle::{LegacyMask, MaskSubject, Pilot, Raw, StepInfo, Subject};
use ti4_engine::dual::{diff_json, Category};
use ti4_engine::rng::RngPositions;

/// Leaves reported per comparison.
const LEAVES: usize = 8;

static NATIVE: Mutex<NativeArms> = Mutex::new(NativeArms::NONE);
static GAPS_LENT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static CLOSING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static KEEP_GOING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static KEPT: Mutex<BTreeMap<String, (u64, String)>> = Mutex::new(BTreeMap::new());
static LENT: Mutex<BTreeMap<String, u64>> = Mutex::new(BTreeMap::new());
static RAN: Mutex<BTreeMap<String, u64>> = Mutex::new(BTreeMap::new());

/// The arms the next `NativeSubject`s run natively (default: none, i.e. every step is lent).
pub fn set_native_arms(arms: NativeArms) {
    *NATIVE.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = arms;
}

/// Count a live window's refusal of a position (`LiveError::Unknown`, the gap of the native playthrough) as a LENT step, with its cause, instead of a
/// difference (`oracle --gaps-lent`): the legacy-led survey of the gaps of whole games. Default off: a bail is a failure.
pub fn set_gaps_are_lent(on: bool) {
    GAPS_LENT.store(on, std::sync::atomic::Ordering::Relaxed);
}

/// Shadow the closing menu too (`oracle --closing`): the `Ask` arm is native for the closing menu of a turn (`driver::turn::ask`), but the menu lists
/// the free options of sources the core does not assemble completely yet (a trade partner of a seat that reaches past adjacency: M1), so the
/// comparison is opt-in and reports a difference where the core's menu is shorter than the legacy one. Default off.
pub fn set_shadow_closing(on: bool) {
    CLOSING.store(on, std::sync::atomic::Ordering::Relaxed);
}

fn shadow_closing() -> bool {
    CLOSING.load(std::sync::atomic::Ordering::Relaxed)
}

/// Do not stop a scenario at the first difference: the follower of a `NativeSubject` is a legacy game, so a difference never makes the two games diverge, and
/// the whole game can be surveyed (`oracle --keep-going`). The differences are collected by kind in [`kept_differences`] instead of being returned to the
/// harness (which would stop at the first).
pub fn set_keep_going(on: bool) {
    KEEP_GOING.store(on, std::sync::atomic::Ordering::Relaxed);
}

/// The differences collected under [`set_keep_going`], by `<arm> <category> <path with the indices blanked>`: how many, and the first one's two sides (taken).
#[must_use]
pub fn kept_differences() -> BTreeMap<String, (u64, String)> {
    std::mem::take(&mut *KEPT.lock().unwrap_or_else(std::sync::PoisonError::into_inner))
}

/// `[12]` and `.12.` of a path to `[]` and `.#.`: the same difference at another place of the board is one kind.
fn blank_indices(path: &str) -> String {
    let mut out = String::new();
    let mut digits = false;
    for c in path.chars() {
        if c.is_ascii_digit() {
            if !digits {
                out.push('#');
            }
            digits = true;
        } else {
            digits = false;
            out.push(c);
        }
    }
    out
}

fn gaps_are_lent() -> bool {
    GAPS_LENT.load(std::sync::atomic::Ordering::Relaxed)
}

fn native_arms() -> NativeArms {
    *NATIVE.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The steps left to the legacy engine since the last call, by `<arm>: <reason>` (taken).
#[must_use]
pub fn lent_counts() -> BTreeMap<String, u64> {
    std::mem::take(&mut *LENT.lock().unwrap_or_else(std::sync::PoisonError::into_inner))
}

/// The steps a native arm ran in shadow since the last call, by arm name (taken). A run with an empty table compared nothing.
#[must_use]
pub fn ran_counts() -> BTreeMap<String, u64> {
    std::mem::take(&mut *RAN.lock().unwrap_or_else(std::sync::PoisonError::into_inner))
}

fn count(table: &Mutex<BTreeMap<String, u64>>, key: String) {
    *table.lock().unwrap_or_else(std::sync::PoisonError::into_inner).entry(key).or_default() += 1;
}

fn lend(arm: Arm, why: &str) {
    count(&LENT, format!("{}: {why}", arms::name(arm)));
    ti4_engine::product::note_lent(&format!("oracle::{}", arms::name(arm)));
}

/// How the observed window of `arm` opens its core window on `core`. `false`: no opener for the arm (the step is lent).
///
/// THE line a lane adds when its arm gets a live function.
pub fn open_arm(arm: Arm, windows: &[Window], core: &mut CoreState) -> bool {
    match arm {
        Arm::Tokens => match windows.iter().find_map(|w| if let Window::Tokens(t) = w { Some(t.clone()) } else { None }) {
            Some(t) => {
                open(core, &TokenGain::new(*t));
                true
            }
            None => false,
        },
        // The transaction window (lane M1): the payload the legacy game holds, free-form deal included.
        Arm::Trade => open_observed(windows, core, WindowKind::Trade),
        // The scoring windows of the status phase and of the secrets timed on an action or an agenda (lane C): the payload the legacy game holds.
        Arm::Scoring => open_observed(windows, core, WindowKind::Scoring),
        Arm::EventScoring => open_observed(windows, core, WindowKind::EventScoring),
        // The jobs of the turn flow open themselves (`driver::turn`): they need no window of the legacy game. `Status` opens the scoring window.
        Arm::TurnPrelude | Arm::Phase | Arm::StrategyAnnounce | Arm::Ask | Arm::Status => true,
        _ => false,
    }
}

/// Open the window of `kind` the legacy game holds on the materialized position. `false`: it holds none.
fn open_observed(windows: &[Window], core: &mut CoreState, kind: WindowKind) -> bool {
    match windows.iter().find(|w| w.kind() == kind) {
        Some(w) => {
            core.pending_mut().open(w.clone());
            true
        }
        None => false,
    }
}

/// Whether the arm's step ends where the legacy step does: its close is the core's too, so the end of the step is compared on everything
/// (the questions, the events and the position), not on the prefix. `Phase` is the whole of `step_phase`, and `Ask` (the draft, the starting technology, the closing menu) is the whole of its step; the prelude and the
/// announcement go on into the question of the same legacy step (the action menu, the draft), which the arm does not own.
const fn close_is_native(arm: Arm) -> bool {
    matches!(arm, Arm::Phase | Arm::Ask | Arm::Status | Arm::Tokens | Arm::Scoring | Arm::EventScoring | Arm::Trade)
}


/// The arms whose native step also runs the close when it applies the LAST answer, where the legacy game closes in its NEXT step (a step
/// that finds nothing to ask): the legacy answer step is compared on the prefix, the legacy close step on everything.
const fn closes_one_step_early(arm: Arm) -> bool {
    matches!(arm, Arm::Scoring | Arm::EventScoring)
}

/// The `private:#staged:event:*` marks of a string position (events staged for the next step's announcement).
fn staged_marks(gs: &ti4_model::state::GameState) -> Vec<&String> {
    gs.faction_marks.keys().filter(|k| k.starts_with("private:#staged:event:")).collect()
}

/// What the legacy game keeps outside its `GameState` and the core keeps in `Pending` (the driver flags, the prepared turn, the closing menu, the
/// actions of the turn): primed from the legacy facts, so the core's own `Facts` and the questions that read them agree with the legacy step's.
fn prime(core: &mut CoreState, facts: &ti4_engine_core::driver::session::Facts, actions_this_turn: u8) {
    use ti4_engine_core::state::DriverFlags as F;
    let (turn_seq, active) = (core.meta().turn_seq, core.meta().active);
    let p = core.pending_mut();
    p.driver.set(F::STATUS_RESOLVED, facts.status_resolved);
    p.driver.set(F::AGENDA_RESOLVED, facts.agenda_resolved);
    p.driver.set(F::STRATEGY_PHASE_ANNOUNCED, facts.strategy_announced);
    p.prepared_turn_seq = facts.turn_prepared.then_some(turn_seq);
    p.turn_closing = if facts.turn_closing { active } else { None };
    p.actions_this_turn = actions_this_turn;
}

/// The slices a position loads: all, and the map only when the game has a string galaxy (as `Game::churn_state`).
fn slices_for(has_galaxy: bool) -> SliceSet {
    if has_galaxy { SliceSet::ALL } else { SliceSet(SliceSet::ALL.0 & !(1 << Slice::Map as u32)) }
}

/// The window kind whose closing is the end of `arm`'s step (`None`: the arm has no window of its own to compare).
const fn window_of(arm: Arm) -> Option<WindowKind> {
    match arm {
        Arm::Tokens => Some(WindowKind::Tokens),
        Arm::Scoring => Some(WindowKind::Scoring),
        Arm::EventScoring => Some(WindowKind::EventScoring),
        Arm::Trade => Some(WindowKind::Trade),
        _ => None,
    }
}

/// What the subject keeps from the position before the step.
struct Shadow {
    arm: Arm,
    core: CoreState,
    cx: Ctx,
    render: Render,
    base: ti4_model::state::GameState,
    sources: ti4_model::content_types::SourceSet,
}

/// A legacy game that shadow-runs the native arms (see the module header).
pub struct NativeSubject {
    inner: MaskSubject<LegacyMask>,
    arms: NativeArms,
    /// The answers given to this subject since its last step (the shadow run takes the same).
    answers: VecDeque<String>,
    extra: Vec<Raw>,
}

impl NativeSubject {
    /// Materialize the position and open the arm's window, or count the step as lent.
    fn prepare(&self) -> Option<Shadow> {
        let g = self.inner.game();
        let facts = g.step_facts();
        // A question is waiting: the legacy step takes the `Ask` arm (or, with nothing to ask, `Phase`).
        let arm = step_arm(&facts, self.inner.options().is_some());
        if !self.arms.has(arm) || live_arm(arm).is_none() {
            lend(arm, "not native");
            return None;
        }
        // The `Ask` arm is native for the sources of question the turn flow owns (the draft, the starting technology; the closing menu with
        // [`set_shadow_closing`]); the others are lent.
        if arm == Arm::Ask && !matches!(live_ask_arm(&facts), AskArm::Draft | AskArm::StartingTech) && !(shadow_closing() && live_ask_arm(&facts) == AskArm::Closing) {
            lend(arm, "question source not native");
            return None;
        }
        let Ok(mut cx) = Ctx::of(&g.state) else {
            lend(arm, "no owners table");
            return None;
        };
        let render = Render::new(cx.owners.ids().to_vec());
        let windows = match g.observed_windows(&render, &cx) {
            Ok(w) => w,
            Err(why) => {
                lend(arm, why);
                return None;
            }
        };
        let slices = slices_for(g.galaxy().is_some());
        let positions = g.rng_positions();
        let loaded = Codec::load(&mut cx, &g.state, Sources { galaxy: g.galaxy(), rng: Some(&positions) }, slices);
        let Ok(mut core) = loaded else {
            lend(arm, "the position does not load");
            return None;
        };
        prime(&mut core, &facts, g.actions_this_turn());
        if !staged_marks(&g.state).is_empty() {
            // The string keeps the events its effects staged as `private:#staged:event:*` marks; the core announces them from `Pending::staged`, which a
            // loaded position does not fill. A step that would announce them is the string's.
            lend(arm, "staged events (string marks)");
            return None;
        }
        if !open_arm(arm, &windows, &mut core) {
            lend(arm, "no opener");
            return None;
        }
        Some(Shadow { arm, core, cx, render, base: g.state.clone(), sources: g.content_sources() })
    }

    /// Hand the differences of the step to the harness, or collect them under [`set_keep_going`].
    fn report(&mut self, arm: Arm, out: Vec<Raw>) {
        if !KEEP_GOING.load(std::sync::atomic::Ordering::Relaxed) {
            self.extra.extend(out);
            return;
        }
        let mut kept = KEPT.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        for r in out {
            let key = format!("{} {:?} {}", arms::name(arm), r.category, blank_indices(&r.path));
            let entry = kept.entry(key).or_insert_with(|| (0, format!("string {} | core {}", r.left, r.right)));
            entry.0 += 1;
        }
    }

    /// Run the arm on the materialized position with the answers of the legacy step and compare with what the legacy step did.
    fn compare(&mut self, mut shadow: Shadow, answers: Vec<String>, records: &[DecisionRecord], events: &[String]) {
        let name = arms::name(shadow.arm);
        // The action menu is the core's, its pass and its transactions too; the body of every other option is a counted gap of the core
        // (`MenuGap::Action`: the entries of the tactical and strategic lanes, the component actions' performers): the legacy step ran it, the
        // shadow counts it lent instead of comparing a step it cannot take.
        if matches!(shadow.arm, Arm::Ask | Arm::TurnPrelude) && answers.iter().any(|a| menu_body_is_lent(a)) {
            lend(shadow.arm, "the body of an action-menu option the core does not run yet");
            return;
        }
        let kernel = live::kernel::kernel_for(shadow.sources);
        let env = live::env::env_with_plays(&shadow.render, &shadow.core, kernel, Rc::new(TextSeeds::of(&shadow.cx)), shadow.sources);
        let cx = live::Cx::new(&shadow.render, &env);
        let answers_taken = answers.clone();
        let mut asker = Scripted::new(answers);
        // The last card of the draft is not asked by the table either.
        let mut lone = live::LoneCard(&mut asker);
        // Every legacy step starts with the reconcile hooks (`Game::step_flow`): the shadow does too, with the same asker.
        let from = shadow.core.ledger.events.len();
        loop {
            match live::live_reconcile(&mut shadow.core, &cx, &mut lone) {
                Ok(step) if step.finished => break,
                Ok(_) => {}
                Err(stop) => {
                    let mut out = Vec::new();
                    leaves(Category::Value, "native.stop", &json!("reconciled"), &json!(format!("{stop:?}")), &mut out);
                    self.report(shadow.arm, out);
                    return;
                }
            }
        }
        let result = live::live_step(&mut shadow.core, shadow.arm, &cx, &mut lone);
        #[cfg(test)]
        tests::tamper(&mut shadow.core);
        let mut out = Vec::new();
        let step = match result {
            Ok(step) => step,
            // The closing menu offers a free option of a source of the action menu the core does not assemble yet (lane F): counted lent, as an
            // arm without an opener is, not a difference of the line.
            Err(live::Stop::Live(LiveError::Unknown(Unknown::TurnGap(TurnGap::MenuSources)))) => {
                lend(shadow.arm, "closing menu sources");
                return;
            }
            Err(live::Stop::Live(LiveError::Unknown(why))) if gaps_are_lent() => {
                lend(shadow.arm, &format!("gap {why:?}"));
                return;
            }
            // The arm closed one legacy step early and the close asks something: the answers are the next legacy step's (the close step),
            // where the arm is run again from the legacy position and compared in full.
            Err(live::Stop::Exhausted) if closes_one_step_early(shadow.arm) => {
                count(&RAN, format!("{name} (the close needs the next legacy step's answers)"));
                return;
            }
            Err(stop) => {
                count(&RAN, name.to_owned());
                leaves(Category::Value, "native.stop", &json!("ran"), &json!(format!("{stop:?}")), &mut out);
                self.report(shadow.arm, out);
                return;
            }
        };
        count(&RAN, name.to_owned());
        let finished = step.finished;
        let open_after = window_of(shadow.arm).map(|kind| self.inner.game().step_facts().open & bit(kind) != 0);
        // A step whose close is the legacy's is compared on the prefix once the window ended; so is a step that closed one legacy step early.
        let early = closes_one_step_early(shadow.arm) && finished && open_after == Some(true);
        let ended = finished && (!close_is_native(shadow.arm) || early);
        if ended {
            count(&RAN, format!("{name} (window ended; the close is {})", if early { "the next legacy step's" } else { "the legacy's" }));
        }
        let native: Vec<Value> = asker.seen.iter().map(|c| json!({ "player": c.player.as_str(), "prompt": c.prompt, "options": c.options.iter().map(|o| o.id.as_str()).collect::<Vec<_>>() })).collect();
        let legacy: Vec<Value> = records.iter().map(|r| json!({ "player": r.player.as_str(), "prompt": r.prompt, "options": r.offered })).collect();
        out.extend(compare_questions(ended, early, &legacy, &native, asker.left()));
        let pushed = ti4_engine::render::events::pushed_since(&shadow.render, &shadow.cx, &shadow.core, from);
        match pushed {
            Some(lines) => {
                let texts: Vec<String> = lines.iter().map(|p| p.text().to_owned()).collect();
                out.extend(compare_events(ended, early, events, &texts));
            }
            None => leaves(Category::Event, "native.events", &json!(events), &json!("a line the tables cannot name"), &mut out),
        }
        // The events an effect staged are marks of JSON in the legacy state and `Pending::staged` in the core: compared by name, in order, and kept
        // out of the position (the position carries only what the codec carries).
        let mut legacy_state = self.inner.state_json();
        let legacy_staged = strip_staged(&mut legacy_state);
        let native_staged: Vec<String> = shadow.core.pending().staged.iter().map(|e| ti4_engine::kernel_sync::text::event_text(&shadow.render, e.kind)).collect();
        if !native_staged.is_empty() || !legacy_staged.is_empty() {
            leaves(Category::Event, "native.staged", &json!(legacy_staged), &json!(native_staged), &mut out);
        }
        match materialize::materialize(&shadow.core, &shadow.cx, &shadow.base) {
            Ok(after) => {
                let mut native_state = serde_json::to_value(&after).unwrap_or(Value::Null);
                strip_staged(&mut native_state);
                out.extend(compare_state(ended, &legacy_state, &native_state));
            }
            Err(e) => leaves(Category::State, "native.state", &json!("materializes"), &json!(e.to_string()), &mut out),
        }
        // The window ended when the legacy window ended.
        if let Some(open_after) = open_after {
            leaves(Category::Value, "native.finished", &json!(!open_after || early), &json!(step.finished), &mut out);
        }
        if !out.is_empty() && std::env::var_os("TI4_NATIVE_TRACE").is_some() {
            eprintln!("native difference in arm {name}: answers {answers_taken:?}, legacy events {events:?}");
        }
        self.report(shadow.arm, out);
    }
}

// A step that ends the window ALSO runs the close the core does not own yet (the string's remainder of the step: steps 81.6-81.8 after the
// last token): the legacy step then asked, answered and emitted more than the arm, and changed the position beyond it (the remainder moves
// the very pools the last token filled). So a step that ends the window is compared on the prefix of the questions and the events only; a
// step that leaves it open must agree on everything, the position included. The position at the end of a window is compared when the
// close is the core's too (KS5).

/// Whether an option id of the action menu names a body the core does not run yet (`driver::turn::action::gap_code`): tactical and strategic
/// actions, the component actions of the leaders, technologies, relics, exploration cards, action cards and factions. The pass and the opening
/// of a transaction (`component|trade|<faction>`) are the core's.
fn menu_body_is_lent(id: &str) -> bool {
    id == "tactical"
        || id == "strategic"
        || id.starts_with("strategic|")
        || id.starts_with("faction|")
        || (id.starts_with("component|") && !id.starts_with("component|trade|"))
        || id.starts_with("action_card|")
        || id.starts_with("purge|")
        || id.starts_with("relic|")
        || id.starts_with("play_area:")
}

/// Take the `private:#staged:event:*` marks out of a position: the `type` of each, in key order.
fn strip_staged(state: &mut Value) -> Vec<String> {
    let mut names = Vec::new();
    if let Some(marks) = state.get_mut("faction_marks").and_then(Value::as_object_mut) {
        let keys: Vec<String> = marks.keys().filter(|k| k.starts_with("private:#staged:event:")).cloned().collect();
        for key in keys {
            let text = marks.remove(&key).and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_default();
            let event: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
            names.push(event.get("type").and_then(Value::as_str).unwrap_or("?").to_owned());
        }
    }
    names
}

/// The questions of the step: the legacy step's against the arm's (`native.questions...`); the answers the arm did not take are a
/// difference unless the window ended (the remainder takes them).
fn compare_questions(finished: bool, early: bool, legacy: &[Value], native: &[Value], unconsumed: usize) -> Vec<Raw> {
    let mut out = Vec::new();
    let legacy = if finished && legacy.len() >= native.len() { &legacy[..native.len()] } else { legacy };
    // The arm closed one legacy step early: what it asked beyond the legacy step is the next legacy step's.
    let native = if early && native.len() >= legacy.len() { &native[..legacy.len()] } else { native };
    leaves(Category::Decision, "native.questions", &Value::Array(legacy.to_vec()), &Value::Array(native.to_vec()), &mut out);
    if unconsumed > 0 && !finished {
        leaves(Category::Decision, "native.questions.unconsumed", &json!(0), &json!(unconsumed), &mut out);
    }
    out
}

/// The plain events of the step (`native.events...`).
fn compare_events(finished: bool, early: bool, legacy: &[String], native: &[String]) -> Vec<Raw> {
    let mut out = Vec::new();
    let legacy = if finished && legacy.len() >= native.len() { &legacy[..native.len()] } else { legacy };
    let native = if early && native.len() >= legacy.len() { &native[..legacy.len()] } else { native };
    leaves(Category::Event, "native.events", &json!(legacy), &json!(native), &mut out);
    out
}

/// The position after the step (`native.state...`); not compared when the window ended.
fn compare_state(finished: bool, legacy: &Value, native: &Value) -> Vec<Raw> {
    let mut out = Vec::new();
    if !finished {
        leaves(Category::State, "native.state", legacy, native, &mut out);
    }
    out
}

/// The first leaf where `left` and `right` differ (`native_play`'s shadow compare), `None` when they agree.
pub(crate) fn first_leaf(left: &Value, right: &Value) -> Option<ti4_engine::dual::JsonDiff> {
    diff_json(left, right, 1).into_iter().next()
}

fn leaves(category: Category, prefix: &str, left: &Value, right: &Value, out: &mut Vec<Raw>) {
    for d in diff_json(left, right, LEAVES) {
        let path = if d.path.is_empty() {
            prefix.to_owned()
        } else if d.path.starts_with('[') {
            format!("{prefix}{}", d.path)
        } else {
            format!("{prefix}.{}", d.path)
        };
        out.push(Raw { category, path, left: d.left, right: d.right });
    }
}

impl Subject for NativeSubject {
    fn new(spec: &GameSpec, pilot: Pilot) -> Result<Self, String> {
        Ok(Self { inner: MaskSubject::new(spec, pilot)?, arms: native_arms(), answers: VecDeque::new(), extra: Vec::new() })
    }

    fn options(&self) -> Option<ti4_engine::choice::Choice> {
        self.inner.options()
    }

    fn answer(&mut self, option_id: &str) {
        self.answers.push_back(option_id.to_owned());
        self.inner.answer(option_id);
    }

    fn step(&mut self) -> StepInfo {
        let answers: Vec<String> = self.answers.drain(..).collect();
        let shadow = self.prepare();
        let info = self.inner.step();
        if let Some(shadow) = shadow {
            let (records, events) = (self.inner.peek_records().to_vec(), self.inner.peek_events().to_vec());
            self.compare(shadow, answers, &records, &events);
        }
        info
    }

    fn state_json(&self) -> Value {
        self.inner.state_json()
    }

    fn events_since(&mut self) -> Vec<String> {
        self.inner.events_since()
    }

    fn decisions_since(&mut self) -> Vec<DecisionRecord> {
        self.inner.decisions_since()
    }

    fn rng_positions(&self) -> RngPositions {
        self.inner.rng_positions()
    }

    // mutants:skip (delegation to the legacy subject)
    fn in_transaction(&self) -> bool {
        self.inner.in_transaction()
    }

    // mutants:skip (delegation to the legacy subject)
    fn finished(&self) -> bool {
        self.inner.finished()
    }

    fn round(&self) -> u32 {
        self.inner.round()
    }

    fn rules_version(&self) -> u32 {
        self.inner.rules_version()
    }

    // mutants:skip (delegation to the legacy subject)
    fn disabled(&self) -> Option<String> {
        self.inner.disabled()
    }

    fn decisions_hash(&self) -> String {
        self.inner.decisions_hash()
    }

    fn take_extra(&mut self) -> Vec<Raw> {
        std::mem::take(&mut self.extra)
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use ti4_engine::seating::LOBBY_FACTIONS;

    use super::*;
    use crate::allow_list::{AllowList, Tally};
    use crate::oracle::{play_scenario, Judge, Limits};

    /// The counters and the native set are process-wide: the tests of this module run one at a time.
    static SERIAL: Mutex<()> = Mutex::new(());

    thread_local! {
        static TAMPER: Cell<u8> = const { Cell::new(0) };
    }

    /// Test-only: perturb the native position after the arm ran (1: a trade good, 2: a spent command token), to see the compare find it.
    pub(super) fn tamper(core: &mut CoreState) {
        match TAMPER.with(Cell::get) {
            1 => {
                if let Some(p) = core.players_mut().by_seat_mut(ti4_engine_core::Seat(0)) {
                    p.econ.trade_goods += 1;
                }
            }
            2 => {
                if let Some(p) = core.players_mut().by_seat_mut(ti4_engine_core::Seat(1)) {
                    p.econ.fleet_tokens += 1;
                }
            }
            _ => {}
        }
    }

    fn spec() -> GameSpec {
        spec_of(&LOBBY_FACTIONS[..4])
    }

    fn spec_of(factions: &[&str]) -> GameSpec {
        GameSpec {
            setup: "native".to_owned(),
            seed: 11,
            players: 4,
            map: "4pOuterRim".to_owned(),
            factions: factions.iter().map(|f| (*f).to_owned()).collect(),
            speaker: "seat".to_owned(),
            card_set: None,
            rotation: None,
            starting_tech: true,
            thunders_edge: false,
            rules_version: None,
        }
    }

    /// Round one of a game, up to and including its status phase.
    const LIMITS: Limits = Limits { max_rounds: 1, max_steps: 20_000 };

    fn play() -> crate::oracle::Played {
        play_spec(&spec())
    }

    fn play_spec(spec: &GameSpec) -> crate::oracle::Played {
        play_spec_for(spec, LIMITS)
    }

    fn play_spec_for(spec: &GameSpec, limits: Limits) -> crate::oracle::Played {
        let list = AllowList::default();
        let mut tally = Tally::new(&list);
        let mut judge = Judge { list: &list, tally: &mut tally, scenario: "native".to_owned() };
        play_scenario::<NativeSubject, MaskSubject<LegacyMask>>(spec, limits, &mut judge).expect("builds").expect("not skipped")
    }

    fn arm_tokens() {
        set_native_arms(NativeArms::of(&[Arm::Tokens]));
        let _ = (lent_counts(), ran_counts());
    }

    /// The token-gain arm of the live loop, run in shadow behind the legacy game through round one: every step of the status phase's token
    /// gain is compared (questions, events, position, end of the window) and the two agree; the other arms are counted lent, not skipped.
    #[test]
    fn the_tokens_arm_shadowed_behind_a_legacy_game_agrees_with_it_and_the_rest_is_counted_lent() {
        let _serial = SERIAL.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        TAMPER.with(|t| t.set(0));
        arm_tokens();
        let played = play();
        let ran = ran_counts();
        let lent = lent_counts();
        assert!(played.report.differences.is_empty(), "{:#?}", played.report.differences);
        assert!(ran.get("tokens").copied().unwrap_or(0) >= 3, "the arm ran in shadow on every token, the last one with its close: {ran:?}");
        assert!(lent.contains_key("tactical: not native") && lent.contains_key("aftermath: not native"), "the other arms are lent and counted: {lent:?}");
        assert!(!lent.keys().any(|k| k.starts_with("tokens:")), "no tokens step was lent: {lent:?}");
    }

    /// The status phase's arms (the opening, the scoring window, the token gain with its close, and the event window when a secret asks for
    /// one) run in shadow behind a legacy game through round one: every step is compared (questions, events, the position when the window
    /// closes) and the two agree.
    #[test]
    fn the_status_phase_arms_shadowed_behind_a_legacy_game_through_round_one_agree_with_it() {
        let _serial = SERIAL.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        TAMPER.with(|t| t.set(0));
        set_native_arms(NativeArms::of(&[Arm::Status, Arm::Scoring, Arm::EventScoring, Arm::Tokens]));
        let _ = (lent_counts(), ran_counts());
        let played = play();
        let ran = ran_counts();
        let other: Vec<_> = played.report.differences.iter().filter(|d| !d.core_side.contains("CommanderUnlock")).collect();
        assert!(other.is_empty(), "{:#?}", other.iter().take(8).collect::<Vec<_>>());
        eprintln!("ran: {ran:?}");
        for arm in ["status", "scoring", "tokens"] {
            assert!(ran.keys().any(|k| k.starts_with(arm)), "{arm} ran in shadow: {ran:?}");
        }
    }

    /// The jobs of the turn flow (the start of a turn, the phase changes, the announcement of the first strategy phase) run in shadow behind
    /// a legacy game through round one, with the reconcile hooks every step starts with: every step is compared (questions, events, and for
    /// the phase change the position) and the two agree.
    #[test]
    fn the_turn_flow_arms_shadowed_behind_a_legacy_game_through_round_one_agree_with_it() {
        let _serial = SERIAL.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        TAMPER.with(|t| t.set(0));
        set_native_arms(NativeArms::of(&[Arm::TurnPrelude, Arm::Phase, Arm::StrategyAnnounce, Arm::Ask]));
        let _ = (lent_counts(), ran_counts());
        let played = play();
        let ran = ran_counts();
        assert!(played.report.differences.is_empty(), "{:#?}", played.report.differences.iter().take(5).collect::<Vec<_>>());
        for arm in ["turn_prelude", "phase", "strategy_announce", "ask"] {
            assert!(ran.get(arm).copied().unwrap_or(0) + ran.get(&format!("{arm} (window ended; the close is the legacy's)")).copied().unwrap_or(0) >= 1, "{arm} ran in shadow: {ran:?}");
        }
    }

    /// The silent differences the turn-flow arms are known to have against the legacy game (`docs/M7_RESUME.md`): an ability the string registers and the core
    /// registry lacks, so the core does not ask the question the string asks. The text is a part of the string side of the first example. A difference that
    /// is not one of these fails the test; when one is fixed its line must be deleted (the test also demands that every listed one is still seen). Each line is
    /// the words that identify the difference and its consequences in the position.
    ///
    /// * Arborec's Mitosis ("at the start of the status phase, place 1 infantry from your reinforcements on a planet you control", `arborec/offer.rs` lists it as
    ///   "points left"): the string asks where, the core asks nothing and the infantry is missing from the board.
    const KNOWN_SILENT: [&[&str]; 1] = [&["Mitosis", "arborec_infantry"]];

    /// Whole games with every lobby faction in turn (six tables of four), the turn-flow arms in shadow and a counted bail a failure: no difference but the
    /// known silent ones (kept going: a difference never makes the follower diverge, the follower is a legacy game), and the arms ran.
    #[test]
    fn the_turn_flow_arms_agree_with_the_legacy_game_for_every_lobby_faction() {
        let _serial = SERIAL.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        TAMPER.with(|t| t.set(0));
        set_keep_going(true);
        let _ = kept_differences();
        let mut seen = vec![false; KNOWN_SILENT.len()];
        for group in LOBBY_FACTIONS.chunks(4) {
            set_native_arms(NativeArms::of(&[Arm::TurnPrelude, Arm::Phase, Arm::StrategyAnnounce, Arm::Ask]));
            let _ = (lent_counts(), ran_counts());
            let played = ti4_engine::dual::with_bail_is_failure(true, || play_spec(&spec_of(group)));
            let ran = ran_counts();
            assert!(played.report.differences.is_empty(), "{group:?}: kept going, nothing is returned to the harness: {:#?}", played.report.differences.iter().take(5).collect::<Vec<_>>());
            assert!(ran.keys().any(|k| k.starts_with("turn_prelude")), "{group:?}: the prelude ran in shadow: {ran:?}");
            let unknown: Vec<String> = kept_differences()
                .into_iter()
                // The count of answers the core did not take is the other face of a question it did not ask.
                .filter(|(kind, _)| !kind.ends_with("native.questions.unconsumed"))
                .filter(|(kind, (_, example))| match KNOWN_SILENT.iter().position(|known| known.iter().any(|w| example.contains(w) || kind.contains(w))) {
                    Some(at) => {
                        seen[at] = true;
                        false
                    }
                    None => true,
                })
                .map(|(kind, (n, example))| format!("{n}x {kind}: {example}"))
                .collect();
            assert!(unknown.is_empty(), "{group:?}: differences nobody knows: {unknown:#?}");
        }
        set_keep_going(false);
        assert!(seen.iter().all(|s| *s), "a known silent difference is gone: delete its line from KNOWN_SILENT ({seen:?})");
    }

    /// Round one with every lobby faction in turn (six tables of four), the status phase's arms in shadow and a counted bail a failure: no
    /// difference but the counted gap of a commander whose unlock condition no hook states yet (`TurnGap::CommanderUnlock`), and the arms ran.
    #[test]
    fn the_status_phase_arms_agree_with_the_legacy_game_for_every_lobby_faction() {
        let _serial = SERIAL.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        TAMPER.with(|t| t.set(0));
        let (mut gaps, mut ran_status) = (0, 0);
        for group in LOBBY_FACTIONS.chunks(4) {
            set_native_arms(NativeArms::of(&[Arm::Status, Arm::Scoring, Arm::EventScoring, Arm::Tokens]));
            let _ = (lent_counts(), ran_counts());
            let played = ti4_engine::dual::with_bail_is_failure(true, || play_spec(&spec_of(group)));
            let ran = ran_counts();
            let other: Vec<_> = played.report.differences.iter().filter(|d| !d.core_side.contains("CommanderUnlock")).collect();
            gaps += played.report.differences.len() - other.len();
            assert!(other.is_empty(), "{group:?}: {:#?}", other.iter().take(5).collect::<Vec<_>>());
            ran_status += ran.get("status").copied().unwrap_or(0);
        }
        assert!(ran_status >= 1, "the opening of the status phase ran in shadow");
        eprintln!("commander gaps counted: {gaps}");
    }

    /// The trade island (lane M1) in shadow behind real games: the transaction windows the rule bot opens in the first rounds of a table of
    /// each group of lobby factions are run by the core with the same answers and compared step by step (questions as rendered, plain events,
    /// the events left staged, the position at every step, the close). A counted bail is a failure. The window must be seen to open and
    /// be answered, or the run proves nothing.
    #[test]
    fn the_trade_arm_agrees_with_the_legacy_game_over_real_transactions_for_every_lobby_faction() {
        let _serial = SERIAL.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        TAMPER.with(|t| t.set(0));
        let mut steps = 0;
        for group in LOBBY_FACTIONS.chunks(4) {
            set_native_arms(NativeArms::of(&[Arm::Trade]));
            let _ = (lent_counts(), ran_counts());
            let played = ti4_engine::dual::with_bail_is_failure(true, || play_spec_for(&spec_of(group), Limits { max_rounds: 3, max_steps: 60_000 }));
            let ran = ran_counts();
            assert!(played.report.differences.is_empty(), "{group:?}: {:#?}", played.report.differences.iter().take(5).collect::<Vec<_>>());
            steps += ran.iter().filter(|(k, _)| k.starts_with("trade")).map(|(_, n)| *n).sum::<u64>();
            eprintln!("{group:?}: trade steps in shadow {:?}", ran.iter().filter(|(k, _)| k.starts_with("trade")).collect::<Vec<_>>());
        }
        assert!(steps >= 10, "transactions were seen: {steps} trade steps in shadow");
    }

    /// The trade arm's compare finds a perturbed native position too: a trade good more at the end of a step is a `State` difference.
    #[test]
    fn a_perturbed_native_trade_position_is_reported_as_a_state_difference() {
        let _serial = SERIAL.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        set_native_arms(NativeArms::of(&[Arm::Trade]));
        let _ = (lent_counts(), ran_counts());
        TAMPER.with(|t| t.set(1));
        let played = play_spec_for(&spec_of(&LOBBY_FACTIONS[..4]), Limits { max_rounds: 3, max_steps: 60_000 });
        TAMPER.with(|t| t.set(0));
        let hit = played.report.differences.iter().find(|d| d.category == Category::State && d.path.starts_with("native.state") && d.path.contains("trade_goods"));
        assert!(hit.is_some(), "{:#?}", played.report.differences.iter().take(3).collect::<Vec<_>>());
    }

    /// The compare finds a perturbed native position (state), and a perturbed pool (the plain events of the step do not change, the
    /// position does): the harness reports it as a `State` difference at the path of the field.
    #[test]
    fn a_perturbed_native_position_is_reported_as_a_state_difference_at_its_path() {
        let _serial = SERIAL.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        for (knob, field) in [(1u8, "trade_goods"), (2u8, "fleet_tokens")] {
            arm_tokens();
            TAMPER.with(|t| t.set(knob));
            let played = play();
            TAMPER.with(|t| t.set(0));
            let hit = played.report.differences.iter().find(|d| d.category == Category::State && d.path.starts_with("native.state") && d.path.contains(field));
            assert!(hit.is_some(), "knob {knob}: {:#?}", played.report.differences);
            assert_eq!(hit.map(|d| d.label.starts_with("oracle::")), Some(true));
        }
    }

    fn q(prompt: &str) -> Value {
        json!({ "player": "a", "prompt": prompt, "options": ["x"] })
    }

    /// A step that leaves the window open must agree on everything; one that ends it is compared on the prefix (the legacy remainder is the
    /// close the core does not own yet).
    #[test]
    fn a_step_that_ends_the_window_is_compared_on_the_prefix_and_one_that_leaves_it_open_on_everything() {
        let (one, two) = (vec![q("p1")], vec![q("p1"), q("p2")]);
        // The legacy step asked more than the arm.
        assert!(compare_questions(true, false, &two, &one, 0).is_empty(), "the remainder's question is not a difference when the window ended");
        let open = compare_questions(false, false, &two, &one, 0);
        assert_eq!(open.len(), 1, "{open:?}");
        assert_eq!((open[0].category, open[0].path.as_str()), (Category::Decision, "native.questions[1]"));
        // The arm asked more than the legacy step: a difference either way.
        assert_eq!(compare_questions(true, false, &one, &two, 0).len(), 1);
        assert_eq!(compare_questions(false, false, &one, &two, 0).len(), 1);
        // A different question in the shared prefix is a difference either way.
        assert_eq!(compare_questions(true, false, &[q("p1")], &[q("other")], 0).len(), 1);
        // The answers the arm did not take: the remainder takes them when the window ended.
        assert!(compare_questions(true, false, &one, &one, 3).is_empty());
        let left = compare_questions(false, false, &one, &one, 3);
        assert_eq!((left.len(), left[0].path.as_str(), left[0].right.as_str()), (1, "native.questions.unconsumed", "3"));
        assert!(compare_questions(false, false, &one, &one, 0).is_empty(), "none left is no difference");
        assert_eq!(compare_questions(false, false, &one, &one, 1).len(), 1, "a single answer left is one");
        // The same shape for the events.
        let (e1, e2) = (vec!["A".to_owned()], vec!["A".to_owned(), "B".to_owned()]);
        assert!(compare_events(true, false, &e2, &e1).is_empty());
        assert_eq!(compare_events(false, false, &e2, &e1).len(), 1);
        assert_eq!(compare_events(true, false, &e1, &e2).len(), 1);
        assert_eq!(compare_events(true, false, &["A".to_owned()], &["Z".to_owned()]).len(), 1);
        // The position: not compared when the window ended, leaf by leaf when it did not, with the path of the leaf.
        let (l, r) = (json!({ "players": [{ "trade_goods": 1 }], "round": 2 }), json!({ "players": [{ "trade_goods": 2 }], "round": 2 }));
        assert!(compare_state(true, &l, &r).is_empty());
        let diff = compare_state(false, &l, &r);
        assert_eq!(diff.len(), 1);
        assert_eq!((diff[0].category, diff[0].path.as_str()), (Category::State, "native.state.players[0].trade_goods"));
        assert!(compare_state(false, &l, &l).is_empty());
    }

    /// The path of a difference joins the prefix with a dot before a key and none before an index.
    #[test]
    fn the_path_of_a_difference_joins_the_prefix_by_key_or_index() {
        let mut out = Vec::new();
        leaves(Category::Value, "p", &json!([1]), &json!([2]), &mut out);
        leaves(Category::Value, "p", &json!({ "k": 1 }), &json!({ "k": 2 }), &mut out);
        leaves(Category::Value, "p", &json!(1), &json!(2), &mut out);
        let paths: Vec<_> = out.iter().map(|r| r.path.as_str()).collect();
        assert_eq!(paths, ["p[0]", "p.k", "p"]);
    }

    /// The opener of an arm: the tokens arm opens its window from the observed one, an arm without an opener (or without the window) does not.
    #[test]
    fn the_opener_of_an_arm_opens_its_window_from_the_observed_one_and_refuses_the_rest() {
        use ti4_engine_core::containers::SmallList;
        use ti4_engine_core::driver::live::is_open;
        use ti4_engine_core::state::{StatusReport, Tokens};
        let observed = [Window::Tokens(Box::new(Tokens { pending: SmallList::from_slice(&[ti4_engine_core::Seat(0)]), placed: SmallList::new(), report: StatusReport::default() }))];
        let mut core = CoreState::all_loaded();
        assert!(!open_arm(Arm::Voting, &observed, &mut core), "no opener for the arm");
        assert!(!is_open::<TokenGain>(&core));
        assert!(!open_arm(Arm::Tokens, &[], &mut core), "the window was not observed");
        assert!(!is_open::<TokenGain>(&core));
        assert!(open_arm(Arm::Tokens, &observed, &mut core));
        assert!(is_open::<TokenGain>(&core), "the window is open on the position");
        assert_eq!(window_of(Arm::Tokens), Some(WindowKind::Tokens));
        assert_eq!(window_of(Arm::Voting), None);
    }

    /// The counters count one per call, and the slices of a position keep the map only when the game has one.
    #[test]
    fn the_counters_count_one_per_call_and_a_game_without_a_map_loads_no_map_slice() {
        let _serial = SERIAL.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let _ = (lent_counts(), ran_counts());
        count(&RAN, "unit-test".to_owned());
        count(&RAN, "unit-test".to_owned());
        lend(Arm::Voting, "unit-test");
        assert_eq!(ran_counts().get("unit-test"), Some(&2));
        assert_eq!(lent_counts().get("voting: unit-test"), Some(&1));
        let (with, without) = (slices_for(true), slices_for(false));
        assert_eq!(with, SliceSet::ALL);
        assert!(!without.has(Slice::Map) && with.has(Slice::Map));
        assert!(Slice::ALL.iter().filter(|s| **s != Slice::Map).all(|s| without.has(*s)), "every other slice stays");
    }

    /// The accessors of the subject are the legacy game's.
    #[test]
    fn a_fresh_subject_reports_the_legacy_games_round_version_and_hash() {
        let _serial = SERIAL.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        set_native_arms(NativeArms::NONE);
        let mut subject = NativeSubject::new(&spec(), Pilot::Bot).expect("builds");
        let plain = <MaskSubject<LegacyMask> as Subject>::new(&spec(), Pilot::Bot).expect("builds");
        assert_eq!(subject.round(), 1);
        assert_eq!(subject.rules_version(), ti4_model::state::RULES_VERSION);
        assert!(!subject.finished());
        assert_eq!(subject.decisions_hash(), plain.decisions_hash());
        for _ in 0..3 {
            let _ = subject.step();
        }
        assert!(!subject.state_json().is_null() && !subject.decisions_hash().is_empty());
        let _ = subject.take_extra();
        let _ = lent_counts();
    }

    /// Without native arms nothing is shadowed and every step is lent (the scaffold's whole point: the count says how much is left).
    #[test]
    fn with_no_native_arm_every_step_is_lent_and_nothing_is_compared() {
        let _serial = SERIAL.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        set_native_arms(NativeArms::NONE);
        let _ = (lent_counts(), ran_counts());
        TAMPER.with(|t| t.set(1));
        let played = play();
        TAMPER.with(|t| t.set(0));
        assert!(played.report.differences.is_empty(), "{:#?}", played.report.differences);
        assert!(ran_counts().is_empty());
        let lent = lent_counts();
        assert!(lent.values().sum::<u64>() >= played.steps as u64, "every step counted: {lent:?} for {} steps", played.steps);
    }
}
