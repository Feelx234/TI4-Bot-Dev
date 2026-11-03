//! The shadow harness of the typed rule bot (docs/todo/D3-bots.md section 5; binary `bot_shadow`).
//!
//! Plays bot games exactly as `workload::golden_games` (and `selfplay run`) does, and at every decider call that
//! has a position (`choose_seeing`, non-empty choice) computes two answers to the same question: the string
//! bot's `(picked, smart, rng position, next word)` and the typed bot's, and compares them. The game itself is
//! always answered with the string pick, so the typed half can never change the game under test.
//!
//! D3-0: the typed half is `ti4_rule_bot_typed::choose_typed_explained`, which answers with the string bot
//! ([`Mode::SelfCheck`]): the harness is proven to compare every call and to catch a difference (the
//! [`Mutant`]s) before any typed code exists. D3-2 adds the real question source ([`TypedSource`], [`Mode::Source`]).

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use rand::RngCore;
use rand_chacha::ChaCha8Rng;
use sha2::{Digest, Sha256};
use ti4_engine::choice::{Choice, ChoiceOption, Decider, IllegalChoice, SeatObservation, SeededRandom};
use ti4_engine_legacy::render::Typed;
use ti4_selfplay::spec::{GameSpec, SEATS};

/// Where the typed question of a decision comes from.
pub trait TypedSource {
    /// The typed twin of `choice` (index-aligned), or `None` when the core has none for this window.
    fn typed_for(&mut self, choice: &Choice, seen: &SeatObservation<'_>) -> Option<Typed>;
}

/// D3-0: no source. In [`Mode::Source`] every call is skipped as `NoTyped`.
pub struct NoTyped;

impl TypedSource for NoTyped {
    fn typed_for(&mut self, _choice: &Choice, _seen: &SeatObservation<'_>) -> Option<Typed> {
        None
    }
}

/// How the typed half is fed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Typed = string: every call is compared (D3-0).
    SelfCheck,
    /// The [`TypedSource`] says whether there is a typed question; calls without one are skipped.
    Source,
}

/// A deliberate bug in the typed half, to prove the harness fails on it (the mutation check of the package).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mutant {
    None,
    /// The typed pick is the next option.
    Pick,
    /// The typed answer claims the opposite opinion.
    Smart,
    /// The typed half draws one word more from the random stream.
    Rng,
}

impl Mutant {
    /// # Errors
    /// An unknown name.
    pub fn parse(name: &str) -> Result<Self, String> {
        match name {
            "none" => Ok(Self::None),
            "pick" => Ok(Self::Pick),
            "smart" => Ok(Self::Smart),
            "rng" => Ok(Self::Rng),
            other => Err(format!("unknown mutant '{other}' (none, pick, smart, rng)")),
        }
    }
}

/// Why a call was not compared.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Reason {
    NoTyped,
    NotPorted(String),
    SeenUnavailable(String),
    Misaligned,
}

impl Reason {
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::NoTyped => "no-typed".to_owned(),
            Self::NotPorted(what) => format!("not-ported:{what}"),
            Self::SeenUnavailable(why) => format!("seen-unavailable:{why}"),
            Self::Misaligned => "misaligned".to_owned(),
        }
    }
}

/// What one decider call came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Compared,
    Skipped(Reason),
}

/// One difference between the two halves.
#[derive(Debug, Clone)]
pub struct Mismatch {
    pub game: usize,
    pub spec: String,
    pub decision: usize,
    pub subtype: String,
    pub options: Vec<String>,
    pub string: Answer,
    pub typed: Answer,
}

/// The triple of a bot answer, with the position of the random stream after it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Answer {
    pub picked: usize,
    pub smart: bool,
    pub word_pos: u128,
    pub next_word: u64,
}

impl Answer {
    fn of(picked: usize, smart: bool, mut rng: ChaCha8Rng) -> Self {
        let word_pos = rng.get_word_pos();
        Self { picked, smart, word_pos, next_word: rng.next_u64() }
    }
}

/// Counts of one decision subtype.
#[derive(Debug, Clone, Default)]
pub struct SubtypeStat {
    /// Decider calls with a position and a non-empty choice.
    pub calls: u64,
    pub compared: u64,
    pub skipped: BTreeMap<String, u64>,
    pub mismatches: u64,
}

