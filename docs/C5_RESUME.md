# Lane C5: thin step state, the `Game` forkable at every question (resume)

Lane C5 of `docs/FORK_RNG_THEORY.md` (branch `core-theory-fork-rng`, sections 1.3-1.6 and 5). Branch `core-search-thin-1` from
`core-integration-28` (`f22ece5e8`). Not pushed, not deployed, live branch untouched, no heavy gate run (by brief).

## What changed

| Finding | Fix |
|---|---|
| **V1** `Native::after_glue` outside the position | The flag is a bit of the position: `DriverFlags::AFTER_GLUE` (`ti4-engine-core/src/state/pending.rs`, L1, no rule reads it). The thin game reads it at the start of a step and writes it at the end (`game/mod.rs`: `after_glue` / `set_after_glue`). `Native::after_glue` and `NativeSnapshot::after_glue` are gone: a core clone alone says how the next step begins. The step's error rollback restores it with the core. |
| **V2** staged events cleared after the whole run | The drop stays where the blocking step has it (the end of each run, `run_live`), and the cut step drops them at the same place: a copy cut inside a run drops them when that run ends, never at the start of a fresh step (the case the theory describes cannot happen: a copy at a question resumes the run, it does not start a new step). Moving the drop into the core was not done: the core reads staged events within a run (`drain_staged`, the Arborec hero), so not staging at all is not equivalent. |
| **V3** events rendered from a step-local mark | The mark is part of the cut step's data (`Begun::from`): a copy renders the whole step's events. |
| **V4** no mid-step API | Feature `search` of the thin crate: `Game::pull()` runs a step to the first question the table's decider would be asked and returns it (`Pulled::Asks(choice)`), or the whole step (`Pulled::Stepped(StepResult)`, as `step()` reports it); `Game::answer(id)` validates and journals the answer and runs on. Between the calls the step is data (`game/mid.rs`, `Mid`), carried by `Game::fork` / `snapshot` / `fork_keeping_log` (the FORK RULE destructuring lists it). Also: `pending_question()`, `Game::replayed(game, answers)` (the load of a game saved at a question), `forget_memos()` and `position_differences()` (test helpers). |
| **V5 / V5b** the codec drops `Pending` fields | **Proven rebuildable, not put in the codec** (see "Saves" below). |

### How the cut step works (`game/mid.rs`)

The blocking step (`native_step`) keeps in its stack frames: the step's `Env`, the reconcile counter and `left_open`, the arm, the run's
loop and its question, the run's combat texts, the logs of the ability table (skipped seats, single-way payments), the asker's `hidden`
flag, the arm's question count, the rollback data (core, code table, journal length) and the events' mark. `Mid` writes each of these
down (table in the module doc). The `Env` is not stored (it holds an `Rc` and borrows): it is rebuilt per call from the position the step
began with (`Begun::start`), which is what the blocking step builds it from; the render's cache makes that a copy.

`live::pull::{begin, answer}` is `live_step_inner` cut at the asker (the same calls in the same order). The asker's own settlements (lone
draft card, a planning fork's declined reaction, a question with no option) run through `TableAsker::screen` (a copy of the first half of `TableAsker::ask`); the table part is
`Table::present` (count, invasion context, agenda details, session callback: what `ask_seeing` does before the decider) and
`Table::settle_pulled` (validate, journal). An answer that is no option leaves the game unchanged (`GameError::IllegalChoice`); a failed
step restores the position the step began with, as `step()` does.

### Production code touched (feature off)

Only V1: the glue flag in the position (`after_glue` / `set_after_glue` in `game/mod.rs`, the `Native` / `NativeSnapshot` field gone, the
`mid` field and the `step()` / `legal_options()` guards behind `search`). `native_step`, `step_native`, `step()`, `TableAsker::ask` and
`Table::ask_seeing` are otherwise as on int-28.

A first version split those functions into shared helpers (`step_env`, `choose_arm`, `commit`, `publish`, `TableAsker::screen`,
`associate`) that both paths called: +896 B raw / +364 gzip / +382 brotli of wasm (`Game::step` +720, `TableAsker::ask` +133; even with
`#[inline(always)]` on every helper still +841 B). So the helpers are now the cut step's own copies behind `search` (each marked as such),
and the blocking step keeps its code inline. The copies are guarded by `tests/fork_mid.rs` (every question of 14 games, against the
blocking step).

