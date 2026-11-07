//! Depth-first helpers: play a search position on to the next point the tree branches on.

use ti4_chance::Request;
use ti4_engine_core::choice::{Choice, Origin};

use crate::agent::SeatAgent;
use crate::chance::{SearchRng, is_hit_node};
use crate::position::{Node, Position, SearchError};

/// The default rule for the asks the tree decides (`docs/MCTS_BOT_PLAN.md` 2.2, `ti4_mcts_features::kind`): the draft, the action menu,
/// the closing menu, the activation (and Thunder's Edge Warfare's free one), each movement, scoring and Imperial's point, with at least
/// two legal answers, and never a question an action card asks (the search plays on past action cards without branching).
#[must_use]
pub fn default_tree(pos: &Position, choice: &Choice) -> bool {
    ti4_mcts_features::kind::tree_kind_of(choice).is_some() && !matches!(choice.ctx.origin, Origin::ActionCard(_)) && pos.legal().len() >= 2
}

/// Where [`advance_to_tree_decision`] stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Halt {
    /// A question the tree decides ([`Position::choice`]).
    Tree,
    /// A chance node ([`Rules::chance_node`]); answer it with `Position::answer_chance`.
    Chance(Request),
    /// The game is over ([`Position::outcome`]).
    End,
}

/// What the search branches on.
#[derive(Clone, Copy)]
pub struct Rules {
    /// The questions that are tree decisions (else the delegate answers).
    pub tree: fn(&Position, &Choice) -> bool,
    /// The chance requests that are chance nodes (else answered once from the search's own stream).
    pub chance_node: fn(&Request) -> bool,
    /// A tree question with a single legal answer is answered at once (never a node).
    pub skip_single: bool,
    /// At most this many answers are given before giving up (a stalled game).
    pub max_answers: usize,
}

impl Default for Rules {
    fn default() -> Self {
        Self { tree: default_tree, chance_node: is_hit_node, skip_single: true, max_answers: 200_000 }
    }
}

/// What one call answered on the way.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Walked {
    /// Questions the delegate answered.
    pub delegated: usize,
    /// Tree questions with one legal answer, answered at once.
    pub single: usize,
    /// Chance requests answered from the search stream.
    pub sampled: usize,
}

/// Play `pos` on until a tree decision, a chance node or the end. On the way: the questions the tree does not decide go to `delegate`
/// (the rule bot inside the search), a tree question with one answer is taken, and a chance request that is no node is answered with
/// faces from `rng`. In [`crate::Mode::Play`] no chance request is seen (the game's streams answer them); a search uses a
/// `Position::fork_search` copy.
///
/// # Errors
/// A delegate error, a failed position, or more than `rules.max_answers` answers.
pub fn advance_to_tree_decision(pos: &mut Position, delegate: &mut dyn SeatAgent, rng: &mut SearchRng, rules: &Rules) -> Result<(Halt, Walked), SearchError> {
    let mut walked = Walked::default();
    for _ in 0..rules.max_answers {
        match pos.node() {
            Node::End => return Ok((Halt::End, walked)),
            Node::Failed(why) => return Err(SearchError::Failed(why.to_owned())),
            Node::Chance(req) => {
                if (rules.chance_node)(&req) {
                    return Ok((Halt::Chance(req), walked));
                }
                let faces = rng.sample(&req);
                pos.answer_chance(faces)?;
                walked.sampled += 1;
            }
            Node::Decide(choice) => {
                if (rules.tree)(pos, choice) {
                    return Ok((Halt::Tree, walked));
                }
                if rules.skip_single && ti4_mcts_features::kind::tree_kind_of(choice).is_some() && pos.legal().len() == 1 {
                    pos.answer_index(0)?;
                    walked.single += 1;
                    continue;
                }
                let choice = choice.clone();
                let decision = delegate.decide(pos, &choice).map_err(SearchError::Failed)?;
                pos.apply(decision)?;
                walked.delegated += 1;
            }
        }
    }
    Err(SearchError::Failed(format!("no tree decision within {} answers", rules.max_answers)))
}

/// Play `pos` to the end with `delegate` answering every question and `rng` every chance request (a rollout). Returns the answers
/// given.
///
/// # Errors
/// As [`advance_to_tree_decision`].
pub fn rollout(pos: &mut Position, delegate: &mut dyn SeatAgent, rng: &mut SearchRng, max_answers: usize) -> Result<usize, SearchError> {
    for n in 0..max_answers {
        match pos.node() {
            Node::End => return Ok(n),
            Node::Failed(why) => return Err(SearchError::Failed(why.to_owned())),
            Node::Chance(req) => {
                let faces = rng.sample(&req);
                pos.answer_chance(faces)?;
            }
            Node::Decide(choice) => {
                let choice = choice.clone();
                let decision = delegate.decide(pos, &choice).map_err(SearchError::Failed)?;
                pos.apply(decision)?;
            }
        }
    }
    Err(SearchError::Failed(format!("not over within {max_answers} answers")))
}