/// One game played.
#[derive(Debug, Clone)]
pub struct GameReport {
    pub index: usize,
    /// Decider calls of the game (with or without a position).
    pub decisions: usize,
    /// Hash chain over the serde JSON of every choice the string bot was asked (guards trap T2 for D3-2).
    pub choices_hash: String,
    /// The game's decision hash, as `workload::golden_games` reports it.
    pub decisions_hash: String,
}

/// The result of a run.
#[derive(Debug, Clone, Default)]
pub struct Report {
    pub by_subtype: BTreeMap<String, SubtypeStat>,
    pub mismatches: Vec<Mismatch>,
    pub games: Vec<GameReport>,
    /// `Observed` and id-constructor calls per subtype (feature `observed-count`), else empty.
    pub view_calls: BTreeMap<String, BTreeMap<&'static str, u64>>,
    /// The typed-view parity (`seen_stride`): positions loaded, positions the typed state could not hold (by
    /// reason), accessor answers compared, and the differences.
    pub parity: Parity,
}

/// The result of the typed-view parity of a run.
#[derive(Debug, Clone, Default)]
pub struct Parity {
    pub positions: u64,
    pub unmapped: BTreeMap<String, u64>,
    pub compared: u64,
    pub skipped: BTreeMap<&'static str, u64>,
    pub mismatches: Vec<String>,
}

impl Report {
    #[must_use]
    pub fn calls(&self) -> u64 {
        self.by_subtype.values().map(|s| s.calls).sum()
    }

    #[must_use]
    pub fn compared(&self) -> u64 {
        self.by_subtype.values().map(|s| s.compared).sum()
    }

    /// Compared / calls of one subtype (`"*"`: all); 1.0 for a subtype never asked.
    #[must_use]
    pub fn coverage(&self, subtype: &str) -> f64 {
        let (calls, compared) = if subtype == "*" {
            (self.calls(), self.compared())
        } else {
            self.by_subtype.get(subtype).map_or((0, 0), |s| (s.calls, s.compared))
        };
        if calls == 0 { 1.0 } else { compared as f64 / calls as f64 }
    }
}

/// What to run.
pub struct Config {
    pub mode: Mode,
    pub mutant: Mutant,
    /// `key=value` overrides of the tuning on top of the matrix's own (`bot_version=N`).
    pub bot: BTreeMap<String, String>,
    /// Stop each game after this many engine steps (the matrix limit when `None`).
    pub max_steps: Option<usize>,
    /// Also compare the typed view (`Seen`) with the string view at every `stride`-th call (0: off).
    pub seen_stride: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self { mode: Mode::SelfCheck, mutant: Mutant::None, bot: BTreeMap::new(), max_steps: None, seen_stride: 0 }
    }
}

struct Shadow {
    seed: u64,
    decided: usize,
    random: SeededRandom,
    mode: Mode,
    mutant: Mutant,
    source: Box<dyn TypedSource>,
    game: usize,
    spec_json: String,
    choices: Sha256,
    report: Report,
    parity_stride: usize,
    calls: usize,
}

fn subtype_of(choice: &Choice) -> String {
    choice.context.as_ref().map_or_else(
        || format!("~{}", choice.options.first().map_or("none", |o| o.kind.as_str())),
        |c| c.subtype.clone(),
    )
}

impl Shadow {
    fn decide(&mut self, choice: &Choice, seen: Option<&SeatObservation<'_>>) -> Result<ChoiceOption, IllegalChoice> {
        let index = self.decided;
        self.decided += 1;
        let Some(seen) = seen.filter(|_| !choice.options.is_empty()) else {
            return self.random.choose(choice);
        };
        let seat = SEATS.iter().position(|s| *s == choice.player.as_str()).unwrap_or(0);
        let subtype = subtype_of(choice);
        if let Ok(json) = serde_json::to_string(choice) {
            self.choices.update(json.as_bytes());
        }
        // The string half: the game's answer. Its view traffic is what `observed-count` counts.
        ti4_model::count::set_scope(Some(&subtype));
        let (picked, smart, rng) = ti4_rule_bot::choose_explained(choice, Some(seen), self.seed, seat, index);
        ti4_model::count::set_scope(None);
        let string = Answer::of(picked, smart, rng);

        self.calls += 1;
        if self.parity_stride > 0 && self.calls % self.parity_stride == 1 % self.parity_stride.max(1) {
            self.check_view(seen, &subtype, index);
        }
        let outcome = self.compare(choice, seen, &subtype, seat, index, string);
        let stat = self.report.by_subtype.entry(subtype).or_default();
        stat.calls += 1;
        match outcome {
            Outcome::Compared => stat.compared += 1,
            Outcome::Skipped(reason) => *stat.skipped.entry(reason.label()).or_default() += 1,
        }
        Ok(choice.options[picked].clone())
    }

