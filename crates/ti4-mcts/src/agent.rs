//! [`MctsAgent`]: a seat that searches its tree decisions (`docs/MCTS_BOT_PLAN.md` 4.4) and lets the rule bot answer the rest.
//!
//! Per question of its seat:
//! * not a tree question (`default_tree`: a tree kind with 2+ legal answers, not an action card's): the rule bot with its view (the
//!   benchmark seat's answer: production, payments, the small asks);
//! * a tree question with one candidate after the activation filter and the canonical move order: that candidate;
//! * else a search: playout cap (`full_fraction` of the decisions with `sims_full`, the rest `sims_fast`; `Move` / `Closing` half),
//!   the root a `fork_search` copy of the game, determinised by the [`Determinize`] hook, the tree reused when the real line of play
//!   reached a node of the last tree.
//!
//! Tree reuse follows the game through [`SeatAgent::observe`] (the driver's `observe = true`): a decision equal to the cursor's question
//! (same question, same decision count) moves the cursor along the pick's edge, a roll of the cursor's chance request along its hit class;
//! any other decision or roll is one the tree played on past (delegated, single, sampled) and leaves the cursor. At the next own decision
//! the cursor must be that question with the same candidates; else the tree is dropped. Without observation the tree is never reused.

use std::cell::RefCell;
use std::rc::Rc;

use ti4_engine_core::choice::Choice;
use ti4_engine_search::Position;
use ti4_engine_search::advance::default_tree;
use ti4_engine_search::agent::{Answer, Applied, Decision, RuleBotAgent, SeatAgent};
use ti4_engine_search::chance::{SearchRng, hits};
use ti4_mcts_features::kind::tree_kind_of;

use crate::canon::{MoveKey, after_pick, candidates, is_move};
use crate::eval::Evaluator;
use crate::search::{Config, SearchResult, Searcher, Stats, new_root};
use crate::tree::{Body, NodeId, Tree, child_key};
use crate::value::SEATS;

/// The determinisation hook at the search root (lane C8): reshuffle what the actor cannot see (the unseen decks) in the root copy, from
/// the search's own stream. Called once per search, on the root's `fork_search` copy, before anything is evaluated.
pub trait Determinize {
    fn determinize(&mut self, pos: &mut Position, rng: &mut SearchRng);
}

/// Perfect information (v1 until C8): the root copy as it is.
#[derive(Debug, Clone, Copy, Default)]
pub struct PerfectInformation;

impl Determinize for PerfectInformation {
    fn determinize(&mut self, _pos: &mut Position, _rng: &mut SearchRng) {}
}

/// Counts of the agent's questions.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Counts {
    /// Answered by the rule bot (not tree questions).
    pub delegated: u64,
    /// Tree questions with one candidate.
    pub forced: u64,
    /// Searched (full ones among them).
    pub searched: u64,
    pub full: u64,
}

/// The search seat.
pub struct MctsAgent {
    pub cfg: Config,
    pub eval: Box<dyn Evaluator>,
    pub determinizer: Box<dyn Determinize>,
    /// The rule bot for the questions the agent does not search.
    pub fallback: RuleBotAgent,
    /// Search counters summed over the game.
    pub stats: Stats,
    pub counts: Counts,
    /// Keep every search's result (`records`): the root targets of the full searches are the training data of lane L8.
    pub record: bool,
    pub records: Vec<SearchResult>,
    /// The last search's result.
    pub last: Option<SearchResult>,
    tree: Option<Tree>,
    cursor: Option<NodeId>,
    last_move: Option<(u8, MoveKey)>,
}

impl MctsAgent {
    #[must_use]
    pub fn new(cfg: Config, eval: Box<dyn Evaluator>) -> Self {
        Self {
            cfg,
            eval,
            determinizer: Box::new(PerfectInformation),
            fallback: RuleBotAgent::seeing(),
            stats: Stats::default(),
            counts: Counts::default(),
            record: false,
            records: Vec::new(),
            last: None,
            tree: None,
            cursor: None,
            last_move: None,
        }
    }

    /// The same with a determinisation hook.
    #[must_use]
    pub fn with_determinizer(mut self, d: Box<dyn Determinize>) -> Self {
        self.determinizer = d;
        self
    }

    /// The tree kept for reuse (nodes, positions held), if any.
    #[must_use]
    pub fn kept_tree(&self) -> Option<(usize, usize)> {
        self.tree.as_ref().map(|t| (t.len(), t.positions()))
    }

    /// Drop the kept tree.
    pub fn clear_tree(&mut self) {
        self.tree = None;
        self.cursor = None;
    }

    /// The reusable subtree for this question, if the cursor stands at it.
    fn reusable(&mut self, pos: &Position, choice: &Choice, cands: &[usize], lm: Option<(u8, MoveKey)>) -> Option<Tree> {
        let (tree, cur) = (self.tree.take()?, self.cursor.take()?);
        if !self.cfg.reuse {
            return None;
        }
        let node = tree.node(cur);
        let d = node.dec()?;
        if d.choice != *choice || node.decisions != pos.decisions() || d.cands != cands || d.last_move != lm {
            return None;
        }
        let sub = tree.subtree(cur);
        (sub.len() <= self.cfg.max_reuse_nodes).then_some(sub)
    }

