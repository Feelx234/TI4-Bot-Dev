//! The search: Gumbel AlphaZero at the root (Gumbel-top-k without replacement, sequential halving, the completed-Q improved policy),
//! Gumbel or PUCT selection below it, hit-class chance nodes, max^n backups (`docs/MCTS_BOT_PLAN.md` 1.2, 1.3, 2.3, 2.6).
//!
//! A simulation descends from the root (its action fixed by the halving schedule), selects below it, and at an empty slot **expands**:
//! the parent's position forked, the edge answered (a pick, or a class's representative faces), then played on with
//! `advance_to_tree_decision` (the rule bot with its view answering the small asks, seeded by the child's key; the chance requests that
//! are no node answered from a `SearchRng` keyed alike; a tree question left with one candidate after the filter answered at once) to
//! the next tree question, hit-node request or the end. A new decision node is evaluated (priors + value) and the value backed up; a new
//! chance node is passed through in the same simulation; a terminal backs up the exact outcome.
//!
//! Deterministic: every random draw comes from streams seeded by the configuration's seed, the root's decision count and the node keys;
//! the game's streams are never read (every position is a `fork_search` copy). A wall-clock limit (`max_time`) breaks this on purpose.

use std::time::{Duration, Instant};

use ti4_engine_core::choice::Pick;
use ti4_engine_search::advance::{Halt, Rules, Walked, advance_to_tree_decision};
use ti4_engine_search::agent::RuleBotAgent;
use ti4_engine_search::chance::{SearchRng, hit_outcomes, hits};
use ti4_engine_search::{Position, SearchError};
use ti4_mcts_features::kind::{TreeKind, tree_kind_of};

use crate::canon::{MoveKey, after_pick, candidates};
use crate::eval::Evaluator;
use crate::gumbel::{completed_q, gumbel, halving_plan, improved_policy, softmax};
use crate::tree::{Body, Ch, Dec, Node, NodeId, Tree, child_key};
use crate::value::{SEATS, Value, outcome_value, to_absolute};

/// Selection below the root.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NonRoot {
    /// Gumbel MuZero's deterministic rule: `argmax improved(a) - n(a) / (1 + sum n)`.
    Gumbel,
    /// AlphaZero's PUCT: `argmax Q(a) + c * prior(a) * sqrt(1 + sum n) / (1 + n(a))` (unvisited Q: `v_mix`).
    Puct { c: f32 },
}

/// The search's knobs.
#[derive(Debug, Clone)]
pub struct Config {
    /// The search seed (the search's own streams; never the game's).
    pub seed: u64,
    /// Simulations of a full search (a recorded decision) and of a fast one (playout-cap randomisation, plan 2.6).
    pub sims_full: usize,
    pub sims_fast: usize,
    /// The fraction of decisions searched with `sims_full`.
    pub full_fraction: f64,
    /// `Move` and `Closing` decisions get half the simulations (user decision 2026-11-06).
    pub half_kinds: bool,
    /// Root candidates considered (Gumbel's `m`).
    pub max_considered: usize,
    /// `sigma`'s constants.
    pub c_visit: f32,
    pub c_scale: f32,
    /// Rescale the completed Q-values to `[0, 1]` per node (mctx's default).
    pub rescale_q: bool,
    /// The Gumbel noise's scale at the root (0: none, the argmax of the logits and Q).
    pub gumbel_scale: f32,
    pub non_root: NonRoot,
    /// Hit classes below this probability are merged into a neighbour (plan 2.3).
    pub chance_min_p: f64,
    /// Nodes per tree; a full tree stops the search early.
    pub max_nodes: usize,
    /// A reused subtree larger than this is dropped (a fresh tree instead).
    pub max_reuse_nodes: usize,
    /// Keep the subtree of the real line between a seat's decisions.
    pub reuse: bool,
    /// A wall-clock limit per search (breaks determinism).
    pub max_time: Option<Duration>,
    /// Answers per segment before the expansion gives up.
    pub max_segment: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            seed: 0x6d63_7473,
            sims_full: 32,
            sims_fast: 8,
            full_fraction: 0.25,
            half_kinds: true,
            max_considered: 8,
            c_visit: 50.0,
            c_scale: 1.0,
            rescale_q: true,
            gumbel_scale: 1.0,
            non_root: NonRoot::Gumbel,
            chance_min_p: 0.01,
            max_nodes: 2048,
            max_reuse_nodes: 1536,
            reuse: true,
            max_time: None,
            max_segment: 20_000,
        }
    }
}

