//! Unit tests of the tree on hand-made trees and of the search on real positions of the MCTS setup.

use ti4_engine_core::choice::Pick;
use ti4_engine_search::advance::default_tree;
use ti4_engine_search::agent::{RuleBotAgent, SeatAgent};
use ti4_engine_search::chance::SearchRng;
use ti4_engine_search::{Node as PosNode, Position, mcts_spec, start};
use ti4_mcts_features::kind::tree_kind_of;

use crate::canon::candidates;
use crate::eval::{RuleBotPrior, Uniform};
use crate::search::{Config, Searcher, Stats, new_root};
use crate::tree::{Body, Ch, Dec, Node, Tree, child_key};
use crate::value::SEATS;

/// The MCTS game of `seed` played by the rule bot (with its view) to its `skip`-th tree question with at least `min` candidates.
pub(crate) fn tree_position(seed: u64, skip: usize, min: usize) -> Position {
    let mut pos = start(&mcts_spec(seed)).unwrap();
    let mut bot = RuleBotAgent::seeing();
    let mut seen = 0;
    while let PosNode::Decide(choice) = pos.node() {
        let choice = choice.clone();
        if default_tree(&pos, &choice) && candidates(&pos, &choice, None).len() >= min {
            if seen == skip {
                return pos;
            }
            seen += 1;
        }
        let d = bot.decide(&pos, &choice).unwrap();
        pos.apply(d).unwrap();
    }
    panic!("the game ended before tree question {skip}");
}

fn leaf(v: [f32; SEATS], key: u64) -> Node {
    Node { n: 0, w: [0.0; SEATS], pos: None, key, decisions: 0, body: Body::Terminal(v) }
}

/// A hand-made tree on a real three-candidate question: candidate 0 a win for seat 1, candidate 1 a chance node (30 %: the actor wins,
/// 70 %: seat 2 wins), candidate 2 a win for the actor. No position is needed (nothing is expanded).
fn hand_tree() -> (Tree, u8) {
    let pos = tree_position(1, 0, 3);
    let choice = pos.choice().unwrap().clone();
    let actor = choice.actor.0;
    let a = usize::from(actor);
    let cands: Vec<usize> = (0..3).collect();
    let picks: Vec<Pick> = cands.iter().map(|&i| choice.pick(i).unwrap()).collect();
    let win = |s: usize| {
        let mut v = [0.0; SEATS];
        v[s] = 1.0;
        v
    };
    let (w_actor, w_other, w_third) = (win(a), win((a + 1) % SEATS), win((a + 2) % SEATS));
    let mut t = Tree::default();
    let root_key = 7;
    let mut w0 = [0.0f64; SEATS];
    w0.iter_mut().for_each(|x| *x = 0.25);
    t.root = t.push(Node {
        n: 1,
        w: w0,
        pos: None,
        key: root_key,
        decisions: 0,
        body: Body::Decision(Box::new(Dec {
            actor,
            kind: tree_kind_of(&choice).unwrap(),
            choice,
            cands,
            picks,
            logits: vec![0.0; 3],
            v_hat: [0.25; SEATS],
            children: vec![None; 3],
            dead: vec![false; 3],
            last_move: None,
        })),
    });
    let c0 = t.push(leaf(w_other, child_key(root_key, 0, 1)));
    let ck = child_key(root_key, 1, 1);
    let req = ti4_chance::Request::new(ti4_chance::Domain::Dice, ti4_chance::Kind::Dice { n: 1, sides: 10, hits_on: Some(8) });
    let faces = ti4_engine_search::chance::hit_outcomes(&req).unwrap();
    let ch = t.push(Node {
        n: 0,
        w: [0.0; SEATS],
        pos: None,
        key: ck,
        decisions: 0,
        body: Body::Chance(Box::new(Ch { req, faces: faces.iter().map(|f| f.0).collect(), p: vec![0.7, 0.3], class_of_hits: vec![0, 1], children: vec![None; 2] })),
    });
    let miss = t.push(leaf(w_third, child_key(ck, 0, 2)));
    let hit = t.push(leaf(w_actor, child_key(ck, 1, 2)));
    let c2 = t.push(leaf(w_actor, child_key(root_key, 2, 1)));
    let set = |t: &mut Tree, id: u32, slot: usize, c: u32| t.node_mut(id).children_mut()[slot] = Some(c);
    set(&mut t, 0, 0, c0);
    set(&mut t, 0, 1, ch);
    set(&mut t, 0, 2, c2);
    set(&mut t, ch, 0, miss);
    set(&mut t, ch, 1, hit);
    // The pre-expanded children count one visit each, as if a simulation had made them (and backed up through the root).
    for (id, v) in [(c0, w_other), (hit, w_actor), (miss, w_third), (c2, w_actor)] {
        let path: Vec<u32> = if id == hit || id == miss { vec![0, ch, id] } else { vec![0, id] };
        for p in path {
            let n = t.node_mut(p);
            n.n += 1;
            for s in 0..SEATS {
                n.w[s] += f64::from(v[s]);
            }
        }
    }
    (t, actor)
}

