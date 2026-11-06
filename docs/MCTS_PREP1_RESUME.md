# MCTS prep 1: resume (lanes M0, M1, L1, L3)

Branch `core-mcts-prep-1`, cut from `core-integration-28` (f22ece5e8). Not pushed. Plan: `docs/MCTS_BOT_PLAN.md` (branch
`core-theory-mcts`). Measurements: `docs/MCTS_M0_M1.md`.

## Done

| Lane | What | Where |
|---|---|---|
| M0 + M1 | Lab driver: the product's native step without the wrapper extras; `Pos` = core + render + glue flag (C6's `Position`); every question handed to a `Decide` with the core `Choice`, the wire `Choice` and its position; rule bot on `SeenView`. Census binary (200 games, 125 s) | `crates/ti4-mcts-lab` (`src/lib.rs`, `src/bin/mcts_census.rs`); results `docs/MCTS_M0_M1.md`, raw `docs/mcts_data/` |
| L1 | `ti4-mcts-features`: `kind` (tree kinds of plan 2.2, plus Thunder's Edge Warfare's free tactical action as an activation), `filter` (the activation hard filter: move / produce / shoot, pass-through), `encode` (actor-relative tokens from `Seen`: 40 system slots x 112, 4 seats x 156, 16 objectives x 18, 18 global, options x 40 + kind id + 3 pointers + mask; packed adjacency), `schema` (JSON), `npy` + `shard` (gzip `.npy` shards, ~330 bytes per record) | `crates/ti4-mcts-features` |
| L1 tests | 7 fixed-position tests (seeded rule-bot games stopped at a fixed decision): widths = constants and unique column names; 37 systems, symmetric adjacency, 4 homes; actor-relative rotation of seat and unit blocks; **hidden information does not reach the features** (other hands and every deck changed: same encoding; own secrets: different); filter invariants; option pointers; shard round trip (plain and gzip) with the outcome rotated to the actor | `crates/ti4-mcts-lab/tests/features.rs`; unit tests in the features crate (f16 rounding, npy header, schema) |
| Smoke exporter | `mcts_shards --games N --out DIR`: every tree node with 2+ kept options, rule-bot pick one-hot (no target when `smart == false`), outcome at the end. Not L2 (no noise, no subsampling) | `crates/ti4-mcts-lab/src/bin/mcts_shards.rs` |
| L3 | Trainer: schema check, shard loader (plain mmap or gzip), batches with padded options, split by game; model of plan 3.2 (170,505 parameters); imitation + value losses (label smoothing 0.05, CE value, 0.1 MSE VP); training CLI; `.t4w` weights export; **NumPy reference forward** (the spec of the Rust port); parity fixtures (200 records, plain `.npy` inputs, f32 outputs, tolerance 1e-4); synthetic shards with a planted teacher | `tools/mcts/ti4mcts/*.py`, `tools/mcts/tests/test_trainer.py` (10 tests), venv `/root/mcts-venv` (torch 2.9.1+cpu, numpy 2.5; 0.94 GB) via `tools/mcts/setup_venv.sh` |
| Core | three read-only accessors on `Seen` (`map_systems`, `adjacent`, `adjacent_of_each`, `hex_of`) and `CoreGalaxy::adjacent_of_each` (holders once) | `crates/ti4-engine-core/src/seen/board.rs`, `galaxy.rs` |
| Checks | `check_production_graph.sh` lists both new crates as dev tools and fails if a production crate reaches them | |

## Key numbers

