# Lane C0: fork tests and the draw census (resume)

Lane C0 of `docs/FORK_RNG_THEORY.md` (on branch `core-theory-fork-rng`, sections 1.6, 3.2 and 5). Branch `core-fork-tests-1`, based on
`core-heavyfix-1` (b7ac383a9). Tests and test-only counters only. Production behaviour is unchanged, and so are both hashes.

## What was added

| File | What |
|---|---|
| `crates/ti4-engine-core-bridge/tests/fork_core.rs` | **new**: T1 (two tests), T2 (three tests, one of them ignored as a known violation), and the census (two tests) |
| `crates/ti4-engine-core/src/state/rng.rs` | `census` module behind the new feature `probe-counters`. It holds thread-local counts of `die` calls, the stream words they use, the start position of each draw, and `shuffle` calls. `Rng::die` reads the word position before and after each draw, only when the feature is on |
| `crates/ti4-engine-core/src/driver/mod.rs` | `Driver::settle` (feature-gated, 2 lines): takes a mark at each run's snapshot. At a rollback it counts the draws since that mark, as either *dry* (a nested question: R1 phantom rest / R2 re-ask) or *error* |
| `crates/ti4-engine-core/src/state/mod.rs`, `Cargo.toml` | `pub use rng::census` and `[features] probe-counters = []` |
| `crates/ti4-engine-core-bridge/Cargo.toml` | `[dev-dependencies] ti4-engine-core = { features = ["probe-counters"] }`. The feature is on only in this crate's test builds; product, gate and wasm builds never have it |

`driver/live.rs` is untouched (lane core-nestq-1 is working there).

### The driver of the tests

The games are played **directly on the core live API**: each arm's `ask` / `advance`, plus `advance_reconcile` / `ask_job`. The driver is
`perf_kernel`'s loop, rewritten so that a step can stop at any question and resume (`Play::step(from: Option<Mid>, stop)`). The bot is
`perf_kernel`'s seeded weighted random pick over the typed options. Between two core calls, the driver keeps only what any caller of the
live API keeps:

* the arm or reconcile round it is in;
* the step's `Env` and the run's `Env`;
* the bot's counters;
* `after_glue` (V1).

A fork has three parts:

* a `CoreState::clone`;
* a copy of that caller state;
* a **cold** `Render`, rebuilt from the code table (what `NativeGame::fork` does). The `Render` is never shared.

When a copy resumes, it first re-derives the pending question with `ask` from its own core, and the question must equal the original's.

