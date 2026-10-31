//! The oracle harness (package D2-6b(a), `docs/todo/D2-remaining.md` 3.2): two engines play the same scenario and are compared after
//! every step. Subject B *leads*: the shipped rule bot answers its questions (through B's own observed views, trap T-H). Subject A
//! *follows*: B's chosen option ids are fed to it ([`Subject::answer`]). After each step the harness compares
//!
//! * the question each side offers before the step (player, prompt, option ids, `DecisionContext::canonical`): category `Decision`,
//! * the decisions the step settled (player, prompt, chosen id, offered ids, and the V1 and V2 `decision_hash` of each): `DecisionHash`,
//! * the step's own result (phase, active seat, finished, error, whether a choice was resolved): `Value`,
//! * the new events: `Event`, the positions of the random streams: `Rng`, and the whole `GameState` as JSON, leaf by leaf: `State`.
//!
//! A difference is a [`Difference`] of area `driver` and label `oracle::<subtype of the question answered in the step>` (`oracle::-`
//! for a step that asked nothing); its `path` is `choice...`, `records...`, `step...`, `events...`, `rng...` or the state's JSON path,
//! so an allow-list entry (`allow_list.rs`) selects it by `area: "driver"`, `label: "oracle::*"` and the path. The report is the
//! ordinary [`DualReport`] (shard files and merge are those of `dual_shard`).
//!
//! # Subjects
//!
//! [`Subject`] is the seam. Today both implementations are the same crate under a different [`AreaMask`] ([`MaskSubject`]:
//! [`LegacyMask`] = `AreaMask::STRING`, the legacy behaviour; [`CoreMask`] = the core mask, `all=core` unless
//! [`set_core_mask`] says otherwise), switched per call with `dual::with_mask` (a thread-local override). At D2-7 the first
//! becomes `ti4_engine_legacy::game::Game`, at D2-8 the second the wrapper `Game`; the harness does not change.
//!
//! # Rules of the run
//!
//! * A scenario whose opening position uses a switched-off feature (`core_sync::unsupported::find`) is skipped and counted, not
//!   allow-listed ([`Skipped`]).
//! * After the first *unallowed* difference of a scenario the scenario stops (the two games diverge from there on and every later step
//!   would report the consequence). An *allowed* difference does not stop the game; a state, event or rng difference whose
//!   `(category, path)` was already reported with the same two values is not reported again (a persistent difference is one hit,
//!   not one per step).
//! * Every difference is judged by the allow-list per step (`AllowList::judge`), so the report keeps only what failed.

use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::Mutex;

use serde_json::{Value, json};
use ti4_engine::choice::{Choice, ChoiceOption, Decider, DecisionRecord, IllegalChoice};
use ti4_engine::dual::{self, Area, AreaCounts, AreaMask, Backend, Category, Difference, DualReport, diff_json};
use ti4_engine::fingerprint::{CanonicalHashVersion, decision_hash};
use ti4_engine::game::Game;
use ti4_engine::rng::RngPositions;
use ti4_selfplay::spec::GameSpec;

use crate::allow_list::{AllowList, Tally};
use crate::dual_shard::GameRecord;

/// Leaves reported per comparison (as the dual run's).
const LEAVES: usize = 8;

/// Who answers a subject's questions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pilot {
    /// The rule bot of the golden games, reading this subject's own views.
    Bot,
    /// The option ids given to [`Subject::answer`], in order.
    Script,
}

/// What one [`Subject::step`] returned (the `StepResult` as comparable text).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepInfo {
    pub phase: String,
    pub active: Option<String>,
    pub finished: bool,
    pub error: Option<String>,
    pub resolved_choice: bool,
}

impl StepInfo {
    fn to_json(&self) -> Value {
        json!({ "phase": self.phase, "active": self.active, "finished": self.finished, "error": self.error, "resolved_choice": self.resolved_choice })
    }
}