* Engine 4,500 steps/s (core live loop, rule bot removed); `Pos::fork` 3.8 us; rule bot 80 % of a rule-bot game.
* Per tree node: engine 0.67 ms + delegated rule bot 0.64 ms -> about **1.4 ms per simulation**; 577 tree nodes per game.
* 1,955 asks per game; activation 36 -> 17 options after the filter; move 2.7 options; draft 5; menu 5.3.
* Encode: about **0.26-0.31 ms** per decision on the loaded box (frame/board 80 us, reach of all systems 90 us, objective progress 70 us).
* Training: about 420 records/s on 3 threads (loaded box). Synthetic: held-out agreement 67 % vs chance ~15 % after 5 epochs.
* **Real data** (smoke exporter, 200 rule-bot games, seeds 1001-1100 and 2001-2100: 112 k records, 37 MB gzip; 255 picks
  outside the filter were put back): 3 epochs, 12 min on 3 threads. Held-out (whole games, 5 %): **top-1 agreement 65.5 %**
  (chance 24.7 %); by kind: score 90 %, imperial 98 %, menu 73 %, move 72 %, closing 73 % (only the 1 % smart answers), activate
  51 %, draft 36 %. **Value overfits**: held-out value CE 1.40 after epoch 1 (uniform = 1.386), 2.31 after epoch 3: 200 games
  are far too few outcomes for the value head (plan: 20-50 k games); L2 must also add the per-game subsampling. Run in
  `/root/deploytmp/mcts-prep1/run-real` (`weights.t4w`, `fixtures/` for L4).

## Checks run

| Check | Result |
|---|---|
| `cargo test --profile gate -p ti4-mcts-features -p ti4-mcts-lab` | 6 + 1 + 7 passed |
| `cargo clippy -p ti4-mcts-features -p ti4-mcts-lab --all-targets` | no warning in the new crates or the core |
| `/root/mcts-venv/bin/python -m unittest discover -s tools/mcts/tests` (with `TI4_MCTS_SHARD` = a real shard) | 10 passed |
| `lint_core.sh` | 578 files, 0 hits |
| `check_layers.py` core / thin / setup / chance | 0 / 0 / 0 / 0 |
| `check_production_graph.sh` (strict) | 0 violations, 0 failed controls |
| `clippy_core.sh` | ok (cargo rc 0, engine warnings 41, baseline 41) |

## Remaining (next lanes)

* **L2 dataset generator**: start from `mcts_shards` (it has the hook: `Decide` sees core `Choice` + position). Add the
  epsilon-noise (play a uniform kept option, record the rule bot's pick), at most one record per segment, subsampling to about
  100 per game, workers, one shard per worker per N games, the 1:1 option-order assertion per record.
* **L4 Rust forward pass**: implement `tools/mcts/ti4mcts/reference.py` in Rust (read `.t4w`; the fixtures of
  `python -m ti4mcts.train ... --out RUN` are in `RUN/fixtures/`; inputs are a plain `.npy` shard). Budget: the encode (0.3 ms)
  is already above the plan's net budget; optimise the encode (see below) together with the forward pass.
* **Encode speed**: precompute per position (not per actor) the board pass; `movable_into` per destination is the core's reach
  per system (90 us); the objective progress for 4 seats (70 us) could be cached per round/position in the search node.
* **Setup flag for "no action cards / no exploration"** (open problem P1 of `docs/MCTS_M0_M1.md`): not in the setup or the
  core today; the lab plays the product rules.
* **Q4 (user)**: `Turn.Closing` (99 % random in the rule bot, 25 % of the tree nodes): tree node at half simulations
  (default) or a fixed answer.

## Join notes

* New crates: `crates/ti4-mcts-features` (deps: `ti4-engine-core`, `flate2`), `crates/ti4-mcts-lab` (deps: thin engine,
  core, rule bot, typed rule bot, selfplay, features). Workspace `members` + `Cargo.lock` gain them. No production crate
  depends on them; the wasm is untouched (the core gains only unused `pub fn`s).
* Core touch points (small, read-only): `seen/board.rs` (+4 accessors), `galaxy.rs` (+`adjacent_of_each`). Conflicts with
  `core-chance-2` / `core-search-thin-1` / `core-multiask-1` are unlikely there.
* When C6 lands, `ti4_mcts_lab::Pos` / `step` should be replaced by `search::Position` / `advance`; the `Decide` trait maps onto
  the plan's `SeatAgent`. When `Ask::Chance` lands (C3/C4), the lab's step must answer it (it will arrive as a question).
* The rule bot's thread-local movement memory (`ti4_rule_bot::backward`) is state outside the position: the search must save and
  restore it per node (the lab does so around its replays).
* Scratch kept: `/root/deploytmp/mcts-prep1/real` (2 real shards) and `run-real` (weights + parity fixtures), about 40 MB; the target dir was deleted.
