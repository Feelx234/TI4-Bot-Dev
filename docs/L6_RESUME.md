# Lane L6 (MCTS core): resume

Branch `core-mcts-core-1`, cut from `core-search-api-1` (C6, `8a32981b2`). Not pushed, not deployed, live checkout untouched. Plan:
`git show core-theory-mcts:docs/MCTS_BOT_PLAN.md` (1.2, 1.3, 2, 4.2, 4.4); decisions in memory `project_mcts_bot_decisions`; the search API
`docs/C6_RESUME.md`. No heavy gate run (by the brief): targeted tests, the micro-bench and an A/B smoke only.

## 1. What there is

New crate `crates/ti4-mcts` (dev tool: not in the production graph, not in the wasm; deps: `ti4-engine-search`, `ti4-mcts-features`, the
core, `ti4-chance`, `serde_json`, `ti4-perf-alloc` for the bench). `check_layers.py mcts` (new mode: the `search` rules, and no play-mode
resolver at all), `check_production_graph.sh` and `clippy_core.sh` know it.

| module | what |
|---|---|
| `search` | `Config`, `Searcher`, `new_root`, `SearchResult`, `Stats`, `NonRoot::{Gumbel, Puct{c}}` |
| `tree` | the arena (`Tree`, `Node { n, w[4], pos, key, decisions, body }`, `Body::{Decision, Chance, Terminal}`), `subtree` (re-rooting), `check_invariants` |
| `gumbel` | Gumbel noise, `completed_q` (`v_mix`, optional min-max rescale), `sigma`, `improved_policy`, `halving_plan` (pure, tested alone) |
| `canon` | the candidates of a tree question: `filter::options` for activations (hard filter, pass-through), the canonical move order (key = capacity first, origin, kind, damaged: the M1 census's), all options otherwise |
| `eval` | `Evaluator { evaluate(pos, choice, cands, key) -> Evaluation { logits per candidate, value actor-relative } }`; stubs `Uniform`, `RuleBotPrior { weight, value: Uniform \| VpShare{temp} }`, `Rollout { horizon, temp, prior }`; the net hook `FnEvaluator` (a closure) |
| `agent` | `MctsAgent: SeatAgent` (search seat), `Determinize` (the C8 hook) + `PerfectInformation`, `Shared<A>` (a seat the caller keeps a handle to) |
| `value` | the 4-seat value vector: absolute in the tree, actor-relative at the evaluator (`to_absolute` / `to_relative`), `outcome_value`, `vp_value` |
| `play` | `play_game(seed, mcts_seats, cfg, EvalSpec, Limits)`: the MCTS setup, rule bot (with its view) at the other seats, the core driver with observation |
| bins | `mcts_bench` (simulations/s per evaluator), `mcts_ab` (A/B smoke vs the rule bot, paired per seed) |

### The search (one decision)

1. `MctsAgent::decide`: not `default_tree` (tree kind, 2+ legal answers, not an action card's ask) -> the rule bot with its view (production,
   payments, the small asks). Candidates (`canon::candidates`, with the ship last moved in this movement step); one candidate -> played.
2. Playout cap: `uniform(rng) < full_fraction` (0.25) -> `sims_full` (32) else `sims_fast` (8); `Move` / `Closing` half (user decision).
   `rng = SearchRng::at(cfg.seed, [decisions, actor, tag])`: the search's own stream (no game stream is ever read: every position is a
   `fork_search` copy, `check_layers.py mcts`).
3. Root: `pos.fork_search()`, then `determinizer.determinize(&mut root, &mut rng)` (**the C8 hook**: no-op `PerfectInformation` until C8
   lands), `trim_ledger()` (C7). A reused subtree (below) or a fresh root (evaluated: its value is its first visit).
