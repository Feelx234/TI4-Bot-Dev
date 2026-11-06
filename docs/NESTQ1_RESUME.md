# nestq-1: the core remembers the nested question it gave out (core-nestq-1, 2026-11-06)

Branch `core-nestq-1` from `core-heavyfix-1` (`b7ac383a9`). Not pushed, not deployed, live branch untouched. Private targets
`/root/deploytmp/nestq1` (gate builds) and `/root/deploytmp/nestq1-dev` (debug tests). Background: R2 and R4 of
`docs/FORK_RNG_THEORY.md` (branch `core-theory-fork-rng`), "the reask cost" table of its section 4.5.

## 1. First check: does `resume` need what `restore_nested` + `reask` leave behind? No.

Read: `Driver::restore_nested` / `reask` / `resume` / `settle` / `restore_stage` (`driver/mod.rs:186-336`), `Tape`
(`kernel/ability.rs:1154-1248`), `Resolver::checkpoint` / `restore` (`kernel/resolver.rs:592-640`), the table's side logs
(`kernel/ability.rs:1011-1014`, drained in `ti4-engine-next/src/game/mod.rs:946-1002`), the lazy play facts (`kernel/ability.rs:555-600`).

`reask` is `settle(op)` on a driver whose `awaiting` is `Nested(op)`; the run finds the tape dry at the same question and `settle`
rolls everything back (`driver/mod.rs:225-239`). What survives that rollback, field by field:

| state | after `restore_nested` only | after `restore_nested` + `reask` | does `resume` read it? |
|---|---|---|---|
| `CoreState` | the call's | `*s = snapshot.0`: the same | yes, equal |
| window, `emitted`, `opened`, `next_id` | from the position | restored from the snapshot | yes, equal |
| resolver | `from_state` + clock | `restore(checkpoint)`: registry truncated to its mark, log / journal / used keys truncated, clock, stacks; `base_depth` is set at construction only (`resolver.rs:436`) | yes, equal (a `Cow` registry may be unshared: no semantic change) |
| `awaiting` | `Nested(op)` | `Nested(op)` again (`mod.rs:238`) | yes, equal |
| tape answers | the stored ones | the same | `resume` pushes the pick and `settle` rewinds (`used`, `needed`, facts, state, `unknown_at_dry` cleared) |
| `tape.unknown` (a `Cell`) | empty | taken by `take_unknown` at the end of the re-run | empty either way |
| `self.unknown` | `None` | the re-run's (a reason noted before the question) | the resume re-runs the same prefix and notes the same first reason (`CondCx::unknown` / `Eff::unknown` keep the first, `ability.rs:186`, `:1373`): the call fails with the same `LiveError::Unknown` |
| `facts` / `facts_state` | `None` | the re-run's | overwritten when the resume stops at a question; unused when it ends (`asking_of` is built only for `Asking`) |
| `table.skipped` / `table.auto_paid` (shared logs) | | the re-run logs its prefix and its phantom rest again | the wrapper dedupes both (`ti4-engine-next/src/game/mod.rs:992`, `:1001`); inside one blocking step the discovering run logged the same entries first, so nothing changes; a pull-style caller loses only the phantom rest's duplicates |
| lazy play facts | | may be computed by the re-run | a function of the call's start position alone (`LazyPlays`): same value |
| stage probe | | one more call | test seam only |

So `resume` is independent of the re-ask. What the re-ask is FOR is the legality check of a live pick (`live.rs:195-201`), which needs
only the question. That question was already built once, by the run that suspended the step.

## 2. The change

* `state::Memo` (`state/pending.rs`): an `Option<Arc<dyn Any + Send + Sync>>` that is equal to every memo, printed as `Memo`, and not in
  any codec; `Nested` gets `asked: Memo` and a hand-written `Debug` that prints what the derived one printed. The state layer (L1) carries
  it; the driver (L5) fills it with its own type (`check_layers` core/thin/setup: 0 violations).
* `driver::live::Asked { kind, asking, at }`: the question the suspending call returned, the window kind, and the position the call left
  (suspension included). Written by `live::suspend` at the four places that wrote `nested = d.nested_journal()` (`live.rs`,
  `windows/live_common.rs`, `windows/tactical/live.rs`, `windows/invasion/live.rs`).
* `live::recall::<M>`: the memo is used only when `kind == M::KIND` and `at == *core` (the whole position; `Cow` fields compare by
  pointer first). Anything that wrote to the position after the call (the aftermath's `mirror_recs` after a nested question,
  `trade::pick` writing the deal terms, a `sync_codes` that added codes) makes it fall back to the old re-ask. A rollback restores a
  whole `CoreState`, memo included, so the memo always describes the position it is in; a clone shares it (one reference count) and each
  copy answers on its own.
* `live::answer_nested` (shared by the four copies): `restore_nested`, then the pick is checked against the memo's question (or, without
  one, against `reask` as before), `LiveError::Illegal` when it is no option, then `resume`.
