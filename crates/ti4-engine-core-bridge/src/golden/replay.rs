//! Answer-driven replay (docs/todo/D4-core-tests.md 4.4) and bot-driven recording.
//!
//! `check` does NOT run the bot: a `Recorded` decider answers every question with the next recorded `chosen` id,
//! after computing the V2 hash of (asked Choice, recorded answer) and comparing it with the recorded V2; at every
//! step with a recorded state hash it compares the state hash first. A bot change (BOT_VERSION, tuning keys, float
//! rounding in the valuation) therefore cannot move a golden file (T2); only `record`/`bless` run the bot.
//!
//! The engine sits behind the [`Replayer`] trait so that the bless guards and the report can be tested with a fake.

use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};
use std::fmt;
use std::path::Path;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use serde_json::Value;
use ti4_engine::choice::{Choice, ChoiceOption, Decider, IllegalChoice, SeatObservation};
use ti4_engine::dual::{self, AreaMask, Backend};
use ti4_selfplay::spec::GameSpec;

use super::corpus::{self, decision_hashes, state_hash};
use super::format::{GameFile, Limits, SLIM_STRIDE, Tier};
use super::sets::{GameEntry, SetMeta};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DivKind {
    /// The state hash at the start of a step differs.
    State,
    /// The V2 hash of (asked Choice, recorded answer) differs: another question, other options, another context.
    Answer,
    /// The engine asked more questions in the step than were recorded.
    Unexpected,
    /// The engine asked fewer questions in the step than were recorded.
    Missing,
    /// The state after the last step, the `finished` flag or the decision-log hash differs.
    End,
    /// The game ended before the recorded step count.
    EarlyFinish,
    /// A step failed.
    EngineError,
}

impl DivKind {
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            DivKind::State => "state",
            DivKind::Answer => "answer",
            DivKind::Unexpected => "unexpected-question",
            DivKind::Missing => "missing-question",
            DivKind::End => "end",
            DivKind::EarlyFinish => "early-finish",
            DivKind::EngineError => "engine-error",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Divergence {
    pub step: usize,
    pub k: usize,
    pub kind: DivKind,
    pub recorded_answer: Option<String>,
    /// The first 12 option ids the engine offered.
    pub offered: Vec<String>,
    pub recorded_v2: Option<String>,
    pub actual_v2: Option<String>,
    /// The subtype of the question (the area of the window), when there was one.
    pub subtype: String,
    pub detail: String,
    /// Full tier: differences between the recorded Choice JSON and the one asked now.
    pub choice_diff: Vec<String>,
}

impl fmt::Display for Divergence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "step {} k {} [{}] subtype `{}`", self.step, self.k, self.kind.name(), self.subtype)?;
        if !self.detail.is_empty() {
            write!(f, ": {}", self.detail)?;
        }
        if let Some(a) = &self.recorded_answer {
            write!(f, "\n    recorded answer: {a}")?;
        }
        if !self.offered.is_empty() {
            write!(f, "\n    offered (first 12): {}", self.offered.join(", "))?;
        }
        if self.recorded_v2.is_some() || self.actual_v2.is_some() {
            write!(f, "\n    recorded {} actual {}", self.recorded_v2.as_deref().unwrap_or("-"), self.actual_v2.as_deref().unwrap_or("-"))?;
        }
        for d in &self.choice_diff {
            write!(f, "\n    choice: {d}")?;
        }
        Ok(())
    }
}

impl Divergence {
    fn plain(step: usize, kind: DivKind, detail: String) -> Self {
        Self { step, k: 0, kind, recorded_answer: None, offered: Vec::new(), recorded_v2: None, actual_v2: None, subtype: String::new(), detail, choice_diff: Vec::new() }
    }
}

/// Differences between two JSON values as `path: recorded => actual`, at most `limit`.
#[must_use]
pub fn json_diff(recorded: &Value, actual: &Value, limit: usize) -> Vec<String> {
    fn walk(path: &str, a: &Value, b: &Value, out: &mut Vec<String>, limit: usize) {
        if out.len() >= limit || a == b {
            return;
        }
        match (a, b) {
            (Value::Object(x), Value::Object(y)) => {
                let keys: std::collections::BTreeSet<&String> = x.keys().chain(y.keys()).collect();
                for k in keys {
                    walk(&format!("{path}.{k}"), x.get(k).unwrap_or(&Value::Null), y.get(k).unwrap_or(&Value::Null), out, limit);
                }
            }
            (Value::Array(x), Value::Array(y)) => {
                for i in 0..x.len().max(y.len()) {
                    walk(&format!("{path}[{i}]"), x.get(i).unwrap_or(&Value::Null), y.get(i).unwrap_or(&Value::Null), out, limit);
                }
            }
            _ => {
                let cut = |v: &Value| {
                    let s = v.to_string();
                    if s.len() > 120 { format!("{}...", &s[..s.char_indices().nth(120).map_or(s.len(), |(i, _)| i)]) } else { s }
                };
                out.push(format!("{path}: {} => {}", cut(a), cut(b)));
            }
        }
    }
    let mut out = Vec::new();
    walk("", recorded, actual, &mut out, limit);
    out
}

