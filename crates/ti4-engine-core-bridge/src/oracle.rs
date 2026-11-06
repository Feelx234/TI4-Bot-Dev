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
use ti4_engine::choice::{Choice, DecisionRecord};
use ti4_engine_legacy::dual::{self, Area, AreaCounts, AreaMask, Backend, Category, Difference, DualReport, diff_json};
use ti4_engine::fingerprint::{CanonicalHashVersion, decision_hash};
use ti4_engine_legacy::game::Game;
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
    /// Whether a transaction window is open, so the step the subject takes next is a trade step. The string's `legal_options()` has no trade branch
    /// (`step_trade` asks the table itself), so before a trade step a legacy subject shows the ACTION MENU where the new engine shows the proposal:
    /// the harness does not compare the offered question of such a step (the decisions the step settles are still compared, by hash and by record).
    fn in_transaction(&self) -> bool {
        false
    }
    /// Differences the subject found itself during its last step, besides what the harness compares (the `NativeSubject`'s shadow run
    /// of a native arm against the legacy step). Taken once per step; the harness adds them to the step's differences.
    fn take_extra(&mut self) -> Vec<Raw> {
        Vec::new()
    }
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

/// One answer of a script: a plain option id, or what ONE answer of the new engine to a multi-ask (`docs/MULTIASK1_RESUME.md`) is to the
/// string engine, which asks the same thing as a sequence of questions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scripted {
    /// One question, answered with this id.
    One(String),
    /// One question per id, in this order (a token gain: a pool per token).
    Seq(VecDeque<String>),
    /// A payment: a question per id while the string asks a payment ("pay ...") that offers one of them, the first offered id of the
    /// list answering it (the string pays a lone option without asking, so it may ask for fewer). The entry ends at any other question.
    Set(Vec<String>),
    /// Leadership's purchase: the string's loop ("spend 3 influence for a command token?" answered `yes` until `buys` tokens are bought,
    /// then `no`; each payment from `atoms` as for [`Self::Set`]). The pool of each token is the next entry's (the core's gain question
    /// after the purchase): the entry lets it through. `bought` counts the follower's yes of a secondary, which is the first purchase.
    Leadership { buys: u8, bought: u8, atoms: Vec<String> },
    /// A removal over the fleet supply or the capacity: a question per unit while the string asks `prompt`, the first answered with the
    /// option labelled `lead` (the core's answer), the others with the first option (what the core's window fills up with).
    Removal { prompt: String, lead: Option<String> },
    /// The hand limit: a question per card while the string asks "over the hand limit", answered with the option whose LABEL (the card's
    /// printed name) is the first of the list it offers (the string's ids are places in the hand, which move as cards go).
    Labels(Vec<String>),
}

/// What an entry does with a question of the string.
enum Verdict {
    /// Answer with this id; `true`: the entry is spent.
    Answer(String, bool),
    /// Not this entry's question: a later entry answers it, this one stays.
    Pass,
    /// This entry is over (the string settled the rest without asking): drop it, a later entry answers the question.
    Drop,
}

impl Scripted {
    fn verdict(&mut self, choice: &ti4_engine_legacy::choice::Choice) -> Verdict {
        let paying = choice.prompt.starts_with("pay ");
        let first_offered = |ids: &mut Vec<String>| ids.iter().position(|id| choice.answer_for(id).is_some()).map(|i| ids.remove(i));
        match self {
            Self::One(id) => Verdict::Answer(std::mem::take(id), true),
            Self::Seq(ids) => {
                // (A sequence carried over a step boundary that the string does not ask on with: over.)
                if ids.front().is_some_and(|id| choice.answer_for(id).is_none()) && ids.len() > 1 {
                    return Verdict::Drop;
                }
                let id = ids.pop_front().unwrap_or_default();
                Verdict::Answer(id, ids.is_empty())
            }
            Self::Set(ids) => match first_offered(ids).filter(|_| paying) {
                Some(id) => Verdict::Answer(id, ids.is_empty()),
                None => Verdict::Drop,
            },
            Self::Removal { prompt, lead } => {
                if choice.prompt != *prompt {
                    return Verdict::Drop;
                }
                let id = match lead.take() {
                    Some(label) => choice.options.iter().find(|o| o.label == label).map(|o| o.id.clone()),
                    None => choice.options.first().map(|o| o.id.clone()),
                };
                match id {
                    Some(id) => Verdict::Answer(id, false),
                    None => Verdict::Drop,
                }
            }
            Self::Labels(names) => {
                let found = choice.prompt.starts_with("over the hand limit")
                    .then(|| names.iter().enumerate().find_map(|(i, name)| choice.options.iter().find(|o| o.label == *name).map(|o| (i, o.id.clone()))))
                    .flatten();
                match found {
                    Some((i, id)) => {
                        names.remove(i);
                        Verdict::Answer(id, names.is_empty())
                    }
                    None => Verdict::Drop,
                }
            }
            Self::Leadership { buys, bought, atoms } => {
                if choice.prompt.contains("influence for a command token") {
                    if bought < buys {
                        *bought += 1;
                        return Verdict::Answer("yes".to_owned(), false);
                    }
                    return Verdict::Answer("no".to_owned(), true);
                }
                if paying && let Some(id) = first_offered(atoms) {
                    return Verdict::Answer(id, false);
                }
                if choice.prompt == crate::multiask::GAIN_PROMPT {
                    return Verdict::Pass;
                }
                Verdict::Drop
            }
        }
    }
}