4. Gumbel root (Danihelka et al. 2022): `g ~ Gumbel` (scale `gumbel_scale`), the top `m = min(max_considered=8, K, budget)` by
   `g + logits` (Gumbel-top-k = sampling without replacement), `halving_plan(n, m)`: `ceil(log2 m)` phases, each remaining candidate
   `max(1, n / (phases * left))` simulations, then the better half by `g + logits + sigma(completed q)`; the plan's leftover round-robin
   over the last survivors; the search stops at exactly `n` attempted simulations. Pick: the argmax of the same score over the survivors.
   Training target (`SearchResult::policy`): `softmax(logits + sigma(completed q))` over all candidates.
5. Below the root: `NonRoot::Gumbel` (default; `argmax improved(a) - n(a) / (1 + sum n)`) or `NonRoot::Puct { c }` (`Q + c prior
   sqrt(1 + sum n) / (1 + n)`, unvisited Q = `v_mix`). Chance nodes: a fresh node draws its class by probability (seeded by the node key),
   afterwards Stochastic MuZero's `argmax p(o) - n(o) / (1 + sum n)`. (The pure deterministic rule took the likeliest class at every fresh
   node, so chains of fresh combat rounds where both sides miss grew to depth 156 in the bench; with the draw at fresh nodes: depth 48.)
6. Expansion of an empty slot: fork the parent's position, answer the edge (the candidate's `Pick`, or the class's representative faces),
   `advance_to_tree_decision` with the rule bot with its view as delegate (`seed = child key`) and `SearchRng::at(child key)` for the
   chance requests that are no node; a tree question with one candidate after the filter is answered on the spot (`forced`); then
   `trim_ledger`. A new decision node is evaluated; a terminal backs up `outcome_value` (winner 1; unfinished: the top VP share it); a
   chance node is passed in the same simulation. A failed expansion drops the edge (`dead`, counted in `errors`: 0 in every test game).
7. Backup: max^n. Every node on the path `n += 1`, `w += v` (the absolute vector); a node's actor reads `w[actor] / n` of its children.
8. Keys: `child_key(parent key, slot, tag)` (tag 1 decision edge, 2 chance class): the delegate's seed, the segment's chance stream and the
   evaluator's seed per node. Same seed + same game = same search (tested); a different `cfg.seed` = another search.

### Tree reuse