    /// The typed view against the string view of this position.
    fn check_view(&mut self, seen: &SeatObservation<'_>, subtype: &str, decision: usize) {
        let p = &mut self.report.parity;
        p.positions += 1;
        match ti4_engine::seen_load::SeenOwned::of(seen.observed()) {
            Err(e) => *p.unmapped.entry(format!("{:?}: {} [{subtype} #{decision}]", e.slice, e.what)).or_default() += 1,
            Ok(owned) => {
                let found = crate::seen_parity::check(seen, &owned);
                p.compared += found.compared;
                for (k, n) in found.skipped {
                    *p.skipped.entry(k).or_default() += n;
                }
                for m in found.mismatches {
                    if p.mismatches.len() < 200 {
                        p.mismatches.push(format!("game {} decision {decision} ({subtype}): {m}", self.game));
                    }
                }
            }
        }
    }

    fn compare(
        &mut self,
        choice: &Choice,
        seen: &SeatObservation<'_>,
        subtype: &str,
        seat: usize,
        index: usize,
        string: Answer,
    ) -> Outcome {
        if self.mode == Mode::Source {
            match self.source.typed_for(choice, seen) {
                None => return Outcome::Skipped(Reason::NoTyped),
                Some(typed) if typed.rendered.options.len() != choice.options.len() => {
                    return Outcome::Skipped(Reason::Misaligned);
                }
                Some(_) => {}
            }
        }
        let (mut tp, mut ts, mut trng) =
            ti4_rule_bot_typed::choose_typed_explained(choice, Some(seen), self.seed, seat, index);
        match self.mutant {
            Mutant::None => {}
            Mutant::Pick => tp = (tp + 1) % choice.options.len(),
            Mutant::Smart => ts = !ts,
            Mutant::Rng => {
                let _ = trng.next_u64();
            }
        }
        let typed = Answer::of(tp, ts, trng);
        if typed != string {
            self.report.by_subtype.entry(subtype.to_owned()).or_default().mismatches += 1;
            self.report.mismatches.push(Mismatch {
                game: self.game,
                spec: self.spec_json.clone(),
                decision: index,
                subtype: subtype.to_owned(),
                options: choice.options.iter().map(|o| o.id.clone()).collect(),
                string,
                typed,
            });
        }
        Outcome::Compared
    }
}

struct Handle(Rc<RefCell<Shadow>>);

impl Decider for Handle {
    fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
        self.0.borrow_mut().decide(choice, None)
    }
    fn choose_seeing(&mut self, choice: &Choice, seen: &SeatObservation<'_>) -> Result<ChoiceOption, IllegalChoice> {
        self.0.borrow_mut().decide(choice, Some(seen))
    }
}

