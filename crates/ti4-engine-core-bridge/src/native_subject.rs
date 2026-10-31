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
use ti4_engine_core::driver::live::{live_arm, open};
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
static LENT: Mutex<BTreeMap<String, u64>> = Mutex::new(BTreeMap::new());
static RAN: Mutex<BTreeMap<String, u64>> = Mutex::new(BTreeMap::new());

/// The arms the next `NativeSubject`s run natively (default: none, i.e. every step is lent).
pub fn set_native_arms(arms: NativeArms) {
    *NATIVE.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = arms;
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
        // The scoring windows of the status phase and of the secrets timed on an action or an agenda (lane C): the payload the legacy game holds.
        Arm::Scoring => open_observed(windows, core, WindowKind::Scoring),
        Arm::EventScoring => open_observed(windows, core, WindowKind::EventScoring),
        // The strategic action (lane E): the follower window, the Winnu hero's follower selection (with the hero resolving) and the end of a game.
        Arm::Secondary => open_observed(windows, core, WindowKind::StrategySecondary),
        Arm::LeaderFollowers => open_observed(windows, core, WindowKind::LeaderStrategy) && open_observed(windows, core, WindowKind::LeaderFollowers),
        Arm::Finished => true,
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
/// (the questions, the events and the position), not on the prefix. `Phase` is the whole of `step_phase`; the prelude and the
/// announcement go on into the question of the same legacy step (the action menu, the draft), which the arm does not own.
const fn close_is_native(arm: Arm) -> bool {
    matches!(arm, Arm::Phase | Arm::Status | Arm::Tokens | Arm::Scoring | Arm::EventScoring | Arm::Secondary | Arm::LeaderFollowers | Arm::Finished)
}

/// The arms whose native step also runs the close when it applies the LAST answer, where the legacy game closes in its NEXT step (a step
/// that finds nothing to ask): the legacy answer step is compared on the prefix, the legacy close step on everything.
const fn closes_one_step_early(arm: Arm) -> bool {
    matches!(arm, Arm::Scoring | Arm::EventScoring | Arm::Secondary)
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
        Arm::Secondary => Some(WindowKind::StrategySecondary),
        Arm::LeaderFollowers => Some(WindowKind::LeaderFollowers),
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
        // The `Ask` arm is native for the sources of question the turn flow owns (the draft, the starting technology); the others are lent.
        if arm == Arm::Ask && !matches!(live_ask_arm(&facts), AskArm::Draft | AskArm::StartingTech) {
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
        // An event the legacy engine staged is a blob of JSON in a mark the core does not read (the core stages its own in `Pending::staged`): the
        // legacy step announces it first thing, the shadow cannot. Counted lent, never skipped silently.
        if core.marks().tail.iter().any(|(k, _)| matches!(k, ti4_engine_core::keys::MarkKey::PrivateStagedEvent(_))) {
            lend(arm, "an event staged by the legacy engine");
            return None;
        }
        if !open_arm(arm, &windows, &mut core) {
            lend(arm, "no opener");
            return None;
        }
        Some(Shadow { arm, core, cx, render, base: g.state.clone(), sources: g.content_sources() })
    }

    /// Run the arm on the materialized position with the answers of the legacy step and compare with what the legacy step did.
    fn compare(&mut self, mut shadow: Shadow, answers: Vec<String>, records: &[DecisionRecord], events: &[String]) {
        let name = arms::name(shadow.arm);
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
                    self.extra.extend(out);
                    return;
                }
            }
        }
        let result = live::live_step(&mut shadow.core, shadow.arm, &cx, &mut lone);
        #[cfg(test)]
        tests::tamper(&mut shadow.core);
        count(&RAN, name.to_owned());
        let mut out = Vec::new();
        let step = match result {
            Ok(step) => step,
            // The arm closed one legacy step early and the close asks something: the answers are the next legacy step's (the close step),
            // where the arm is run again from the legacy position and compared in full.
            Err(live::Stop::Exhausted) if closes_one_step_early(shadow.arm) => {
                count(&RAN, format!("{name} (the close needs the next legacy step's answers)"));
                return;
            }
            Err(stop) => {
                leaves(Category::Value, "native.stop", &json!("ran"), &json!(format!("{stop:?}")), &mut out);
                self.extra.extend(out);
                return;
            }
        };
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
        match materialize::materialize(&shadow.core, &shadow.cx, &shadow.base) {
            Ok(after) => {
                let native_state = serde_json::to_value(&after).unwrap_or(Value::Null);
                out.extend(compare_state(ended, &self.inner.state_json(), &native_state));
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
        self.extra.extend(out);
    }
}