impl Config {
    /// The simulations for a decision of `kind`: `full` or fast, halved for `Move` / `Closing` (at least 1).
    #[must_use]
    pub fn budget(&self, kind: TreeKind, full: bool) -> usize {
        let n = if full { self.sims_full } else { self.sims_fast };
        if self.half_kinds && matches!(kind, TreeKind::Move | TreeKind::Closing) { (n / 2).max(1) } else { n }
    }
}

/// Counters of one search (or summed over many).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Stats {
    pub searches: u64,
    /// Simulations run (each one descent + backup, or a dropped edge).
    pub sims: u64,
    /// Nodes created (expansions; a chance chain makes several per simulation).
    pub expanded: u64,
    pub evals: u64,
    /// Positions re-derived after a reuse, and of them the ones whose halt changed (replaced by a fresh node).
    pub rederived: u64,
    pub mismatched: u64,
    /// Searches that started from a reused subtree, and the nodes it brought.
    pub reused: u64,
    pub reused_nodes: u64,
    /// Expansions that failed (the edge is dropped).
    pub errors: u64,
    /// Searches stopped by the node bound or the clock.
    pub full: u64,
    pub timed_out: u64,
    /// Answers inside the segments: delegated, single, sampled chance; forced (one candidate after the filter).
    pub delegated: u64,
    pub single: u64,
    pub sampled: u64,
    pub forced: u64,
    pub chance_nodes: u64,
    pub terminals: u64,
    pub max_depth: u64,
    /// Time in the evaluator, in the engine (fork + answer + segment) and in all.
    pub eval_ns: u64,
    pub engine_ns: u64,
    pub wall_ns: u64,
}

impl Stats {
    pub fn add(&mut self, o: &Self) {
        self.searches += o.searches;
        self.sims += o.sims;
        self.expanded += o.expanded;
        self.evals += o.evals;
        self.rederived += o.rederived;
        self.mismatched += o.mismatched;
        self.reused += o.reused;
        self.reused_nodes += o.reused_nodes;
        self.errors += o.errors;
        self.full += o.full;
        self.timed_out += o.timed_out;
        self.delegated += o.delegated;
        self.single += o.single;
        self.sampled += o.sampled;
        self.forced += o.forced;
        self.chance_nodes += o.chance_nodes;
        self.terminals += o.terminals;
        self.max_depth = self.max_depth.max(o.max_depth);
        self.eval_ns += o.eval_ns;
        self.engine_ns += o.engine_ns;
        self.wall_ns += o.wall_ns;
    }

    /// Simulations per second of wall time.
    #[must_use]
    pub fn sims_per_s(&self) -> f64 {
        if self.wall_ns == 0 { 0.0 } else { self.sims as f64 / (self.wall_ns as f64 / 1e9) }
    }
}

/// What a search found at the root.
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// The chosen candidate (index into `cands`), its option index and pick.
    pub chosen: usize,
    pub option: usize,
    pub pick: Pick,
    pub actor: u8,
    pub kind: TreeKind,
    pub cands: Vec<usize>,
    /// The completed-Q improved policy over `cands` (the training target).
    pub policy: Vec<f32>,
    /// The root's prior (softmax of the logits), visits and Q (the actor's; `NaN` unvisited) per candidate.
    pub prior: Vec<f32>,
    pub visits: Vec<u32>,
    pub q: Vec<f32>,
    /// The root's value (absolute, all visits).
    pub root_value: Value,
    /// Simulations backed up in this search, simulations attempted (backed or dropped: what the budget counts), and the budget.
    pub sims: usize,
    pub attempted: usize,
    pub budget: usize,
    pub full: bool,
    pub stats: Stats,
}