// ---- the recorded decider ------------------------------------------------------------------------------

struct Expect {
    k: usize,
    chosen: String,
    v2: String,
    raw: Option<String>,
}

#[derive(Default)]
struct Rec {
    step: usize,
    queue: VecDeque<Expect>,
    asked: usize,
    diverged: Option<Divergence>,
}

impl Rec {
    fn answer(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
        let fallback = |choice: &Choice| {
            choice.options.first().cloned().ok_or_else(|| IllegalChoice::NoOptions { player: choice.player.clone(), prompt: choice.prompt.clone() })
        };
        if self.diverged.is_some() {
            return fallback(choice);
        }
        let k = self.asked;
        self.asked += 1;
        let subtype = choice.context.as_ref().map(|c| c.subtype.clone()).unwrap_or_default();
        let offered: Vec<String> = choice.options.iter().take(12).map(|o| o.id.clone()).collect();
        let Some(exp) = self.queue.pop_front() else {
            self.diverged = Some(Divergence {
                step: self.step,
                k,
                kind: DivKind::Unexpected,
                recorded_answer: None,
                offered,
                recorded_v2: None,
                actual_v2: None,
                subtype,
                detail: "the engine asked a question the recording does not have".to_owned(),
                choice_diff: Vec::new(),
            });
            return fallback(choice);
        };
        let (_, actual) = decision_hashes(choice, &exp.chosen);
        if actual != exp.v2 {
            let choice_diff = exp
                .raw
                .as_deref()
                .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
                .and_then(|v| v.get("choice").cloned())
                .zip(serde_json::to_value(choice).ok())
                .map(|(rec, now)| json_diff(&rec, &now, 12))
                .unwrap_or_default();
            self.diverged = Some(Divergence {
                step: self.step,
                k: exp.k,
                kind: DivKind::Answer,
                recorded_answer: Some(exp.chosen),
                offered,
                recorded_v2: Some(exp.v2),
                actual_v2: Some(actual),
                subtype,
                detail: String::new(),
                choice_diff,
            });
            return fallback(choice);
        }
        Ok(choice.options.iter().find(|o| o.id == exp.chosen).cloned().unwrap_or_else(|| ChoiceOption::new(exp.chosen, "")))
    }
}

struct Recorded(Rc<RefCell<Rec>>);

impl Decider for Recorded {
    fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
        self.0.borrow_mut().answer(choice)
    }
    fn choose_seeing(&mut self, choice: &Choice, _seen: &SeatObservation<'_>) -> Result<ChoiceOption, IllegalChoice> {
        self.0.borrow_mut().answer(choice)
    }
}

/// Replay one recorded game by its answers on the current thread's mask. `Ok(None)`: it reproduced.
///
/// # Errors
/// A game that cannot be built.
pub fn replay_game(g: &GameFile) -> Result<Option<Divergence>, String> {
    let Some(spec) = &g.header.spec else { return Ok(static_lines(g)) };
    let mut game = spec.build()?;
    let rec = Rc::new(RefCell::new(Rec::default()));
    game.table.set_default(Box::new(Recorded(rec.clone())));
    let checks = g.state_checks();
    let mut next_check = 0usize;
    let mut cursor = 0usize;
    for step in 0..g.header.steps {
        if game.state.finished {
            return Ok(Some(Divergence::plain(step, DivKind::EarlyFinish, format!("the game ended after {step} of {} recorded steps", g.header.steps))));
        }
        if checks.get(next_check).is_some_and(|(s, _)| *s == step) {
            let want = checks[next_check].1;
            next_check += 1;
            let have = state_hash(&game.state);
            if have != want {
                let mut d = Divergence::plain(step, DivKind::State, "the state at the start of the step differs from the recording".to_owned());
                d.recorded_v2 = Some(want.to_owned());
                d.actual_v2 = Some(have);
                return Ok(Some(d));
            }
        }
        {
            let mut r = rec.borrow_mut();
            r.step = step;
            r.asked = 0;
            r.queue.clear();
            while let Some(d) = g.decs.get(cursor).filter(|d| d.step == step) {
                r.queue.push_back(Expect { k: d.k, chosen: d.chosen.clone(), v2: d.v2.clone(), raw: d.raw.clone() });
                cursor += 1;
            }
        }
        let result = game.step();
        let mut r = rec.borrow_mut();
        if let Some(d) = r.diverged.take() {
            return Ok(Some(d));
        }
        if let Some(e) = result.error {
            return Ok(Some(Divergence::plain(step, DivKind::EngineError, e.to_string())));
        }
        if let Some(missing) = r.queue.front() {
            let mut d = Divergence::plain(step, DivKind::Missing, format!("{} recorded question(s) were not asked", r.queue.len()));
            d.k = missing.k;
            d.recorded_answer = Some(missing.chosen.clone());
            d.recorded_v2 = Some(missing.v2.clone());
            return Ok(Some(d));
        }
    }
    let end = g.header.steps;
    if game.state.finished != g.header.finished {
        return Ok(Some(Divergence::plain(end, DivKind::End, format!("finished is {} now, {} recorded", game.state.finished, g.header.finished))));
    }
    if let Some(want) = &g.header.end_state {
        let have = state_hash(&game.state);
        if &have != want {
            let mut d = Divergence::plain(end, DivKind::End, "the state after the last step differs".to_owned());
            d.recorded_v2 = Some(want.clone());
            d.actual_v2 = Some(have);
            return Ok(Some(d));
        }
    }
    let hash = ti4_wasm::host_session::decisions_hash(&game.table.log.records);
    if hash != g.header.decisions_hash {
        return Ok(Some(Divergence::plain(end, DivKind::End, format!("decision-log hash {hash} now, {} recorded", g.header.decisions_hash))));
    }
    Ok(None)
}