* `live::ask`: with a memo, returns it (0 runs).
* The display positions of a READ (`Asking::nested.1`, `Asking::at`): the suspending run put the question in a position whose stack still
  held the call's START payload and resolver; a replay starts from the position the call left. `live::as_replayed` writes the window
  payload and the resolver of the position into copies of them (what `advance` writes at its end), so `ask` returns what the replay
  returned before. (Without it the check failed at once: `pending.resolver.initiative` `[]` against `[Seat(0)]`.) Done on a read only:
  an answer needs the question alone. The question `advance` itself returns is unchanged.
* `containers::Log` `==`: two logs that share their sealed chunks (a clone and its original) compare only the open chunk (exact: shared
  chunks are equal). Without it the position check of `recall` walked the whole ledger: the first build of this change was SLOWER than the
  base (600.8 M instructions against 577.8 M).
* Check mode (`live::verify_remembered`, debug builds only, on by default there): every use of a memo also rebuilds the question with a
  fresh driver on a scratch copy and asserts the choice and the display facts equal (a read: the whole `Asking`, display positions included). Every debug test runs with it.
* The three private copies of `is_option` (tactical, invasion, live_common) are gone; `live::is_option` is the one.
* `perf_kernel`: `PERF_KERNEL_RUNS=1` with `game` prints the stage-probe calls (one per run of an operation and one per stop).

## 3. Tests

New, in `driver/live.rs` (they use a test-only run counter `driver::RUNS`, incremented in `Driver::once`, with the check off):

* `a_remembered_nested_question_is_answered_with_one_run_and_read_with_none`: discover 1 run; `ask` after it 0 runs (was 1); the answer 1
  run (was 2: re-ask + resume); the same answer with the memo stripped 2 runs and the SAME position; a position changed after the call is
  not trusted (2 runs); a memo of another window kind is not taken; the memo leaves with the window.
* `an_illegal_pick_to_a_remembered_question_is_refused_and_the_memo_stays_usable`: three wrong picks (`pick(9)`, `Number(3)`, `Done`)
  give `LiveError::Illegal`, the position equal to before, the memo still recalled; a legal pick then resumes in 1 run.
* `copies_of_a_suspended_position_answer_independently`: one suspended position cloned; the original answers ability + decline, the fork
  declines: 1 and 0 trade goods, each equal to a memo-free straight run with those picks; ask-clone-answer equals ask-answer.

The existing tests run unchanged. Runs (all debug, so with the check on: every memo use is also re-run and compared):

| suite | result |
|---|---|
| `cargo test -p ti4-engine-core --lib` | 488 passed |
| `cargo test -p ti4-engine-core-tests -p ti4-engine-core-factions` (the driver / live / window tests and the factions lib) | 2,194 passed, 0 failed (on the final code) |
| thin crate `cargo test -p ti4-engine --lib live::` | 22 passed (run on the version before the `as_replayed` move; the move only changes `ask`'s read path, covered by the check in the two runs above) |
| `clippy_core.sh` | ok (engine warnings 41 = baseline) |
| `check_layers.py` core / thin / setup, `lint_core.sh` | 0 violations each, 0 lint hits |

## 4. Counters and instructions (`perf_kernel game 4 1 sol`, gate profile, `perf stat -e instructions:u`, `PERF_KERNEL_NOSTATE=1`)

| build | probe calls (`PERF_KERNEL_RUNS=1`) | instructions |
|---|---|---|
| base (`b7ac383a9` + the probe counter) | 8,226 | 577.8 M |
| base + `Log` `==` shortcut only | | 575.5 M |
| first try (memo, deep position check, display positions patched at every suspension) | 7,400 | 600.8 M |
| **final** | **7,400** | **513.9 M** |

826 operation runs fewer in the quick game (10 % of all runs): one per answered nested question, every answer hit its memo there (the
kernel bench renders no questions, so nothing syncs codes into the position between calls; in the product a code minted by the render
makes the next answer fall back). Instructions -63.9 M (-11.1 %), of which -61.6 M the memo and -2.3 M the `Log` shortcut on its own.
Same chain `c339cb8ece4c2187` and state `1e0e3a049d9d06e2` in all builds.

Pins: `native_play --quick --shadow --twice` fingerprint `538dc68f23dddcc9`, deterministic, ZERO GAP (unchanged); `perf_kernel suite 1`
`suite_hash 775f4136e60a99d5` (unchanged, run once).

## 5. Not run

Heavy gates (end_of_d1, dual, soak, bench, mutation); the debug check in a whole game (only the unit and window tests ran with it);
wasm size (not measured; a type, a few small functions, no table). `ask` with a nested question pending is not a product path in the
blocking loop (a step never ends with one pending), so the read saving matters for pull-style callers only; the answer saving is on the
product path. Open risk the check guards and the heavy gates would cover: a caller that hands the next call a different `Env` (the
memo trusts the position, not the environment).