/// The number of an entry a step began part way through and the step before used ([`Script::clear`] keeps it): no question of the native
/// group it answers.
pub(crate) const CARRIED: usize = usize::MAX;

/// The follower's queue: the entries (each with its number since the queue was last cleared), and the entry that answered each question.
#[derive(Debug, Default, Clone)]
pub(crate) struct Script {
    entries: VecDeque<(usize, Option<String>, Scripted)>,
    /// The entries that answered a question and are not spent (a multi-ask's expansion the string's step ended inside).
    touched: Vec<usize>,
    /// The number the next entry gets.
    next: usize,
    /// For each question answered since the queue was cleared, the entry that answered it.
    pub(crate) answered_by: Vec<usize>,
    /// The follower's answers the string was not offered since the queue was cleared, as the string's error text (`script wanted ..`): a
    /// step that swallows its decider's error (an `emit` whose result is dropped) goes on without one.
    pub(crate) diverged: Vec<String>,
}

impl Script {
    /// Drop what the step did not use, but keep an entry it began and did not finish (one answer of the core that the string asks over two of
    /// its steps): it answers the next step's questions first, under the number [`CARRIED`].
    fn clear(&mut self) {
        let carried = self.entries.iter().find(|(at, _, _)| self.touched.contains(at)).map(|(_, who, e)| (who.clone(), e.clone()));
        *self = Self::default();
        if let Some((who, entry)) = carried {
            self.entries.push_back((CARRIED, who, entry));
        }
    }

    /// Queue `entry`; `player`: the seat a multi-ask's expansion answers for (a question of another seat is not its own).
    fn push(&mut self, player: Option<String>, entry: Scripted) {
        self.entries.push_back((self.next, player, entry));
        self.next += 1;
    }

    /// Whether the entry that would answer `choice` is one a step answered a question with and did not spend (the string ended its step inside
    /// one multi-ask's expansion: a payment it asks an atom per step, a gain it asks a pool per step).
    fn answers_mid(&self, choice: &ti4_engine_legacy::choice::Choice) -> bool {
        let mut probe = self.clone();
        let before = probe.answered_by.len();
        let id = probe.take(choice);
        !id.is_empty() && probe.answered_by.get(before).is_some_and(|at| self.touched.contains(at))
    }

    /// The id that answers `choice`, taken off the queue ("" when the queue is dry or names nothing offered).
    fn take(&mut self, choice: &ti4_engine_legacy::choice::Choice) -> String {
        let mut i = 0;
        loop {
            let Some((at, who, entry)) = self.entries.get_mut(i) else { return String::new() };
            let at = *at;
            // Another seat's question: a multi-ask expansion lets it through (a plain answer is played as given: a wrong seat is a difference).
            let verdict = if who.as_deref().is_some_and(|w| w != choice.player.as_str()) { Verdict::Pass } else { entry.verdict(choice) };
            match verdict {
                Verdict::Answer(id, done) => {
                    if done {
                        self.entries.remove(i);
                    } else {
                        self.touched.push(at);
                    }
                    self.answered_by.push(at);
                    return id;
                }
                Verdict::Pass => i += 1,
                Verdict::Drop => {
                    self.entries.remove(i);
                }
            }
        }
    }
}