// A step that ends the window ALSO runs the close the core does not own yet (the string's remainder of the step: steps 81.6-81.8 after the
// last token): the legacy step then asked, answered and emitted more than the arm, and changed the position beyond it (the remainder moves
// the very pools the last token filled). So a step that ends the window is compared on the prefix of the questions and the events only; a
// step that leaves it open must agree on everything, the position included. The position at the end of a window is compared when the
// close is the core's too (KS5).

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
        let list = AllowList::default();
        let mut tally = Tally::new(&list);
        let mut judge = Judge { list: &list, tally: &mut tally, scenario: "native".to_owned() };
        play_scenario::<NativeSubject, MaskSubject<LegacyMask>>(spec, LIMITS, &mut judge).expect("builds").expect("not skipped")
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

    /// Round one of a real game with every lobby faction in turn (six tables of four), the turn-flow arms in shadow and a counted bail a
    /// failure: no difference but the counted gap of a commander whose unlock condition no hook states yet (`TurnGap::CommanderUnlock`,
    /// the 12 factions of `docs/D2_LF2_RESUME.md`: delete the allowance when the hook slots are filled), and the arms ran.
    #[test]
    fn the_turn_flow_arms_agree_with_the_legacy_game_for_every_lobby_faction() {
        let _serial = SERIAL.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        TAMPER.with(|t| t.set(0));
        let mut gaps = 0;
        for group in LOBBY_FACTIONS.chunks(4) {
            set_native_arms(NativeArms::of(&[Arm::TurnPrelude, Arm::Phase, Arm::StrategyAnnounce, Arm::Ask]));
            let _ = (lent_counts(), ran_counts());
            let played = ti4_engine::dual::with_bail_is_failure(true, || play_spec(&spec_of(group)));
            let ran = ran_counts();
            let other: Vec<_> = played.report.differences.iter().filter(|d| !d.core_side.contains("CommanderUnlock")).collect();
            gaps += played.report.differences.len() - other.len();
            assert!(other.is_empty(), "{group:?}: {:#?}", other.iter().take(5).collect::<Vec<_>>());
            assert!(ran.keys().any(|k| k.starts_with("turn_prelude")), "{group:?}: the prelude ran in shadow: {ran:?}");
        }
        assert!(gaps > 0, "the commanders without a hook are still counted, not skipped");
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

    /// The gaps the strategic arms count that other lanes close: the closing menu's free options (lane F: `MenuSources`), a locked commander
    /// no hook states (`CommanderUnlock`) and Xxcha's Peace Accords (`PeaceAccords`, the annexation window is not ported).
    fn strategic_allowed(d: &&ti4_engine::dual::Difference) -> bool {
        ["MenuSources", "CommanderUnlock", "PeaceAccords"].iter().any(|g| d.core_side.contains(g))
    }

    /// The strategic action's follower arms run in shadow behind a legacy game through round one: every follower step is compared (the
    /// questions with their display facts, the events, the position when the window ends) and the two agree.
    #[test]
    fn the_strategic_action_arms_shadowed_behind_a_legacy_game_through_round_one_agree_with_it() {
        let _serial = SERIAL.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        TAMPER.with(|t| t.set(0));
        set_native_arms(NativeArms::of(&[Arm::Secondary, Arm::LeaderFollowers, Arm::Finished]));
        let _ = (lent_counts(), ran_counts());
        let played = play();
        let ran = ran_counts();
        let other: Vec<_> = played.report.differences.iter().filter(|d| !strategic_allowed(d)).collect();
        assert!(other.is_empty(), "{:#?}", other.iter().take(5).collect::<Vec<_>>());
        assert!(ran.keys().any(|k| k.starts_with("secondary")), "the follower window ran in shadow: {ran:?}");
    }

    /// Round one with every lobby faction in turn (six tables of four), the strategic arms in shadow and a counted bail a failure: no
    /// difference but the counted gaps other lanes close ([`strategic_allowed`]), and the follower window ran.
    #[test]
    fn the_strategic_action_arms_agree_with_the_legacy_game_for_every_lobby_faction() {
        let _serial = SERIAL.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        TAMPER.with(|t| t.set(0));
        let (mut gaps, mut ran_secondary) = (0, 0);
        for group in LOBBY_FACTIONS.chunks(4) {
            set_native_arms(NativeArms::of(&[Arm::Secondary, Arm::LeaderFollowers, Arm::Finished]));
            let _ = (lent_counts(), ran_counts());
            let played = ti4_engine::dual::with_bail_is_failure(true, || play_spec(&spec_of(group)));
            let ran = ran_counts();
            let other: Vec<_> = played.report.differences.iter().filter(|d| !strategic_allowed(d)).collect();
            gaps += played.report.differences.len() - other.len();
            assert!(other.is_empty(), "{group:?}: {:#?}", other.iter().take(5).collect::<Vec<_>>());
            ran_secondary += ran.iter().filter(|(k, _)| k.starts_with("secondary")).map(|(_, n)| *n).sum::<u64>();
        }
        assert!(ran_secondary >= 6, "the follower window ran in shadow at every table: {ran_secondary}");
        eprintln!("strategic gaps counted: {gaps}");
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