/// One engine as the oracle sees it.
pub trait Subject: Sized {
    /// The scenario's opening position, answered by `pilot`.
    ///
    /// # Errors
    /// A scenario the engine cannot build.
    fn new(spec: &GameSpec, pilot: Pilot) -> Result<Self, String>;
    /// The question currently offered.
    fn options(&self) -> Option<Choice>;
    /// Queue the option id the next question asked of a [`Pilot::Script`] subject is answered with.
    fn answer(&mut self, option_id: &str);
    /// One engine step.
    fn step(&mut self) -> StepInfo;
    /// The whole `GameState` as JSON.
    fn state_json(&self) -> Value;
    /// The events pushed since the last call.
    fn events_since(&mut self) -> Vec<String>;
    /// The decisions settled since the last call.
    fn decisions_since(&mut self) -> Vec<DecisionRecord>;
    /// Where every random stream stands.
    fn rng_positions(&self) -> RngPositions;
    fn finished(&self) -> bool;
    fn round(&self) -> u32;
    fn rules_version(&self) -> u32;
    /// The first switched-off feature the opening position uses, if any (the scenario is then skipped).
    fn disabled(&self) -> Option<String>;
    /// The decision hash of the whole game so far (what `dual_run` compares).
    fn decisions_hash(&self) -> String;
}

// --- the mask subjects ---------------------------------------------------------------------------

/// Which mask a [`MaskSubject`] runs under.
pub trait MaskChoice {
    fn mask() -> AreaMask;
}

/// `AreaMask::STRING`: the legacy behaviour.
#[derive(Debug, Clone, Copy)]
pub struct LegacyMask;

impl MaskChoice for LegacyMask {
    fn mask() -> AreaMask {
        AreaMask::STRING
    }
}

static CORE_MASK: Mutex<Option<AreaMask>> = Mutex::new(None);

/// The mask [`CoreMask`] subjects run under (default `all=core`).
// mutants:skip (process-wide setter, read back by `core_mask`, tested through `CoreMask::mask`)
pub fn set_core_mask(mask: AreaMask) {
    *CORE_MASK.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(mask);
}

/// The core mask: every area on `Core`, unless [`set_core_mask`] was called.
#[derive(Debug, Clone, Copy)]
pub struct CoreMask;

impl MaskChoice for CoreMask {
    fn mask() -> AreaMask {
        CORE_MASK.lock().unwrap_or_else(std::sync::PoisonError::into_inner).unwrap_or_else(|| AreaMask::all(Backend::Core))
    }
}

/// Answers from a queue of option ids (the follower).
struct Follower(Rc<RefCell<VecDeque<String>>>);

impl Decider for Follower {
    fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
        let wanted = self.0.borrow_mut().pop_front().unwrap_or_default();
        choice.answer_for(&wanted).ok_or_else(|| IllegalChoice::ScriptDiverged {
            player: choice.player.clone(),
            wanted,
            offered: choice.ids().into_iter().map(str::to_owned).collect(),
        })
    }
}

/// One game of this crate, run under the mask `M` on every call.
pub struct MaskSubject<M: MaskChoice> {
    game: Game<'static>,
    mask: AreaMask,
    script: Rc<RefCell<VecDeque<String>>>,
    events_seen: usize,
    records_seen: usize,
    _mask: PhantomData<M>,
}

impl<M: MaskChoice> Subject for MaskSubject<M> {
    fn new(spec: &GameSpec, pilot: Pilot) -> Result<Self, String> {
        let mask = M::mask();
        let mut game = dual::with_mask(mask, || spec.build())?;
        let script = Rc::new(RefCell::new(VecDeque::new()));
        match pilot {
            Pilot::Bot => game.table.set_default(crate::workload::golden_decider(spec.seed)),
            Pilot::Script => game.table.set_default(Box::new(Follower(Rc::clone(&script)))),
        }
        let (events_seen, records_seen) = (game.events.len(), game.table.log.records.len());
        Ok(Self { game, mask, script, events_seen, records_seen, _mask: PhantomData })
    }

