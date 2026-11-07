# MCTS prep 2: resume (lanes L2, L4, L7, L5 stopgap)

Branch `core-mcts-prep-2`, cut from `core-mcts-prep-1` (6e2ac1488). Not pushed. Plan: `docs/MCTS_BOT_PLAN.md` (branch
`core-theory-mcts`); prep 1: `docs/MCTS_PREP1_RESUME.md`, `docs/MCTS_M0_M1.md`.

## Done

| Lane | What | Where |
|---|---|---|
| L2 generator | `mcts_dataset`: rule-bot games, seeds `first..first+N` (seating `seed % 24`), at every tree-kind ask with 2+ kept options the rule bot answers and with eps 0.15 (0.3 in rounds 1-2) a uniform kept option is **played** instead; the **target** stays the rule bot's pick (none when `smart == false`). Per-kind record rates (draft 0.4, menu 0.12, closing 0.07, activate 0.3, move 0.15, score 1, imperial 1: about 110 records per game, cap 200), decided before the encoding. 1:1 wire/core option assertion per record. Threads; one gzip shard per worker per 100 games; `summary-<first>.json`. Meta gains `played`, `noise` | `crates/ti4-mcts-lab/src/bin/mcts_dataset.rs`, `crates/ti4-mcts-features/src/shard.rs` |
| L2 trainer | `ti4mcts.stream` (`train.py --stream`): split by shard (whole games), shard pools per epoch, value loss subsampled per game (`w = min(1, 10 / records of the game)`), policy weight per tree kind `clip(sqrt(median/count), 0.5, 3)`; held-out report: top-1 per kind and chance, value CE vs uniform and vs a faction prior, by round. `losses()` takes optional weights | `tools/mcts/ti4mcts/stream.py`, `model.py`, `train.py` |
| L4 forward pass | `ti4-mcts-net` (new dev crate): `.t4w` loader, forward pass of `reference.py` with exact rewrites (empty slots compacted out of the attention, sparse first layers, policy head split so only option features are per option, kind embeddings folded at load, register-tiled matmul, vectorisable exp); `greedy`; `plain_shard` reader; `mcts_net_bench` | `crates/ti4-mcts-net` |
| L4 parity | max abs diff **9e-6** on the 200 prep-1 fixtures (`TI4_MCTS_FIXTURES=DIR`), a committed tiny fixture (24 real records, small random model, 450 KB; `tools/mcts/make_tiny_fixture.py`) always runs | `crates/ti4-mcts-net/tests/parity.rs` |
| L4 encode | `Encoder` (per table): map slots, adjacency, distances and homes cached (rebuilt when the map's systems or adjacency change); the reach of all systems counted at once by the new read-only `Seen::movable_counts` (one rules clone retargeted per destination, `SeatReach::retarget`; one route search per group of equal ships, `SeatFleet::movable_count`); generic `Sink`. `encode()` stays the reference; records are identical (test over every tree ask of two games, and `movable_counts == movable_into().len()` for every seat and system) | `crates/ti4-mcts-features/src/encode.rs`, `filter.rs`, core `query/reach.rs`, `seen/reach.rs`; `mcts_encode_bench` |
| L5 stopgap | `agents::Agent` per seat: `rule`, `policy:W.t4w` (greedy net on the tree asks with 2+ kept options, features rounded to f16 as in training; rule bot elsewhere), `random` | `crates/ti4-mcts-lab/src/agents.rs` |
| L7 harness | `mcts_eval`: bench (per seed the paired all-rule baseline + the agent at each of the 4 places), gate (2 vs 2, swapped); win share with Wilson 95 % overall / per faction / per place, mean VP, margin over the best rival, paired Δ win share and Δ VP (normal CI), Wald SPRT with tie-split outcomes, net timing, in-play agreement (`--agreement`); report JSON. `tools/mcts/report_page.py` renders a small HTML page | `crates/ti4-mcts-lab/src/bin/mcts_eval.rs`, `src/eval.rs` |

## Key numbers

The box was heavily shared during the run (load 8 to 22 on 8 cores, other lanes building and testing); wall times are loose.

* **Dataset** (`/root/deploytmp/mcts-data/v1`, seeds 100000..107999, 4 workers): **8,000 games, 934 k records** (117 per game),
  **408 MB** gzip (454 bytes per record). By kind: draft 148 k, menu 178 k, closing 82 k, activate 201 k, move 149 k, score 145 k,
  imperial 32 k; 771 k with a policy target; 174 k played by the noise. The run was started for 24 k games and stopped at 8 k
  (the box was too loaded to train and generate inside half the cores); stopping by kill leaves no `summary-*.json`, so the
  counts come from `tools/mcts/dataset_stats.py` (`/root/deploytmp/mcts-data/v1-stats.json`). The 24-game test run had 0
  errors, 0.4 % picks put back into the filter.
* **Training** (`/root/deploytmp/mcts-data/run-v1`: `weights.t4w`, `model.pt`, `log.jsonl`, `fixtures/`): streaming, 4 threads,
  lr 1e-3 one-cycle, batch 256, 76 train shards, 4 held-out shards (400 games, 46.7 k records). Stopped by the 150-minute cap in
  epoch 4 of 6 (about 3.8 epochs; the one-cycle schedule did not finish). Held-out after the last epoch:

  | | top-1 | records |
  |---|---:|---:|
  | **all** (uniform among kept options: 26.3 %) | **81.1 %** | 38.5 k |
  | draft | 72.9 % | 6.4 k |
  | menu | 88.4 % | 6.1 k |
  | closing (smart answers only) | 97 % | 35 |
  | activate | 64.2 % | 9.8 k |
  | move | 89.1 % | 7.3 k |
  | score | 94.0 % | 7.2 k |
  | imperial | 96.2 % | 1.6 k |

  **Value CE 1.034** against **1.386 uniform** and **1.332 for a faction prior** (no overfitting with 8 k games and the per-game
  subsampling; prep 1 had 1.40 rising to 2.31). By round: 1.28 (r1), 1.10 (r3), 0.83 (r6), 1.40 (r9, few games). Epoch by
  epoch top-1: 72.5, 76.7, 79.6, 81.1 %; value CE 1.096, 1.083, 1.094, 1.034. Still improving: more epochs and the full 24 k games
  should help.
* **Rust forward pass**: parity max abs diff 6e-6 (v1 fixtures), 8e-6 (prep-1), 3e-7 (tiny). **About 0.6 ms per decision**
  (10.7 options, 2.4 GHz, idle box; 1.46 M cycles), 0.64 ms in play. A build with `-C target-cpu=x86-64-v3` takes 25 % fewer cycles.
  This is well above the plan's 0.05-0.2 ms: see open problems.
* **Encode**: reference 177 us, **`Encoder` 112 us** per decision (idle box, identical records). Left: the route searches of
  `movable_counts` 56 us (the core's `CoreRules::search_path`), objective progress 27 us.
* **Stepping stone 1** (`/root/deploytmp/mcts-data/eval-v1-bench240.json`; `mcts_eval --agent policy:run-v1/weights.t4w --seeds 240
  --first 900000 --workers 4 --agreement`, 1,200 games in 7.3 min, every seating 10 times): the greedy net at one seat against three
  rule bots wins **22.8 % (Wilson 95 % 20.2-25.5 %)**, 958 games; **paired Δ win share -2.1 ± 3.5 pts**, Δ VP -0.14 ± 0.18 against
  the rule bot on the same seat and seed. Mean VP 7.07; margin over the best rival -2.4. SPRT 0.25 vs 0.30: H0 after 101 games (it
  is not 30 % or better). Agreement with the rule bot in play 80.3 %. By faction (agent vs the rule bot on that faction): Sol
  33.5 % vs 40.1 %, Letnev 24.9 vs 26.3, Jol-Nar 19.8 vs 17.3, Xxcha 13.0 vs 16.3. Inside the plan's 20-25 % for the policy bot:
  features and option mapping work; it is a little weaker than the rule bot, not significantly.
* Artifact page: https://claude.ai/artifact/PS4taBW3QZyFdxkHrvVrdh (private; `tools/mcts/report_page.py`).

## Checks run

| Check | Result |
|---|---|
| `cargo test --profile gate -p ti4-mcts-features -p ti4-mcts-lab -p ti4-mcts-net` (targeted) | features 8 + 1, lab lib 4, dataset 1, harness 2, parity 3 (with `TI4_MCTS_FIXTURES`) |
| `tools/mcts/tests` (venv) | 11 passed, 1 skipped (real shard) |
| `cargo clippy -p ti4-mcts-features -p ti4-mcts-lab -p ti4-mcts-net --all-targets` | no warning in the new crates or the touched core files |
| `clippy_core.sh` | ok (cargo rc 0, engine warnings 41, baseline 41) |
| `lint_core.sh` | 578 files, 0 hits |
| `check_layers.py` core / thin / setup / chance | 0 / 0 / 0 / 0 |
| `check_production_graph.sh` (strict; `ti4-mcts-net` added as a dev tool) | 0 violations, 0 failed controls |

## Open problems

* **P1 net speed**: 0.6 ms per forward is about half of a simulation (1.4 ms). Options: build the search binaries with
  `-C target-cpu=x86-64-v3` (-25 %; the workspace forbids `unsafe`, so no runtime dispatch); a smaller net (d 48, ff 96 is
  about half the work; check the accuracy cost); batch the leaf evaluations of 2-4 concurrent games into one matrix product
  (plan 3.2); a value-only pass at leaves where the policy is not needed.
* **P2 encode**: 112 us, under the 150 us goal; the remaining bulk is the core's route search (`search_path`, `VecDeque`
  calls not inlined across codegen units). A per-node cache in the search, or a cheaper exact reach in the core, is the next step.
* **P3 engine error in 3 of 1,200 games**: `ask: Live(Unknown(TurnGap(StrategyAbility)))` (seeds 900139 at place 1, 900168 in the
  all-rule baseline, 900235 at place 3): not the net (the baseline game hits it too); a lab or core issue for the core lanes.
* **P4 dataset size**: 8 k games, not the 20-50 k target. Resume with `mcts_dataset --first 108000 --games 16000 --out
  /root/deploytmp/mcts-data/v1` (same directory; new shard names) when the box has room, then retrain longer (the curves still
  rise at epoch 4).
* The rule-bot faction imbalance (Sol 40 %, Xxcha 16 % in the baseline) means per-faction numbers must be read against the
  rule bot on the same faction (the report has both).

## Remaining (next lanes)

* L6 (search) needs C6; the harness (`mcts_eval`) takes an `MctsAgent` as a new `Agent` arm, and `gate` mode is ready for
  candidate-vs-best SPRT (H0 0.5, H1 0.55).
* Plackett-Luce ladder of plan 4.3 is not done (only pairwise bench and gate).
* L8 self-play loop: `mcts_dataset` is the template for the recording side (meta `net`, `sims` are there).

## Join notes

* New crate `crates/ti4-mcts-net` (deps: `ti4-mcts-features`, `serde_json`), workspace member, in the dev-tool list of
  `check_production_graph.sh`; `ti4-mcts-lab` gains it. Nothing in production or the wasm depends on any MCTS crate.
* Core touch points (read-only additions): `query/reach.rs` (`SeatReach::retarget`, `SeatFleet::movable_count`),
  `seen/reach.rs` (`Seen::movable_counts`). Small, additive; conflicts with `core-chance-2` / `core-search-thin-1` /
  `core-multiask-1` are unlikely.
* Shard schema: meta has 19 columns now (`played`, `noise` added); prep-1 shards (17) do not mix with v1 shards in one training run.
  `FEATURE_VERSION` is unchanged (the features are identical).
* Data and runs: `/root/deploytmp/mcts-data` (`v1/` 408 MB, `run-v1/`, `eval-v1-bench240.json`, `bin/` copies of the binaries,
  logs). The worktree's target dir is deleted at the end.
