//! What the search asks of a position: priors over the candidates and a 4-seat value (`docs/MCTS_BOT_PLAN.md` 1.2, 3.2).
//!
//! * [`Uniform`]: flat priors, uniform value (the plumbing floor).
//! * [`RuleBotPrior`]: the rule bot's pick as the prior (`weight` of the mass on it), a heuristic value (uniform or the VP share).
//! * [`Rollout`]: a rule-bot rollout from the position (to the end, or `horizon` answers then the VP share) as the value; flat or
//!   rule-bot priors.
//! * [`NetEvaluator`]: the policy + value net of `ti4-mcts-net` (lane L4) on the features of `ti4-mcts-features`: the option logits
//!   of the candidates and the win head as the value (wired at integration 30).
//! * [`FnEvaluator`]: any closure that maps the position, the choice and the candidates to logits and an actor-relative value.

use std::path::Path;
use std::sync::Arc;

use ti4_engine_core::choice::Choice;
use ti4_mcts_features::encode::{self, Encoded, Encoder, FA, FG, FO, FP, FS, MAX_OBJ, MAX_SYS};
use ti4_mcts_features::npy;
use ti4_mcts_net::{Net, Scratch};
use ti4_engine_search::agent::{RuleBotAgent, SeatAgent};
use ti4_engine_search::chance::SearchRng;
use ti4_engine_search::{Node, Position};

use crate::value::{SEATS, UNIFORM, Value, outcome_value, to_relative, vp_value};

/// An evaluation of a decision position.
#[derive(Debug, Clone, PartialEq)]
pub struct Evaluation {
    /// One logit per candidate (the order of the candidate list the search passed).
    pub logits: Vec<f32>,
    /// Win probabilities, **actor-relative** (`value[0]` the actor of the choice, then the seats after it in seat order).
    pub value: Value,
}

/// Priors and value of decision positions.
pub trait Evaluator {
    /// Evaluate the question `choice` at `pos` over `candidates` (option indexes of `choice`). `key` names the tree node (a stochastic
    /// evaluator seeds itself with it, so the search stays deterministic). `pos` may be forked, never changed.
    ///
    /// # Errors
    /// A message that fails the expansion (the search drops the edge).
    fn evaluate(&mut self, pos: &Position, choice: &Choice, candidates: &[usize], key: u64) -> Result<Evaluation, String>;

    /// A short name for reports.
    fn name(&self) -> String {
        "evaluator".to_owned()
    }
}

/// Flat priors, uniform value.
#[derive(Debug, Clone, Copy, Default)]
pub struct Uniform;

impl Evaluator for Uniform {
    fn evaluate(&mut self, _pos: &Position, _choice: &Choice, candidates: &[usize], _key: u64) -> Result<Evaluation, String> {
        Ok(Evaluation { logits: vec![0.0; candidates.len()], value: UNIFORM })
    }

    fn name(&self) -> String {
        "uniform".to_owned()
    }
}

/// The leaf value of the non-rollout stubs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LeafValue {
    Uniform,
    /// `softmax(vp / temp)` of the position's points.
    VpShare { temp: f32 },
}

impl LeafValue {
    fn of(self, pos: &Position) -> Value {
        match self {
            Self::Uniform => UNIFORM,
            Self::VpShare { temp } => vp_value(&pos.vp(), temp),
        }
    }
}

/// Logits that give `weight` of the softmax mass to candidate `at` and spread the rest evenly (flat when `at` is `None`).
#[must_use]
pub fn one_hot_logits(k: usize, at: Option<usize>, weight: f32) -> Vec<f32> {
    let mut logits = vec![0.0; k];
    if let Some(at) = at.filter(|a| *a < k)
        && k > 1
    {
        let w = weight.clamp(1e-3, 1.0 - 1e-3);
        logits[at] = (w / (1.0 - w) * (k - 1) as f32).ln();
    }
    logits
}

/// The rule bot's pick as the prior.
#[derive(Debug, Clone, Copy)]
pub struct RuleBotPrior {
    /// The prior mass on the rule bot's pick (when it is a candidate).
    pub weight: f32,
    pub value: LeafValue,
}