/// Play the games `include` accepts (indices into the config's expansion) and compare the two bots at every call.
///
/// # Errors
/// An unreadable config, an unknown tuning key, a game that fails.
pub fn run(
    config_path: &str,
    cfg: &Config,
    include: &dyn Fn(usize) -> bool,
    mut make_source: impl FnMut() -> Box<dyn TypedSource>,
) -> Result<Report, String> {
    let (matrix, specs) = crate::workload::specs(config_path)?;
    let mut overrides = matrix.bot.clone();
    overrides.extend(cfg.bot.clone());
    // The thread plays one tuning for both halves (trap T13): the shipped one, or the config's / the flags'.
    ti4_rule_bot::tuning::set(ti4_selfplay::driver::tuning_from(&overrides)?);
    let max_steps = cfg.max_steps.unwrap_or(matrix.limits.max_steps);
    let mut report = Report::default();
    let mut games = Vec::new();
    for (index, spec) in specs.iter().enumerate() {
        if include(index) {
            games.push((index, spec));
        }
    }
    for (index, spec) in games {
        let mut game = crate::legacy_game::legacy_game(spec)?;
        let shadow = Rc::new(RefCell::new(Shadow {
            seed: spec.seed,
            decided: 0,
            random: SeededRandom::new(spec.seed),
            mode: cfg.mode,
            mutant: cfg.mutant,
            source: make_source(),
            game: index,
            spec_json: spec_json(spec),
            choices: Sha256::new(),
            report: Report::default(),
            parity_stride: cfg.seen_stride,
            calls: 0,
        }));
        game.table.set_default(Box::new(crate::legacy_adapter::ThinDecider::new(Box::new(Handle(Rc::clone(&shadow))))));
        let mut steps = 0usize;
        while !game.state.finished && game.state.round <= matrix.limits.max_rounds && steps < max_steps {
            let result = game.step();
            steps += 1;
            if let Some(error) = result.error {
                return Err(format!("game {index} (seed {}): {error}", spec.seed));
            }
        }
        let decisions_hash = ti4_wasm::host_session::decisions_hash(&crate::legacy_adapter::thin_records(&game.table.log.records));
        drop(game);
        let mut shadow = Rc::try_unwrap(shadow).map_err(|_| "the shadow decider is still shared".to_owned())?.into_inner();
        let choices_hash = hex_of(&std::mem::take(&mut shadow.choices).finalize());
        report.games.push(GameReport { index, decisions: shadow.decided, choices_hash, decisions_hash });
        merge(&mut report, shadow.report);
    }
    ti4_model::count::set_scope(None);
    report.view_calls = ti4_model::count::take();
    Ok(report)
}

fn spec_json(spec: &GameSpec) -> String {
    serde_json::to_string(spec).unwrap_or_default()
}

fn hex_of(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

fn merge(into: &mut Report, from: Report) {
    for (subtype, stat) in from.by_subtype {
        let slot = into.by_subtype.entry(subtype).or_default();
        slot.calls += stat.calls;
        slot.compared += stat.compared;
        slot.mismatches += stat.mismatches;
        for (reason, n) in stat.skipped {
            *slot.skipped.entry(reason).or_default() += n;
        }
    }
    into.mismatches.extend(from.mismatches);
    into.parity.positions += from.parity.positions;
    into.parity.compared += from.parity.compared;
    for (k, n) in from.parity.skipped {
        *into.parity.skipped.entry(k).or_default() += n;
    }
    for (k, n) in from.parity.unmapped {
        *into.parity.unmapped.entry(k).or_default() += n;
    }
    into.parity.mismatches.extend(from.parity.mismatches);
}

/// The report as text: totals, the coverage table by subtype and the mismatches.
#[must_use]
pub fn render(report: &Report, mode: Mode) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    let _ = writeln!(
        out,
        "bot_shadow mode={mode:?}: {} games, {} calls with a position, {} compared ({:.2}%), {} mismatches",
        report.games.len(),
        report.calls(),
        report.compared(),
        100.0 * report.coverage("*"),
        report.mismatches.len()
    );
    for game in &report.games {
        let _ = writeln!(
            out,
            "  game {}: {} decisions, decisions_hash {}, choices_hash {}",
            game.index, game.decisions, game.decisions_hash, &game.choices_hash[..16]
        );
    }
    let _ = writeln!(out, "{:<44} {:>8} {:>8} {:>8}  skipped", "subtype", "calls", "compared", "mismatch");
    let mut rows: Vec<(&String, &SubtypeStat)> = report.by_subtype.iter().collect();
    rows.sort_by(|a, b| b.1.calls.cmp(&a.1.calls).then(a.0.cmp(b.0)));
    for (subtype, stat) in rows {
        let skipped: Vec<String> = stat.skipped.iter().map(|(r, n)| format!("{r}={n}")).collect();
        let _ = writeln!(
            out,
            "{subtype:<44} {:>8} {:>8} {:>8}  {}",
            stat.calls,
            stat.compared,
            stat.mismatches,
            skipped.join(" ")
        );
    }
    if report.parity.positions > 0 {
        let p = &report.parity;
        let _ = writeln!(
            out,
            "seen parity: {} positions, {} unmapped, {} accessor answers compared ({} bailed to the string side), {} differences",
            p.positions,
            p.unmapped.values().sum::<u64>(),
            p.compared,
            p.skipped.values().sum::<u64>(),
            p.mismatches.len()
        );
        for (accessor, n) in &p.skipped {
            let _ = writeln!(out, "  bailed to the string side: {accessor}: {n}");
        }
        for (reason, n) in &p.unmapped {
            let _ = writeln!(out, "  unmapped {reason}: {n}");
        }
        for m in p.mismatches.iter().take(40) {
            let _ = writeln!(out, "  DIFF {m}");
        }
    }
    for m in &report.mismatches {
        let _ = writeln!(
            out,
            "MISMATCH game {} decision {} subtype {}\n  spec {}\n  options {:?}\n  string {:?}\n  typed  {:?}",
            m.game, m.decision, m.subtype, m.spec, m.options, m.string, m.typed
        );
    }
    out
}