/// One step of a descent.
#[derive(Clone, Copy)]
enum Edge {
    Pick(usize),
    Class(usize),
}

/// What a derived position stands at.
enum Halted {
    Decision { cands: Vec<usize>, last_move: Option<(u8, MoveKey)> },
    Chance(ti4_chance::Request),
    End,
}

/// What [`Searcher::ensure_positions`] did.
enum Ensured {
    /// Every node on the path has its position.
    Ready,
    /// A node's halt changed: a fresh node (evaluated: its value; a chance node: `None`) ends the path.
    Replaced(Option<Value>),
}

/// How a simulation ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Sim {
    Backed,
    /// An edge was dropped (failed expansion) or a node had nothing left to select.
    Dropped,
    /// The node bound is reached.
    Full,
}

/// A search over one tree.
pub struct Searcher<'a> {
    pub cfg: &'a Config,
    pub eval: &'a mut dyn Evaluator,
    pub tree: Tree,
    pub rules: Rules,
    pub stats: Stats,
}

/// The root of a fresh tree at `pos` (a search copy), with its candidates (2+), evaluated.
///
/// # Errors
/// A position that asks no tree question, or the evaluator's error.
pub fn new_root(pos: Position, cands: Vec<usize>, last_move: Option<(u8, MoveKey)>, key: u64, eval: &mut dyn Evaluator, stats: &mut Stats) -> Result<Tree, String> {
    let mut tree = Tree::default();
    let decisions = pos.decisions();
    let body = decision_body(&pos, cands, last_move, key, eval, stats)?;
    let v = match &body {
        Body::Decision(d) => d.v_hat,
        _ => return Err("the root is no decision".to_owned()),
    };
    let mut w = [0.0f64; SEATS];
    for (s, x) in v.iter().enumerate() {
        w[s] = f64::from(*x);
    }
    tree.root = tree.push(Node { n: 1, w, pos: Some(pos), key, decisions, body });
    Ok(tree)
}

fn decision_body(pos: &Position, cands: Vec<usize>, last_move: Option<(u8, MoveKey)>, key: u64, eval: &mut dyn Evaluator, stats: &mut Stats) -> Result<Body, String> {
    let choice = pos.choice().ok_or("no question")?.clone();
    let kind = tree_kind_of(&choice).ok_or("no tree kind")?;
    let picks: Vec<Pick> = cands.iter().map(|&i| choice.pick(i).ok_or_else(|| format!("option {i} has no pick"))).collect::<Result<_, _>>()?;
    let t = Instant::now();
    let e = eval.evaluate(pos, &choice, &cands, key)?;
    stats.eval_ns += u64::try_from(t.elapsed().as_nanos()).unwrap_or(u64::MAX);
    stats.evals += 1;
    if e.logits.len() != cands.len() {
        return Err(format!("the evaluator gave {} logits for {} candidates", e.logits.len(), cands.len()));
    }
    let actor = choice.actor.0;
    let v_hat = to_absolute(&e.value, usize::from(actor) % SEATS);
    let k = cands.len();
    Ok(Body::Decision(Box::new(Dec { actor, choice, kind, cands, picks, logits: e.logits, v_hat, children: vec![None; k], dead: vec![false; k], last_move })))
}