    fn options(&self) -> Option<Choice> {
        dual::with_mask(self.mask, || self.game.legal_options())
    }

    fn answer(&mut self, option_id: &str) {
        self.script.borrow_mut().push_back(option_id.to_owned());
    }

    fn step(&mut self) -> StepInfo {
        let r = dual::with_mask(self.mask, || self.game.step());
        StepInfo {
            phase: format!("{:?}", r.phase),
            active: r.active.map(|p| p.as_str().to_owned()),
            finished: r.finished,
            error: r.error.map(|e| e.to_string()),
            resolved_choice: r.resolved_choice,
        }
    }

    fn state_json(&self) -> Value {
        serde_json::to_value(&self.game.state).expect("a game state is JSON")
    }

    fn events_since(&mut self) -> Vec<String> {
        let new = self.game.events.get(self.events_seen..).unwrap_or_default().to_vec();
        self.events_seen = self.game.events.len();
        new
    }

    fn decisions_since(&mut self) -> Vec<DecisionRecord> {
        let new = self.game.table.log.records.get(self.records_seen..).unwrap_or_default().to_vec();
        self.records_seen = self.game.table.log.records.len();
        new
    }

    fn rng_positions(&self) -> RngPositions {
        self.game.rng_positions()
    }

    fn finished(&self) -> bool {
        self.game.state.finished
    }

    fn round(&self) -> u32 {
        self.game.state.round
    }

    fn rules_version(&self) -> u32 {
        self.game.state.rules_version
    }

    fn disabled(&self) -> Option<String> {
        ti4_engine::core_sync::unsupported::find(&self.game.state).map(|d| format!("{d:?}"))
    }

    fn decisions_hash(&self) -> String {
        ti4_wasm::host_session::decisions_hash(&self.game.table.log.records)
    }
}

// --- one comparison ------------------------------------------------------------------------------

/// One difference before it has an area and a label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Raw {
    pub category: Category,
    pub path: String,
    pub left: String,
    pub right: String,
}

fn leaves(category: Category, prefix: &str, left: &Value, right: &Value, out: &mut Vec<Raw>) {
    for d in diff_json(left, right, LEAVES) {
        let path = match (prefix.is_empty(), d.path.is_empty()) {
            (true, _) => d.path,
            (false, true) => prefix.to_owned(),
            (false, false) if d.path.starts_with('[') => format!("{prefix}{}", d.path),
            (false, false) => format!("{prefix}.{}", d.path),
        };
        out.push(Raw { category, path, left: d.left, right: d.right });
    }
}

fn choice_view(c: Option<&Choice>) -> Value {
    match c {
        None => Value::Null,
        Some(c) => json!({
            "player": c.player.as_str(),
            "prompt": c.prompt,
            "options": c.options.iter().map(|o| o.id.as_str()).collect::<Vec<_>>(),
            "context": c.context.as_ref().map(ti4_engine::decision_context::DecisionContext::canonical),
        }),
    }
}

fn records_view(records: &[DecisionRecord]) -> Value {
    Value::Array(
        records
            .iter()
            .map(|r| {
                json!({
                    "player": r.player.as_str(), "prompt": r.prompt, "chosen": r.chosen, "offered": r.offered,
                    "v1": decision_hash(CanonicalHashVersion::V1, r).digest, "v2": decision_hash(CanonicalHashVersion::V2, r).digest,
                })
            })
            .collect(),
    )
}

/// What one subject showed around one step.
#[derive(Debug, Clone)]
pub struct StepView {
    /// The question offered before the step.
    pub offered: Option<Choice>,
    pub info: StepInfo,
    pub records: Vec<DecisionRecord>,
    pub events: Vec<String>,
    pub rng: RngPositions,
    pub state: Value,
}

