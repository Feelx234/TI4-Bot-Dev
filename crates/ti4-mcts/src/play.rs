//! Whole games with MCTS seats (the tests, `mcts_ab`, `mcts_bench`): the MCTS setup, the rule bot with its view at every other seat, the
//! core driver with observation on (tree reuse).

use std::sync::Arc;
use std::time::Instant;

use ti4_engine_search::agent::{RuleBotAgent, SeatAgent};
use ti4_engine_search::driver::{Limits, Record, play};
use ti4_engine_search::{Outcome, mcts_spec, start};

use crate::agent::{Counts, MctsAgent, Shared};
use ti4_mcts_net::Net;

use crate::eval::{Evaluator, NetEvaluator, RuleBotPrior, Rollout, Uniform};
use crate::search::{Config, Stats};

/// Which evaluator: `uniform`, `rule` (rule-bot prior, VP-share value), `rule-flat` (rule-bot prior, uniform value), `rollout:H` (flat
/// priors, rule-bot rollout of `H` answers), `rollout-rule:H` (the same with rule-bot priors), `net:PATH` (the policy + value net of a
/// `.t4w` file, [`NetEvaluator`]).
#[derive(Clone)]
pub enum EvalSpec {
    Uniform,
    RulePrior,
    RuleFlat,
    Rollout { horizon: usize, rule_prior: bool },
    Net(Arc<Net>),
}

impl std::fmt::Debug for EvalSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Uniform => f.write_str("Uniform"),
            Self::RulePrior => f.write_str("RulePrior"),
            Self::RuleFlat => f.write_str("RuleFlat"),
            Self::Rollout { horizon, rule_prior } => write!(f, "Rollout {{ horizon: {horizon}, rule_prior: {rule_prior} }}"),
            Self::Net(net) => write!(f, "Net({:?})", net.dims),
        }
    }
}

impl EvalSpec {
    /// # Errors
    /// An unknown name.
    pub fn parse(text: &str) -> Result<Self, String> {
        match text.split_once(':') {
            None if text == "uniform" => Ok(Self::Uniform),
            None if text == "rule" => Ok(Self::RulePrior),
            None if text == "rule-flat" => Ok(Self::RuleFlat),
            Some(("rollout", h)) => Ok(Self::Rollout { horizon: h.parse().map_err(|_| format!("horizon {h}"))?, rule_prior: false }),
            Some(("rollout-rule", h)) => Ok(Self::Rollout { horizon: h.parse().map_err(|_| format!("horizon {h}"))?, rule_prior: true }),
            Some(("net", path)) => Ok(Self::Net(Arc::new(NetEvaluator::load(std::path::Path::new(path))?))),
            _ => Err(format!("evaluator {text}: want uniform, rule, rule-flat, rollout:H, rollout-rule:H or net:PATH")),
        }
    }

    #[must_use]
    pub fn build(&self) -> Box<dyn Evaluator> {
        match self {
            Self::Uniform => Box::new(Uniform),
            Self::RulePrior => Box::new(RuleBotPrior::default()),
            Self::RuleFlat => Box::new(RuleBotPrior { value: crate::eval::LeafValue::Uniform, ..RuleBotPrior::default() }),
            Self::Rollout { horizon, rule_prior: false } => Box::new(Rollout::new(*horizon)),
            Self::Rollout { horizon, rule_prior: true } => Box::new(Rollout::new(*horizon).with_rule_prior()),
            Self::Net(net) => Box::new(NetEvaluator::new(Arc::clone(net))),
        }
    }
}

/// One game's result.
#[derive(Debug, Clone)]
pub struct GameReport {
    pub seed: u64,
    /// The MCTS seats.
    pub mcts: Vec<u8>,
    pub outcome: Outcome,
    pub steps: usize,
    pub decisions: usize,
    pub error: Option<String>,
    /// Summed over the MCTS seats.
    pub stats: Stats,
    pub counts: Counts,
    pub secs: f64,
    pub records: Vec<Record>,
}

/// Play the MCTS game of `seed` with an [`MctsAgent`] (`cfg`, evaluator `eval`, the search seed mixed with the seat) at each seat of
/// `mcts` and the rule bot with its view elsewhere, to the end or `limits`.
///
/// # Errors
/// The start of the game.
pub fn play_game(seed: u64, mcts: &[u8], cfg: &Config, eval: &EvalSpec, limits: Limits) -> Result<GameReport, String> {
    let mut pos = start(&mcts_spec(seed))?;
    let mut handles = Vec::new();
    let mut agents: Vec<Box<dyn SeatAgent>> = Vec::new();
    for s in 0..pos.table().seats.len() {
        let seat = u8::try_from(s).unwrap_or(u8::MAX);
        if mcts.contains(&seat) {
            let cfg = Config { seed: cfg.seed ^ seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ (u64::from(seat) << 56), ..cfg.clone() };
            let (shared, handle) = Shared::new(MctsAgent::new(cfg, eval.build()));
            handles.push(handle);
            agents.push(Box::new(shared));
        } else {
            agents.push(Box::new(RuleBotAgent::seeing()));
        }
    }
    let t = Instant::now();
    let mut records = Vec::new();
    let ended = play(&mut pos, &mut agents, limits, !handles.is_empty(), |_, r| records.push(r.clone()));
    let secs = t.elapsed().as_secs_f64();
    let (mut stats, mut counts) = (Stats::default(), Counts::default());
    for h in &handles {
        let a = h.borrow();
        stats.add(&a.stats);
        counts.searched += a.counts.searched;
        counts.full += a.counts.full;
        counts.forced += a.counts.forced;
        counts.delegated += a.counts.delegated;
    }
    Ok(GameReport { seed, mcts: mcts.to_vec(), outcome: ended.outcome, steps: ended.steps, decisions: ended.decisions, error: ended.error, stats, counts, secs, records })
}