#[test]
fn backups_on_a_hand_made_tree_are_max_n_vectors_with_exact_counts() {
    let (tree, actor) = hand_tree();
    tree.check_invariants().unwrap();
    let cfg = Config { gumbel_scale: 0.0, ..Config::default() };
    let mut eval = Uniform;
    let mut s = Searcher::new(&cfg, &mut eval, tree);
    let mut rng = SearchRng::new(1);
    let r = s.run(40, &mut rng, true).unwrap();
    s.tree.check_invariants().unwrap();
    assert_eq!(r.attempted, 40, "the budget is spent exactly");
    assert_eq!(r.sims, 40, "every simulation reached a terminal");
    let root = s.tree.node(0);
    assert_eq!(root.n, 1 + 4 + 40);
    let a = usize::from(actor);
    // Candidate 2 always wins for the actor: the search picks it, and its Q (for the actor) is exactly 1.
    assert_eq!(r.chosen, 2, "{r:?}");
    assert!((r.q[2] - 1.0).abs() < 1e-6);
    assert!(r.q[0].abs() < 1e-6, "candidate 0 never wins for the actor");
    // The chance node's visits follow its distribution (Stochastic MuZero's selection): within one of 30 % / 70 %.
    let ch = s.tree.node(2);
    let (miss, hit) = (s.tree.node(ch.children()[0].unwrap()).n, s.tree.node(ch.children()[1].unwrap()).n);
    let total = f64::from(miss + hit);
    assert!((f64::from(hit) - 0.3 * total).abs() <= 1.5, "hits {hit} of {total}");
    // max^n: the root's w is the sum of the backed-up vectors; the actor's component is its Q.
    let wsum: f64 = root.w.iter().sum();
    assert!((wsum - f64::from(root.n)).abs() < 1e-6);
    assert!(root.q(a).unwrap() > 0.5, "the actor's root value leans to its winning line");
    // The improved policy prefers the sure win.
    assert!(r.policy[2] > r.policy[1] && r.policy[1] > r.policy[0], "{:?}", r.policy);
}

#[test]
fn sequential_halving_spends_the_budget_and_keeps_the_best() {
    for (budget, max_considered) in [(8usize, 8usize), (16, 3), (5, 2), (2, 8), (1, 8), (33, 8)] {
        let (tree, _) = hand_tree();
        let cfg = Config { max_considered, ..Config::default() };
        let mut eval = Uniform;
        let mut s = Searcher::new(&cfg, &mut eval, tree);
        let mut rng = SearchRng::new(budget as u64);
        let r = s.run(budget, &mut rng, false).unwrap();
        assert_eq!(r.attempted, budget, "budget {budget}, m {max_considered}");
        assert_eq!(r.visits.iter().sum::<u32>() as usize, 4 + budget, "the visits of the root's children");
        s.tree.check_invariants().unwrap();
        if budget >= 3 * 2 {
            // With every candidate considered and enough simulations the sure win survives the halving.
            assert_eq!(r.chosen, 2, "budget {budget}, m {max_considered}: {r:?}");
        }
    }
}

