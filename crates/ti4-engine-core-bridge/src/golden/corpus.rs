//! The sources of the decision corpus and the bot-driven play that records them (moved here from
//! `bin/decision_corpus.rs` so that `golden` reuses it; the bin keeps its CLI, its files and its output byte for byte).
//!
//! `play` runs one game exactly as `selfplay run` does (the rule bot at every seat) and records every decision:
//! one JSON line per decision (`{"s","g","step","k","state","choice","chosen","v1","v2"}`, the format of the X2 corpus),
//! plus the same facts in structured form (`DecMeta`) for the slim tier.

use std::collections::BTreeMap;
use std::{cell::RefCell, rc::Rc};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use ti4_engine::choice::{Choice, ChoiceOption, Decider, DecisionLog, IllegalChoice, SeatObservation, SeededRandom};
use ti4_engine::fingerprint::{CanonicalHashVersion, decision_hash};
use ti4_selfplay::spec::{GameSpec, Matrix, SEATS};

pub const SOURCES: [&str; 10] = [
    "golden20", "bench", "bench-v1", "bench-v2", "bench-v3", "bench-v4", "bench-v5", "templates", "te", "fixtures",
];
/// The bench games recorded in full: seeds 21, 22 of the 4-player setup and seeds 21, 22 of the 6-player
/// setup (indices into the expanded config; the first ten of each setup are `golden20`).
pub const BENCH_PICKS: [usize; 4] = [20, 21, 120, 121];
/// Steps played from a template's opening.
pub const OPENING_STEPS: usize = 60;
/// Steps played at a Thunder's Edge table.
pub const TE_STEPS: usize = 5000;

#[must_use]
pub fn sha_hex(bytes: &[u8], digits: usize) -> String {
    let d = Sha256::digest(bytes);
    let mut s = hex_of(&d);
    s.truncate(digits);
    s
}

#[must_use]
pub fn hex_of(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// The first 16 hex digits of the SHA-256 of the serialised state: the only trace of a state a golden file keeps.
#[must_use]
pub fn state_hash(state: &ti4_model::state::GameState) -> String {
    sha_hex(serde_json::to_string(state).expect("state").as_bytes(), 16)
}

// ---- playing -------------------------------------------------------------------------------------------

type Captured = Rc<RefCell<Vec<(Choice, String)>>>;

struct Bot {
    seed: u64,
    decided: usize,
    random: SeededRandom,
    captured: Captured,
}

impl Bot {
    fn decide(&mut self, choice: &Choice, seen: Option<&SeatObservation<'_>>) -> Result<ChoiceOption, IllegalChoice> {
        let index = self.decided;
        let option = match seen {
            Some(seen) if !choice.options.is_empty() => {
                let seat = SEATS.iter().position(|s| *s == choice.player.as_str()).unwrap_or(0);
                // The thread's tuning was installed by the caller (`selfplay run --bot bot_version=N`).
                let (picked, _, _) = ti4_rule_bot::choose_explained(choice, Some(seen), self.seed, seat, index);
                choice.options[picked].clone()
            }
            _ => self.random.choose(choice)?,
        };
        self.decided += 1;
        self.captured.borrow_mut().push((choice.clone(), option.id.clone()));
        Ok(option)
    }
}

struct Shared(Rc<RefCell<Bot>>);

impl Decider for Shared {
    fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
        self.0.borrow_mut().decide(choice, None)
    }
    fn choose_seeing(&mut self, choice: &Choice, seen: &SeatObservation<'_>) -> Result<ChoiceOption, IllegalChoice> {
        self.0.borrow_mut().decide(choice, Some(seen))
    }
}

/// V1 and V2 (first 16 hex digits) of the decision record of `choice` answered with `chosen`.
#[must_use]
pub fn decision_hashes(choice: &Choice, chosen: &str) -> (String, String) {
    let mut log = DecisionLog::default();
    log.record(choice, &ChoiceOption::new(chosen, ""));
    let record = &log.records[0];
    let v1 = decision_hash(CanonicalHashVersion::V1, record).digest;
    let v2 = decision_hash(CanonicalHashVersion::V2, record).digest;
    (v1[..16].to_owned(), v2[..16].to_owned())
}