/// Answers from a queue of option ids (the follower).
struct Follower(Rc<RefCell<Script>>);

impl ti4_engine_legacy::choice::Decider for Follower {
    fn choose(&mut self, choice: &ti4_engine_legacy::choice::Choice) -> Result<ti4_engine_legacy::choice::ChoiceOption, ti4_engine_legacy::choice::IllegalChoice> {
        let wanted = self.0.borrow_mut().take(choice);
        if std::env::var_os("TI4_NATIVE_PLAY_TRACE").is_some_and(|v| v == "2") {
            eprintln!("    follower: {} {:?} wanted {wanted:?} offered {:?}", choice.player, choice.prompt, choice.ids());
        }
        choice.answer_for(&wanted).ok_or_else(|| {
            let error = ti4_engine_legacy::choice::IllegalChoice::ScriptDiverged {
                player: choice.player.clone(),
                wanted,
                offered: choice.ids().into_iter().map(str::to_owned).collect(),
            };
            self.0.borrow_mut().diverged.push(error.to_string());
            error
        })
    }
}

/// One game of this crate, run under the mask `M` on every call.
pub struct MaskSubject<M: MaskChoice> {
    game: Game<'static>,
    mask: AreaMask,
    script: Rc<RefCell<Script>>,
    events_seen: usize,
    records_seen: usize,
    _mask: PhantomData<M>,
}

impl<M: MaskChoice> MaskSubject<M> {
    /// The legacy game (the lender of the `NativeSubject`).
    pub(crate) const fn game(&self) -> &Game<'static> {
        &self.game
    }

    /// The decisions settled since the last [`Subject::decisions_since`], not consumed.
    pub(crate) fn peek_records(&self) -> &[ti4_engine_legacy::choice::DecisionRecord] {
        self.game.table.log.records.get(self.records_seen..).unwrap_or_default()
    }

    /// Drop the answers a step did not use.
    pub(crate) fn clear_script(&self) {
        self.script.borrow_mut().clear();
    }

    /// Queue one scripted entry (a multi-ask answer of the new engine, expanded for the string's questions).
    pub(crate) fn answer_scripted(&mut self, player: Option<String>, entry: Scripted) {
        self.script.borrow_mut().push(player, entry);
    }

    /// For each question answered since the script was last cleared, the entry that answered it.
    pub(crate) fn answered_by(&self) -> Vec<usize> {
        self.script.borrow().answered_by.clone()
    }

    /// The follower's answers the string was not offered since the script was last cleared (the error texts).
    pub(crate) fn diverged(&self) -> Vec<String> {
        self.script.borrow().diverged.clone()
    }

    /// Whether the string's next question is the rest of a multi-ask's expansion the last step ended inside ([`Script::answers_mid`]): the
    /// string asks it in a step of its own where the core asked one question, so the shadow steps the string again before it compares.
    pub(crate) fn next_is_mid_entry(&self) -> bool {
        dual::with_mask(self.mask, || self.game.legal_options()).is_some_and(|choice| self.script.borrow().answers_mid(&choice))
    }

    /// The events pushed since the last [`Subject::events_since`], not consumed.
    pub(crate) fn peek_events(&self) -> &[String] {
        self.game.events.get(self.events_seen..).unwrap_or_default()
    }
}

impl<M: MaskChoice> Subject for MaskSubject<M> {
    fn new(spec: &GameSpec, pilot: Pilot) -> Result<Self, String> {
        let mask = M::mask();
        let mut game = dual::with_mask(mask, || crate::legacy_game::legacy_game(spec))?;
        let script = Rc::new(RefCell::new(Script::default()));
        match pilot {
            Pilot::Bot => game.table.set_default(Box::new(crate::legacy_adapter::ThinDecider::new(crate::workload::golden_decider(spec.seed)))),
            Pilot::Script => game.table.set_default(Box::new(Follower(Rc::clone(&script)))),
        }
        let (events_seen, records_seen) = (game.events.len(), game.table.log.records.len());
        Ok(Self { game, mask, script, events_seen, records_seen, _mask: PhantomData })
    }

    fn options(&self) -> Option<Choice> {
        dual::with_mask(self.mask, || self.game.legal_options()).map(|c| crate::legacy_adapter::convert(&c))
    }

    fn in_transaction(&self) -> bool {
        self.game.step_facts().open & ti4_engine_core::driver::session::bit(ti4_engine_core::state::WindowKind::Trade) != 0
    }