/// The hit classes of a roll group, the small ones merged into the neighbour towards the mean.
fn chance_body(req: ti4_chance::Request, min_p: f64) -> Result<Body, String> {
    let classes = hit_outcomes(&req).ok_or("a chance node without hit classes")?;
    // Hit counts of the classes (the representative's), ascending.
    let counts: Vec<usize> = classes.iter().map(|(f, _)| hits(&req, f).unwrap_or(0)).collect();
    let keep: Vec<bool> = classes.iter().map(|(_, p)| *p >= min_p).collect();
    let mut faces = Vec::new();
    let mut p = Vec::new();
    let mut kept_counts = Vec::new();
    for (i, (f, w)) in classes.iter().enumerate() {
        if keep[i] {
            faces.push(*f);
            p.push(*w);
            kept_counts.push(counts[i]);
        }
    }
    if faces.is_empty() {
        // Every class is small: keep the most likely.
        let best = (0..classes.len()).max_by(|a, b| classes[*a].1.total_cmp(&classes[*b].1)).unwrap_or(0);
        faces.push(classes[best].0);
        p.push(0.0);
        kept_counts.push(counts[best]);
    }
    let max_hits = counts.iter().copied().max().unwrap_or(0);
    let mut class_of_hits = vec![0usize; max_hits + 1];
    for (h, slot) in class_of_hits.iter_mut().enumerate() {
        // The kept class with the nearest hit count (ties: towards the middle class).
        let mid = kept_counts.len() / 2;
        *slot = (0..kept_counts.len()).min_by_key(|&j| (kept_counts[j].abs_diff(h), j.abs_diff(mid))).unwrap_or(0);
    }
    for (i, (_, w)) in classes.iter().enumerate() {
        if !keep[i] {
            p[class_of_hits[counts[i]]] += *w;
        }
    }
    let total: f64 = p.iter().sum();
    for x in &mut p {
        *x /= total;
    }
    let k = faces.len();
    Ok(Body::Chance(Box::new(Ch { req, faces, p, class_of_hits, children: vec![None; k] })))
}

impl<'a> Searcher<'a> {
    #[must_use]
    pub fn new(cfg: &'a Config, eval: &'a mut dyn Evaluator, tree: Tree) -> Self {
        let rules = Rules { max_answers: cfg.max_segment, ..Rules::default() };
        Self { cfg, eval, tree, rules, stats: Stats::default() }
    }

    /// Fork `parent`, answer the edge, play on to the next halt (a tree question with 2+ candidates, a hit-node request, the end).
    fn derive(&mut self, parent: NodeId, edge: Edge, key: u64) -> Result<(Position, Halted), SearchError> {
        let t = Instant::now();
        let result = self.derive_inner(parent, edge, key);
        self.stats.engine_ns += u64::try_from(t.elapsed().as_nanos()).unwrap_or(u64::MAX);
        result
    }

    fn derive_inner(&mut self, parent: NodeId, edge: Edge, key: u64) -> Result<(Position, Halted), SearchError> {
        let node = self.tree.node(parent);
        let pos = node.pos.as_ref().ok_or_else(|| SearchError::Failed("the parent has no position".to_owned()))?;
        let mut p = pos.fork_search();
        let mut last_move = match (&node.body, edge) {
            (Body::Decision(d), Edge::Pick(slot)) => {
                let pick = d.picks[slot];
                p.answer(pick)?;
                after_pick(&d.choice, d.choice.opts.as_slice().get(d.cands[slot]))
            }
            (Body::Chance(c), Edge::Class(slot)) => {
                p.answer_chance(c.faces[slot])?;
                None
            }
            _ => return Err(SearchError::Failed("an edge of another node kind".to_owned())),
        };
        let mut delegate = RuleBotAgent { view: true, seed: Some(key), index: None };
        let mut rng = SearchRng::at(key, &[0x7365_676d]);
        let mut walked = Walked::default();
        let halted = loop {
            let (halt, w) = advance_to_tree_decision(&mut p, &mut delegate, &mut rng, &self.rules)?;
            walked.delegated += w.delegated;
            walked.single += w.single;
            walked.sampled += w.sampled;
            match halt {
                Halt::End => break Halted::End,
                Halt::Chance(req) => break Halted::Chance(req),
                Halt::Tree => {
                    let choice = p.choice().ok_or(SearchError::NotAsking)?;
                    let lm = last_move.filter(|(seat, _)| crate::canon::is_move(choice) && *seat == choice.actor.0);
                    let cands = candidates(&p, choice, lm);
                    if cands.len() == 1 {
                        let opt = choice.opts.as_slice().get(cands[0]).copied();
                        last_move = after_pick(choice, opt.as_ref());
                        let pick = choice.pick(cands[0]).ok_or(SearchError::Illegal)?;
                        p.answer(pick)?;
                        self.stats.forced += 1;
                        continue;
                    }
                    break Halted::Decision { cands, last_move: lm };
                }
            }
        };
        self.stats.delegated += walked.delegated as u64;
        self.stats.single += walked.single as u64;
        self.stats.sampled += walked.sampled as u64;
        p.trim_ledger();
        Ok((p, halted))
    }