/// The entry line of one decision.
#[must_use]
pub fn line(source: &str, game: usize, step: usize, k: usize, state: &str, choice: &Choice, chosen: &str) -> String {
    let (v1, v2) = decision_hashes(choice, chosen);
    let entry = json!({
        "s": source, "g": game, "step": step, "k": k, "state": state,
        "choice": serde_json::to_value(choice).expect("choice"),
        "chosen": chosen, "v1": v1, "v2": v2,
    });
    serde_json::to_string(&entry).expect("line")
}

/// The structured form of one recorded decision (what the slim tier keeps).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecMeta {
    pub step: usize,
    pub k: usize,
    pub chosen: String,
    pub v2: String,
    pub subtype: String,
}

pub struct Played {
    pub lines: Vec<String>,
    pub meta: Vec<DecMeta>,
    /// `(step, state hash)` at the start of every step `n` with `n % checkpoint_every == 0` (empty without it).
    pub checkpoints: Vec<(usize, String)>,
    pub game_hash: String,
    pub steps: usize,
    pub finished: bool,
    /// Hash of the state after the last step.
    pub end_state: String,
    /// `GameState::rules_version` the game was played with.
    pub rules_version: u32,
}

/// Play one game exactly as `selfplay run` (the rule bot at every seat), recording every decision.
///
/// # Errors
/// A game that cannot be built or a step that fails.
pub fn play(
    source: &str,
    game_no: usize,
    spec: &GameSpec,
    max_rounds: u32,
    max_steps: usize,
    checkpoint_every: Option<usize>,
) -> Result<Played, String> {
    let mut game = spec.build()?;
    let rules_version = game.state.rules_version;
    let captured: Captured = Rc::default();
    game.table.set_default(Box::new(Shared(Rc::new(RefCell::new(Bot {
        seed: spec.seed,
        decided: 0,
        random: SeededRandom::new(spec.seed),
        captured: captured.clone(),
    })))));
    let mut lines = Vec::new();
    let mut meta = Vec::new();
    let mut checkpoints = Vec::new();
    let mut steps = 0usize;
    let mut log = 0usize;
    while !game.state.finished && game.state.round <= max_rounds && steps < max_steps {
        let before = game.state.clone();
        if checkpoint_every.is_some_and(|n| steps % n.max(1) == 0) {
            checkpoints.push((steps, state_hash(&before)));
        }
        let result = game.step();
        if let Some(error) = result.error {
            return Err(format!("{source} game {game_no} seed {}: step {steps}: {error}", spec.seed));
        }
        let asked = std::mem::take(&mut *captured.borrow_mut());
        if !asked.is_empty() {
            let state = state_hash(&before);
            for (k, (choice, chosen)) in asked.iter().enumerate() {
                lines.push(line(source, game_no, steps, k, &state, choice, chosen));
                meta.push(DecMeta {
                    step: steps,
                    k,
                    chosen: chosen.clone(),
                    v2: decision_hashes(choice, chosen).1,
                    subtype: choice.context.as_ref().map(|c| c.subtype.clone()).unwrap_or_default(),
                });
            }
            log += asked.len();
        }
        steps += 1;
    }
    if log != game.table.log.records.len() {
        return Err(format!(
            "{source} game {game_no}: {log} decisions captured, {} logged",
            game.table.log.records.len()
        ));
    }
    Ok(Played {
        lines,
        meta,
        checkpoints,
        game_hash: ti4_wasm::host_session::decisions_hash(&game.table.log.records),
        steps,
        finished: game.state.finished,
        end_state: state_hash(&game.state),
        rules_version,
    })
}

pub struct Job {
    pub game_no: usize,
    pub spec: GameSpec,
    pub max_steps: usize,
}