impl Default for RuleBotPrior {
    fn default() -> Self {
        Self { weight: 0.6, value: LeafValue::VpShare { temp: 2.0 } }
    }
}

impl RuleBotPrior {
    /// The logits alone.
    ///
    /// # Errors
    /// The rule bot's error.
    pub fn logits(&self, pos: &Position, candidates: &[usize], key: u64) -> Result<Vec<f32>, String> {
        let bot = RuleBotAgent { view: true, seed: Some(key), index: None };
        let (picked, _, _, _) = bot.pick(pos)?;
        Ok(one_hot_logits(candidates.len(), candidates.iter().position(|c| *c == picked), self.weight))
    }
}

impl Evaluator for RuleBotPrior {
    fn evaluate(&mut self, pos: &Position, choice: &Choice, candidates: &[usize], key: u64) -> Result<Evaluation, String> {
        let logits = self.logits(pos, candidates, key)?;
        Ok(Evaluation { logits, value: to_relative(&self.value.of(pos), usize::from(choice.actor.0) % SEATS) })
    }

    fn name(&self) -> String {
        "rule-prior".to_owned()
    }
}

/// A rollout by the rule bot (with its view, seeded by the node) as the value: to the end of the game, or `horizon` answers and then
/// the VP share at `temp`. Chance requests are answered from the search's own stream (seeded by the node).
#[derive(Debug, Clone, Copy)]
pub struct Rollout {
    pub horizon: usize,
    pub temp: f32,
    /// Rule-bot priors (else flat).
    pub prior: Option<RuleBotPrior>,
    /// Answers given by all rollouts (accounting).
    pub answers: u64,
}

impl Rollout {
    /// A rollout of at most `horizon` answers with flat priors.
    #[must_use]
    pub const fn new(horizon: usize) -> Self {
        Self { horizon, temp: 2.0, prior: None, answers: 0 }
    }

    /// The same with rule-bot priors.
    #[must_use]
    pub const fn with_rule_prior(mut self) -> Self {
        self.prior = Some(RuleBotPrior { weight: 0.6, value: LeafValue::Uniform });
        self
    }

    /// The absolute value of a rollout from `pos`.
    ///
    /// # Errors
    /// A failed position or a rule-bot error.
    pub fn value(&mut self, pos: &Position, key: u64) -> Result<Value, String> {
        let mut p = pos.fork_search();
        let mut rng = SearchRng::at(key, &[0x726f_6c6c]);
        let mut bot = RuleBotAgent { view: true, seed: Some(key ^ 0x726f_6c6c), index: None };
        for _ in 0..self.horizon {
            match p.node() {
                Node::End => return Ok(outcome_value(&p.outcome())),
                Node::Failed(why) => return Err(format!("rollout: {why}")),
                Node::Chance(req) => {
                    let faces = rng.sample(&req);
                    p.answer_chance(faces).map_err(|e| e.to_string())?;
                }
                Node::Decide(choice) => {
                    let choice = choice.clone();
                    let d = bot.decide(&p, &choice)?;
                    p.apply(d).map_err(|e| e.to_string())?;
                    self.answers += 1;
                }
            }
        }
        Ok(if p.is_terminal() { outcome_value(&p.outcome()) } else { vp_value(&p.vp(), self.temp) })
    }
}

impl Evaluator for Rollout {
    fn evaluate(&mut self, pos: &Position, choice: &Choice, candidates: &[usize], key: u64) -> Result<Evaluation, String> {
        let logits = match self.prior {
            Some(p) => p.logits(pos, candidates, key)?,
            None => vec![0.0; candidates.len()],
        };
        let value = self.value(pos, key)?;
        Ok(Evaluation { logits, value: to_relative(&value, usize::from(choice.actor.0) % SEATS) })
    }

    fn name(&self) -> String {
        format!("rollout{}{}", self.horizon, if self.prior.is_some() { "+rule-prior" } else { "" })
    }
}

/// The evaluator signature of [`FnEvaluator`].
pub type EvalFn = dyn FnMut(&Position, &Choice, &[usize], u64) -> Result<Evaluation, String>;