/// Compare what two subjects showed: the differences, category by category.
#[must_use]
pub fn compare_views(a: &StepView, b: &StepView) -> Vec<Raw> {
    let mut out = Vec::new();
    leaves(Category::Decision, "choice", &choice_view(a.offered.as_ref()), &choice_view(b.offered.as_ref()), &mut out);
    leaves(Category::DecisionHash, "records", &records_view(&a.records), &records_view(&b.records), &mut out);
    leaves(Category::Value, "step", &a.info.to_json(), &b.info.to_json(), &mut out);
    leaves(Category::Event, "events", &json!(a.events), &json!(b.events), &mut out);
    leaves(Category::Rng, "rng", &json!(a.rng), &json!(b.rng), &mut out);
    leaves(Category::State, "", &a.state, &b.state, &mut out);
    out
}

// --- one scenario --------------------------------------------------------------------------------

/// A scenario that was not played.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skipped(pub String);

/// The limits of a scenario (a config's `limits`).
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub max_rounds: u32,
    pub max_steps: usize,
}

/// What judging one scenario needs.
pub struct Judge<'a> {
    pub list: &'a AllowList,
    pub tally: &'a mut Tally,
    /// The name allow-list entries match in their `scenario` field.
    pub scenario: String,
}

/// The result of one scenario.
#[derive(Debug)]
pub struct Played {
    pub steps: usize,
    /// Calls, comparisons and the differences the allow-list did not cover.
    pub report: DualReport,
    pub hash_a: String,
    pub hash_b: String,
    /// Why the scenario ended before the game did (an unallowed difference, a limit).
    pub stopped: Option<String>,
}

fn count(report: &mut DualReport, category: Category, differing: u64) {
    let c: &mut AreaCounts = report.areas.entry(Area::Driver).or_default();
    c.compared[category as usize] += 1;
    c.differing[category as usize] += differing;
}

/// Play one scenario on `A` (follows) and `B` (leads) and compare after every step.
///
/// # Errors
/// A scenario one of the subjects cannot build, or the skip of a scenario that uses a switched-off feature.
pub fn play_scenario<A: Subject, B: Subject>(spec: &GameSpec, limits: Limits, judge: &mut Judge<'_>) -> Result<Result<Played, Skipped>, String> {
    let mut b = B::new(spec, Pilot::Bot)?;
    if let Some(why) = b.disabled() {
        return Ok(Err(Skipped(why)));
    }
    let mut a = A::new(spec, Pilot::Script)?;
    let rules_version = b.rules_version();
    let mut report = DualReport::default();
    let mut reported: BTreeMap<(usize, String), (String, String)> = BTreeMap::new();
    let (mut steps, mut stopped) = (0usize, None);
    while !b.finished() {
        if steps >= limits.max_steps || b.round() > limits.max_rounds {
            stopped = Some(format!("limit: {steps} steps, round {}", b.round()));
            break;
        }
        let offered_a = a.options();
        let offered_b = b.options();
        let subtype = offered_b
            .as_ref()
            .and_then(|c| c.context.as_ref())
            .map_or_else(|| "-".to_owned(), |c| c.subtype.clone());
        let info_b = b.step();
        let records_b = b.decisions_since();
        for r in &records_b {
            a.answer(&r.chosen);
        }
        let info_a = a.step();
        let records_a = a.decisions_since();
        steps += 1;
        let view_a = StepView { offered: offered_a, info: info_a, records: records_a, events: a.events_since(), rng: a.rng_positions(), state: a.state_json() };
        let view_b = StepView { offered: offered_b, info: info_b, records: records_b, events: b.events_since(), rng: b.rng_positions(), state: b.state_json() };
        let raw = compare_views(&view_a, &view_b);
        report.areas.entry(Area::Driver).or_default().calls += 1;
        for category in [Category::Decision, Category::DecisionHash, Category::Value, Category::Event, Category::Rng, Category::State] {
            count(&mut report, category, raw.iter().filter(|r| r.category == category).count() as u64);
        }
        // A persistent difference is one report: the same two values at the same place as last time are not new.
        let fresh: Vec<Difference> = raw
            .into_iter()
            .filter(|r| reported.insert((r.category as usize, r.path.clone()), (r.left.clone(), r.right.clone())).as_ref() != Some(&(r.left.clone(), r.right.clone())))
            .map(|r| Difference { area: Area::Driver, category: r.category, label: format!("oracle::{subtype}"), path: r.path, string_side: r.left, core_side: r.right })
            .collect();
        if fresh.is_empty() {
            continue;
        }
        let step_report = DualReport { differences: fresh, ..DualReport::default() };
        let verdict = judge.list.judge(&step_report, rules_version, &judge.scenario, judge.tally);
        if !verdict.ok() {
            stopped = Some(format!("first unallowed difference at step {steps}"));
            report.differences.extend(verdict.unallowed);
            break;
        }
    }
    Ok(Ok(Played { steps, report, hash_a: a.decisions_hash(), hash_b: b.decisions_hash(), stopped }))
}

