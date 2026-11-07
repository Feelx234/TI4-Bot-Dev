//! The search tree: an arena of nodes, each holding (a cache of) its position.
//!
//! * [`Body::Decision`]: a tree question (`ti4_engine_search::advance::default_tree` with 2+ candidates after the filter): its actor, the
//!   question, the candidates (option indexes) and their picks, the evaluator's logits and value, one child slot per candidate.
//! * [`Body::Chance`]: a combat roll group with a threshold: one child slot per hit-count class (classes under `chance_min_p` merged into
//!   a neighbour), with the class probabilities.
//! * [`Body::Terminal`]: the game is over: the exact value.
//!
//! Statistics: `n` (visits) and `w` (the sum of the backed-up **absolute** value vectors): Q of a node for seat `s` is `w[s] / n`
//! (max^n: the actor of the parent reads its own component). A decision node's own evaluation counts as its first visit, so
//! `n = 1 + sum(children n)` for a decision node, `n = sum(children n)` for a chance node (it is created on the way to its first child),
//! until a reuse or a re-derivation mismatch drops a subtree ([`Tree::check_invariants`] tells).
//!
//! Positions are a cache: every node of a fresh tree has one (trimmed ledger, lane C7). A reused tree keeps the statistics and drops the
//! positions; a node's position is re-derived from its parent's when a simulation first passes it (the edge replayed with the same
//! node-keyed seeds), and a node whose re-derived halt is not the stored one (another question, another request) is replaced by a fresh
//! node.

use ti4_chance::{Faces, Request};
use ti4_engine_core::choice::{Choice, Pick};
use ti4_engine_search::Position;
use ti4_mcts_features::kind::TreeKind;

use crate::canon::MoveKey;
use crate::value::{SEATS, Value};

/// A node's index in the arena.
pub type NodeId = u32;

/// A tree question.
#[derive(Debug, Clone)]
pub struct Dec {
    pub actor: u8,
    pub choice: Choice,
    pub kind: TreeKind,
    /// The candidates (option indexes of `choice`) and their picks.
    pub cands: Vec<usize>,
    pub picks: Vec<Pick>,
    /// The evaluator's logits, one per candidate.
    pub logits: Vec<f32>,
    /// The evaluator's value (absolute).
    pub v_hat: Value,
    pub children: Vec<Option<NodeId>>,
    /// Candidates whose expansion failed (never selected again).
    pub dead: Vec<bool>,
    /// The ship moved before this `Move` ask in its movement step (the canonical-order constraint the candidates obey).
    pub last_move: Option<(u8, MoveKey)>,
}

/// A chance node: the hit-count classes of a roll group.
#[derive(Debug, Clone)]
pub struct Ch {
    pub req: Request,
    /// Representative faces and probability per class.
    pub faces: Vec<Faces>,
    pub p: Vec<f64>,
    /// Hit count -> class (a merged small class maps to its neighbour).
    pub class_of_hits: Vec<usize>,
    pub children: Vec<Option<NodeId>>,
}

#[derive(Debug, Clone)]
pub enum Body {
    Decision(Box<Dec>),
    Chance(Box<Ch>),
    Terminal(Value),
}

/// One node.
pub struct Node {
    pub n: u32,
    pub w: [f64; SEATS],
    /// The position at the node (a cache; `None` after a reuse until re-derived, always `None` for a terminal).
    pub pos: Option<Position>,
    /// The node's key: seeds its segment's delegate and stream, and its evaluation (`mix(parent key, slot)`).
    pub key: u64,
    /// The position's decision count (the real game's numbering), for matching a reused node.
    pub decisions: usize,
    pub body: Body,
}

impl Node {
    /// Q of this node for `seat` (`None` unvisited).
    #[must_use]
    pub fn q(&self, seat: usize) -> Option<f32> {
        (self.n > 0).then(|| (self.w[seat] / f64::from(self.n)) as f32)
    }

    /// The child slots.
    #[must_use]
    pub fn children(&self) -> &[Option<NodeId>] {
        match &self.body {
            Body::Decision(d) => &d.children,
            Body::Chance(c) => &c.children,
            Body::Terminal(_) => &[],
        }
    }

    pub(crate) fn children_mut(&mut self) -> &mut [Option<NodeId>] {
        match &mut self.body {
            Body::Decision(d) => &mut d.children,
            Body::Chance(c) => &mut c.children,
            Body::Terminal(_) => &mut [],
        }
    }

    #[must_use]
    pub fn dec(&self) -> Option<&Dec> {
        match &self.body {
            Body::Decision(d) => Some(d),
            _ => None,
        }
    }
}