/// The net hook: a closure (the forward pass of `ti4-mcts-net` on the features of `ti4-mcts-features`: encode the actor's `Seen`, take
/// the option logits of the candidates, rotate nothing: the net's value head is already actor-relative).
pub struct FnEvaluator {
    pub name: String,
    pub f: Box<EvalFn>,
}

impl Evaluator for FnEvaluator {
    fn evaluate(&mut self, pos: &Position, choice: &Choice, candidates: &[usize], key: u64) -> Result<Evaluation, String> {
        (self.f)(pos, choice, candidates, key)
    }

    fn name(&self) -> String {
        self.name.clone()
    }
}

/// The net as the evaluator: encode the actor's `Seen` (as the lab's greedy `PolicyAgent` does, every float rounded to the f16 the
/// shards store), one forward pass, the raw option logits of the candidates and the softmax of the win head.
///
/// Seat order: the net's value head is relative to the **seating order** (`encode::relative_seats`: the actor, then clockwise), this
/// crate's [`Evaluation::value`] to the **seat index** (`(actor + k) % 4`). The win probabilities are put back on their absolute seats
/// first and then turned actor-relative with [`to_relative`], so the two orders need not agree (they do in the MCTS setup).
pub struct NetEvaluator {
    pub net: Arc<Net>,
    enc: Encoder,
    buf: Encoded,
    scratch: Scratch,
    /// Forward passes made (accounting).
    pub passes: u64,
}

impl NetEvaluator {
    #[must_use]
    pub fn new(net: Arc<Net>) -> Self {
        Self { net, enc: Encoder::default(), buf: Encoded::default(), scratch: Scratch::default(), passes: 0 }
    }

    /// Load a `.t4w` file and check that it fits this build's features.
    ///
    /// # Errors
    /// Weights that do not load, or feature widths that are not this build's.
    pub fn load(path: &Path) -> Result<Net, String> {
        let net = Net::load(path)?;
        if !net.fits(FS, FP, FO, FG, FA, MAX_SYS, MAX_OBJ, SEATS) {
            return Err(format!("{}: the net's feature widths {:?} are not this build's", path.display(), net.dims));
        }
        Ok(net)
    }
}

impl Evaluator for NetEvaluator {
    fn evaluate(&mut self, pos: &Position, choice: &Choice, candidates: &[usize], _key: u64) -> Result<Evaluation, String> {
        let seen = pos.seen().ok_or("net: the position asks no question")?;
        self.enc.encode_into(&seen, choice, &mut self.buf);
        let e = &mut self.buf;
        for v in e.sys.iter_mut().chain(e.seat.iter_mut()).chain(e.obj.iter_mut()).chain(e.glob.iter_mut()).chain(e.opt.iter_mut()) {
            *v = npy::f16_round(*v);
        }
        let out = self.net.forward(&self.buf, &mut self.scratch);
        self.passes += 1;
        let logits = candidates.iter().map(|&c| out.logits.get(c).copied().ok_or_else(|| format!("net: no logit for option {c}"))).collect::<Result<Vec<_>, _>>()?;
        let win = out.win_probs();
        let mut abs = [0.0; SEATS];
        for (k, seat) in encode::relative_seats(&seen, choice.actor).iter().enumerate() {
            if let (Some(slot), Some(p)) = (abs.get_mut(usize::from(seat.0) % SEATS), win.get(k)) {
                *slot = *p;
            }
        }
        Ok(Evaluation { logits, value: to_relative(&abs, usize::from(choice.actor.0) % SEATS) })
    }

    fn name(&self) -> String {
        "net".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gumbel::softmax;

    #[test]
    fn one_hot_logits_put_the_weight_on_the_pick() {
        let p = softmax(&one_hot_logits(5, Some(2), 0.6));
        assert!((p[2] - 0.6).abs() < 1e-5, "{p:?}");
        assert!((p[0] - 0.1).abs() < 1e-5);
        assert_eq!(one_hot_logits(3, None, 0.6), vec![0.0; 3]);
        assert_eq!(one_hot_logits(1, Some(0), 0.6), vec![0.0]);
    }
}