/// A static game (the Choice objects of the wire fixtures) has nothing to play: every line is checked on its own.
pub fn static_lines(g: &GameFile) -> Option<Divergence> {
    for (i, d) in g.decs.iter().enumerate() {
        if let Some(raw) = &d.raw
            && let Err(e) = corpus::check_line(raw)
        {
            let mut v = Divergence::plain(d.step, DivKind::Answer, format!("line {i}: {e}"));
            v.k = d.k;
            return Some(v);
        }
    }
    None
}

// ---- the engine behind a trait -------------------------------------------------------------------------

/// What `record` needs to play a game again with the bot.
#[derive(Debug, Clone)]
pub struct RecordReq {
    pub spec: GameSpec,
    pub bot: BTreeMap<String, String>,
    pub limits: Limits,
    pub tier: Tier,
    pub source: Option<String>,
    pub game: Option<usize>,
}

pub enum Recording {
    Game(Box<GameFile>),
    /// Not recorded, with the reason (a dual difference: T13, never freeze a bug).
    Excluded(String),
}

pub trait Replayer: Sync {
    /// Replay by the recorded answers.
    ///
    /// # Errors
    /// A game that cannot be built.
    fn check(&self, game: &GameFile) -> Result<Option<Divergence>, String>;
    /// Play the game with the bot and record it.
    ///
    /// # Errors
    /// A game that cannot be played.
    fn record(&self, req: &RecordReq) -> Result<Recording, String>;
    /// Whether `record` must run one game at a time (the dual report is process-wide).
    fn serial(&self) -> bool {
        false
    }
}

/// The real engine. `mask`: the area mask of the replay (`None`: the process default, `TI4_AREAS`).
/// `require_clean`: `record` first plays the game under this mask (normally `all=dual`) and refuses a game with a
/// difference, then plays it under `all=core` and demands identical lines.
#[derive(Clone, Copy, Default)]
pub struct EngineReplayer {
    pub mask: Option<AreaMask>,
    pub require_clean: Option<AreaMask>,
}

fn with_optional_mask<T>(mask: Option<AreaMask>, body: impl FnOnce() -> T) -> T {
    match mask {
        Some(m) => dual::with_mask(m, body),
        None => body(),
    }
}

/// Per mille of the steps whose arm the core ran without a lend, from the `d1::step` census of a dual run.
#[must_use]
pub fn pure_milli(report: &dual::DualReport) -> Option<u32> {
    let compared = report.windows.get("d1::step")?.compared;
    if compared == 0 {
        return None;
    }
    let bailed: u64 = report.windows.iter().filter(|(k, _)| k.starts_with("d1::step [bail:")).map(|(_, w)| w.bailed).sum();
    Some((compared.saturating_sub(bailed) * 1000 / compared) as u32)
}

impl Replayer for EngineReplayer {
    fn check(&self, game: &GameFile) -> Result<Option<Divergence>, String> {
        with_optional_mask(self.mask, || replay_game(game))
    }

    fn serial(&self) -> bool {
        self.require_clean.is_some()
    }