pub struct Plan {
    pub jobs: Vec<Job>,
    pub max_rounds: u32,
    pub bot: BTreeMap<String, String>,
    pub note: String,
}

/// # Errors
/// An unreadable config.
pub fn config_plan(config: &str, bot_version: Option<&str>, picks: Option<&[usize]>) -> Result<Plan, String> {
    let (matrix, specs) = crate::workload::specs(config)?;
    let Matrix { limits, mut bot, .. } = matrix;
    if let Some(v) = bot_version {
        bot.insert("bot_version".to_owned(), v.to_owned());
    }
    let jobs = specs
        .into_iter()
        .enumerate()
        .filter(|(i, _)| picks.is_none_or(|p| p.contains(i)))
        .map(|(game_no, spec)| Job { game_no, spec, max_steps: limits.max_steps })
        .collect();
    let note = format!("{config} bot {bot:?}{}", picks.map_or(String::new(), |p| format!(" games {p:?}")));
    Ok(Plan { jobs, max_rounds: limits.max_rounds, bot, note })
}

#[must_use]
pub fn opening_spec(alias: &str, players: usize, thunders_edge: bool, turn: usize) -> GameSpec {
    let pool = ti4_engine::seating::LOBBY_FACTIONS;
    GameSpec {
        setup: "templates".to_owned(),
        seed: 1,
        players,
        map: alias.to_owned(),
        factions: (0..players).map(|s| pool[(turn + s) % pool.len()].to_owned()).collect(),
        speaker: "seat".to_owned(),
        card_set: None,
        rotation: None,
        starting_tech: true,
        thunders_edge,
        rules_version: None,
    }
}

/// The games of an X2 source (`None` for `fixtures`, which has no games).
///
/// # Errors
/// A config or template list that cannot be read.
pub fn plan_for(source: &str) -> Result<Option<Plan>, String> {
    let rb = "tools/rulebot";
    Ok(Some(match source {
        "golden20" => config_plan("tools/engine_profile/profile-20.json", None, None)?,
        "bench" => config_plan(&format!("{rb}/golden.json"), None, Some(&BENCH_PICKS))?,
        "bench-v1" => config_plan(&format!("{rb}/golden-v2.json"), Some("1"), Some(&BENCH_PICKS))?,
        "bench-v2" => config_plan(&format!("{rb}/golden-v2.json"), Some("2"), Some(&BENCH_PICKS))?,
        "bench-v3" => config_plan(&format!("{rb}/golden-v2.json"), Some("3"), Some(&BENCH_PICKS))?,
        "bench-v4" => config_plan(&format!("{rb}/golden-v4.json"), Some("4"), Some(&BENCH_PICKS))?,
        "bench-v5" => config_plan(&format!("{rb}/golden-v5.json"), Some("5"), Some(&BENCH_PICKS))?,
        "templates" => {
            let loader = ti4_host::maps::template_loader::TemplateLoader::load().map_err(|e| format!("{e:?}"))?;
            let mut jobs = Vec::new();
            let mut turn = 0usize;
            for summary in loader.list_summaries() {
                for te in [false, true] {
                    let spec = opening_spec(&summary.alias, summary.player_count, te, turn);
                    turn += summary.player_count;
                    // Templates the host refuses to seat (e.g. a one-player map) have no opening.
                    if spec.build().is_ok() {
                        jobs.push(Job { game_no: jobs.len(), spec, max_steps: OPENING_STEPS });
                    }
                }
            }
            Plan { jobs, max_rounds: 50, bot: BTreeMap::new(), note: format!("every template opening, {OPENING_STEPS} steps") }
        }
        "te" => {
            let pool = ti4_engine::seating::LOBBY_FACTIONS;
            let jobs = (0..pool.len() / 6)
                .map(|i| Job {
                    game_no: i,
                    spec: GameSpec { seed: 7 + i as u64, ..opening_spec("6pStandard", 6, true, i * 6) },
                    max_steps: TE_STEPS,
                })
                .collect();
            Plan { jobs, max_rounds: 50, bot: BTreeMap::new(), note: format!("Thunder's Edge 6p tables, every lobby faction, {TE_STEPS} steps") }
        }
        _ => return Ok(None),
    }))
}