    fn answer(&mut self, option_id: &str) {
        self.script.borrow_mut().push(None, Scripted::One(option_id.to_owned()));
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
        let new = self.game.table.log.records.get(self.records_seen..).unwrap_or_default().iter().map(crate::legacy_adapter::convert).collect();
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
        ti4_engine_legacy::core_sync::unsupported::find(&self.game.state).map(|d| format!("{d:?}"))
    }

    fn decisions_hash(&self) -> String {
        ti4_wasm::host_session::decisions_hash(&crate::legacy_adapter::thin_records(&self.game.table.log.records))
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
        // T-I5-1 (docs/todo/D2-islands-machines.md, question 6): the pre-step question of a trade step is not comparable.
        let trading = a.in_transaction() || b.in_transaction();
        let (offered_a, offered_b) = if trading { (None, None) } else { (a.options(), b.options()) };
        let subtype = if trading {
            "transaction".to_owned()
        } else {
            offered_b.as_ref().and_then(|c| c.context.as_ref()).map_or_else(|| "-".to_owned(), |c| c.subtype.clone())
        };
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
        let mut raw = compare_views(&view_a, &view_b);
        raw.extend(a.take_extra());
        raw.extend(b.take_extra());
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

    fn legacy_choice(player: &str, prompt: &str, ids: &[&str]) -> ti4_engine_legacy::choice::Choice {
        let options = ids.iter().map(|id| ti4_engine_legacy::choice::ChoiceOption::new(*id, "test")).collect();
        ti4_engine_legacy::choice::Choice::new(ti4_model::id::PlayerId::new(player), prompt, options)
    }

    /// The string asks a production's payment an atom per step where the core asks the set once (nekro 4p, titans 5p, hacan 5p seed 1 on the
    /// multi-ask line): the shadow steps the string on while its next question is the rest of the entry a step ended inside, and not for the
    /// question of the next entry.
    #[test]
    fn the_rest_of_a_payment_a_step_ended_inside_is_told_from_the_next_question() {
        let pay = |n: u8| legacy_choice("d", &format!("pay {n} more resources"), &["exhaust|jol", "trade_good"]);
        let produce = legacy_choice("d", "produce in 12 (1 left)", &["build|fighter|2", "done_producing"]);
        let mut script = Script::default();
        script.push(Some("d".to_owned()), Scripted::Set(vec!["trade_good".to_owned(), "trade_good".to_owned()]));
        script.push(None, Scripted::One("build|fighter|2".to_owned()));
        assert!(!script.answers_mid(&pay(2)), "nothing answered yet: no step ended inside the entry");
        assert_eq!(script.take(&pay(2)), "trade_good");
        assert!(script.answers_mid(&pay(1)), "the step ended after the first atom: the next payment question is the rest");
        assert!(script.answers_mid(&pay(1)), "the probe takes nothing off the queue");
        assert!(!script.answers_mid(&produce), "the next entry's question");
        assert_eq!(script.take(&pay(1)), "trade_good");
        assert!(!script.answers_mid(&produce), "spent");
        assert_eq!(script.take(&produce), "build|fighter|2");
    }

    /// A follower's answer the string was not offered is kept as the string's error text, also where the string's step swallows the error (a
    /// window inside an `emit` whose result is dropped: arborec 6p seed 1, S1 with the first seat's question missing from the records).
    #[test]
    fn a_diverged_answer_is_kept_as_the_error_text_until_the_script_is_cleared() {
        use ti4_engine_legacy::choice::Decider;
        let script = Rc::new(RefCell::new(Script::default()));
        script.borrow_mut().push(None, Scripted::One("reaction:l1z1x:ACTION_CARD_PLAYED:when".to_owned()));
        let mut follower = Follower(Rc::clone(&script));
        let asked = legacy_choice("c", "when ACTION_CARD_PLAYED", &["reaction:xxcha:ACTION_CARD_PLAYED:when", "decline"]);
        assert!(follower.choose(&asked).is_err());
        let diverged = script.borrow().diverged.clone();
        assert_eq!(diverged, vec![r#"script wanted "reaction:l1z1x:ACTION_CARD_PLAYED:when" but c was offered ["reaction:xxcha:ACTION_CARD_PLAYED:when", "decline"]"#.to_owned()]);
        script.borrow_mut().clear();
        assert!(script.borrow().diverged.is_empty());
    }

    /// A small golden-like scenario: four seats of the lobby factions on a template map.
    fn spec() -> GameSpec {
        GameSpec {
            setup: "4p".to_owned(),
            seed: 7,
            players: 4,
            map: "4pOuterRim".to_owned(),
            factions: ti4_engine_legacy::seating::LOBBY_FACTIONS.iter().take(4).map(|f| (*f).to_owned()).collect(),
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

    #[test]
    fn the_masks_are_the_legacy_and_the_core_ones() {
        assert_eq!(LegacyMask::mask(), AreaMask::STRING);
        assert_ne!(CoreMask::mask(), AreaMask::STRING);
        assert_eq!(CoreMask::mask().backend(Area::Combat), Backend::Core);
    }

    #[test]
    fn a_mask_subject_reports_what_the_game_it_wraps_holds() {
        let mut s = MaskSubject::<LegacyMask>::new(&spec(), Pilot::Bot).expect("builds");
        assert!(!s.finished());
        assert!(s.round() >= 1);
        assert_ne!(s.rules_version(), 0);
        let (mut asked, mut events, mut rng, mut records) = (false, 0, false, 0);
        for _ in 0..600 {
            asked |= s.options().is_some_and(|c| !c.options.is_empty() && !c.prompt.is_empty());
            let _ = s.step();
            events += s.events_since().len();
            records += s.decisions_since().len();
            rng |= !s.rng_positions().is_empty();
        }
        assert!(asked, "a question was offered");
        assert!(events > 0 && records > 0, "events {events}, decisions {records}");
        assert!(rng, "the random streams moved in 600 steps");
        assert!(s.events_since().is_empty(), "drained");
        // a switched-off feature is named
        let mut diplomacy = MaskSubject::<LegacyMask>::new(&spec(), Pilot::Bot).expect("builds");
        diplomacy.game.state.diplomacy.enabled = true;
        assert_eq!(diplomacy.disabled().as_deref(), Some("Diplomacy"));
    }

    #[test]
    fn the_views_carry_the_question_and_the_decisions() {
        let mut s = MaskSubject::<LegacyMask>::new(&spec(), Pilot::Bot).expect("builds");
        let mut choice = None;
        for _ in 0..200 {
            choice = s.options();
            if choice.is_some() {
                break;
            }
            let _ = s.step();
        }
        let choice = choice.expect("a question within 200 steps");
        let view = choice_view(Some(&choice));
        assert_eq!(view["prompt"], json!(choice.prompt));
        assert_eq!(view["options"].as_array().map(Vec::len), Some(choice.options.len()));
        assert_eq!(choice_view(None), Value::Null);
        let _ = s.step();
        let records = s.decisions_since();
        assert!(!records.is_empty());
        let rv = records_view(&records);
        assert_eq!(rv.as_array().map(Vec::len), Some(records.len()));
        assert_eq!(rv[0]["chosen"], json!(records[0].chosen));
        assert_eq!(rv[0]["v1"].as_str().map(str::len), Some(64));
    }

    #[test]
    fn the_round_limit_stops_a_scenario_at_the_round_and_the_counters_split_by_category() {
        let (list, mut tally) = (AllowList::empty(), Tally::default());
        let played = play_scenario::<MaskSubject<LegacyMask>, Tamper<LegacyMask>>(&spec(), LIMITS, &mut judge(&list, &mut tally)).expect("builds").expect("not skipped");
        let c = &played.report.areas[&Area::Driver];
        assert_eq!(c.differing[Category::State as usize], 1, "the one leaf");
        for other in [Category::Decision, Category::DecisionHash, Category::Value, Category::Event, Category::Rng] {
            assert_eq!(c.differing[other as usize], 0, "{other:?}");
            assert_eq!(c.compared[other as usize], 5, "{other:?} compared at each of the five steps");
        }
        let one_round = Limits { max_rounds: 1, max_steps: 100_000 };
        let (list, mut tally) = (AllowList::empty(), Tally::default());
        let played = play_scenario::<MaskSubject<LegacyMask>, MaskSubject<LegacyMask>>(&spec(), one_round, &mut judge(&list, &mut tally)).expect("builds").expect("not skipped");
        assert!(played.stopped.as_deref().is_some_and(|s| s.contains("round 2")), "stops as soon as round 2 starts: {:?}", played.stopped);
    }
}