/// The report as JSON (for `--json`).
#[must_use]
pub fn to_json(report: &Report) -> serde_json::Value {
    let by_subtype: serde_json::Map<String, serde_json::Value> = report
        .by_subtype
        .iter()
        .map(|(k, s)| {
            (
                k.clone(),
                serde_json::json!({
                    "calls": s.calls, "compared": s.compared, "mismatches": s.mismatches, "skipped": s.skipped,
                }),
            )
        })
        .collect();
    let games: Vec<serde_json::Value> = report
        .games
        .iter()
        .map(|g| {
            serde_json::json!({
                "index": g.index, "decisions": g.decisions,
                "decisions_hash": g.decisions_hash, "choices_hash": g.choices_hash,
            })
        })
        .collect();
    serde_json::json!({
        "calls": report.calls(), "compared": report.compared(), "mismatches": report.mismatches.len(),
        "seen_parity": { "positions": report.parity.positions, "unmapped": report.parity.unmapped, "compared": report.parity.compared, "differences": report.parity.mismatches.len() },
        "games": games, "by_subtype": by_subtype, "view_calls": report.view_calls,
    })
}

/// A config file text for one game given as a `GameSpec` JSON (the `spec` of a mismatch report): the matrix whose
/// expansion is exactly that game, so a mismatch is replayed with `--spec`.
///
/// # Errors
/// A text that is not a game spec.
pub fn config_of_spec(spec_json: &str) -> Result<String, String> {
    let spec: GameSpec = serde_json::from_str(spec_json).map_err(|e| format!("--spec: {e}"))?;
    let factions: Vec<&str> = spec.factions.iter().map(String::as_str).collect();
    let mut setup = serde_json::json!({
        "name": spec.setup, "players": spec.players, "maps": [spec.map], "map_policy": "cycle",
        "factions": factions, "speaker": spec.speaker, "seeds": spec.seed.to_string(),
        "starting_tech": spec.starting_tech, "thunders_edge": spec.thunders_edge,
    });
    if let Some(set) = &spec.card_set {
        setup["card_set"] = serde_json::json!(set);
    }
    if let Some(v) = spec.rules_version {
        setup["rules_version"] = serde_json::json!(v);
    }
    let config = serde_json::json!({
        "limits": { "max_rounds": 50, "max_steps": 400_000, "max_seconds": 300, "loop_steps": 5000 },
        "setups": [setup],
    });
    Ok(config.to_string())
}

/// The view-call counts (feature `observed-count`): per subtype the calls of every counted name, and the total
/// per name over the run.
#[must_use]
pub fn render_view_calls(report: &Report) -> String {
    use std::fmt::Write;
    let mut out = String::from("view calls (ti4_model::count) by subtype\n");
    let mut total: BTreeMap<&'static str, u64> = BTreeMap::new();
    for (subtype, names) in &report.view_calls {
        let calls = report.by_subtype.get(subtype).map_or(0, |s| s.calls).max(1);
        let sum: u64 = names.values().sum();
        let _ = writeln!(out, "  {subtype}: {sum} calls over {calls} decisions = {:.1} per decision", sum as f64 / calls as f64);
        let mut rows: Vec<_> = names.iter().collect();
        rows.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
        for (name, n) in rows.into_iter().take(6) {
            let _ = writeln!(out, "      {name}: {n} ({:.2} per decision)", *n as f64 / calls as f64);
        }
        for (name, n) in names {
            *total.entry(name).or_default() += n;
        }
    }
    let _ = writeln!(out, "  total per name:");
    let mut rows: Vec<_> = total.into_iter().collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    for (name, n) in rows {
        let _ = writeln!(out, "      {name}: {n}");
    }
    out
}