/// The shard record of a played or skipped scenario (`dual_hash` is the leader's, `string_hash` the follower's).
#[must_use]
pub fn record_of(index: usize, played: Result<Played, Skipped>) -> GameRecord {
    match played {
        Ok(p) => GameRecord { index, dual_hash: p.hash_b, string_hash: p.hash_a, report: p.report, skipped: None },
        Err(Skipped(why)) => GameRecord { index, dual_hash: String::new(), string_hash: String::new(), report: DualReport::default(), skipped: Some(why) },
    }
}

// --- the end of a run ----------------------------------------------------------------------------

/// The verdict of a whole run.
///
/// # Errors
/// Every problem, one per line: the run has an unallowed difference (or one it could not verify) or a changed hash, and a
/// `full` run has a stale or expired allow-list entry.
pub fn conclude(merged_ok: bool, list: &AllowList, tally: &Tally, rules_version: u32, full: bool) -> Result<(), String> {
    let mut problems = String::new();
    if !merged_ok {
        problems.push_str("the run has an unallowed difference or a changed decision hash\n");
    }
    if full && let Err(stale) = tally.check_stale(list, rules_version) {
        problems.push_str(&stale);
    }
    if problems.is_empty() { Ok(()) } else { Err(problems) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::allow_list::AllowList;

    /// A small golden-like scenario: four seats of the lobby factions on a template map.
    fn spec() -> GameSpec {
        GameSpec {
            setup: "4p".to_owned(),
            seed: 7,
            players: 4,
            map: "4pOuterRim".to_owned(),
            factions: ti4_engine::seating::LOBBY_FACTIONS.iter().take(4).map(|f| (*f).to_owned()).collect(),
            speaker: "seat".to_owned(),
            card_set: None,
            rotation: None,
            starting_tech: true,
            thunders_edge: false,
            rules_version: None,
        }
    }

    const LIMITS: Limits = Limits { max_rounds: 99, max_steps: 40 };

    fn judge<'a>(list: &'a AllowList, tally: &'a mut Tally) -> Judge<'a> {
        Judge { list, tally, scenario: "t".to_owned() }
    }

    /// Delegates to a [`MaskSubject`] and changes one thing the harness reads.
    struct Tamper<M: MaskChoice> {
        inner: MaskSubject<M>,
        steps: usize,
    }

    impl<M: MaskChoice> Subject for Tamper<M> {
        fn new(spec: &GameSpec, pilot: Pilot) -> Result<Self, String> {
            Ok(Self { inner: MaskSubject::new(spec, pilot)?, steps: 0 })
        }
        fn options(&self) -> Option<Choice> {
            self.inner.options()
        }
        fn answer(&mut self, id: &str) {
            self.inner.answer(id);
        }
        fn step(&mut self) -> StepInfo {
            self.steps += 1;
            self.inner.step()
        }
        fn state_json(&self) -> Value {
            let mut v = self.inner.state_json();
            if self.steps >= 5 {
                // The injected one-field difference, from the fifth step on.
                v["players"][1]["trade_goods"] = json!(12_345);
            }
            v
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
    }

    #[test]
    fn a_subject_compared_with_itself_reports_nothing() {
        let (list, mut tally) = (AllowList::empty(), Tally::default());
        let played = play_scenario::<MaskSubject<LegacyMask>, MaskSubject<LegacyMask>>(&spec(), LIMITS, &mut judge(&list, &mut tally))
            .expect("builds")
            .expect("not skipped");
        assert_eq!(played.report.total_differences(), 0, "{}", played.report);
        assert!(played.steps >= 20, "the scenario was played ({} steps)", played.steps);
        let c = &played.report.areas[&Area::Driver];
        assert_eq!(c.calls, played.steps as u64);
        assert_eq!(c.compared[Category::State as usize], played.steps as u64, "every step compared the state");
        assert_eq!(played.hash_a, played.hash_b);
        assert!(!played.hash_a.is_empty());
    }

    #[test]
    fn the_core_mask_subject_agrees_with_the_legacy_one() {
        let (list, mut tally) = (AllowList::empty(), Tally::default());
        let played = play_scenario::<MaskSubject<LegacyMask>, MaskSubject<CoreMask>>(&spec(), LIMITS, &mut judge(&list, &mut tally))
            .expect("builds")
            .expect("not skipped");
        assert_eq!(played.report.total_differences(), 0, "{}", played.report);
        assert_eq!(played.hash_a, played.hash_b);
    }

    #[test]
    fn an_injected_one_field_difference_is_reported_with_its_path_and_stops_the_scenario() {
        let (list, mut tally) = (AllowList::empty(), Tally::default());
        let played = play_scenario::<MaskSubject<LegacyMask>, Tamper<LegacyMask>>(&spec(), LIMITS, &mut judge(&list, &mut tally))
            .expect("builds")
            .expect("not skipped");
        let d = &played.report.differences;
        assert_eq!(d.len(), 1, "one leaf, once: {d:?}");
        assert_eq!(d[0].path, "players[1].trade_goods");
        assert_eq!(d[0].category, Category::State);
        assert_eq!(d[0].core_side, "12345", "the leader's (B's) value is the second side");
        assert!(d[0].label.starts_with("oracle::"));
        assert_eq!(played.steps, 5, "stopped at the step that showed it");
        assert!(played.stopped.as_deref().is_some_and(|s| s.contains("unallowed")));
        assert!(!conclude(false, &list, &tally, 1, true).is_ok());
    }

    fn allow_trade_goods() -> AllowList {
        AllowList::parse(
            r#"{ "version": 1, "entries": [ { "id": "AL-001", "reason": "test", "ticket": "T", "rules_version_min": 1,
                "area": "driver", "label": "oracle::*", "path": "players[*].trade_goods" } ] }"#,
        )
        .expect("a valid list")
    }

    #[test]
    fn an_allow_listed_difference_passes_is_counted_once_and_does_not_stop_the_game() {
        let (list, mut tally) = (allow_trade_goods(), Tally::default());
        let played = play_scenario::<MaskSubject<LegacyMask>, Tamper<LegacyMask>>(&spec(), LIMITS, &mut judge(&list, &mut tally))
            .expect("builds")
            .expect("not skipped");
        assert_eq!(played.report.differences, Vec::new());
        assert_eq!(played.steps, 40, "played to the step limit");
        assert_eq!(tally.hits("AL-001"), 1, "the persistent difference is one hit");
        assert!(conclude(true, &list, &tally, 1, true).is_ok());
    }

    #[test]
    fn a_stale_entry_fails_a_full_run_and_not_a_partial_one() {
        let (list, tally) = (allow_trade_goods(), Tally::new(&allow_trade_goods()));
        assert!(conclude(true, &list, &tally, 1, false).is_ok());
        let err = conclude(true, &list, &tally, 1, true).expect_err("stale");
        assert!(err.contains("STALE allow-list entry AL-001"), "{err}");
    }

    #[test]
    fn a_scenario_that_uses_a_switched_off_feature_is_skipped_and_counted() {
        struct Off(MaskSubject<LegacyMask>);
        impl Subject for Off {
            fn new(spec: &GameSpec, pilot: Pilot) -> Result<Self, String> {
                Ok(Self(MaskSubject::new(spec, pilot)?))
            }
            fn options(&self) -> Option<Choice> {
                self.0.options()
            }
            fn answer(&mut self, id: &str) {
                self.0.answer(id);
            }
            fn step(&mut self) -> StepInfo {
                self.0.step()
            }
            fn state_json(&self) -> Value {
                self.0.state_json()
            }
            fn events_since(&mut self) -> Vec<String> {
                self.0.events_since()
            }
            fn decisions_since(&mut self) -> Vec<DecisionRecord> {
                self.0.decisions_since()
            }
            fn rng_positions(&self) -> RngPositions {
                self.0.rng_positions()
            }
            fn finished(&self) -> bool {
                self.0.finished()
            }
            fn round(&self) -> u32 {
                self.0.round()
            }
            fn rules_version(&self) -> u32 {
                self.0.rules_version()
            }
            fn disabled(&self) -> Option<String> {
                Some("Diplomacy".to_owned())
            }
            fn decisions_hash(&self) -> String {
                self.0.decisions_hash()
            }
        }
        let (list, mut tally) = (AllowList::empty(), Tally::default());
        let out = play_scenario::<MaskSubject<LegacyMask>, Off>(&spec(), LIMITS, &mut judge(&list, &mut tally)).expect("builds");
        assert_eq!(out.as_ref().err(), Some(&Skipped("Diplomacy".to_owned())));
        let record = record_of(3, out);
        assert_eq!(record.skipped.as_deref(), Some("Diplomacy"));
        // A real opening of the golden kind is not skipped.
        let plain = MaskSubject::<LegacyMask>::new(&spec(), Pilot::Bot).expect("builds");
        assert_eq!(plain.disabled(), None);
    }

    #[test]
    fn a_follower_given_no_answer_or_a_wrong_one_errors_instead_of_choosing() {
        for answer in [None, Some("no-such-option")] {
            let mut a = MaskSubject::<LegacyMask>::new(&spec(), Pilot::Script).expect("builds");
            let mut error = None;
            for _ in 0..60 {
                if let Some(id) = answer {
                    a.answer(id);
                }
                if let Some(e) = a.step().error {
                    error = Some(e);
                    break;
                }
            }
            let error = error.expect("the first question the follower is asked is not answered by itself");
            assert!(error.contains("script wanted"), "{error}");
        }
    }

    #[test]
    fn views_that_differ_in_each_category_are_told_apart() {
        let base = StepView {
            offered: None,
            info: StepInfo { phase: "Action".into(), active: Some("a".into()), finished: false, error: None, resolved_choice: true },
            records: Vec::new(),
            events: vec!["E1".into()],
            rng: RngPositions::new(),
            state: json!({ "round": 1, "players": [{ "trade_goods": 2 }] }),
        };
        assert!(compare_views(&base, &base.clone()).is_empty());
        let mut other = base.clone();
        other.events.push("E2".into());
        other.info.phase = "Status".into();
        other.rng.insert("combat".into(), 4);
        other.state["players"][0]["trade_goods"] = json!(3);
        let got: Vec<(Category, String)> = compare_views(&base, &other).into_iter().map(|r| (r.category, r.path)).collect();
        assert!(got.contains(&(Category::Event, "events[1]".to_owned())), "{got:?}");
        assert!(got.contains(&(Category::Value, "step.phase".to_owned())), "{got:?}");
        assert!(got.contains(&(Category::Rng, "rng.combat".to_owned())), "{got:?}");
        assert!(got.contains(&(Category::State, "players[0].trade_goods".to_owned())), "{got:?}");
    }
}