#[test]
fn a_search_from_a_real_position_keeps_its_invariants_and_leaves_the_game_alone() {
    for (seed, skip) in [(1u64, 3usize), (2, 20), (3, 60)] {
        let pos = tree_position(seed, skip, 2);
        let (hash, streams) = (pos.state_hash(), pos.stream_positions());
        let choice = pos.choice().unwrap().clone();
        let cands = candidates(&pos, &choice, None);
        let cfg = Config::default();
        let mut eval = RuleBotPrior::default();
        let mut stats = Stats::default();
        let mut root = pos.fork_search();
        root.trim_ledger();
        let tree = new_root(root, cands.clone(), None, 11, &mut eval, &mut stats).unwrap();
        let mut s = Searcher::new(&cfg, &mut eval, tree);
        let r = s.run(24, &mut SearchRng::new(5), true).unwrap();
        s.tree.check_invariants().unwrap();
        assert_eq!(r.attempted, 24);
        assert!(r.sims > 0);
        assert!(cands.contains(&r.option));
        assert!((r.policy.iter().sum::<f32>() - 1.0).abs() < 1e-4);
        assert_eq!(s.tree.positions() + s.tree.nodes.iter().filter(|n| matches!(n.body, Body::Terminal(_))).count(), s.tree.len(), "a fresh tree holds a position at every non-terminal node");
        assert_eq!(pos.state_hash(), hash, "the search changed the game");
        assert_eq!(pos.stream_positions(), streams, "the search moved the game's streams");
    }
}

#[test]
fn a_reused_subtree_rederives_its_positions_and_keeps_its_statistics() {
    let pos = tree_position(2, 5, 2);
    let choice = pos.choice().unwrap().clone();
    let cands = candidates(&pos, &choice, None);
    let cfg = Config::default();
    let mut eval = Uniform;
    let mut stats = Stats::default();
    let tree = new_root(pos.fork_search(), cands, None, 3, &mut eval, &mut stats).unwrap();
    let mut s = Searcher::new(&cfg, &mut eval, tree);
    let first = s.run(16, &mut SearchRng::new(2), true).unwrap();
    // Re-root at the same question (as the agent does when nothing happened): every position dropped, the root's given back.
    let n_before = s.tree.node(0).n;
    let mut t = std::mem::take(&mut s.tree).subtree(0);
    assert_eq!(t.positions(), 0);
    let reach = t.reachable();
    assert_eq!(reach, t.len(), "the subtree holds only reachable nodes");
    t.node_mut(0).pos = Some(pos.fork_search());
    let mut s2 = Searcher::new(&cfg, &mut eval, t);
    let second = s2.run(16, &mut SearchRng::new(2), true).unwrap();
    assert_eq!(s2.tree.node(0).n, n_before + second.sims as u32, "the statistics carry over");
    assert!(s2.stats.rederived > 0, "the reused nodes' positions are re-derived");
    assert_eq!(s2.stats.mismatched, 0, "the same position re-derives the same halts");
    s2.tree.check_invariants().unwrap();
    assert!(first.sims > 0 && second.sims > 0);
}

#[test]
fn the_node_bound_stops_the_search() {
    let pos = tree_position(3, 10, 3);
    let choice = pos.choice().unwrap().clone();
    let cands = candidates(&pos, &choice, None);
    let cfg = Config { max_nodes: 12, ..Config::default() };
    let mut eval = Uniform;
    let mut stats = Stats::default();
    let tree = new_root(pos.fork_search(), cands, None, 5, &mut eval, &mut stats).unwrap();
    let mut s = Searcher::new(&cfg, &mut eval, tree);
    let r = s.run(64, &mut SearchRng::new(3), true).unwrap();
    assert!(s.tree.len() <= 12, "{} nodes", s.tree.len());
    assert_eq!(r.stats.full, 1, "the search reports the full tree");
    assert!(r.attempted < 64);
    s.tree.check_invariants().unwrap();
}

#[test]
fn the_budget_follows_the_playout_cap_and_halves_moves_and_closings() {
    use ti4_mcts_features::kind::TreeKind;
    let cfg = Config::default();
    assert_eq!(cfg.budget(TreeKind::Activate, true), 32);
    assert_eq!(cfg.budget(TreeKind::Draft, false), 8);
    assert_eq!(cfg.budget(TreeKind::Move, true), 16);
    assert_eq!(cfg.budget(TreeKind::Closing, false), 4);
    let tiny = Config { sims_fast: 1, ..Config::default() };
    assert_eq!(tiny.budget(TreeKind::Move, false), 1, "never zero");
}