The driver (with `observe = true`) now reports every decision with its question and the core pick the answer named
(`Applied::Decision { seat, answer, choice: Box<Choice>, pick }`, new `Position::resolve(&Answer) -> Option<Pick>`, small additive change
to C6). `MctsAgent::observe` follows the kept tree: a decision equal to the cursor's question at the same decision count moves along the
pick's edge; a roll of the cursor's chance request moves along its hit class (`class_of_hits`, merged classes included); anything else
(delegated, single, sampled, other seats' small asks) leaves the cursor. At the next own decision: the cursor must be that question with the
same candidates and move constraint, and the subtree must be at most `max_reuse_nodes` (1,536); else a fresh tree.

The subtree is copied into a new arena (`Tree::subtree`), **every position dropped**: the real line differs from the tree's line in what
the tree does not branch on (the real dice faces vs the class representatives, the real seats' small answers vs the tree's delegate, later
the C8 determinisation). The statistics are kept; a node's position is re-derived lazily from its parent (the edge replayed with the
node's key) when a simulation is about to expand below it; a node whose re-derived halt differs (another question, candidates, request) is
replaced by a fresh evaluated node (`mismatched`; 0 in the test games so far). So positions are always derived from the real game.

### Memory and time

* `max_nodes` (2,048) per tree: a full tree stops the search (`Stats::full`). Positions held = nodes minus terminals; live heap **~30 KB
  per node** (bench: the position with its trimmed ledger and the segment's appends), so ~60 MB per full tree.
* `max_time` (off): a wall-clock cap per search (breaks determinism on purpose).
* `Stats`: searches, simulations, expansions, evaluations, re-derivations / mismatches, reuses and reused nodes, errors, full / timed-out
  searches, delegated / single / sampled / forced answers inside segments, chance nodes, terminals, max depth, evaluator / engine / wall ns.

## 2. Tests (`cargo test --profile gate -p ti4-mcts`; 13 unit + 4 game tests, ~35 s on the loaded box)

| test | what |
|---|---|
| `backups_on_a_hand_made_tree_are_max_n_vectors_with_exact_counts` | a hand-made tree on a real 3-option question (a loss, a 30/70 chance node, a sure win for the actor; no position needed): 40 sims, `n = 1 + 4 + 40` at the root, Q of the sure win exactly 1, of the loss 0, the chance node's visits within 1.5 of 30 %, `w` sums to `n`, the improved policy orders the three, invariants hold |
| `sequential_halving_spends_the_budget_and_keeps_the_best` | budgets 1, 2, 5, 8, 16, 33 with `m` 2 / 3 / 8: exactly `budget` simulations attempted and backed into the root's children; the sure win survives the halving whenever all are considered |
| `a_search_from_a_real_position_keeps_its_invariants_and_leaves_the_game_alone` | 3 real positions (seeds 1-3, early to mid game), 24 sims with the rule-bot prior: invariants, budget, a candidate chosen, a position at every non-terminal node, **the game's `state_hash` and stream positions unchanged** |
| `a_reused_subtree_rederives_its_positions_and_keeps_its_statistics` | re-root at the same question: 0 positions, reachable = len, root `n` carries over, re-derivations happen, 0 mismatches |
| `the_node_bound_stops_the_search` | `max_nodes = 12`: at most 12 nodes, `full` reported, invariants |
| `the_budget_follows_the_playout_cap_and_halves_moves_and_closings` | `Config::budget` |
| `candidates_apply_the_activation_filter_and_the_canonical_move_order` | two rule-bot games: every activation's candidates = `filter::options`; at every movement with 3+ ships a mid key as the last move keeps exactly the ships at or after it plus done; another seat's move constrains nothing |
| `a_search_with_a_points_value_scores_when_it_can` | the value's perspective on real positions: at every scoring question of two games where the answers gain different points, a search (flat priors, VP-share value, no noise) picks the answer with the largest gain |
| gumbel / value / eval unit tests (5) | halving plan shape and budget, `completed_q` / `v_mix` / rescale, improved policy, rotations, outcome and VP values, one-hot prior logits |
| `an_mcts_seat_with_rollouts_plays_whole_games` (tests/games.rs) | `Rollout` (40 answers, flat priors), 8/2 sims, one MCTS seat, seeds 1-3 (seats 0, 1, 2): **whole games to the end, no engine error, no failed expansion**; 73-114 searches per game, reuse in 36-64 of them, chance nodes 0-15; each game **replays from its recorded answers alone to the same outcome and step count** (the search leaked nothing into the real game) |
| `the_search_is_deterministic` | seed 4, MCTS at seats 0 and 2 (rule-bot prior), 400 steps, twice: identical records, simulations and expansions; another search seed: another search |
| `four_mcts_seats_play_a_whole_game` | the self-play table: 4 MCTS seats (rule-bot prior, 4/2 sims), seed 5: finished in round 9, 663 searches, 4.5 s |
| `the_ab_smoke_against_the_rule_bot_runs` | 2 seeds paired with the all-rule-bot game: both end; prints the totals (plumbing) |

C6's own tests still pass with the `Applied` change (`cargo test -p ti4-engine-search`: 9 + 1 passed, 2 ignored as before).

## 3. Numbers

### Micro-bench (`mcts_bench --games 2 --every 4 --sims 32`, gate profile, shared box load ~7)

Fresh searches of 32 simulations at every 4th tree question (2+ candidates) of two rule-bot games (223 searches):

| evaluator | sims | sims/s | us/sim | of it: evaluator / engine | nodes per search | max depth | chance nodes | live heap per node |
|---|---|---|---|---|---|---|---|---|
| `uniform` (Gumbel below the root) | 7,136 | **1,089** | 918 | 0 / 912 | 40.2 | 48 | 1,657 | 30.8 KB |
| `uniform`, `--puct 1.5` | 7,136 | 1,107 | 903 | 0 / 897 | 40.3 | 48 | 1,657 | 30.8 KB |
| `rule` (rule-bot prior, VP share) | 7,136 | **520** | 1,922 | 1,082 / 866 | 41.6 | 48 | 1,967 | 30.6 KB |
| `rollout:40` (rule-bot rollout of 40 answers) | 7,136 | **42** | 23,883 | 23,486 / 1,051 | 41.1 | 37 | 1,846 | 31.1 KB |

Engine cost per simulation ~0.9 ms (fork + answer + ~2.3 delegated rule-bot answers + trim; C6 measured 0.59 ms per bare segment);
the rule bot as a prior costs another 1.1 ms per leaf (its `SeenView` and pick), so a net at ~0.3 ms per leaf would put the search at
~800 sims/s per core. Per decision: 32 sims ~30 ms (uniform), 8 sims ~7 ms.

### A/B smoke (`mcts_ab`, one MCTS seat at place `seed % 4` vs the all-rule-bot game of the same seed, sims 8 / 2)

**Plumbing, not strength** (16 games give a +-24 % interval at p = 0.25; L7's `mcts_eval` is the harness for claims):

| run | MCTS seat: wins / mean VP | rule bot in that seat (same seeds): wins / mean VP | s per game |
|---|---|---|---|
| `rule` (VP share), 16 seeds 101-116 | 0 / 4.56 | 6 / 7.31 | 1.7 vs 1.2 |
| `rule`, `--rescale 0` | 0 / 4.25 | 6 / 7.31 | 1.5 |
| `rule`, **32 / 8 sims** | 1 / 5.69 | 6 / 7.31 | 3.7 |
| `rule-flat` (flat value), Gumbel noise 1 | 0 / 3.62 | 6 / 7.31 | 1.4 |
| `rule-flat`, **no Gumbel noise** (`--gumbel 0`) | 3 / 7.00 | 6 / 7.31 | 1.6 |
| `rollout-rule:200`, 8 seeds 101-108 | 0 / 3.88 | 2 / 7.00 | 47 |

Reading: every deviation from the rule bot's pick costs points at this level. The flat-value, noise-free run (= the rule bot's argmax at
the tree asks, up to the filter, the canonical order and the bot memory) plays at the rule bot's level: the plumbing holds. The Gumbel
noise alone (a sample from a prior with 0.6 on the rule bot's pick) costs as much as the noisy values; more simulations help (4.6 -> 5.7
VP from 8/2 to 32/8). A value-perspective check on real positions passes (`a_search_with_a_points_value_scores_when_it_can`). Raw:
`/root/deploytmp/l6-logs/ab-*.{txt,json}`.

## 4. Remaining / open problems

* **No stub beats or matches the rule bot** (section 3): with 1-4 visits per root candidate the choice among the Gumbel-sampled pair
  is decided by single noisy leaf values (VP share one segment ahead, or a 200-answer rollout then the VP share), and the Gumbel noise
  itself samples away from the rule bot's pick. Expected for stubs; the real leaf is the value net (L4/L8). For **benchmark play use
  `gumbel_scale = 0`** (the noise is self-play exploration) and a sharper prior; L7 must report the setting.
* **The net evaluator is a hook, not wired.** `core-mcts-prep-2` (`ti4-mcts-net`, the `Encoder`, `Seen::movable_counts`) is not on this
  branch (it is cut from prep-1, not from C6; merging it here would drag its core change along). Wiring after the join, ~20 lines in
  `eval.rs` behind no feature (both are dev crates):
  ```rust
  let mut enc = Encoder::default(); let mut buf = Encoded::default(); let mut scratch = Scratch::default();
  FnEvaluator { name: "net".into(), f: Box::new(move |pos, choice, cands, _key| {
      let seen = pos.seen().ok_or("no question")?;
      enc.encode_into(&seen, choice, &mut buf); round_halves(&mut buf);       // as agents::Table4 (the f16 rounding of training)
      let out = net.forward(&buf, &mut scratch);
      let p = out.win_probs();
      Ok(Evaluation { logits: cands.iter().map(|&c| out.logits[c]).collect(), value: [p[0], p[1], p[2], p[3]] })
  }) }
  ```
  Check then that the net's actor-relative seat order is `(actor + k) % 4` (this crate's `to_relative`), and move the greedy
  `PolicyAgent` / L7's `mcts_eval` onto `MctsAgent` (`--agent mcts:net=PATH,sims=32/8`).
* **Training records.** `MctsAgent { record: true }` keeps every `SearchResult` (candidates, improved policy, visits, Q, root value,
  `full`); L8 must turn the `full` ones into shards (features of the root, the policy target, the outcome / TD(lambda) value).
* **Bot memory.** A tree pick answers by `Pick`, so the rule bot's movement memory of the MCTS seat is not updated by its own tree
  decisions (the fallback rule bot then answers the seat's small asks with the memory of its last own answer). Small; part of why
  `rule-flat --gumbel 0` is not the rule bot's game exactly (with the filter and the canonical order).
* **Chance nodes**: the Jol-Nar flagship caveat of C6 (class representatives) stands; non-hit requests are sampled once per segment.
  Merged small classes (< 1 %) are mapped back by hit count when following the real dice.
* **Transpositions**: none beyond the canonical move order (plan v1).
* **Time**: the playout cap's 32/8 at ~1-2 ms per simulation (uniform / rule prior) is 15-30 ms per decision; with the net the evaluator
  adds ~0.3 ms encode + forward per leaf (prep-1/2 numbers). A searched 4-seat game with 663 searches took 4.5 s at 4/2 sims.
* C8: `Determinize` is called on the root copy before the root is evaluated; a reused subtree is re-derived from the determinised root,
  so a new deck order per search is consistent with reuse (a node whose halt changes is replaced).
* Not done (by the brief): heavy gates.

## 5. Checks

| check | result |
|---|---|
| `check_layers.py` core / thin / setup / chance / resolver / search / **mcts** (new) | 576 / 193 / 14 / 5 / 1 / 10 / 12 files, 0 violations; the mcts mode's negative control (an `advance_observed` call and a `.rng` read in a scratch file) fails with 2 violations as it must |
| `check_production_graph.sh` (strict; `ti4-mcts` added to the dev tools and the "never in a production tree" list) | 0 violations, 0 failed controls |
| `clippy_core.sh` (now also `-p ti4-mcts`) | ok, engine warnings 41 = baseline (the `Applied` variant was boxed for `large_enum_variant`) |
| `lint_core.sh` | 578 files, 0 hits |

## 6. Join notes

* C6 change (additive): `Position::resolve`, `Applied::Decision` gains `choice: Box<Choice>` and `pick: Option<Pick>` (only the driver
  builds it; nothing outside the C6 crate matched on it). The driver resolves the pick only when `observe` is on.
* Workspace: `crates/ti4-mcts` in `members`; `Cargo.lock` gains it. Tools: `check_layers.py` (`mcts` mode, `check_search(resolver_file)`),
  `check_production_graph.sh` and `clippy_core.sh` list it.
* With `core-mcts-prep-2`: no file overlap (prep-2 touches `ti4-mcts-features`, `ti4-mcts-lab`, `ti4-mcts-net`, `tools/mcts`, core
  `query/reach.rs` / `seen/reach.rs`). Wire the net as in section 4.
* With C8: implement `agent::Determinize` (the root copy and the search's stream are handed in), pass it with
  `MctsAgent::with_determinizer`.
* With the multi-ask re-merge (integration 29+): multi-pick asks are never tree kinds, so the search only sees them through the delegate.

Disk: target dir `/root/deploytmp/l6-target` deleted at the end; logs kept in `/root/deploytmp/l6-logs` (A/B JSON and text, clippy).