    /// Search the tree question `choice` at `pos` (2+ candidates).
    ///
    /// # Errors
    /// The evaluator's error at the root, or a root that asks nothing.
    pub fn search(&mut self, pos: &Position, choice: &Choice, cands: Vec<usize>, lm: Option<(u8, MoveKey)>) -> Result<SearchResult, String> {
        let actor = choice.actor.0;
        let kind = tree_kind_of(choice).ok_or("no tree kind")?;
        let decisions = pos.decisions() as u64;
        let mut rng = SearchRng::at(self.cfg.seed, &[decisions, u64::from(actor), 0x726f_6f74]);
        let full = crate::gumbel::uniform(&mut rng) < self.cfg.full_fraction;
        let budget = self.cfg.budget(kind, full);
        let mut root_pos = pos.fork_search();
        self.determinizer.determinize(&mut root_pos, &mut rng);
        root_pos.trim_ledger();
        let mut stats = Stats::default();
        let tree = match self.reusable(pos, choice, &cands, lm) {
            Some(mut t) => {
                stats.reused += 1;
                stats.reused_nodes += t.len() as u64;
                let root = t.root;
                t.node_mut(root).pos = Some(root_pos);
                t
            }
            None => {
                let key = child_key(self.cfg.seed ^ decisions.wrapping_mul(0x2545_f491_4f6c_dd1d), usize::from(actor), 3);
                new_root(root_pos, cands, lm, key, self.eval.as_mut(), &mut stats)?
            }
        };
        let mut s = Searcher::new(&self.cfg, self.eval.as_mut(), tree);
        s.stats = stats;
        let result = s.run(budget, &mut rng, full)?;
        let tree = s.tree;
        self.stats.add(&result.stats);
        self.counts.searched += 1;
        self.counts.full += u64::from(full);
        if self.cfg.reuse {
            self.cursor = Some(tree.root);
            self.tree = Some(tree);
        }
        if self.record {
            self.records.push(result.clone());
        }
        self.last = Some(result.clone());
        Ok(result)
    }
}

impl SeatAgent for MctsAgent {
    fn decide(&mut self, pos: &Position, choice: &Choice) -> Result<Decision, String> {
        if pos.table().seats.len() != SEATS {
            return Err(format!("the MCTS agent plays {SEATS}-seat tables, this one has {}", pos.table().seats.len()));
        }
        if !default_tree(pos, choice) {
            self.counts.delegated += 1;
            return self.fallback.decide(pos, choice);
        }
        let actor = choice.actor.0;
        let lm = self.last_move.filter(|(s, _)| is_move(choice) && *s == actor);
        let cands = candidates(pos, choice, lm);
        let option = if cands.len() == 1 {
            self.counts.forced += 1;
            self.tree = None;
            self.cursor = None;
            cands[0]
        } else {
            self.search(pos, choice, cands, lm)?.option
        };
        let pick = choice.pick(option).ok_or("the chosen option has no pick")?;
        self.last_move = after_pick(choice, choice.opts.as_slice().get(option));
        Ok(Answer::Pick(pick).into())
    }

    fn observe(&mut self, pos: &Position, applied: &Applied) {
        match applied {
            Applied::Decision { seat, choice, .. } if tree_kind_of(choice).is_some() && self.last_move.is_some_and(|(s, _)| s != *seat) => {
                self.last_move = None;
            }
            _ => {}
        }
        let (Some(tree), Some(cur)) = (&self.tree, self.cursor) else { return };
        let node = tree.node(cur);
        let next = match (&node.body, applied) {
            (Body::Decision(d), Applied::Decision { choice, pick, .. }) if d.choice == **choice && node.decisions + 1 == pos.decisions() => {
                Some(pick.and_then(|p| d.picks.iter().position(|x| *x == p)).and_then(|slot| d.children[slot]))
            }
            (Body::Chance(c), Applied::Chance { request, faces }) if c.req == *request => {
                Some(hits(request, faces).and_then(|h| c.class_of_hits.get(h).copied()).and_then(|slot| c.children[slot]))
            }
            _ => None,
        };
        if let Some(next) = next {
            self.cursor = next;
            if next.is_none() {
                self.tree = None;
            }
        }
    }
}

/// A seat agent shared with its caller (the driver owns a `Box<dyn SeatAgent>`; the caller keeps a handle to read the counters after
/// the game).
pub struct Shared<A>(pub Rc<RefCell<A>>);

impl<A> Shared<A> {
    /// The seat for the driver and the caller's handle.
    #[must_use]
    pub fn new(agent: A) -> (Self, Rc<RefCell<A>>) {
        let rc = Rc::new(RefCell::new(agent));
        (Self(Rc::clone(&rc)), rc)
    }
}

impl<A: SeatAgent> SeatAgent for Shared<A> {
    fn decide(&mut self, pos: &Position, choice: &Choice) -> Result<Decision, String> {
        self.0.borrow_mut().decide(pos, choice)
    }

    fn observe(&mut self, pos: &Position, applied: &Applied) {
        self.0.borrow_mut().observe(pos, applied);
    }
}