    fn record(&self, req: &RecordReq) -> Result<Recording, String> {
        ti4_rule_bot::tuning::set(ti4_selfplay::driver::tuning_from(&req.bot)?);
        let checkpoint = (req.tier == Tier::Slim).then_some(SLIM_STRIDE);
        let source = req.source.as_deref().unwrap_or("golden");
        let game_no = req.game.unwrap_or(0);
        let play = || corpus::play(source, game_no, &req.spec, req.limits.max_rounds, req.limits.max_steps, checkpoint);
        let mut header = super::format::blank_header(Some(req.spec.clone()));
        header.bot = req.bot.clone();
        header.limits = req.limits.clone();
        header.source = req.source.clone();
        header.game = req.game;
        let Some(dual_mask) = self.require_clean else {
            let played = with_optional_mask(self.mask, play)?;
            return Ok(Recording::Game(Box::new(GameFile::from_played(req.tier, header, played))));
        };
        // The dual report is process-wide: the caller plays one game at a time (workers = 1) so the counts are this game's.
        let _ = dual::take_report();
        let played = dual::with_mask(dual_mask, play)?;
        let report = dual::take_report();
        if report.total_differences() > 0 {
            let first = report.differences.first().map(ToString::to_string).unwrap_or_default();
            return Ok(Recording::Excluded(format!("{} dual difference(s); first: {}", report.total_differences(), first.lines().next().unwrap_or(""))));
        }
        let pure = pure_milli(&report);
        let core = dual::with_mask(AreaMask::all(Backend::Core), play)?;
        if core.lines != played.lines || core.game_hash != played.game_hash || core.end_state != played.end_state {
            return Err(format!(
                "{source} game {game_no}: the all=core play differs from the all=dual play (decisions {} vs {}, hash {} vs {}): a porting bug, not a recording problem",
                core.lines.len(),
                played.lines.len(),
                core.game_hash,
                played.game_hash
            ));
        }
        let mut g = GameFile::from_played(req.tier, header, played);
        g.header.pure_milli = pure;
        Ok(Recording::Game(Box::new(g)))
    }
}

// ---- checking a set ------------------------------------------------------------------------------------

#[derive(Debug)]
pub struct GameResult {
    pub file: String,
    pub decisions: usize,
    pub millis: u128,
    pub outcome: Result<Option<Divergence>, String>,
}

impl GameResult {
    #[must_use]
    pub fn ok(&self) -> bool {
        matches!(self.outcome, Ok(None))
    }
}

/// Replay the games of a set (all, or those whose file name contains one of `only`) on `threads` threads.
/// Also verifies each file against `set.json` (sha256 of the text, decision count, header version).
///
/// # Errors
/// An unreadable `set.json`.
pub fn check_set(dir: &Path, meta: &SetMeta, replayer: &dyn Replayer, only: &[String], smoke: bool, threads: usize) -> Result<Vec<GameResult>, String> {
    let files: Vec<&GameEntry> = meta
        .games
        .iter()
        .filter(|g| if smoke { meta.smoke.contains(&g.file) } else { only.is_empty() || only.iter().any(|o| g.file.contains(o.as_str())) })
        .collect();
    let next = AtomicUsize::new(0);
    let results: Vec<Mutex<Option<GameResult>>> = files.iter().map(|_| Mutex::new(None)).collect();
    std::thread::scope(|scope| {
        for _ in 0..threads.max(1) {
            scope.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::SeqCst);
                    let Some(entry) = files.get(i) else { break };
                    let started = std::time::Instant::now();
                    let outcome = (|| -> Result<Option<Divergence>, String> {
                        let (game, sha) = GameFile::read(&dir.join(&entry.file))?;
                        if sha != entry.sha256 {
                            return Err(format!("{}: sha256 of the text differs from set.json (the file was edited outside `golden bless`)", entry.file));
                        }
                        if game.decs.len() != entry.decisions || game.header.decisions != entry.decisions {
                            return Err(format!("{}: decision count differs from set.json", entry.file));
                        }
                        if !meta.ladder && game.header.rules_version != meta.rules_version {
                            return Err(format!("{}: header rules_version {:?} differs from the set's {:?}", entry.file, game.header.rules_version, meta.rules_version));
                        }
                        replayer.check(&game)
                    })();
                    *results[i].lock().expect("lock") =
                        Some(GameResult { file: entry.file.clone(), decisions: entry.decisions, millis: started.elapsed().as_millis(), outcome });
                }
            });
        }
    });
    Ok(results.into_iter().map(|c| c.into_inner().expect("lock").expect("ran")).collect())
}

/// A replayer that only checks what needs no engine: file integrity (sha256, counts, versions, done by `check_set`)
/// and, for the full tier, every line's Choice round trip and V1/V2 hashes (`corpus::check_line`).
pub struct StaticOnly;

impl Replayer for StaticOnly {
    fn check(&self, g: &GameFile) -> Result<Option<Divergence>, String> {
        Ok(static_lines(g))
    }
    fn record(&self, _: &RecordReq) -> Result<Recording, String> {
        Err("a static replayer does not record".to_owned())
    }
}