/// A key for a child slot: `mix(parent, slot, tag)` (tag 1 a decision edge, 2 a chance class).
#[must_use]
pub const fn child_key(parent: u64, slot: usize, tag: u64) -> u64 {
    let mut h = parent ^ 0x9e37_79b9_7f4a_7c15u64.wrapping_mul(slot as u64 + 1) ^ tag.wrapping_mul(0xff51_afd7_ed55_8ccd);
    h ^= h >> 33;
    h = h.wrapping_mul(0xff51_afd7_ed55_8ccd);
    h ^= h >> 33;
    h = h.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
    h ^ (h >> 33)
}

/// The arena.
#[derive(Default)]
pub struct Tree {
    pub nodes: Vec<Node>,
    pub root: NodeId,
}

impl Tree {
    #[must_use]
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    #[must_use]
    pub fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id as usize]
    }

    pub(crate) fn node_mut(&mut self, id: NodeId) -> &mut Node {
        &mut self.nodes[id as usize]
    }

    pub(crate) fn push(&mut self, node: Node) -> NodeId {
        let id = NodeId::try_from(self.nodes.len()).unwrap_or(NodeId::MAX);
        self.nodes.push(node);
        id
    }

    /// The nodes reachable from the root.
    #[must_use]
    pub fn reachable(&self) -> usize {
        if self.nodes.is_empty() {
            return 0;
        }
        let mut stack = vec![self.root];
        let mut n = 0;
        while let Some(id) = stack.pop() {
            n += 1;
            stack.extend(self.node(id).children().iter().flatten());
        }
        n
    }

    /// Positions held (the memory that matters: ~2 KB + the segment's appends per position).
    #[must_use]
    pub fn positions(&self) -> usize {
        self.nodes.iter().filter(|n| n.pos.is_some()).count()
    }

    /// The subtree at `at` as a new tree (the rest dropped), every position dropped (a reuse re-derives them).
    #[must_use]
    pub fn subtree(self, at: NodeId) -> Self {
        let mut old: Vec<Option<Node>> = self.nodes.into_iter().map(Some).collect();
        let mut out = Self::default();
        let mut queue = std::collections::VecDeque::new();
        // (old id, new parent, slot)
        queue.push_back((at, None::<(NodeId, usize)>));
        while let Some((id, parent)) = queue.pop_front() {
            let Some(mut node) = old.get_mut(id as usize).and_then(Option::take) else { continue };
            node.pos = None;
            let kids: Vec<Option<NodeId>> = node.children().to_vec();
            for slot in node.children_mut() {
                *slot = None;
            }
            let new = out.push(node);
            if let Some((p, slot)) = parent {
                out.node_mut(p).children_mut()[slot] = Some(new);
            }
            for (slot, k) in kids.into_iter().enumerate() {
                if let Some(k) = k {
                    queue.push_back((k, Some((new, slot))));
                }
            }
        }
        out.root = 0;
        out
    }

    /// The visit-count invariants of a tree that saw no drop: a decision node `n = 1 + sum(children)`, a chance node `n = sum(children)`,
    /// every node visited, every child's key derived from its parent's. The first violation.
    ///
    /// # Errors
    /// A description of the first violated invariant.
    pub fn check_invariants(&self) -> Result<(), String> {
        for (i, node) in self.nodes.iter().enumerate() {
            let sum: u32 = node.children().iter().flatten().map(|c| self.node(*c).n).sum();
            match &node.body {
                Body::Decision(d) => {
                    if node.n != 1 + sum {
                        return Err(format!("decision node {i}: n {} != 1 + children {sum}", node.n));
                    }
                    if d.cands.len() != d.children.len() || d.picks.len() != d.cands.len() || d.logits.len() != d.cands.len() {
                        return Err(format!("decision node {i}: slot lengths differ"));
                    }
                    for (slot, c) in d.children.iter().enumerate() {
                        if let Some(c) = c
                            && self.node(*c).key != child_key(node.key, slot, 1)
                        {
                            return Err(format!("decision node {i}: child {c} key"));
                        }
                    }
                }
                Body::Chance(c) => {
                    if node.n != sum {
                        return Err(format!("chance node {i}: n {} != children {sum}", node.n));
                    }
                    let total: f64 = c.p.iter().sum();
                    if (total - 1.0).abs() > 1e-6 {
                        return Err(format!("chance node {i}: probabilities sum to {total}"));
                    }
                }
                Body::Terminal(_) => {}
            }
            if node.n == 0 {
                return Err(format!("node {i} never visited"));
            }
            let wsum: f64 = node.w.iter().sum();
            if (wsum - f64::from(node.n)).abs() > 1e-3 * f64::from(node.n).max(1.0) {
                return Err(format!("node {i}: the value vectors do not sum to one per visit ({wsum} over {})", node.n));
            }
        }
        Ok(())
    }
}