/// The candidates: an activation keeps the hard filter's options; a movement after a ship keeps the ships at or after its key in the
/// canonical order, and `DoneMoving`.
#[test]
fn candidates_apply_the_activation_filter_and_the_canonical_move_order() {
    use crate::canon::{is_move, move_key};
    use ti4_mcts_features::filter;
    let (mut activations, mut moves) = (0, 0);
    for seed in [1u64, 2] {
        let mut pos = start(&mcts_spec(seed)).unwrap();
        let mut bot = RuleBotAgent::seeing();
        while let PosNode::Decide(choice) = pos.node() {
            let choice = choice.clone();
            if filter::is_activation(&choice.ask) {
                let seen = pos.seen().unwrap();
                assert_eq!(candidates(&pos, &choice, None), filter::options(&seen, &choice));
                activations += 1;
            }
            if is_move(&choice) {
                let keys: Vec<_> = choice.opts.as_slice().iter().map(move_key).collect();
                let mut ships: Vec<_> = keys.iter().flatten().copied().collect();
                ships.sort();
                if ships.len() >= 3 {
                    let last = ships[ships.len() / 2];
                    let c = candidates(&pos, &choice, Some((choice.actor.0, last)));
                    for (i, k) in keys.iter().enumerate() {
                        assert_eq!(c.contains(&i), k.is_none_or(|k| k >= last), "option {i} key {k:?} vs {last:?}");
                    }
                    let other = candidates(&pos, &choice, Some((choice.actor.0 ^ 1, last)));
                    assert_eq!(other.len(), choice.opts.len(), "another seat's move constrains nothing");
                    moves += 1;
                }
            }
            let d = bot.decide(&pos, &choice).unwrap();
            pos.apply(d).unwrap();
        }
    }
    assert!(activations > 50 && moves > 5, "{activations} activations, {moves} moves");
}

/// The value's perspective on real positions: at every scoring question of two rule-bot games, a search with flat priors and the VP
/// share as the value (no Gumbel noise) picks an answer that gains the actor the most points right away (scoring beats not scoring).
#[test]
fn a_search_with_a_points_value_scores_when_it_can() {
    use crate::eval::{Evaluation, FnEvaluator};
    use crate::value::{to_relative, vp_value};
    use ti4_mcts_features::kind::TreeKind;
    let mut checked = 0;
    for seed in [1u64, 2] {
        let mut pos = start(&mcts_spec(seed)).unwrap();
        let mut bot = RuleBotAgent::seeing();
        while let PosNode::Decide(choice) = pos.node() {
            let choice = choice.clone();
            if default_tree(&pos, &choice) && tree_kind_of(&choice) == Some(TreeKind::Score) {
                let cands = candidates(&pos, &choice, None);
                let me = usize::from(choice.actor.0);
                let gain: Vec<i16> = cands
                    .iter()
                    .map(|&c| {
                        let mut f = pos.fork_search();
                        f.answer_index(c).unwrap();
                        f.vp()[me] - pos.vp()[me]
                    })
                    .collect();
                if cands.len() >= 2 && gain.iter().any(|g| *g != gain[0]) {
                    let mut eval = FnEvaluator {
                        name: "vp".into(),
                        f: Box::new(|p, c, k, _| Ok(Evaluation { logits: vec![0.0; k.len()], value: to_relative(&vp_value(&p.vp(), 1.0), usize::from(c.actor.0)) })),
                    };
                    let cfg = Config { gumbel_scale: 0.0, ..Config::default() };
                    let mut stats = Stats::default();
                    let tree = new_root(pos.fork_search(), cands.clone(), None, 9, &mut eval, &mut stats).unwrap();
                    let mut s = Searcher::new(&cfg, &mut eval, tree);
                    let r = s.run(16, &mut SearchRng::new(1), true).unwrap();
                    let best = *gain.iter().max().unwrap();
                    assert_eq!(gain[r.chosen], best, "seed {seed}: gains {gain:?}, chose {} (q {:?})", r.chosen, r.q);
                    checked += 1;
                }
            }
            let d = bot.decide(&pos, &choice).unwrap();
            pos.apply(d).unwrap();
        }
    }
    assert!(checked >= 3, "{checked} scoring questions checked");
}