// ---- fixtures ------------------------------------------------------------------------------------------

fn find_choices(v: &Value, out: &mut Vec<Value>) {
    match v {
        Value::Object(m) => {
            let is_choice = m.get("prompt").is_some_and(Value::is_string)
                && m.get("options").and_then(Value::as_array).is_some_and(|o| o.iter().all(|x| x.get("id").is_some()));
            if is_choice {
                out.push(v.clone());
            }
            m.values().for_each(|x| find_choices(x, out));
        }
        Value::Array(a) => a.iter().for_each(|x| find_choices(x, out)),
        _ => {}
    }
}

/// The Choice objects inside the checked-in wire fixtures, as corpus lines.
///
/// # Errors
/// A fixture that cannot be read or does not parse as a `Choice`.
pub fn fixture_lines() -> Result<Vec<String>, String> {
    let mut files = Vec::new();
    for dir in ["fixtures/protocol", "crates/ti4-server/tests/fixtures", "crates/ti4-host/tests/fixtures", "crates/ti4-wasm/tests/it/fixtures"] {
        let Ok(rd) = std::fs::read_dir(dir) else { continue };
        for e in rd.flatten() {
            if e.path().extension().is_some_and(|x| x == "json") {
                files.push(e.path());
            }
        }
    }
    files.sort();
    let mut lines = Vec::new();
    for (g, path) in files.iter().enumerate() {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let Ok(value) = serde_json::from_str::<Value>(&text) else { continue };
        let mut found = Vec::new();
        find_choices(&value, &mut found);
        for (k, choice_json) in found.into_iter().enumerate() {
            let choice: Choice = serde_json::from_value(choice_json.clone()).map_err(|e| format!("{}: {e}", path.display()))?;
            // A fixture has no state; its Choice is recorded as the engine's types read and write it.
            let chosen = choice.options.first().map_or(String::new(), |o| o.id.clone());
            let name = path.display().to_string();
            let mut l = line("fixtures", g, 0, k, &sha_hex(name.as_bytes(), 16), &choice, &chosen);
            // The file the Choice came from, for the reader.
            l.insert_str(1, &format!("\"file\":{},", serde_json::to_string(&name).expect("name")));
            lines.push(l);
        }
    }
    Ok(lines)
}

/// Parse a recorded line and recompute what can be recomputed without an engine run: the Choice JSON must
/// round-trip through `ti4_engine::choice::Choice`, the chosen id must have been offered, V1 and V2 must equal the
/// hashes of the decision record.
///
/// # Errors
/// A message naming the first thing that does not hold.
pub fn check_line(l: &str) -> Result<(), String> {
    let v: Value = serde_json::from_str(l).map_err(|e| e.to_string())?;
    let choice_json = v.get("choice").ok_or("no choice")?;
    let choice: Choice = serde_json::from_value(choice_json.clone()).map_err(|e| e.to_string())?;
    if serde_json::to_value(&choice).map_err(|e| e.to_string())? != *choice_json {
        return Err("Choice JSON does not round-trip through ti4_engine::choice::Choice".to_owned());
    }
    let chosen = v["chosen"].as_str().ok_or("no chosen")?;
    if choice.free_form.is_none() && !choice.options.iter().any(|o| o.id == chosen) && v["s"] != "fixtures" {
        return Err(format!("chosen {chosen} not among the offered"));
    }
    let (v1, v2) = decision_hashes(&choice, chosen);
    for (key, h) in [("v1", v1), ("v2", v2)] {
        if v[key].as_str() != Some(&h) {
            return Err(format!("{key} differs: recorded {}, computed {h}", v[key]));
        }
    }
    Ok(())
}