Everything new (`mid.rs`, `live::pull`, the helper copies, `Table::present` / `settle_pulled`, the `mid` field) is behind `search`; no
production crate turns it on (`check_production_graph.sh`: 0 violations).

## Saves at a question (V5 / V5b)

A save of the product is a replay (seed + journal). `Game::replayed(game_at_opening, answers)` replays the journal through `pull` /
`answer` and stops at the first question after the last answer: the window stack, `Pending::nested`, the remembered question and the parked
strategy step (`Nested::asked`, nestq-1 / stepmachine-1), the V5b fields (`driver` with the glue flag, `prepared_turn_seq`, `turn_closing`,
`actions_this_turn`, `staged`, `failed_*`) and the cut step's own data are all rebuilt by the replay. The test loads at every 150th
question and requires the loaded game to equal the original (core `==`, the cut step field by field, the question, events, rolls,
fingerprint) and to play on identically.

The memos (`Nested::asked`: nestq memo and park) are derived: a copy with them forgotten (`forget_memos`) answers to the same game (tested
at every question). So nothing of them needs a codec.

What stays true of the codec (`core_sync::codec`): `Codec::load(materialize(core))` still drops the window stack, `nested` and the V5b
fields (C0's T2 strict test stays `#[ignore]`d, known V5b). A codec save at a question (or a distributed search that ships `GameState`)
is therefore still not supported; the way to ship a position is the journal (O(game) to load) or, later, a serde of `CoreState` (lane C9,
not in the wasm). Storing the V5b fields in `GameState` would change the materialised JSON and so the fingerprint, which the brief keeps.

## Tests

New: `crates/ti4-engine-core-bridge/tests/fork_mid.rs` (dev-dependency `ti4-engine` with `search`). Per game, at EVERY question of the
pulled game:

1. a copy answers ANOTHER legal option (the first one the bot did not pick) and plays to the end of the step and 2 more steps; an
   independent replay from the step-boundary snapshot through the BLOCKING `step()` (scripted with this step's answers so far and the other
   option, then the bot) must agree after each step in: the step result (errors included), fingerprint (position JSON + journal chain),
   all events, rolls, step count, observation latch (randomness, hidden information), every question shown to the deciders, and
   `position_differences` (core `==`, the cut step field by field);
2. a copy with the core's memos forgotten answers the same other option: equal to copy 1 at the end of the step;
3. a copy answering the original's answer: equal to the original at the end of the step;
4. the original, compared with a copy taken before 1-3 played, is unchanged;
5. every 150th question: saved and loaded (`Game::replayed`), equal, and one step on equal.

The original then answers with the bot; at the end its fingerprint and step count equal the game played with the blocking `step()`.

| Test | Result |
|---|---|
| `the_quick_game_played_by_pull_is_the_blocking_game` | pass: blocking `538dc68f23dddcc9`, 1,165 steps; the pulled game the same |
| `the_quick_game_copied_at_every_question_continues_as_its_replay_and_the_original_is_untouched` | pass: 1,223 questions (1,140 with another answer, 492 inside a step after its first question), 3,668 steps compared, 8 loads; 28 s (lockstep) |
| `the_suite_copied_at_every_question_continues_as_its_replay` (`--ignored`, the 14 games of `perf_kernel suite`) | pass: 14 games, 15,357 questions (14,093 with another answer, 5,740 inside a step after its first question), 46,052 steps compared, 96 save/loads, 0 copy errors; each game ends with its blocking fingerprint; 7.5-8 min (lockstep) |

Negative controls (mutations in the copy, then reverted): the events' mark shifted -> fails at the first fork ("events 1 vs 2"); the
skipped-seat log dropped -> fails at "the original after its copies played: the positions differ in [skipped]". A flipped `left_open` of a
reconcile run was not caught in the quick game (no question there that is followed by a run whose arm choice depends on it).

Run:

```text
CARGO_TARGET_DIR=... cargo test --profile lockstep -p ti4-engine-core-bridge --test fork_mid -- --nocapture             # quick game, 2 tests
CARGO_TARGET_DIR=... cargo test --profile lockstep -p ti4-engine-core-bridge --test fork_mid -- --ignored --nocapture   # the suite (C5_GAME=4-1-sol for one)
```

Gates: quick-game fingerprint `538dc68f23dddcc9` (1,165 steps; blocking and pulled); `perf_kernel suite 1` `suite_hash 775f4136e60a99d5`
(14,738 steps); `check_layers.py` core / thin / setup 0 violations; `check_production_graph.sh` 0 violations; `clippy_core.sh` ok (engine
warnings 41 = baseline); `lint_core.sh` 0 hits; `cargo test -p ti4-engine --lib -- live:: native:: core_sync::` 109 passed; `cargo check`
of the thin crate with and without `search` clean. Not run (by brief): the heavy gate, `ti4-server` tests (`fork_equivalence` there
covers step-boundary forks; the glue flag now rides in the core it clones), dual / soak / bench / mutation.

## Costs

| | int-28 (`f22ece5e8`) | this branch |
|---|---|---|
| wasm, `wasm-release` + wasm-opt with the `build-wasm.mjs` flags (raw / gzip 9 / brotli 11) | 6,624,713 / 2,106,859 / 1,516,834 | **6,624,716 / 2,106,858 / 1,516,801** (+3 / -1 / -33 B) |
| budget | 6,630,000 / 2,110,000 / 1,520,000 | margins 5.3 / 3.1 / 3.2 KB, as before |

Both measured here from clean builds in one target dir (`/root/deploytmp/c5-wasm`; the int-28 number equals `INTEGRATION28.md`'s real
build). Speed: the blocking step is the int-28 code, so the product's speed is unchanged (not measured). The cut step rebuilds the step's
`Env` per call (a cache hit: a registry and table copy) and renders, materialises and journals each question as the blocking step does;
a copy (`fork_keeping_log`) is the step-boundary fork plus the `Mid` (a few `CoreState` clones, all `Cow`-shared). The suite test (15,357
questions, 4 copies each, 46,052 compared steps) takes about 7.5 min in the lockstep profile.

## Remaining / open

* The codec still drops `Pending` (V5/V5b); saves at a question are replays. A serialising search needs lane C9 (serde of `CoreState`).
* `Game::fork` stays O(game length) (events, rolls, two `GameState`s) and cold (`Render` rebuilt): the search should fork at the core
  level (lane C6). `mid.rs` is the thin model for C6's pull API: C6 can reuse `live::pull` and the `Mid` layout without the `GameState` mirror.
* A copy's table starts with an empty per-step repeat counter (`Table::asks_this_step`, the `NoProgress` guard after 500 identical asks):
  a copy may allow up to 500 more repeats in the step it was cut in. Not game state.
* `Game::fork()` (without the log) of a cut step: a failure of that step truncates the copy's journal to empty (the records before the
  cut are not in it). `fork_keeping_log` is exact.