    /// A node for a derived position: evaluated (decision), exact (terminal) or passed through (chance: no value).
    fn make_node(&mut self, pos: Position, halted: Halted, key: u64) -> Result<(Node, Option<Value>), String> {
        let decisions = pos.decisions();
        match halted {
            Halted::End => {
                let v = outcome_value(&pos.outcome());
                self.stats.terminals += 1;
                Ok((Node { n: 0, w: [0.0; SEATS], pos: None, key, decisions, body: Body::Terminal(v) }, Some(v)))
            }
            Halted::Chance(req) => {
                self.stats.chance_nodes += 1;
                let body = chance_body(req, self.cfg.chance_min_p)?;
                Ok((Node { n: 0, w: [0.0; SEATS], pos: Some(pos), key, decisions, body }, None))
            }
            Halted::Decision { cands, last_move } => {
                let body = decision_body(&pos, cands, last_move, key, self.eval, &mut self.stats)?;
                let v = match &body {
                    Body::Decision(d) => d.v_hat,
                    _ => unreachable!(),
                };
                Ok((Node { n: 0, w: [0.0; SEATS], pos: Some(pos), key, decisions, body }, Some(v)))
            }
        }
    }

    /// Whether a re-derived halt is the node's.
    fn same_halt(node: &Node, pos: &Position, halted: &Halted) -> bool {
        match (&node.body, halted) {
            (Body::Terminal(_), Halted::End) => true,
            (Body::Chance(c), Halted::Chance(req)) => c.req == *req,
            (Body::Decision(d), Halted::Decision { cands, last_move }) => pos.choice() == Some(&d.choice) && *cands == d.cands && *last_move == d.last_move,
            _ => false,
        }
    }

    fn slot_of(&self, id: NodeId, edge: Edge) -> Option<NodeId> {
        let slot = match edge {
            Edge::Pick(s) | Edge::Class(s) => s,
        };
        self.tree.node(id).children().get(slot).copied().flatten()
    }

    fn set_slot(&mut self, id: NodeId, edge: Edge, child: Option<NodeId>) {
        let slot = match edge {
            Edge::Pick(s) | Edge::Class(s) => s,
        };
        self.tree.node_mut(id).children_mut()[slot] = child;
    }

    fn kill(&mut self, id: NodeId, edge: Edge) {
        if let (Body::Decision(d), Edge::Pick(s)) = (&mut self.tree.node_mut(id).body, edge) {
            d.dead[s] = true;
        }
        self.stats.errors += 1;
    }

    /// The selection at a decision node below the root.
    fn select_decision(&self, id: NodeId) -> Option<usize> {
        let node = self.tree.node(id);
        let d = node.dec()?;
        let actor = usize::from(d.actor) % SEATS;
        let (n, q) = self.child_stats(d, actor);
        let sum_n: u32 = n.iter().sum();
        let prior = softmax(&d.logits);
        let cq = completed_q(&prior, &n, &q, d.v_hat[actor], self.cfg.rescale_q);
        let score: Vec<f32> = match self.cfg.non_root {
            NonRoot::Gumbel => {
                let max_n = n.iter().copied().max().unwrap_or(0);
                let pi = improved_policy(&d.logits, &cq, max_n, self.cfg.c_visit, self.cfg.c_scale);
                (0..n.len()).map(|a| pi[a] - n[a] as f32 / (1.0 + sum_n as f32)).collect()
            }
            NonRoot::Puct { c } => {
                let raw = completed_q(&prior, &n, &q, d.v_hat[actor], false);
                (0..n.len()).map(|a| raw[a] + c * prior[a] * (1.0 + sum_n as f32).sqrt() / (1.0 + n[a] as f32)).collect()
            }
        };
        (0..score.len()).filter(|a| !d.dead[*a]).max_by(|a, b| score[*a].total_cmp(&score[*b]).then(b.cmp(a)))
    }

