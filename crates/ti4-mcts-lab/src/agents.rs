//! Seat agents of the lab (docs/MCTS_BOT_PLAN.md 4.4, lane L5 stopgap): a game gives each seat its own agent.
//!
//! * [`Agent::Rule`]: the rule bot on the typed view (every ask).
//! * [`Agent::Policy`]: the **greedy policy net** (stepping stone 1): the net answers the tree-kind asks with two or more options
//!   after the hard filter (the best kept option), the rule bot everything else. No search.
//! * [`Agent::Random`]: uniform among the kept options at the tree asks, the rule bot elsewhere (a floor for the harness).
//!
//! The stopgap of the plan: it decides on the lab's `Decide` hook (no forking); the final `SeatAgent` trait comes with C6.

use std::sync::Arc;

use ti4_engine_core::seen::Seen;
use ti4_mcts_features::encode::{self, Encoder, Encoded, FA, FG, FO, FP, FS, MAX_OBJ, MAX_SYS, SEATS};
use ti4_mcts_features::{filter, kind, npy};
use ti4_mcts_net::{Net, Scratch};

use crate::{mix, rule_bot, Asked, Decide, Table};

/// Who plays a seat.
#[derive(Clone)]
pub enum Agent {
    Rule,
    Policy(Arc<Net>),
    Random,
}

impl Agent {
    /// `rule`, `random`, or `policy:PATH` (a `.t4w` file).
    ///
    /// # Errors
    /// An unknown name, or weights that do not load or do not fit the features.
    pub fn parse(text: &str) -> Result<Self, String> {
        match text.split_once(':') {
            None if text == "rule" => Ok(Self::Rule),
            None if text == "random" => Ok(Self::Random),
            Some(("policy", path)) => {
                let net = Net::load(std::path::Path::new(path))?;
                if !net.fits(FS, FP, FO, FG, FA, MAX_SYS, MAX_OBJ, SEATS) {
                    return Err(format!("{path}: the net's feature widths {:?} are not this build's", net.dims));
                }
                Ok(Self::Policy(Arc::new(net)))
            }
            _ => Err(format!("agent {text}: want rule, random or policy:PATH")),
        }
    }

    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Rule => "rule",
            Self::Policy(_) => "policy",
            Self::Random => "random",
        }
    }
}

/// Counters of one game's net decisions.
#[derive(Debug, Clone, Copy, Default)]
pub struct NetStats {
    /// Tree asks the net answered.
    pub answered: u64,
    /// Of them, the net's answer equals the rule bot's (computed only when `agreement` is on).
    pub agree: u64,
    pub compared: u64,
    /// Nanoseconds in encoding and in the forward pass.
    pub encode_ns: u128,
    pub net_ns: u128,
}

/// The deciders of one game: one agent per seat place.
pub struct Table4 {
    pub agents: Vec<Agent>,
    pub enc: Encoder,
    pub scratch: Scratch,
    pub buf: Encoded,
    pub seed: u64,
    /// Also ask the rule bot at the net's asks and count the agreement (costs the rule bot's time).
    pub agreement: bool,
    pub stats: NetStats,
}

impl Table4 {
    #[must_use]
    pub fn new(agents: Vec<Agent>, seed: u64) -> Self {
        Self { agents, enc: Encoder::default(), scratch: Scratch::default(), buf: Encoded::default(), seed, agreement: false, stats: NetStats::default() }
    }
}

/// Round every float feature to the half the shards store (the net was trained on those values).
fn round_halves(e: &mut Encoded) {
    for v in e.sys.iter_mut().chain(e.seat.iter_mut()).chain(e.obj.iter_mut()).chain(e.glob.iter_mut()).chain(e.opt.iter_mut()) {
        *v = npy::f16_round(*v);
    }
}

impl Decide for Table4 {
    fn decide(&mut self, t: &Table, a: &Asked<'_>) -> Result<usize, String> {
        let agent = self.agents.get(a.place).cloned().unwrap_or(Agent::Rule);
        if matches!(agent, Agent::Rule) || kind::tree_kind_of(a.choice).is_none() {
            return Ok(rule_bot(t, a).0);
        }
        let seen = Seen::new(a.at, t.kernel, a.choice.actor);
        match agent {
            Agent::Random => {
                let kept = filter::options(&seen, a.choice);
                if kept.len() < 2 {
                    return Ok(rule_bot(t, a).0);
                }
                Ok(kept[(mix(self.seed ^ (a.index as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15)) % kept.len() as u64) as usize])
            }
            Agent::Policy(net) => {
                let t0 = std::time::Instant::now();
                self.enc.encode_into(&seen, a.choice, &mut self.buf);
                if self.buf.mask.iter().filter(|m| **m).count() < 2 {
                    return Ok(rule_bot(t, a).0);
                }
                round_halves(&mut self.buf);
                let t1 = std::time::Instant::now();
                let pick = net.greedy(&self.buf, &mut self.scratch);
                self.stats.encode_ns += (t1 - t0).as_nanos();
                self.stats.net_ns += t1.elapsed().as_nanos();
                let Some(pick) = pick else { return Ok(rule_bot(t, a).0) };
                self.stats.answered += 1;
                if self.agreement {
                    let (rb, smart, _, _) = rule_bot(t, a);
                    if smart {
                        self.stats.compared += 1;
                        self.stats.agree += u64::from(rb == pick);
                    }
                }
                Ok(pick)
            }
            Agent::Rule => Ok(rule_bot(t, a).0),
        }
    }
}

/// The encoding of a policy decision, for tests: what [`Table4`] feeds the net.
#[must_use]
pub fn policy_input(seen: &Seen<'_>, choice: &ti4_engine_core::choice::Choice) -> Encoded {
    let mut e = encode::encode(seen, choice);
    round_halves(&mut e);
    e
}
