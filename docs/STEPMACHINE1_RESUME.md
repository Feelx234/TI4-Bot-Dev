# stepmachine-1: the strategy card's ability as a parked step machine (core-stepmachine-1, 2026-11-06)

Branch `core-stepmachine-1` from `core-nestq-1` (`3cb3e9ad6`). Not pushed, not deployed, live branch untouched, no heavy gate run.
Targets: `/root/deploytmp/sm1` (gate build), `/root/deploytmp/sm1-dev` (debug tests), `/root/deploytmp/sm1-count` (gate + `perf-count`),
base build of `core-nestq-1` from a `git archive` in `/root/deploytmp/sm1-basesrc` -> `/root/deploytmp/sm1-basetgt`.
Background: `git show core-theory-nested:docs/NESTED_OPS_THEORY.md`, `docs/NESTQ1_RESUME.md`.

**Question.** Is keeping a nested question's state as data (where the operation stands plus what it has done so far, held in `Pending`,
resumed without replaying the prefix) more efficient and practical than replay plus nestq, especially for the frequent questions?

**Short answer.** It is more efficient: -13.5 % instructions on the 14-game suite for one converted path. The conversion is exact: same
hashes, and every parked answer was checked against the replay in a whole-suite run. It is practical for the strategy-card perform
only, because that path already has the shape a step machine needs. Recommendation: **keep it for the top card paths (this one), do not
convert further families now**.

## 1. What was converted, and how

The questions counted by path are in section 3.2. The strategy-card child loop alone (`driver/windows/strategic/perform.rs`) is 58 % of
all nested answers and 98 % of the F1/F2 questions of the theory. The top paths are Leadership primary `GainCommandToken`, TE Warfare
secondary `Production(Choose)`, Politics primary `PlaceAgenda`, TE Warfare secondary `Pay`, Technology primary `ResearchTechnology`, and
Leadership secondary `Pay`. Every card goes through the same loop, so converting "one card" and converting "all cards" is the same code.
The whole loop was converted, for both of its converted callers:

* **The follower's secondary.** `SecondaryWin::answer` -> `follow` -> perform. The op is the window's answer; afterwards, `resolved`.
* **The primary.** `begin::run_begin` (`TurnJob::Strategic`, or the menu's `TurnJob::Action`) -> perform. The op is the job's step;
  afterwards, the new `begin::after_primary` (the rest of `run_begin`, split out without change).
* The third caller, the agenda Crisis Pact (`agenda/crisis.rs`), stays on replay (`Caller::Inline`).

**The parked state** (`driver::Park`, plain data, `Clone`, no closures, no futures):

* `state`: a `CoreState` clone at the question.
* `resolver`: a `Resolver` clone.
* `next_id`, `emitted` and `opened`: what the driver had at the question.
* `at: Resume::Perform(Parked)`. `Parked` holds:
  * `frame`: who called the perform: `Secondary(Box<SecondaryWin>)` (the follower window as it stood mid-answer), or `Primary`.
  * the loop's locals `Perform { who, card, primary, trade_primary, trade_source, child: ErasedChild }`.
  * The child is the card's own window, already a stage machine, cloned through `ErasedWindow`.

**Where it lives.** The park is NOT a hashed or codec part of the position. It rides on the nestq memo: `live::Asked.park`, inside
`Nested::asked: Memo`. That is the only correct place:

* The position at a nested question must stay what replay leaves (the op's start plus `Pending::nested`). Otherwise the chain, the state
  hashes, the bot's view and the codec would all change.
* A mid-op `CoreState` cannot be a hashed field of a `CoreState`.

**How it is trusted.** It is trusted exactly when nestq trusts its memo: same window kind and `asked.at == *core`. Without a park, or in
another position, the answer replays as before, so replay is the always-correct fallback. Forking: a clone shares the park (one
reference count, as the memo already did); each copy resumes on its own clones.

**Mechanics.**

* *Capture.* `perform::drive` (the old loop, now a function over `Perform`) finds the tape dry at its child's question (`Tape::exhausted`
  before the ask). If the caller is converted, no emission is open (`resolver.emission_stack()` empty) and nothing is in flight, it puts a
  `Park` into `Tape::parked` (a `Memo`, layer L3 carries it, L5 fills it).
* *Hand-off.* `Driver::settle` takes it before its rollback (`take_park`). It keeps it only if it `fits` the op the driver ran: `Answer`
  of a `SecondaryWin`, or `Step` of a `TurnFlowWin` on a strategic job. `live::suspend` stores it with the memo.
* *Resume.* `live::answer_nested` checks the pick against the remembered question and calls `Driver::resume_parked`:
  * It installs the parked state, resolver, counters and window, and puts the tape at "all answers read".
  * `as_called` writes into the parked state what the suspending call wrote at its END. These are the window payload (`Held::to_pending`)
    and `pending.resolver` (`Driver::store`). It is the same correction nestq's `as_replayed` makes: replay starts from the position that
    call LEFT, the park from the one it began with. The first build without it failed the check at once (`pending.resolver.initiative`
    `[]` against `[Seat(0)]`).
  * It runs `Parked::resume`: `child.answer(pick)`, the loop on, then the caller's tail.
  * After that it behaves like `settle`: a dry question re-parks and rolls back to the call's position; `Answered` steps on through
    `settle(Op::Step)`; `Stepped` is `Done`.
* *Switch.* `driver::set_parking(false)` (`PERF_KERNEL_PARK=0`) turns capture and use off, so one build can compare the two paths.
* *Check.* `live::verify_parked(true)` is on by default in debug builds, and `PERF_KERNEL_VERIFY_PARK=1` turns it on in any build. Every
  parked answer is also answered by the replay, on a scratch copy with a fresh driver. The two must agree in:
  * status, position, window payload;
  * emitted events, journal, opened windows;
  * the resolver's persistent part;
  * the next suspension, its display facts and its display position.

  A difference panics and prints the first differing line.

**Rules kept.**

* `check_layers` core / thin / setup: 0 violations each.
* `lint_core`: 0 hits (one `lint-allow`, on the panic message of the check).
* `clippy_core`: ok, engine warnings 41 = baseline.

The `Window` trait gained `Send + Sync + 'static` and `ErasedWindow` `Send + Sync`, so that a park can live in a `Memo`. Every existing
window already satisfied them; no impl changed.

## 2. Exactness and tests

| check | result |
|---|---|
| `native_play --quick --shadow --twice` | fingerprint **`538dc68f23dddcc9`**, deterministic, shadow compared 808, ZERO GAP |
| `perf_kernel suite 1` | `suite_hash` **`775f4136e60a99d5`** (parking on, off, and on with the check) |
| `perf_kernel game 4 1 sol` / `game 6 2` | chain `c339cb8ece4c2187` / state `1e0e3a049d9d06e2`; chain `46901c63b044afc8` / state `237bcb05b1339dcb`, the same as the nestq build |
| whole-suite check (`PERF_KERNEL_VERIFY_PARK=1`, release) | 3,353 parked answers, each equal to its replay in every field above: no panic |
| debug: `ti4-engine-core` lib, `ti4-engine-core-tests`, `ti4-engine-core-factions` (check on) | 2,714 passed, 0 failed |
| thin crate `ti4-engine --lib live::` | 22 passed |
| new `crates/ti4-engine-core-tests/tests/stepmachine.rs` | 5 passed |

The new tests:

* `a_followers_ability_forked_at_every_question_continues_identically_and_equals_the_replay` (Leadership followers, 6 questions).
  * At EVERY nested question the position is forked. The original and a fork answer the same; a third copy answers by the replay
    (parking off). All three must give equal `(Live, Out)` and equal positions.
  * A fourth copy takes another option. It must equal its own replay, and the original must be untouched.
* `a_primary_forked_at_every_question_continues_identically_and_equals_the_replay`: the same for the Leadership (7 questions), Trade (3)
  and Politics primaries, through `begin_strategic` and the job arm.
* `an_illegal_pick_to_a_parked_question_is_refused_and_the_park_stays_usable`:
  * `Number(99)`, `PrimaryInstead` and `Done` are refused with `Illegal` and the position is unchanged.
  * The legal pick then resumes the park (`park_stats` +1) and equals the replay.
* `a_park_is_shared_by_a_clone_and_each_copy_resumes_on_its_own`: three copies of one suspended position answer three options, each
  equal to its replay.
* `the_size_of_a_park`: prints the inline sizes (section 3.3).

The fork tests of `core-fork-tests-1` (`fork_core.rs`, c3088542e) were not cherry-picked. They need the `probe-counters` census, which is
not on this line. The tests above fork at every question of the converted path, which is the part this change touches.

## 3. Numbers

### 3.1 Instructions (`perf stat -e instructions:u`, gate profile, whole process; `PERF_KERNEL_NOSTATE=1` for the games)

| run | nestq build (`3cb3e9ad6`) | this build, parking off | this build, parking on | on vs nestq |
|---|---|---|---|---|
| `game 4 1 sol` | 584.8 M | 583.3 M | **507.6 M** | **-77.2 M (-13.2 %)** |
| `game 6 2` | 780.5 M | 780.8 M | **700.5 M** | **-80.1 M (-10.3 %)** |
| `suite 1` (14 games) | 8,105.8 M | 8,103.0 M | **7,007.5 M** | **-1,098.3 M (-13.5 %)** |

* Counts repeat to 0.01 %.
* Between builds, codegen moves the totals by up to 2 %. An intermediate build of the same logic measured "off" 2 % below nestq and "on"
  at 6,835 M. The final build's "off" equals nestq.
* The in-build difference (on vs off) was about -1,090 M on the suite in both builds.

**Runs.** No run is saved by count: an answer is still one run. The work saved is the prefix that run no longer replays. Before, it
re-ran from the start of the op: the follower's answer, `follower_facts`, `perform_facts`, `fx_perform::begin`, and the child's earlier
stages and answers. Now it starts at the question. `probe_calls` stays 7,400 in the quick game.

**Shares.**

| | |
|---|---|
| the theory's measure (wall, before nestq) | non-committing runs 26 % of play; re-ask 13.1 % (removed by nestq); discovery 13.2 %; F1+F2 74 % of the overhead |
| measured here (instructions, after nestq) | the conversion removes 13.5 % of all suite instructions, setup and env included, so a larger share of play |

The conversion removes more than the theory's "discovery runs of F1+F2", because it also drops the prefix of the COMMITTING run of each
op. Inferred, not counted: if F1+F2 are 74 % of what is left after nestq, the remaining replay overhead is about 4-5 % of the suite's
instructions.

### 3.2 Coverage (`PERF_KERNEL_RUNS=1 perf_kernel suite 1`, the new per-path counter)

All 5,777 nested answers of the suite: the same number as the theory doc.

| | answers |
|---|---|
| **parked** (resumed without replay) | **3,353 (58 %)**: primaries 2,139, secondaries 1,214 |
| replayed, turn-flow jobs | 2,170 |
| replayed, tactical action | 228 |
| replayed, combat / invasion / production / aftermath / follower window | 26 |

Top parked paths:

| path | answers |
|---|---|
| Leadership/primary `GainCommandToken` | 339 |
| TE Warfare/secondary `Production(Choose)` | 315 |
| Politics/primary `PlaceAgenda` | 194 |
| TE Warfare/secondary `Pay` | 151 |
| Technology/primary `ResearchTechnology` | 149 |
| Leadership/secondary `Pay(Influence)` | 131 |
| ConstructionTE/primary `Choose` | 105 |
| DiplomacyCodex/primary `ChooseSystem` | 104 |

Top replayed paths:

| path | family | answers |
|---|---|---|
| `StatusRedistributeTokens` | F4 status finish | 496 |
| `Tactical(RemoveUnit)` | fleet sweep, F3 | 232 |
| `DiscardOverHandLimit` | F4 | 231 |
| `PredictiveIntelligenceRedistribute` | F3 | 208 |
| `Reaction(Window SystemActivated After)` | F9 | 133 |
| TE `WarfareRedistributeTokens` | after the free tactical action, the `AfterTactical` job | 103 |

About 70 F1/F2 questions stay on replay. They are reaction windows inside the perform (`StrategicActionBegan` When, emissions of the
child) and Peace Accords: questions inside an emission, which a park cannot hold.

Quick game (`game 4 1 sol`): 302 parked of 473 nested answers.

### 3.3 State size and cost

| item | cost |
|---|---|
| a park, inline | **3,192 bytes**: `CoreState` 2,928, `Resolver` 176, `Resume` 32, plus the vectors. It sits in one `Arc` inside the memo; the memo in the position stays 16 bytes. |
| a park, heap | the `CoreState` clone shares its `Cow` slices (pointer copies). Unshared: the resolver's log, journal and used keys, `emitted`, the boxed child window and the boxed follower window. |
| **allocations** (`perf-count`) | quick game +2,448 (+1.7 %, about 8 per parked answer); suite +18,490 (+0.9 %, about 5.5 per parked answer) |
| **bytes allocated** | -0.4 MB quick game, -8 MB suite (-1 %): the prefix replays allocated more than the parks do |
| **clone of a position holding a park** | one reference-count increment. Nothing is copied until a copy answers. |
| per resume | one `CoreState` clone and one `Resolver` clone out of the park. The replay's own call snapshot is still taken. |

### 3.4 Draws reverted

Not measured: the `probe-counters` census is on `core-fork-tests-1`, not on this line. Expected unchanged. Strategy-card performs draw no
dice (theory 1.2: "dice: no" for F1 and F2), so none of the reverted draws (63 %, 948 draws in 14 games) come from the converted path.

### 3.5 Code

| part | lines |
|---|---|
| generic machinery | 245 |
| ... `driver/mod.rs`: `Park`, `Resume`, `take_park`, `resume_parked`, the switch | ~165 |
| ... `kernel/ability.rs`: `Tape::parked`, `set_consumed`, `exhausted` | 18 |
| ... `driver/live.rs`: the park in `Asked`, `answer_nested`, the replay check | ~60 core + ~50 check |
| path-specific, `perform.rs` | +170 / -40: `Caller`, `Frame`, `Parked`, `Perform`, the loop as `drive`, `park`, `Parked::{fits, resume, key}` |
| path-specific, `begin.rs` | +6: `after_primary` split out |
| path-specific, `SecondaryWin::follow` | 1 line |
| counters, bench only | ~60: `park_stats`, `park_detail`, `PathKey`, `perf_kernel` env and printout |
| tests | 213 |
| **total** | 566 insertions, 47 deletions in 9 files, plus the test file |

Call sites changed:

* 2 callers of `perform::run` (`run_from` with a `Caller`); the third is unchanged.
* 1 shared function, `answer_nested`, used by all four live arms; the four arms are untouched.
* 2 trait bounds (`Window`, `ErasedWindow`). `suspend`, the arms and the `Window` impls are unchanged.

This fits the theory's estimate of 400-700 lines.

## 4. Verdict

**Efficient: yes.**

* -13.5 % instructions on the suite on top of nestq, -10 % to -13 % on single games.
* For each question, about 8 more allocations and 1 % fewer bytes.
* Zero extra cost to clone a position.

**Practical: for this path, yes.** It was cheap here because the path already had the right shape:

1. **One chokepoint.** Every card's ability is the same loop over an `ErasedChild`, already a stage machine and already clonable.
2. **The caller's frame is tiny and known.** Two callers, each with a short tail and few locals: `who`, `card`, `primary`, the follower
   window, `free`.
3. **The question is at the top of the op.** No emission is open, so no resolver stack or effect closure is suspended. A question inside an
   emission (a reaction window, an `Eff::ask`) cannot be parked: its state is the Rust stack of `run_window` and the `resolve` closure.
4. **No dice between the questions.** Even if there were, the faces are in the parked state, so a park needs no re-draw.

What makes a path convertible: questions asked by a child-window loop at emission depth 0 of the op; few locals between them; a caller tail
that can be split into a function; one window type per op (`fits`).

**Next families, honestly.**

| family (theory share of overhead) | assessment |
|---|---|
| **F4 status finish** (2.5 %) | The single largest replayed question (`StatusRedistributeTokens`, 496) is a `for seat in order { inline::run(Redistribute) }` loop: easy (position = the seat index, the child as data, the tail split once). But the prefixes are cheap, so it is worth about 1 %. |
| **F3 turn finish / end** (5.5 %) | The fleet sweep (`EnforceAll`), Minister of War and the legendary planets are `inline::run` children, but `ACTION_COMPLETED` and its reaction windows are emitted before them. The fleet-sweep questions are convertible; the reaction ones are not. Each `inline::run` call site needs its own resume point in the job: a job becomes an enum of phases with the locals between them. Medium. |
| **F9 tactical action** (5.5 %) | Mostly reaction windows of activation and movement emissions (`SystemActivated After`, 133): not convertible without CPS of the resolver. |
| **F6 leaders** (3.8 %), **F7 action cards** (3.4 %) | Per-leader / per-card job functions with several inline windows and emissions between them; the in-window plays (F7) sit inside emissions. A few component flows are convertible, one by one, each with its own `Frame`. |

At most 4-5 % of instructions is left to win, spread over about 75 `inline::run` sites and the resolver. The pattern does not pay there.

**A reusable helper, if more is converted.** The generic part already exists: `Park`, `Resume`, `resume_parked`, the memo trust, the
`as_called` correction and the replay check. A new path adds a `Resume` variant with `fits` / `resume` / `key` and a `park` call at its
loop's ask. The natural next generalisation is a parkable `inline::run_parked(sx, child, tail: Tail)`:

* the child stored as a `Box<dyn ErasedWindow>`;
* `Tail` an enum of job phases, plain data with the locals.

It would serve F4 and the fleet sweep. It does not help any question inside an emission.

**Risks.**

* *Two paths to keep in step.* Any change to the perform loop or its callers must keep `drive`, `Parked::resume` and the tails in step.
  The debug check runs in every debug test, and `PERF_KERNEL_VERIFY_PARK=1` covers whole games; both catch a slip at once (the first slip,
  the stored resolver, was caught by the existing strategic tests).
* *Hidden state.* The park freezes values the replay recomputes from the call position: the play-facts table and `sync_clock`. Both are
  functions of a position the memo pins (`asked.at == *core`), so they cannot differ. Anything outside `CoreState` that a caller changes
  between calls (another `Env`) is the same open risk nestq already noted. Two things the replay would rewrite are corrected by
  `as_called`: `pending.resolver` and the window payload.
* *Debugging.* A resumed run starts mid-function. Turning parking off reproduces any game on the replay path, and the check prints the
  first field that differs.
* *Determinism.* The park holds the rng streams of the moment, so nothing is redrawn. The hashes are unchanged in all three modes.

**Forking / MCTS.**

* Forking a position at a converted question costs a clone (one reference count for the park). Each child then pays only the work
  after the question, not the op's prefix. For a search that branches on the questions inside the strategic action, the per-node cost
  drops by the prefix.
* The park is not in the codec. A save at a nested question is refused as before (FORK_RNG V5), so saves are unaffected.

**Randomness design (always-ask core).** A chance ask is a nested question inside the combat or effect op that rolls.

* A step machine makes it cheap ONLY where the roll is asked by a child loop at depth 0 of its op: the combat machine's `ask_tape`,
  Munitions, `run_on_tape` production children.
* A roll asked inside a resolver window or an ability effect (`Eff::roll`: Strike Wing, Infantry II, Arborec) is inside an emission and
  stays a replay, exactly like the reaction questions here.
* So a parked chance path would need its own `Resume` variant per combat child loop: small, but it only pays once Search mode exists
  (theory 3.2 (b)).

**Against replay + nestq alone.**

* nestq gave -11 % (one run less per answer) at low risk. This prototype gives a further -13.5 %, for about 245 generic and 180
  path-specific lines.
* The cost is a second execution path for one module, guarded by an always-on debug check and an exact replay fallback.
* For interactive play neither matters (well under 1 ms per answer). For self-play and search throughput the gain is real.

## 5. Recommendation: convert only the top card paths (this one), and stop there

* Keep this conversion if self-play or search throughput matters; land it after a heavy gate (end_of_d1, dual, soak; none run here).
* Do not convert the turn-flow jobs, the tactical action or the resolver windows now. That would be 4-5 % at most, over many call sites.
  The status-finish redistribute loop is the only cheap further candidate, at about 1 %.

**For the user to decide.**

1. Land this (a second execution path for the strategy-card perform, guarded by the check), or keep replay + nestq only.
2. If landed: should the release check (`verify_parked`) also run in the nightly sweep? It is cheap enough for one game.