Games: `(4,1,sol)` (the quick game's table; with this bot it is 1,251 steps, not the 1,165 of the rule bot), `(3,3,yssaril)`,
`(4,3,jolnar)`, `(5,2,l1z1x)`.

## Tests and results

Run:

```text
CARGO_TARGET_DIR=... cargo test --profile lockstep -p ti4-engine-core-bridge --test fork_core -- --nocapture             # 6 pass, 1 ignored
CARGO_TARGET_DIR=... cargo test --profile lockstep -p ti4-engine-core-bridge --test fork_core -- --ignored strict          # the known V5b failure
```

All 7 tests together run in about 4 s, after the build.

| Test | What it checks | Result |
|---|---|---|
| `a_core_clone_at_any_question_resumes_to_the_same_game_and_is_independent` (T1) | 69 forks in 4 games (38 at nested questions, 23 at the first question after a committed die, the rest at plain stage questions; at most one per step). For each fork: (1) a copy with the same answers plays to fork step + 40 (the last fork of each game plays to the end); (2) a copy with other answers plays on; (3) the original resumes. Required: `CoreState ==` against a game that was never forked (board, players, decks, streams, dice and event ledger, pending stack, texts), plus the same answer chain, the same `advance` output chain (`emitted`, `journal`, `opened`), the same question re-derived by `ask` in the copy, and the same `after_glue`. The original, forked again and again, ends equal to the unforked game | **pass**. All 69 copies with other answers diverged, and the originals were unaffected |
| `a_core_clone_right_after_a_roll_at_a_step_boundary_resumes_to_the_same_game` (T1) | 27 forks at the step boundary right after a step that committed a die; a copy, a copy with other answers, and the original each play on 40 steps | **pass** |
| `the_codec_round_trip_at_a_quiet_step_boundary_gives_the_same_core_and_plays_on_identically` (T2) | 282 quiet boundaries (no window, no nested question; every 7th step). The core is stored into `GameState` with `materialize`, then loaded with `Codec::load`. Handed in: the resolver, the stream positions, the map, and the dice and event logs. Only the fields in `CODEC_DROPS` may differ. With those fields handed in too, the loaded core must be `==` the original and play 40 steps identically | **pass**: everything outside `Pending` survives the codec, and with the dropped `Pending` fields lent the loaded core plays on identically |
| `the_codec_round_trip_at_a_quiet_step_boundary_is_exact_strict` (T2 strict) | the same, with nothing handed in but the resolver and the positions | **ignored: known violation V5b** (below) |
| `the_codec_cannot_carry_an_open_window_or_a_nested_question_v5` (T2, expected loss) | Two positions with open windows: a nested question and a stage question. The round trip succeeds without an error, but the window stack and `Pending::nested` come back empty | pass (pins V5) |
| `the_draw_census_counts_the_draws_the_rollbacks_throw_away` | census on the core driver, 4 games | pass (numbers below) |
| `the_draw_census_of_the_quick_game_and_two_bot_games_on_the_thin_game` | census on the thin `Game` with the rule bot (the quick game, 1,165 steps, plus 2 games) | pass (numbers below) |

Gates and checks:

* Both hashes are unchanged. `perf_kernel suite 1` gives `suite_hash 775f4136e60a99d5` (14 games, 14,738 steps; feature off).
* The quick-game fingerprint `538dc68f23dddcc9` was checked with `wrapper_native::the_wrapper_plays_the_quick_game...`. That test build has the feature on, which shows the counters do not change a draw.
* `check_layers.py core`: 0 violations.

## Violations found

* **None at the core live API.** A `CoreState` clone, the same `Env`, and the caller's few counters were always enough to resume. This
  held at nested questions (inside an emission), right after a die, and at step boundaries; the questions were re-derived from the copy alone.
  The theory's verdict for the core level (1.4, "no violation in the core crate") is confirmed on 96 forks. V1 (`after_glue`) is caller
  state here, carried by the driver as the thin `Game` carries it in `Native`. A bare `CoreState` still does not say how to resume, as the
  theory says. V2 and V3 belong to the thin `Game` and are not reachable from this driver; the doc's thin-level T1 with a mid-step
  `Game` resume is **not** written, see "Not done".
* **V5b (new, codec, low).** `core_sync/codec/pending.rs` stores only `Pending::tag`, so even a **quiet** boundary loses:
  * `Pending::driver` (`DriverFlags`: `STRATEGY_PHASE_ANNOUNCED` and the others), lost at 282 of 282 sampled boundaries, first at
    `(4,1,sol)` step 3 of this driver;
  * `prepared_turn_seq` (276 boundaries);
  * `turn_closing` and `actions_this_turn` (34 each);
  * `staged` (4).

  Loaded without these fields, 154 of the 282 copies then played differently: other answers, or another position outside these fields.
  The product is not affected, because saves are replays (seed + answers), and `edit_position` / `prepare_hypothetical_*` drop the turn state
  on purpose. A save or distributed search that serialises a core through the codec would be wrong, though. Repro: the strict test
  above. Fix (lane C5 or C9): give these `Pending` fields a home in the codec (or in `Ctx`, like the resolver).

## Census (draws made vs committed)

*Committed* means the final stream positions minus the start positions (in stream words), cross-checked against the number of distinct
draw start positions below the final position. The two always agree, because every die took exactly one word (`words == draws`
everywhere). `ledger faces` is the sum of `RollRec::faces` in the ledger (`Game::rolls()` faces for the thin game). It is close to the
committed draws, but not equal. Not every draw writes a ledger row (choose-element draws such as Neuraloop or the Yssaril hero, and some
ability rolls), and a reroll row repeats faces.

**Core live API, `perf_kernel` bot:**

| Game | Steps | Draws | Committed | Kept | Draws per committed | Dry rollbacks (runs / draws) | Error rollbacks | `ask` scratch draws | Failed advances |
|---|---|---|---|---|---|---|---|---|---|
| 4p 1 sol | 1,251 | 117 | 25 (ledger 26) | 21.4 % | 4.68 | 904 / 92 | 0 | 0 | 0 |
| 3p 3 yssaril | 780 | 44 | 12 (14) | 27.3 % | 3.67 | 632 / 32 | 0 | 0 | 0 |
| 4p 3 jolnar | 1,193 | 185 | 63 (67) | 34.1 % | 2.94 | 856 / 122 | 0 | 0 | 0 |
| 5p 2 l1z1x | 995 | 48 | 20 (12) | 41.7 % | 2.40 | 728 / 28 | 0 | 0 | 0 |
| **total** | 4,219 | **394** | **120** (119) | **30.5 %** | **3.28** | 3,120 / **274 (69.5 %)** | 0 | 0 | 0 |

**Thin `Game`, rule bot:**

| Game | Steps | Draws | Committed | Kept | Dry rollbacks (runs / draws) | Error rollbacks |
|---|---|---|---|---|---|---|
| quick game 4p 1 sol (`538dc68f23dddcc9`) | 1,165 | 76 | 42 (rolls faces 41, 33 roll rows) | 55.3 % | 1,028 / 34 | 0 |
| 3p 3 yssaril | 798 | 3 | 3 | 100 % | 776 / 0 | 0 |
| 5p 2 l1z1x | 971 | 30 | 16 (14) | 53.3 % | 832 / 14 | 0 |
| **total** | 2,934 | **109** | **61** | **56.0 %** | 2,636 / **48 (44.0 %)** | 0 |

Reading:

* Every draw that was not committed was thrown away by a **dry rollback**, a `settle` run undone because a nested question found the
  tape dry. This is the default-answer continuation and phantom rest (R1), together with the dry re-ask of a nested answer (R2). In both
  tables, draws = committed + dry-rollback draws, exactly. No draw was lost to an error rollback (R5: 0 of the ~18 sites fired, 0
  failed `advance` calls) or to an `ask` on a scratch clone (R4: 0 on the core driver).
* The theory estimated that 65-85 % of combat draws are reverted (one half to four fifths of all draws). The measurement:
  * **69.5 %** of all draws reverted with the random typed bot;
  * **44 %** with the rule bot on the thin game. The rule bot plays fewer reaction cards, so fewer nested questions follow a roll.

  The estimate holds for the first and is a little high for the second. These bot games have little combat (394 draws in 4,219 steps),
  so the absolute numbers are small.
* No in-game shuffle happened, as the theory says.

What was **not** measured:

* the R1 / R2 split (both count as dry rollbacks);
* R3 as its own number. The prefix re-draws of a resumed run are committed by that run, so they are not a revert;
* R4 on the thin game (`legal_options` was not called; `Game::step` does not call it);
* draws per roll group;
* combat draws apart from other draws;
* any of this for the `SeededRandom` or human tables.

## Not done / for later

* The doc's T1 at the **thin** level: resuming a `Game` from a mid-step `(core, after_glue)` clone. That would trip V1/V2/V3 by design.
  `Game` has no API to continue from inside a step (V4), so writing it means adding one; that belongs to lane C5.
* Cheap checks that passed:
  * `clippy_core.sh` (rc 0, engine warnings 41 = baseline);
  * `cargo test -p ti4-engine-core --features probe-counters --lib driver` (72 passed);
  * the `state::rng` unit tests;
  * `check_layers.py core`.
* No heavy gates were run (by brief): no `end_of_d1`, dual, soak, bench or mutation ratchet. No wasm build was made. The feature is not
  in any production dependency edge, so the wasm size cannot change.