* `legal_options()` at a cut step returns the pending question (as shown to the decider, with its invasion context).

## Join notes

* `core-chance-2` (randomness as a question): a chance ask is one more `Live::Asks`; in the cut step it would be a `RunStop::Asks` whose
  answer comes from the resolver in Play mode. `live::pull::answer` takes an option id; a chance answer needs its own entry (or
  `pull::answer` generalised to a `Pick`). Conflicts expected only in `live/mod.rs` (the new `pull` module sits after `live_step_inner`)
  and `game/mod.rs` if C3 touches `native_step`. **Rule for every lane: a change to `native_step`, `step_native`, `step()`,
  `TableAsker::ask`, `Table::ask_seeing` or `live_step_inner` must be made in its `search` copy too** (`commit` / `choose_arm` /
  `step_env` / `publish` / `observe` in `game/mod.rs`, `screen` / `shown` / `answered` in `asker.rs`, `associate` in `choice.rs`,
  `live::pull`, `mid.rs`'s `run`); `tests/fork_mid.rs` fails when they drift.
* `core-multiask-1` (multi-pick asks): `pull::answer` mirrors `live_step_inner`'s answer path (`trade::pick` / `pick_of`); a multi-pick
  change there must be made in both (or `live_step_inner` rewritten over `pull`).
* The core change is one constant (`DriverFlags::AFTER_GLUE`, bit 3). `perf_kernel` and `native_play` keep their own `after_glue`
  (they never set the bit), so `suite_hash` is unaffected.