    /// Visits and the actor's Q per candidate.
    fn child_stats(&self, d: &Dec, actor: usize) -> (Vec<u32>, Vec<f32>) {
        let mut n = vec![0u32; d.children.len()];
        let mut q = vec![0.0f32; d.children.len()];
        for (a, c) in d.children.iter().enumerate() {
            if let Some(c) = c {
                let child = self.tree.node(*c);
                n[a] = child.n;
                q[a] = child.q(actor).unwrap_or(0.0);
            }
        }
        (n, q)
    }

    /// The outcome of a chance node: `argmax p(o) - n(o) / (1 + sum n)` (Stochastic MuZero's rule).
    fn select_class(&self, id: NodeId) -> usize {
        let Body::Chance(c) = &self.tree.node(id).body else { return 0 };
        let n: Vec<u32> = c.children.iter().map(|k| k.map_or(0, |k| self.tree.node(k).n)).collect();
        let total: u32 = n.iter().sum();
        if total == 0 {
            // A fresh node: a class drawn by its probability (the deterministic rule would take the likeliest class at every fresh
            // node, and a chain of fresh combat rounds would then never see a hit). Seeded by the node: deterministic.
            let u = crate::gumbel::uniform(&mut SearchRng::at(self.tree.node(id).key, &[0x6368_616e]));
            let mut acc = 0.0;
            for (o, p) in c.p.iter().enumerate() {
                acc += p;
                if u < acc {
                    return o;
                }
            }
            return c.p.len().saturating_sub(1);
        }
        (0..n.len())
            .max_by(|a, b| {
                let sa = c.p[*a] - f64::from(n[*a]) / (1.0 + f64::from(total));
                let sb = c.p[*b] - f64::from(n[*b]) / (1.0 + f64::from(total));
                sa.total_cmp(&sb).then(b.cmp(a))
            })
            .unwrap_or(0)
    }

    /// Give every node on `path` its position (a reused tree re-derives them from the root down, lazily: only when a simulation is about
    /// to expand below them). A node whose re-derived halt is not its own is replaced by a fresh node; the path is cut there.
    fn ensure_positions(&mut self, path: &mut Vec<NodeId>, edges: &mut Vec<Edge>) -> Result<Ensured, ()> {
        for i in 1..path.len() {
            let (parent, id, edge) = (path[i - 1], path[i], edges[i - 1]);
            let node = self.tree.node(id);
            if node.pos.is_some() || matches!(node.body, Body::Terminal(_)) {
                continue;
            }
            let key = node.key;
            self.stats.rederived += 1;
            let Ok((pos, halted)) = self.derive(parent, edge, key) else {
                self.set_slot(parent, edge, None);
                self.kill(parent, edge);
                return Err(());
            };
            if Self::same_halt(self.tree.node(id), &pos, &halted) {
                self.tree.node_mut(id).pos = Some(pos);
                continue;
            }
            self.stats.mismatched += 1;
            let Ok((node, v)) = self.make_node(pos, halted, key) else {
                self.set_slot(parent, edge, None);
                self.kill(parent, edge);
                return Err(());
            };
            let new = self.tree.push(node);
            self.stats.expanded += 1;
            self.set_slot(parent, edge, Some(new));
            path.truncate(i);
            edges.truncate(i - 1);
            path.push(new);
            edges.push(edge);
            return Ok(Ensured::Replaced(v));
        }
        Ok(Ensured::Ready)
    }

