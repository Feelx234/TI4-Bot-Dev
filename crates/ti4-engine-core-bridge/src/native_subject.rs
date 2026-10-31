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
use ti4_engine_core::driver::session::{bit, step_arm, Arm};
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
        _ => false,
    }
}

/// The window kind whose closing is the end of `arm`'s step (`None`: the arm has no window of its own to compare).
const fn window_of(arm: Arm) -> Option<WindowKind> {
    match arm {
        Arm::Tokens => Some(WindowKind::Tokens),
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
        let arm = step_arm(&g.step_facts(), false);
        if !self.arms.has(arm) || live_arm(arm).is_none() {
            lend(arm, "not native");
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
        let mut slices = SliceSet::ALL;
        if g.galaxy().is_none() {
            slices = SliceSet(slices.0 & !(1 << Slice::Map as u32));
        }
        let positions = g.rng_positions();
        let loaded = Codec::load(&mut cx, &g.state, Sources { galaxy: g.galaxy(), rng: Some(&positions) }, slices);
        let Ok(mut core) = loaded else {
            lend(arm, "the position does not load");
            return None;
        };
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
        let mut asker = Scripted::new(answers);
        let result = live::live_step(&mut shadow.core, shadow.arm, &cx, &mut asker);
        #[cfg(test)]
        tests::tamper(&mut shadow.core);
        count(&RAN, name.to_owned());
        let mut out = Vec::new();
        let step = match result {
            Ok(step) => step,
            Err(stop) => {
                leaves(Category::Value, "native.stop", &json!("ran"), &json!(format!("{stop:?}")), &mut out);
                self.extra.extend(out);
                return;
            }
        };
        // A step that ends the window ALSO runs the close the core does not own yet (the string's remainder of the step: steps 81.6-81.8
        // after the last token): the legacy step then asked, answered and emitted more than the arm, and changed the position beyond it
        // (the remainder moves the very pools the last token filled). So a step that ends the window is compared on the prefix of the
        // questions and the events only; a step that leaves it open must agree on everything, the position included. The position at
        // the end of a window is compared when the close is the core's too (KS5).
        let finished = step.finished;
        if finished {
            count(&RAN, format!("{name} (window ended; the close is the legacy's)"));
        }
        // The questions.
        let native: Vec<Value> = step.asked.iter().map(|c| json!({ "player": c.player.as_str(), "prompt": c.prompt, "options": c.options.iter().map(|o| o.id.as_str()).collect::<Vec<_>>() })).collect();
        let mut legacy: Vec<Value> = records.iter().map(|r| json!({ "player": r.player.as_str(), "prompt": r.prompt, "options": r.offered })).collect();
        if finished && legacy.len() >= native.len() {
            legacy.truncate(native.len());
        }
        leaves(Category::Decision, "native.questions", &Value::Array(legacy), &Value::Array(native), &mut out);
        if asker.left() > 0 && !finished {
            leaves(Category::Decision, "native.questions.unconsumed", &json!(0), &json!(asker.left()), &mut out);
        }
        // The plain events.
        let pushed = ti4_engine::render::events::pushed_since(&shadow.render, &shadow.cx, &shadow.core, step.events_from);
        match pushed {
            Some(lines) => {
                let texts: Vec<&str> = lines.iter().map(|p| p.text()).collect();
                let mut legacy_events = events.to_vec();
                if finished && legacy_events.len() >= texts.len() {
                    legacy_events.truncate(texts.len());
                }
                leaves(Category::Event, "native.events", &json!(legacy_events), &json!(texts), &mut out);
            }
            None => leaves(Category::Event, "native.events", &json!(events), &json!("a line the tables cannot name"), &mut out),
        }
        // The position.
        match materialize::materialize(&shadow.core, &shadow.cx, &shadow.base) {
            Ok(after) => {
                let native_state = serde_json::to_value(&after).unwrap_or(Value::Null);
                let legacy_state = self.inner.state_json();
                if !finished {
                    leaves(Category::State, "native.state", &legacy_state, &native_state, &mut out);
                }
            }
            Err(e) => leaves(Category::State, "native.state", &json!("materializes"), &json!(e.to_string()), &mut out),
        }
        // The window ended when the legacy window ended.
        if let Some(kind) = window_of(shadow.arm) {
            let open_after = self.inner.game().step_facts().open & bit(kind) != 0;
            leaves(Category::Value, "native.finished", &json!(!open_after), &json!(step.finished), &mut out);
        }
        self.extra.extend(out);
    }
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

    fn finished(&self) -> bool {
        self.inner.finished()
    }

    fn round(&self) -> u32 {
        self.inner.round()
    }

    fn rules_version(&self) -> u32 {
        self.inner.rules_version()
    }

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
        GameSpec {
            setup: "native".to_owned(),
            seed: 11,
            players: 4,
            map: "4pOuterRim".to_owned(),
            factions: LOBBY_FACTIONS.iter().take(4).map(|f| (*f).to_owned()).collect(),
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
        let list = AllowList::default();
        let mut tally = Tally::new(&list);
        let mut judge = Judge { list: &list, tally: &mut tally, scenario: "native".to_owned() };
        play_scenario::<NativeSubject, MaskSubject<LegacyMask>>(&spec(), LIMITS, &mut judge).expect("builds").expect("not skipped")
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
        assert!(ran.get("tokens").copied().unwrap_or(0) >= 2, "the arm ran in shadow at least on the first and the last token: {ran:?}");
        assert!(lent.contains_key("tactical: not native") && lent.contains_key("aftermath: not native"), "the other arms are lent and counted: {lent:?}");
        assert!(!lent.keys().any(|k| k.starts_with("tokens:")), "no tokens step was lent: {lent:?}");
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
