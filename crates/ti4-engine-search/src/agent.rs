//! Who answers a seat's questions (`docs/MCTS_BOT_PLAN.md` 4.4): the [`SeatAgent`] trait and its first implementations.

use ti4_chance::{Faces, Request};
use ti4_engine_core::choice::{Choice, Pick};
use ti4_rule_bot_typed::seen_view::SeenView;

use crate::BotMemory;
use crate::chance::SearchRng;
use crate::position::Position;

/// An answer, in one of the forms a position reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    /// A core pick (a search, a policy).
    Pick(Pick),
    /// The legal answer `i` (`Position::legal`, option `i` of the question).
    Index(usize),
    /// A wire option id, read as the product's live loop reads it (the rule bot's answer).
    Id(String),
}

/// What an agent decided: the answer, and the rule bot's movement memory after it (for an agent that runs the rule bot with its view).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    pub answer: Answer,
    pub memory: Option<BotMemory>,
}

impl From<Pick> for Decision {
    fn from(pick: Pick) -> Self {
        Self { answer: Answer::Pick(pick), memory: None }
    }
}

impl From<Answer> for Decision {
    fn from(answer: Answer) -> Self {
        Self { answer, memory: None }
    }
}

/// What was applied to a game (for an agent that keeps a tree between its decisions).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Applied {
    /// A seat answered a question.
    Decision { seat: u8, answer: Answer },
    /// The game's streams answered a chance request (play).
    Chance { request: Request, faces: Faces },
}

/// One seat's decision maker.
pub trait SeatAgent {
    /// The answer to `choice` (the position's question). `pos` may be forked freely; it is never changed here.
    ///
    /// # Errors
    /// A message that stops the game.
    fn decide(&mut self, pos: &Position, choice: &Choice) -> Result<Decision, String>;

    /// Something was applied to the game this agent plays in (another seat's answer, the real dice).
    fn observe(&mut self, pos: &Position, applied: &Applied) {
        let _ = (pos, applied);
    }
}

/// The rule bot (`ti4_rule_bot::choose_explained_view`) on the question as the product renders it.
///
/// * `view: false`: no view of the position, the rule bot's random pick (the bot of `native_play --quick` and the bridge's native
///   playthrough): the quick game through this agent is the product's quick game.
/// * `view: true`: the typed view (`Seen` -> `SeenView`, as the MCTS lab); its movement memory is restored from the position before the
///   answer and handed back with it ([`Decision::memory`]), so a search that asks it in many branches never leaks one branch's memory
///   into another.
///
/// The stream of a decision is `(seed, the seat's place, index)`: `seed` defaults to the game's, `index` to the position's decision count
/// (the product's numbering); a search sets both per node.
#[derive(Debug, Clone, Default)]
pub struct RuleBotAgent {
    pub view: bool,
    pub seed: Option<u64>,
    pub index: Option<usize>,
}

impl RuleBotAgent {
    /// The rule bot with its view (the MCTS benchmark seat).
    #[must_use]
    pub const fn seeing() -> Self {
        Self { view: true, seed: None, index: None }
    }

    /// The rule bot's random pick (no view).
    #[must_use]
    pub const fn blind() -> Self {
        Self { view: false, seed: None, index: None }
    }

    /// The pick, the "smart" flag (the rule bot's own opinion decided) and the memory after it.
    ///
    /// # Errors
    /// A position that asks nothing, or a question without options.
    pub fn pick(&self, pos: &Position) -> Result<(usize, String, bool, Option<BotMemory>), String> {
        let rendered = pos.rendered().ok_or("the position asks nothing")?;
        if rendered.options.is_empty() {
            return Err(format!("a question with no option: {}", rendered.prompt));
        }
        let t = pos.table();
        let place = t.seats.iter().position(|s| s == rendered.player.as_str()).unwrap_or(0);
        let seed = self.seed.unwrap_or(t.seed);
        let index = self.index.unwrap_or_else(|| pos.decisions());
        let (picked, smart, memory) = if self.view {
            ti4_rule_bot::backward::set_memory(pos.bot_memory().clone());
            let seen = pos.seen().ok_or("the position asks nothing")?;
            let view = SeenView::new(&seen, &t.owners, t.content, t.sources, Some(&t.galaxy));
            let (picked, smart, _) = ti4_rule_bot::choose_explained_view(&rendered, Some(&view), seed, place, index);
            (picked, smart, Some(ti4_rule_bot::backward::memory()))
        } else {
            let (picked, smart, _) = ti4_rule_bot::choose_explained_view(&rendered, None, seed, place, index);
            (picked, smart, None)
        };
        let picked = picked.min(rendered.options.len() - 1);
        Ok((picked, rendered.options[picked].id.clone(), smart, memory))
    }
}

impl SeatAgent for RuleBotAgent {
    fn decide(&mut self, pos: &Position, _choice: &Choice) -> Result<Decision, String> {
        let (_, id, _, memory) = self.pick(pos)?;
        Ok(Decision { answer: Answer::Id(id), memory })
    }
}

/// The scores of a policy for the legal answers of a question (one per answer; the highest is played).
pub type Scorer = dyn FnMut(&Position, &Choice, &[Pick]) -> Vec<f32>;

/// The greedy policy-net hook: on the asks `tree` names, the legal answer the scorer rates highest (ties: the first); everything else is
/// answered by `fallback` (the rule bot with its view, as inside the search).
pub struct PolicyAgent {
    scorer: Box<Scorer>,
    tree: fn(&Position, &Choice) -> bool,
    pub fallback: RuleBotAgent,
}

impl PolicyAgent {
    #[must_use]
    pub fn new(scorer: Box<Scorer>) -> Self {
        Self { scorer, tree: crate::advance::default_tree, fallback: RuleBotAgent::seeing() }
    }

    /// The same with another rule for which asks the policy answers.
    #[must_use]
    pub fn with_tree(mut self, tree: fn(&Position, &Choice) -> bool) -> Self {
        self.tree = tree;
        self
    }
}

impl SeatAgent for PolicyAgent {
    fn decide(&mut self, pos: &Position, choice: &Choice) -> Result<Decision, String> {
        if !(self.tree)(pos, choice) {
            return self.fallback.decide(pos, choice);
        }
        let legal = pos.legal();
        let scores = (self.scorer)(pos, choice, &legal);
        let best = scores.iter().take(legal.len()).enumerate().fold(None::<(usize, f32)>, |best, (i, &s)| match best {
            Some((_, b)) if b >= s => best,
            _ => Some((i, s)),
        });
        let (i, _) = best.ok_or("the policy scored no answer")?;
        Ok(Answer::Pick(legal[i]).into())
    }
}

/// A uniform random legal answer from its own stream.
pub struct RandomAgent(pub SearchRng);

impl SeatAgent for RandomAgent {
    fn decide(&mut self, pos: &Position, _choice: &Choice) -> Result<Decision, String> {
        let n = pos.legal().len();
        if n == 0 {
            return Err("a question with no option".to_owned());
        }
        Ok(Answer::Index(self.0.below(n)).into())
    }
}