    /// One simulation, its first action at the root fixed (`None`: selected).
    fn simulate(&mut self, root_action: Option<usize>) -> Sim {
        let mut path = vec![self.tree.root];
        let mut edges: Vec<Edge> = Vec::new();
        let mut cur = self.tree.root;
        let mut forced = root_action;
        let value = 'descent: loop {
            let edge = match &self.tree.node(cur).body {
                Body::Terminal(v) => break *v,
                Body::Decision(d) => {
                    let a = match forced.take() {
                        Some(a) if !d.dead[a] => Some(a),
                        Some(_) => None,
                        None => self.select_decision(cur),
                    };
                    let Some(a) = a else { return Sim::Dropped };
                    Edge::Pick(a)
                }
                Body::Chance(_) => Edge::Class(self.select_class(cur)),
            };
            if let Some(child) = self.slot_of(cur, edge) {
                path.push(child);
                edges.push(edge);
                cur = child;
                continue;
            }
            // An empty slot: expand it, from the position of `cur` (re-derived along the path if a reuse dropped it).
            if self.tree.len() >= self.cfg.max_nodes {
                return Sim::Full;
            }
            if self.tree.node(cur).pos.is_none() {
                match self.ensure_positions(&mut path, &mut edges) {
                    Err(()) => return Sim::Dropped,
                    Ok(Ensured::Ready) => {}
                    Ok(Ensured::Replaced(Some(v))) => break 'descent v,
                    Ok(Ensured::Replaced(None)) => {
                        // A fresh chance node: go on below it.
                        cur = *path.last().unwrap_or(&self.tree.root);
                        continue;
                    }
                }
            }
            let (slot, tag) = match edge {
                Edge::Pick(s) => (s, 1),
                Edge::Class(s) => (s, 2),
            };
            let key = child_key(self.tree.node(cur).key, slot, tag);
            let made = self.derive(cur, edge, key).map_err(|e| e.to_string()).and_then(|(pos, halted)| self.make_node(pos, halted, key));
            match made {
                Ok((node, v)) => {
                    let id = self.tree.push(node);
                    self.stats.expanded += 1;
                    self.set_slot(cur, edge, Some(id));
                    path.push(id);
                    edges.push(edge);
                    match v {
                        Some(v) => break v,
                        None => cur = id,
                    }
                }
                Err(_) => {
                    self.kill(cur, edge);
                    return Sim::Dropped;
                }
            }
        };
        self.stats.max_depth = self.stats.max_depth.max(path.len() as u64 - 1);
        for id in path {
            let node = self.tree.node_mut(id);
            node.n += 1;
            for (s, x) in value.iter().enumerate() {
                node.w[s] += f64::from(*x);
            }
        }
        Sim::Backed
    }

    /// The root's candidates' score for the halving and the final pick: `g + logits + sigma(completed q)`.
    fn root_scores(&self, g: &[f32]) -> Vec<f32> {
        let root = self.tree.node(self.tree.root);
        let Some(d) = root.dec() else { return Vec::new() };
        let actor = usize::from(d.actor) % SEATS;
        let (n, q) = self.child_stats(d, actor);
        let prior = softmax(&d.logits);
        let cq = completed_q(&prior, &n, &q, d.v_hat[actor], self.cfg.rescale_q);
        let max_n = n.iter().copied().max().unwrap_or(0);
        (0..n.len()).map(|a| g[a] + d.logits[a] + crate::gumbel::sigma(cq[a], max_n, self.cfg.c_visit, self.cfg.c_scale)).collect()
    }

    /// Run `budget` simulations from the root (Gumbel-top-k + sequential halving) and pick the root action. `rng` draws the Gumbel
    /// noise.
    ///
    /// # Errors
    /// A root that is no decision.
    pub fn run(&mut self, budget: usize, rng: &mut SearchRng, full: bool) -> Result<SearchResult, String> {
        let started = Instant::now();
        let root = self.tree.root;
        let (k, logits) = match self.tree.node(root).dec() {
            Some(d) => (d.cands.len(), d.logits.clone()),
            None => return Err("the root is no decision".to_owned()),
        };
        let g: Vec<f32> = (0..k).map(|_| gumbel(rng) * self.cfg.gumbel_scale).collect();
        let m = self.cfg.max_considered.min(k).min(budget.max(1)).max(1);
        let mut order: Vec<usize> = (0..k).collect();
        order.sort_by(|a, b| (g[*b] + logits[*b]).total_cmp(&(g[*a] + logits[*a])).then(a.cmp(b)));
        let mut remaining: Vec<usize> = order.into_iter().take(m).collect();
        let plan = halving_plan(budget, m);
        let mut used = 0usize;
        let mut stop = false;
        let mut sims = 0usize;
        let over = |s: &Self, used: usize| used >= budget || s.cfg.max_time.is_some_and(|t| started.elapsed() >= t);
        'phases: for (phase, &(_, per)) in plan.iter().enumerate() {
            for &a in &remaining.clone() {
                for _ in 0..per {
                    if over(self, used) {
                        break 'phases;
                    }
                    used += 1;
                    match self.simulate(Some(a)) {
                        Sim::Backed => sims += 1,
                        Sim::Dropped => {}
                        Sim::Full => {
                            stop = true;
                            break 'phases;
                        }
                    }
                }
            }
            if phase + 1 < plan.len() {
                let s = self.root_scores(&g);
                remaining.sort_by(|a, b| s[*b].total_cmp(&s[*a]).then(a.cmp(b)));
                remaining.truncate(remaining.len().div_ceil(2));
            }
        }
        // The plan's leftover, round-robin over the last phase's actions.
        let mut i = 0usize;
        while !stop && !over(self, used) && !remaining.is_empty() {
            let a = remaining[i % remaining.len()];
            i += 1;
            used += 1;
            match self.simulate(Some(a)) {
                Sim::Backed => sims += 1,
                Sim::Dropped => {}
                Sim::Full => stop = true,
            }
        }
        if stop {
            self.stats.full += 1;
        } else if used < budget {
            self.stats.timed_out += 1;
        }
        let d = self.tree.node(root).dec().ok_or("the root is no decision")?.clone();
        let s = self.root_scores(&g);
        let alive: Vec<usize> = remaining.iter().copied().filter(|a| !d.dead[*a]).collect();
        let pool = if alive.is_empty() { (0..k).filter(|a| !d.dead[*a]).collect::<Vec<_>>() } else { alive };
        let chosen = pool.iter().copied().max_by(|a, b| s[*a].total_cmp(&s[*b]).then(b.cmp(a))).unwrap_or(0);
        let actor = usize::from(d.actor) % SEATS;
        let (n, q) = self.child_stats(&d, actor);
        let prior = softmax(&d.logits);
        let cq = completed_q(&prior, &n, &q, d.v_hat[actor], self.cfg.rescale_q);
        let max_n = n.iter().copied().max().unwrap_or(0);
        let policy = improved_policy(&d.logits, &cq, max_n, self.cfg.c_visit, self.cfg.c_scale);
        let rn = self.tree.node(root);
        let mut root_value = [0.0f32; SEATS];
        for (s, x) in root_value.iter_mut().enumerate() {
            *x = rn.q(s).unwrap_or(0.25);
        }
        self.stats.searches += 1;
        self.stats.sims += sims as u64;
        self.stats.wall_ns += u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX);
        Ok(SearchResult {
            chosen,
            option: d.cands[chosen],
            pick: d.picks[chosen],
            actor: d.actor,
            kind: d.kind,
            cands: d.cands.clone(),
            policy,
            prior,
            visits: n.clone(),
            q: (0..n.len()).map(|a| if n[a] > 0 { q[a] } else { f32::NAN }).collect(),
            root_value,
            sims,
            attempted: used,
            budget,
            full,
            stats: self.stats,
        })
    }
}
