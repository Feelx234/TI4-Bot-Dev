# TODO D2-8: the wrapper, ordered work list D2-8b..g and what the flip needs of D2-9 (2026-11-01)

Theory document (read-only analysis: no builds, no tests; `git`, `grep`, `python3 -I` scripts over the tree, `git merge-tree`). Branch `core-todo-d28`, cut from `core-d27-split` @ `2014339a4`
(`core-integration-15` + the legacy split of D2-7). Only this file changed. The live branch `nightly-fixes-2026-10-26` was not touched.

**Labels.** *Measured* = counted in the tree for this document. *Cited* = from a document or a branch (named). *Estimated* = my judgement (agent-hours). **VERIFY** = a check the implementation
agent does first (cheap). **DECISION** = needs the user (section 8).

**Decisions of the user this file builds on** (brief of 2026-11-01): Architecture A (the core owns the loop; the legacy `Game` only as a bridge-only test harness, never in production); only
`RULES_VERSION` 35 is supported (older saves refused at creation, load and recovery: work item 21 of `docs/BAIL_MAP.md`, branch `core-bailmap`); `disable-xf123` refuses Last Bastion, Crimson,
Keleres, Deepwrought, Firmament, Obsidian, Ral Nel (unmerged); `product_init()`: library defaults off, product binaries on; the real `lend_table::BAILS` is empty; Thunder's Edge content stays
out (the TE *strategy card set* is in); the no-op `Role` comes last.

**Base facts (measured on `core-d27-split`).** `crates/ti4-engine` = package `ti4-engine-legacy` (375 files, 391 k lines); `crates/ti4-engine-next` = package `ti4-engine`, one line
(`pub use ti4_engine_legacy::*;`). The core is 112 k lines (`ti4-engine-core`) + 18 k (`-factions`), both free of any `ti4-engine*` edge. `live_arm` registers 19 of the 21 arms; the two
that are not are `Diplomacy` and `AgendaTalks` (dropped features). `RULES_VERSION` is 35 on this line and **27 on the live branch** (section 4.1).

---

## 0. Summary

**Verdict: GO for D2-8b..g, in six lanes (section 6). NO-GO for merging the flip into the live branch until the five gates of section 7.4 are green.** Total estimate **80-130 agent-hours**
(section 7), about 4 parallel lanes; the critical path is the type swap (8b), then the wrapper (8e) overlapping its tail, then the equivalence gates (8g) and D2-9: roughly 45-70 hours of sequential work. The old
estimates (30-45 h A / 53-81 h A-with-islands, `D2-remaining.md` 3.4, `D2_8_0_AUDIT.md` 6) assumed a lender and island holders; with Architecture A and a core that plays 94-96 of 96 sweep games
natively, the loop is mostly written (it is `NativeGame` in the bridge) and the cost moved to **the surface** (207 paths + `Observed`), **the setup** and **the product edges**.

Findings that change the plan (each is explained where it is used):

1. **`reverse_lend::with_string` has no production caller at all** (measured: only its own 11 tests), `lend_table::BAILS` is empty, `lent_close` returns `None`, `Step::Lend` is only the old
   dual's. A native game never lends. So the `lend-string` feature of D2-7 §2.3 guards almost nothing; the right design is *no lend code in the thin crate* (section 3). The "lent census"
   is empty **by construction** the day the native loop's gap count is zero on the production corpus.
2. **The type swap is atomic.** `Choice`, `Table`, `Decider`, `Observed`, `Game`, `GameSnapshot`, `StepResult`, `GameError` and everything in `render/`, `live/`, `core_sync/` mention each other;
   they cannot be shadowed one at a time (a thin `Choice` is a different type from the legacy `Choice`). Only the pure modules (constants, queries over `GameState`, setup builders) can be
   shadowed incrementally (section 1.4). So 8b is one commit stack, and the bridge must be retargeted in the same stack.
3. **The glue's dependency on non-glue legacy code is small and bounded: 81 distinct items in 34 modules** (measured, Appendix A), not "118 modules". `live/` still names `dual_window::{NestedPos,
   complete_nested, nested_position, Pushed, tables}`, `combat_live`, `exploring_dual`, `technology_core`, `reactions_core`, `core_adapter` despite its header saying it never imports `dual*`.
4. **`choice.rs` is two files in one**: the wire/format part (lines 1-440 and 2354-3176: `Choice`, `ChoiceOption`, `DecisionRecord`, `DecisionLog`, `Decider`, `Table`, ...) and `Observed`/`SeatObservation`
   (lines 661-2350, 1 700 lines, 69 + 12 methods; 37 references to string `objectives`, 17 `factions`, 14 `tactical`, 14 `core_adapter`, 5 `dual_typed!` sites). The string rule bot calls `Observed` 500 times;
   it stays until D3, so the thin crate needs a native `Observed` (8b3). The typed `Seen` view already exists in the core (`ti4-engine-core/src/seen`, 1 150 lines, D3-1) and `seen_parity` compares them.
5. **All 14 stored alpha games are `rules_version` 0** (measured on `/var/lib/ti4-public`: 14 `init.json`, all without the field; written 2026-10-12). With "only 35 supported" **all 14 are refused**;
   the alpha replay preflight (`deploy.sh --preflight-only`) becomes vacuous and the recovery/replay proof must come from other corpora (section 4.3). Any game saved by the live branch today is version
   25-27 (live is at 27) and is refused too. Four of the 14 are 8-seat games; the native sweep covers 3-6 seats only.
6. **The benches cannot run on a version-35-only engine as they are**: `golden.json` plays `rules_version` 23, `golden-v2.json` 11, `golden-v5.json` 18 (measured). `bench` and `bench-v1..v5` must be
   re-recorded at 35 with the frozen legacy engine as the recorder, once, and the wrapper must reproduce them hash for hash (section 6.3). The G7 row 5 / row 3 text of `D2-remaining.md` must change.
7. **The five conflicts of `disable-xf123` are not the commit's**: the single commit `f472fd4e1` (20 files) merges into `core-d27-split` with **0 conflicts** (`git merge-tree`, measured); the 5 conflicts
   (`ti4-host/src/view/scenario/mod.rs`, `ti4-server/src/session/{mod,registry,worker}.rs`, `ti4-server/tests/it/main.rs`) appear when the **live branch's 8 newer commits** are merged. Merge live first (needed
   anyway), then take `f472fd4e1` (section 5).
8. **Product features that read engine side channels the live path does not feed yet** (section 2.4): `ExecutionObservation` latches (the server's per-seat undo and the host's plan preview),
   `Table::{auto_resolve, take_auto_resolved, window_skipped, observed_offer}` (toasts, the hidden-reaction wait banner), `on_strategic_action_chosen`, `on_aftermath`, `rolls()`. None is a
   zero-bail concern (nothing bails), all are silent differences if forgotten.
9. **The deciders use errors as control flow** (wasm `Asked`/`Seat`, server `Watched`/`RemoteHumanDecider`: `IllegalChoice::DeciderFailed`, "the host changed its seats", "taken back"). The live
   loop's `Stop` carries no payload for it (`Stop::{Illegal, Exhausted, Blocked}`): the wrapper needs `Stop::Asker(IllegalChoice)` so `step().error` is what the host expects and the position is restored.
10. **Gate coverage is narrower than the product**: the server offers 2-8 seats (`PLAYER_COUNTS = 2..=8`), three card sets (`te`, `pok`, `base_game_codex1`), `starting_tech` on/off, speaker random/seat,
    start presets and scenarios, templates; `native_play` plays 3-6 seats, `te`, tech on, no preset (section 6.4). The nightly sweep also switches Thunder's Edge *content* on in 40 % of its games.
11. **`native_play` does work per step the wrapper must not**: `NativeGame::step` clones the `CoreState` for the rollback and `run_live` clones it again; `env_with_plays` (the registry derivation) is rebuilt on
    every step although its header says "built once per game". Both are performance items of 8e (section 7.2).
12. **Numbers disagree**: the brief says 96/96 and 288/288 sweep games finish; `docs/INTEGRATION15.md` in this tree records 94/96 and 285/288 (the two gaps were the Technology research window with Doctor
    Sucaban, `CombatUnported` at yin 6p). A later lane evidently closed them; the first job of 8g is to re-measure on the merged line, not to trust either number.

---

## 1. The thin `ti4-engine`: its real API

### 1.1 What the consumers use today (measured with `tools/engine_core/boundary_inventory.py` helpers, production code only, tests cut off)

| group | distinct engine paths | modules |
|---|---:|---:|
| `ti4-host` + `ti4-server` + `ti4-wasm` | 171 | 50 |
| `ti4-rule-bot` (string bot) | 47 | 15 |
| `ti4-rule-bot-typed` | 2 | 1 |
| `ti4-selfplay` | 14 | 6 |
| **union of the four product groups** | **207** (104 functions, 51 types, 34 constants, 18 whole-module/glob uses) | 54 |
| dev tools only (`policy, training, sim, mlp, review, replayer, advisor, calc, bridge, cli, bot-agent`) on top | +102 (44 types, 34 fns, 21 consts); 3 modules used by nobody else (`opening` 7, `draft` 1, `event` 1) | |
| total (the 309 of D2-7 appendix A) | 309 | 57 |

(All counts below are greps over non-`tests/` source, in-file test modules included: upper bounds.)

The `Game` itself: `Game::{new, with_seeded_random, with_table}` (host 8 call sites, server 1, wasm 4), `with_galaxy` (host 9, server 6, wasm 4, selfplay/sim/training/policy 11), `step` (host 10, server 4, wasm 7,
mlp 7, ...), `run` (server 4, wasm 3, sim 4, training 4, policy 1), `fork` (host 3, server 3, wasm 5), `snapshot` (server 6, wasm 2) / `snapshot_unlogged` (host 1), `rolls` (host 2, server 3, wasm 2, selfplay 3,
review 4), `fork_for_worker` (server 2), `install_table` (server 1), `legal_options` (sim 2), `open_secondary` (host 3), `open_windows` (wasm 1), `bind_observation` (host 1, server 1), `on_aftermath`,
`on_strategic_action_chosen`, `prepare_hypothetical_turn/_secondary`, `secondary_resolutions` (host 1 each), `flush_rng_sync`, `rng_sync_pending`, `rng_positions`, `set_rng_sync` (host 1 each, all turn-redo).
Field reads: `game.state` ~45 (host) + 20 (server) + 28 (wasm) + 15 (selfplay) + 24 (review) + 15 (training) + 9 (sim) (upper bounds, a grep), `game.table` 30 + 17 + 33, `game.events` 6 + 2 + 3,
`game.timing` 2 (host) + 4 (review) + 1 (training). Consumers use of `GameError`: **none by variant** (they read `step().error` as an opaque `Option`), of `StepResult` only `.error`/`.finished`.

### 1.2 Classification (Appendix B has the per-module table)

| class | what | size | owner |
|---|---|---|---|
| **R** pure re-export / copy of data | wire/format types (`Choice`, `ChoiceOption`, `DecisionRecord`, `DecisionLog`, `IllegalChoice`, `DecisionContext` and friends, `CanonicalHash`, `Event`, `Roll`, `RngPositions`, previews), the 34 constants (option kinds and ids), the generated tables | ~5 k lines + `generated/` 11 k (one source, section 1.4) | 8b1 |
| **G** glue to copy | `core_sync` 7 411, `render` 6 934, `live` 2 825, `kernel_sync` 1 000, `resolver_sync` 741, `product` 208, `seen_load` 141 (~20 300 lines, D2-7 §1.4) | cut 81 ties (Appendix A) | 8b2 |
| **O** `Observed`/`SeatObservation` | `choice.rs` 661-2350 + `redacted_full_state`, `ask_private`, `held_secret_progress*`: needs native queries instead of `objectives::*`, `tactical::*`, `factions::*`, `core_adapter::*` | 1 700 lines | 8b3 |
| **Q** pure queries over `&GameState` | ~70 of the 104 functions (`technology::prerequisites`, `planets::in_system`, `objectives::points_for`, `production::capacity`, `tactical::ship_routes`, `fleet_strength::matchup`, `secrets::progress`, ...). Delegate to the core `query::*` layer (6 045 lines, 19 files) where it exists (`board`, `economy`, `fleet`, `laws`, `leaders`, `objectives`, `promissory`, `reach`, `relic`, `seats`, `tech`, `trade`, `transit`) else copy the pure function | 8d | 8d |
| **M** mutating helpers on a `GameState` | `promissory::{take, receive, support}`, `exploration::{explore, gain_fragment, mark_legacy_device_ids}`, `objectives::adjust_victory_points`, `movement::{apply_map_edit, replay_map_edits}`, `laws::apply_to_galaxy`, `technology::apply_unit_upgrades`, `action_cards::apply_movement_effects`. **All are used to build start positions** (host `preset.rs`, `scenario`, the offline editors) and by the host's map reconstruction, not by the game loop. They stay position-builders over `GameState` (copy, or via a core entry point); they are *setup* | 8c | 8c |
| **S** setup | `setup`, `seating` (2 172), `deck`, `opening`, `starting_tech`, `map_feasibility`, `fixtures` + `ti4_host::map` (already host) | 4.7 k lines, ties in Appendix A | 8c |
| **W** the wrapper | `Game`, `GameSnapshot`, `StepResult`, `GameError`, `RunError`, the loop | new, section 2 | 8e |

**What the core already provides (measured):** `CoreState` and the codec (`Codec::load/store`, `materialize`, byte-identical JSON round trip, `core_sync_gate`), `driver::live::{advance, ask, open, is_open}`
(+ the arm registry), `driver::session::{Facts::of_core, step_arm, live_ask_arm}`, `driver::turn::{open_finish, advance_reconcile, ask_job}`, every window of the game (19 arms), `driver::menu` (the action menu and
its 11 sources), `kernel` + `ti4-engine-core-factions::register_all`, `query::*`, `seen::Seen`, `unsupported::{check, find, guard}`, the dropped-feature policy. What it does **not** provide: any text (all render
is in the glue), setup, the `Table`/decider protocol, `Observed`, the wire types.

### 1.3 `Game`'s public methods: what the wrapper does with each (31 `pub fn` of `impl Game` + the fields)

| method / field | use | disposition |
|---|---|---|
| `new`, `with_seeded_random`, `with_table` | host/server/wasm creation and replay | `try_*` forms returning `Result<Game, GameError>` (load + guards); `new` keeps panicking for tests |
| `with_galaxy` | host/server/wasm/bots | keeps the `Galaxy` (shared type of `ti4-content`) next to the core map; used by `Observed` and the host views |
| `step`, `run`, `legal_options` | loop | section 2 |
| `fork`, `fork_keeping_log`, `fork_for_worker`, `snapshot`, `snapshot_unlogged`, `GameSnapshot::{instantiate, into_game, decisions}` | undo, worker handoff, wasm checkpoints | `CoreState::clone` + `TableState` + `Render` code table + events len; the FORK RULE (destructure without `..`) stays |
| `rolls` | dice display (server, wasm host, replay, review) | `Vec<Roll>` rendered from the core's dice history (`ledger` + `RollPurpose`, `Roll` copied as wire type); **VERIFY** the core keeps every field of `Roll` (`rerolled`, `by`, `system`, `outcome`, `source`) |
| `install_table`, `table` (field) | server/host/wasm | `Table` is copied wire code; stays a public field |
| `state` (field) -> `state()` | ~160 reads | `&GameState` from a cached materialisation (`OnceLock`, invalidated by every `&mut self` method) |
| `events` (field) | 11 reads | `Vec<String>` pushed by `render::events::pushed_since` per step |
| `timing` (field), `timing_mut` | host 2, review 4, training 1 (`set_participation`, planning fork) | **DECISION-free**: planning's `guard_fork` sets `timing.set_participation`; replace by a wrapper knob `Game::participation(Arc<dyn Fn(&Ability)->bool>)` read by the live `Env` table; the other readers use `resolver_image()` |
| `bind_observation`, `ExecutionObservation` | undo + plan preview | section 2.4 item 1 |
| `on_aftermath`, `on_strategic_action_chosen` | plan preview, strategic-action watch (`view/secondary.rs`, production) | section 2.4 item 4 |
| `open_secondary`, `secondary_resolutions`, `prepare_hypothetical_turn/_secondary`, `open_windows` | secondary preview, plan | read/prime `Pending` windows (`Facts::of_core`) |
| `flush_rng_sync`, `rng_sync_pending`, `set_rng_sync`, `rng_positions`, `set_rng_positions`, `with_round_income_seat`, `with_round_income` | turn-redo (switched off), experiments | **drop with XF-2**; `round_income` stays a `Knob` for `ti4-training` only (dev) |
| `step_facts`, `observed_windows`, `content_sources`, `actions_this_turn` | `native-subject` (legacy only) | stay in legacy, bridge-only |

### 1.4 How the crate is filled (ordering inside 8b; this replaces D2-7 §4)

* **Incremental shadows that need no swap** (each is a commit; the facade keeps compiling; the module name in `lib.rs` shadows the glob): `generated` (`#[path]` include of `crates/ti4-engine/src/generated/mod.rs`, the
  same as `ti4-engine-core/tests/adapter.rs:11`; **do not emit two copies**; the xtask `adapter_out` moves only when legacy is deleted or in D2-9), the 34 constants (a `kinds` module re-exported at the old paths:
  `production::PAY_KIND` etc.), the pure queries of Appendix B class Q that take only model types, the position builders (class M), `fleet_strength`, `valuation`, `dice::Roll` as a *copy with an identical
  serde shape*. None of these has a type that crosses into a legacy function.
* **The swap (8b1-b3 in one stack, bridge retargeted in the same stack):** wire types, glue, `Observed`, then `Game` (8e) behind `crate::game`. Until 8e lands the thin crate keeps `pub use
  ti4_engine_legacy::game::*` so the workspace compiles; the commit that switches `Choice` must also switch `render/live/core_sync` (they build `Choice`s), `Table`, `Decider`, `Observed`.
  The bridge then holds two `Choice`/`Game` types: compare through serde/JSON (`oracle.rs` `Subject` already normalises to JSON + ids); the cheap check is `cargo check -p ti4-engine-core-bridge --all-targets`.
* **Wire parity (the drift guard):** the format types are *copied*, so a parity test is the contract: for every `Choice`, `DecisionRecord`, `DecisionContext`, `Roll`, `Event` produced by a golden game, the
  thin type's JSON equals the legacy type's, deserialises into the other, and the V1/V2 `decision_hash`/`event_hash` agree (`tools/engine_core/golden`, `decision_corpus`). `ts-rs` derives (23 sites in
  5 modules: `choice`, `decision_context`, `strategy`, `secondary_preview`, `reach_query`) are copied with the same `export_to`; `cargo wire-web` (broken since the alias puts `--config` last, D27_RESUME;
  fix it in 8b1 by moving the `--config` before `--`) must leave `git status` clean. **Alternative (DECISION 2, section 8):** put the pure format types in a leaf crate used by both engines. I recommend
  the copy plus the parity test because legacy is frozen (nothing can drift on its side) and a leaf crate would force a 118-module import edit in the frozen crate.

---

## 2. How production calls the core loop

### 2.1 The loop (cited: `ti4-engine-core-bridge/src/native_play.rs` `NativeGame::step_inner`, `D2_LF_RESUME.md` §2, `M7_RESUME.md`)

One `Game::step()` is today's `NativeGame::step_inner`, with the table instead of the bot:

```text
step():
  0. if self.blocked: return StepResult{error}                       (a refused/unmapped position never steps)
  1. env   = cached Env (live::env::env_with_plays(render, core, kernel, seeds, sources))      <- cache per game, rebuild when abilities change
  2. loop up to 8x: live::live_reconcile(core, cx, asker) until step.finished                  <- the per-step hooks of Game::step_flow
  3. facts = Facts::of_core(core, self.blocked_flag); asks = self.asks()      (open job | draft | starting tech | menu with a seat in turn)
     arm   = step_arm(&facts, asks);  Arm::Finished -> finished = true
     (Arm::Ask on a source the core does not run does not exist any more: all sources are native; keep the Gap path as GameError::Unmapped)
  4. out = live::live_step(core, arm, cx, TableAsker{table, observed})                        <- questions go to table.ask_seeing
  5. if out.finished { after_arm(core, arm) }                                                 <- Tactical closed => turn::open_finish (the end of the action)
  6. commit: events += render::events::pushed_since; render.sync_codes(core.texts.codes); invalidate the cached GameState;
     resolved_choice = out.asked non-empty
  on any Err: *core = before; keep the decider's IllegalChoice in StepResult.error (section 2.3)
```

Where **reconcile must run**: at the start of *every* step (step 2), with the same asker, because an emission there can open a reaction window; **never** in `legal_options()` (read-only). `live_reconcile` has
one caller today outside tests (`NativeGame`), `open_finish` one (`after_arm`), `live_step` one (`NativeGame`) plus `NativeSubject`: **after the flip the wrapper is the only caller** and the bridge calls
the wrapper (BAIL_MAP G1-G3 closed). `env_with_plays` (not the bare `env_for`) is mandatory: a card played inside a window without the play facts stops with `Unknown(PlayFacts)`.

`asker`: `TableAsker` implements `live::Asker`: `ask(&Question) -> Result<String, Stop>` builds the `Observed` of the materialised state (lazily: only when the seat's decider overrides `choose_seeing`,
see 2.5), calls `table.ask_seeing(&q.rendered, &observed)` (which journals the `DecisionRecord`, applies `count_ask`/`NoProgress`, and `settle`s) and returns the chosen option id. `LoneCard` wraps it for the draft
(the last card is never a question, never journaled). `auto_resolve` calls stay where the string had them (section 2.4).

### 2.2 The wrapper type

```rust
pub struct Game<'a> {
    pub table: Table,                       // wire code, copied (deciders + DecisionLog)
    pub events: Vec<String>,                // plain event lines, appended per step
    core: CoreState,                        // the authority
    cx: Ctx,                                // owners table (seat <-> player id)
    render: Render,                         // code table + factions; rebuilt from core.texts.codes on restore
    env: Option<EnvCache>,                  // registry/table/seeds; rebuilt when seats' abilities change
    seeds: Rc<TextSeeds>, kernel: &'static Kernel,
    content: &'a ContentStore, sources: SourceSet,
    galaxy: Option<Galaxy>,                 // for Observed and the host views; the core map is the authority
    base: GameState,                        // fields no loaded slice carries (materialize over it)
    cache: OnceLock<GameState>,             // state(); invalidated by every &mut self method
    knobs: Knobs,                           // round_income (dev), participation filter, observers
    latch: Option<ExecutionObservation>, strategic_observer: Option<..>, aftermath_observer: Option<..>,
    blocked: Option<GameError>, decided: usize, chain: ...,
}
```

`state()` needs `&self`: `OnceLock` (the server shares `&Game` across a worker). `Game` is `!Sync` today only through `Rc` in `TextSeeds`/`Render`: the server moves it to a worker thread (`fork_for_worker` returns
a `Send` closure): keep `Rc` out of the `Send` path (`GameSnapshot` is plain data) or switch to `Arc`; **VERIFY** at the first compile of `ti4-server` (T-S1). The FORK RULE test (`tests/fork_equivalence.rs`) is copied
as the contract of `snapshot/instantiate/into_game`.

`GameError` shrinks to what is observable: `IllegalChoice(IllegalChoice)`, `Unmapped(Unmapped)` (new: refusals of the load), `Disabled(&'static str)`, `Core(String)` for a `Stop::Live` defect (never a lend), `Blocked(String)`.
`RunError` keeps `Step` and `StepLimit{max_steps, round, phase, repeats, recent}` (the watchdog of `run`, copied).

### 2.3 Errors, rollback and the deciders' control flow

`Stop` gains `Asker(IllegalChoice)`; `TableAsker` keeps the error the decider returned (wasm: `DeciderFailed{"the host changed its seats" | LEFT | TAKEN_BACK}`; server: `Watched` and
`RemoteHumanDecider` cancel; `ScriptDiverged` from the replay deciders) and `Game::step` returns it in `StepResult.error` exactly as the string did, after restoring the position
(`run_live` already restores on `Err`). A test per decider kind (the 8 server + 4 wasm + 9 host deciders) is the contract: `ti4-server` 416 tests, `ti4-wasm` 59, `ti4-host` 306 already cover them if they
run against the wrapper unchanged (section 6.3).

### 2.4 Side channels the live path must feed (silent differences if forgotten; each gets a regression test from the string behaviour)

1. **`ExecutionObservation`** (`bind_observation`; server `session/undo.rs` per-seat undo, host `planning/{undo,fork,draft}.rs`): latches `randomness`, `hidden_information`, `unsupported_participation`,
   `unsupported_segment`. The string sets them from `GameRng` (`observe_randomness` in dice and shuffles; `observe_hidden_information` at 3 sites: `action_cards.rs:370,467`, `factions/yssaril.rs:885`).
   Design: randomness = *any rng stream position advanced during the step* (compare `Slice::Rng` positions before and after: cheap, no core change); hidden information = the private marks
   (`cards:reveal`, `cards:staged`, `private:`) the core writes for a shown card, plus a deck read; port the 3 sites as core markers. **VERIFY** against `ti4-server/tests/it` undo tests.
2. **`Table::auto_resolve`/`take_auto_resolved`** (toast "it was the only way left to pay"): string sites `production.rs:1023`, `thunders_edge.rs:193,241`, `game.rs:2682` (the draft, already `LoneCard`).
   The live production payment must call `table.auto_resolve` for a single legal payment; **VERIFY** whether `driver/windows/production` already collapses it (then the note is missing: add it).
3. **`Table::window_skipped`** (the hidden-reaction wait banner, screenshots folder `AP-hidden-reaction-wait`): string `timing.rs:1301`; the core side has a hook (`kernel_sync/shim.rs:452`, `window_skipped`).
   **VERIFY** the live `Env` reaches it; if not, call it from the wrapper's nested-question path.
4. **`on_strategic_action_chosen`** (production: the secondary watch, `ti4-host/src/view/secondary.rs:92`, `ti4-server` `basis.watch.bind`) = after the strategic action's primary is chosen, before it
   resolves; **`on_aftermath`** (plan preview) = the automatic handoff after movement; **`observed_offer`** callback (undo). Natural places: `Arm::Ask` dispatch of `Opt::Strategic` and the
   `Aftermath` arm's first call.
5. **`rolls()`** and the `rolls_seen` bookkeeping of the server worker and wasm host (`worker.rs:1525,1735`, `host_session.rs:853`).
6. **`legal_options()`** = `ask_arm_fn(arm)` rendered with `render_asking` without advancing (sim uses it 2x).

### 2.5 `Observed` per ask

The default `Decider::choose_seeing` ignores the view, so human deciders do not need it. The wrapper cannot know which decider overrides it; add `Decider::wants_view(&self) -> bool` (default `false`;
true for the rule-bot deciders in `ti4-server/random_bot.rs`, `ti4-wasm` `Seat`, `ti4-selfplay`, `ti4-policy`) so the materialisation (est. 20-60 us, `ENGINE_CORE_STATE.md` 3.2, *not measured*) is paid only for
bots. The host re-materialises once per step anyway for its views (`state_update_for`), so the cached `state()` is shared.

---

## 3. The `lend-string` split

**Finding (measured):** nothing in the product loop lends. `reverse_lend.rs` (346 lines) and `lend_table.rs` (233) are used only by their own tests; `Step::Lend`/`Lent::*` only by the old dual; the native
loop has `Stop::Live(Unknown)` (a counted *gap*, never a lend) and `Gap{ArmNotNative, AskSource, Bail, LentClose}`. The mechanism D2-5r planned (wrappers per bail label, `Bail.kind/args/when`, lend-proof
mode R5) was **never built and is not needed**: Architecture A plus a core that plays the game removes the lender. The remaining honest statement is: *the lent census is the gap matrix of
`native_play`; it is empty when `Matrix::total_gaps() == 0` on the production corpus (section 6.4) and the wrapper never returns `GameError::Core`*.

Recommended design (**DECISION 3**, default as stated):

| item | where it lives | production | tests/oracle |
|---|---|---|---|
| `reverse_lend`, `lend_table`, `StringTape`, `TI4_BAIL_PROBE`, `dual*`, `*_dual`, `*_core` adapters, `AreaMask`, `churn`, `d1_*` | **stay in `ti4-engine-legacy`** (not copied) | absent | the bridge calls `ti4_engine_legacy::...` directly |
| the legacy `Game` as oracle subject / position factory | bridge (`oracle.rs` `MaskSubject`, `native_subject`, `shadow_play`, `dual_run`) depends on `ti4-engine-legacy` as a normal dependency, features `typed-core`, `native-subject` | absent | on |
| feature `lend-string` of the thin crate | `lend-string = ["dep:ti4-engine-legacy"]`: **only the dev-tool surface** (Appendix B column "dev"): `pub use ti4_engine_legacy::{...}` for the 102 dev-only paths whose functions take model types only (so no type mismatch), and `Game::from_legacy(&ti4_engine_legacy::game::Game)` (loads a legacy game into a wrapper: used by the bridge and by the `ti4-training --diplomacy`-style tools that must keep a legacy position source) | **never** enabled by `ti4-server`, `ti4-wasm`, `ti4-host`, `ti4-rule-bot`, `ti4-selfplay` | enabled by `ti4-engine-core-bridge` and the dev-tool crates |
| `ti4-engine-legacy` as a dependency of the thin crate | `optional = true`, only through `lend-string` | `cargo tree -p ti4-server -e normal,features | grep -c ti4-engine-legacy` = 0 | |

How the bridge turns it on: `ti4-engine = { workspace = true, features = ["lend-string"] }` next to `ti4-engine-legacy`. Feature unification note: a workspace-wide `cargo build/test` that includes the bridge
enables the feature for the thin crate in that build; production builds use `-p ti4-server` / `-p ti4-wasm` (`deploy.sh:178`, `build-wasm.mjs`: measured), and the new CI line is
`cargo tree -p ti4-server -e normal,features` and `-p ti4-wasm` must not mention `ti4-engine-legacy` or `lend-string`. **Dev-tool crates that stay on `lend-string`** are a D4/D3 matter (the research tools
need diplomacy and wide rosters, which only legacy has); they must not be dependencies of `ti4-server`/`ti4-wasm`/`ti4-host` (today `ti4-advisor` and `ti4-bot-agent` depend on `ti4-host`: **VERIFY** neither is
linked into the server).

**When the census becomes empty** = the exit of D2-9 "phase b" in the old text, now immediate: there is no phase a (no production lend exists). The gate is: `native_play` Z-gate matrix with zero gaps over the widened
matrix of 6.4, the 96 + 288 sweeps, the golden/rv35 replays, and a **runtime counter** `product::gap_count()` (new, 10 lines: every `Stop::Live(Unknown)` the wrapper turns into `GameError::Core`
increments it; the server logs and exposes it on `/api/health`-style metrics) staying 0 over a full nightly. A nonzero counter in production is a defect report with the cause text, not a fallback.

---

## 4. Save, load, replay, recovery

### 4.1 Formats stay (measured / cited)

* **Unchanged**: `GameState` serde (ti4-model), `RULES_VERSION` semantics (`docs/RULES_VERSION.md`: `#[serde(skip_serializing_if = zero)]`, the checksum rule), `init.json` envelope (`format_version`, `content_identity`,
  `rules_identity`, `payload.initial_state`, `checksum`), `decisions.jsonl` (`DecisionRecord`), `snapshot.json`, `history.json`, `HostSave` format 2, wire TS and fixtures, V1/V2 decision/event hashes. The wrapper
  *writes nothing new*: a save is still "initial state + decisions" replayed through `Game::with_table`. This is why wire parity (1.4) is the format contract.
* `RULES_VERSION`: **35 on the integration line, 27 on `nightly-fixes-2026-10-26`** (`state.rs:1182` vs `:1232`). The line added 28 `PLAGIARIZE_OWNED`, 29 `MOVEMENT_ROUTES`, 30 `OFFICIAL_ACTION_DECK`, 31 `CRISIS_PACT`,
  32 `CLASSIFIED_LEAKS`, 33 `COVERT_LEGISLATION`, 34 `DMZ_EXPLORATION`, 35 `BASE_DIPLOMACY`. **The flip merge therefore moves new games from 27 to 35 and refuses every live save**; no nightly fix may bump
  the version on live between now and then (a 28 on live would collide with the line's 28). Tell the nightly fixers; **VERIFY** at merge time (`git diff` of `state.rs`).

### 4.2 Refusing old versions (work item 21 of `BAIL_MAP.md`; none exists today)

Add `ti4_model::state::MIN_SUPPORTED_RULES_VERSION: u32` (0 until the flip, `= RULES_VERSION` in the flip merge) and one function `GameState::check_playable() -> Result<(), Refusal>` that also calls
`unsupported::find` (diplomacy on, deck reserve, the 7 dropped factions) and, new, `thunders_edge == true` (TE content, DECISION 4). Four call sites, one error type `Refusal::{OlderVersion, Disabled(family), TeContent}`:

| site | today | change |
|---|---|---|
| creation: `ti4_host::setup::create` (server lobby, wasm new game) | `guard::ids_supported`, `factions_known`, `scenario` | `check_playable` on the built state; `TableSetup::validate` refuses `thunders_edge`, the 7 factions, `diplomacy` (xf123 does the latter two) |
| server recovery: `storage.rs::recover_from_init` / `recover_session`, `registry.rs::recover_one_game` | `StorageError::Disabled` exists on `disable-xf123` | `StorageError::OlderVersion{found, min}` (+ `Disabled`) -> `RecoveryReport.failed` with the files kept untouched |
| host replay: `ti4_host::replay::replay_session*`, `GameInitRecord` load | none | same `check_playable` before `Game::with_table` |
| wasm resume: `host_api.rs:405` (`resume.rules_version.unwrap_or(0)`) and import of a `HostSave` | accepts 0 | refuse `< MIN` with the "older version" status the offline page already has (`web/src/offline/OfflineGamesList.tsx:132`, `OfflineHome.tsx`, precedent `docs/OFFLINE_SAVE_RESET_NOTE.md`: "not read, only deleted, with a confirmation") |

**UI work** (CLAUDE.md "UI work"): the online "My games" list must say why a recovered game is missing (today a failed recovery is silent apart from the report), and the lobby must not offer the TE content
switch (`CreateGameRequest.thunders_edge`, `TableSetup.thunders_edge`) or the dropped factions (already out of `LOBBY_FACTIONS`). Ship the screenshots and the artifact from a new folder under `web/e2e/screenshots/`
with `capture-*.ts` + `manifest.json`; the `web/` diff is the one allowed exception to G7 row 2 ("immutable surfaces").

### 4.3 The 14 alpha saves, and what proves recovery instead

* Measured: 14 games with `init.json` in `/var/lib/ti4-public` (the other 10 directories hold `lobby.json` only), **all `rules_version` 0**, 4 of them 8-seat; none seats the dropped factions or has diplomacy on; 3 have
  turn-redo history (`game_935f5618...`, `game_a36f40db...`, `game_d26c07f1...`). After the flip all 14 are `OlderVersion`: **listed as failed, files kept, not deleted, not converted** (the same policy as the
  offline format reset). `deploy/public/deploy.sh --preflight-only` must treat `OlderVersion` as "skipped, expected" (a new category in `recovery-check.sh`, not an `--allow-failed` list of 14 ids), and print the count.
  Tell the testers once (like `OFFLINE_SAVE_RESET_NOTE.md`).
* **The recovery proof must come from rv35 corpora** (the preflight proves nothing now): (1) `tools/engine_core/golden` set `v35-std` (+ a new `v35-wide` with 7/8 seats) replayed *through the wrapper* by recorded
  answers (`golden check --subject wrapper`: new, 8g), state hash and V2 hash at every step; (2) a **server-level recovery test**: create N games through the registry with random bots on the wrapper, stop at random
  steps, `recover_all_games_report`, continue to the end, compare the final `state_checksum` with an uninterrupted run (`ti4-server/tests/it`); (3) the same for the wasm `HostSave` (`host_session` tests);
  (4) the nightly offline sweep's resume check (`web/e2e/resume_real.spec.ts`).
* `alpha_roundtrip` (`core_sync_gate` on stored games) loses its corpus: keep the bin for rv35 data directories only; refuse older ones with the new error instead of reporting differences.

### 4.4 The replay endpoint's seed leak (`GET /api/games/{id}/replay`, cited `docs/HIDDEN_INFORMATION_AUDIT.md` on branch `redact-hidden-decks`, commit `be14e2b4d`, not merged)

The audit's finding: any seated player can fetch the seed and every seat's decisions mid-game; replaying them reveals all hands and the deck order. "Needs a product decision". Facts for the decision:
(a) a replay *cannot* be redacted per viewer (it is the seed plus all decisions), so the only options are "only when the game is finished" (also for spectators), "host/owner only", or accept; (b) the flip changes
nothing about the leak (the wrapper's `state()` is the same `GameState`; `rng_seed` is materialised from the core's `Rng` slice); (c) the rest of that branch (`conceal_piles`, 7 files, `ti4-model/view.rs`,
`ti4-host/projection`, `ti4-server/tests/it/hidden_piles.rs`) is engine-independent and ships in the same sweep as the product edges. **Recommendation (DECISION 5): serve the replay only after the game is
finished (HTTP 403 with a message while running), disable the "copy out the history" button while running; merge `redact-hidden-decks` with P1.** One hour plus the UI state; independent of the flip, can ship
before it. Note the testers' feedback triage replays from the server data directory (`tools/public/list_feedback.sh`), not from the endpoint, so triage is unaffected.

---

## 5. `product_init()`, `disable-xf123`, the real `BAILS`

### 5.1 `product_init()` (cited `D2-remaining.md` 3.4 8c, 7 Q3)

One function in the thin crate: `pub fn product_init() { product::set_strict(true); core_sync::unmapped::set_strict(true); core_sync::unsupported::set_disabled_policy(Policy::Panic); }`.
Called from `ti4-server/src/bin/server.rs main` and from the wasm entry (`host_api::prepare`, once per module via a `Once`; there is no `#[wasm_bindgen(start)]`). **Not** from library code; the
dev tools keep `Off`. Two rules: (1) a *known* disabled feature must never reach a panic in production: it is refused by `check_playable` at creation/load (`Err`), `Panic` is only for "cannot happen"
positions; (2) on wasm (`panic = "abort"`) a panic kills the page: `try_*` constructors return `Err` for everything the codec or the policy can refuse, so the page shows a message instead.
`TI4_CORE_STRICT=1` stays the gate/nightly spelling. Tests must use the thread-local `with_strict/with_policy` forms (T13 of `D2-remaining.md`).

### 5.2 Merge plan for `disable-xf123` (measured with `git merge-tree`)

| step | what | conflicts |
|---|---|---|
| 1 | merge the **live branch** into the integration line (8 commits, 24 files, `crates/` 11 files; the one engine file is `promissory.rs`) | 5: `ti4-host/src/view/scenario/mod.rs`, `ti4-server/src/session/{mod,registry,worker}.rs`, `ti4-server/tests/it/main.rs` (resolve keeping both sides; none touches the engine crate except `promissory.rs`, auto-merged) |
| 2 | cherry-pick/merge `f472fd4e1` (the whole branch is that one commit: `ti4-host/src/disabled.rs` 128 lines, `setup.rs`, `http/games.rs`, `storage.rs`, tests `disabled_features.rs` 281, `docs/DISABLE_XF123.md`, `config.sh`, `HJ-turn-redo-off` screenshots, `capabilities.ts`) | **0** (clean on `core-d27-split`) |
| 3 | extend `ti4_host::disabled` with the five factions the engine already lists as disabled (`keleres, deepwrought, firmament, obsidian, ralnel` = `core_sync::unsupported::Disabled` variants), by *calling* `ti4_engine::core_sync::unsupported::find` instead of a second list; add `thunders_edge` (DECISION 4) and `OlderVersion` | none |
| 4 | the nightly config: `NIGHTLY_REDO_PROBABILITY=0` (done in the commit), `NIGHTLY_TE_PROBABILITY=0` and no `expedition` preset (`tools/nightly-smoke/config.sh`, README "Thunder's Edge content": 40 %) | none |

Steps 1-4 do not need the new engine: they can land on the integration line (and on live, before the flip) first, with `MIN_SUPPORTED_RULES_VERSION = 0` so nothing else changes. That shrinks the flip's
blast radius to the engine swap.

### 5.3 The real `BAILS` table

`lend_table::BAILS: &[Bail] = &[]` is the right final value and the table's tests (`missing_entries`, `stale_entries`) run on synthetic input. There is no probe run to wait for: **D2-5r (R1-R5) is cancelled**
under Architecture A (nothing consumes a table). `BAIL_MAP.md` (cited, `core-bailmap`, int-10 base) keeps its role as the *work list of silent gaps*: its 207 agreed silent gaps (72 string abilities with no core
twin, 17 hook slots, 75 subtypes without a renderer, 4 promissory notes + 11 faction technologies + 16 leaders + Nano-Forge + Peace Accords without a trace) are what a native game plays *wrongly without a bail*.
They are not D2-8 work, but they are the content of go/no-go gate 7.4 (3). `lend_table.rs` and `reverse_lend.rs` stay in legacy (section 3); the file `tools/engine_core/lent_sites.tsv` and `bail_map.tsv` stay as
historical inventories.

---

## 6. Lanes, order, ownership, verification

### 6.1 Packages (the letters keep the old numbering; 8a `window_driver`/armed dual is dead under A and stays unmerged)

| id | package | content | needs | est. h |
|---|---|---|---|---:|
| **P1** | product edges | live merge, `f472fd4e1`, five more factions, TE refusal, `MIN_SUPPORTED_RULES_VERSION` + `check_playable` at 4 sites, "older version" UI + screenshots, `product_init`, replay endpoint decision, `redact-hidden-decks`, nightly config, fixer/summary prompts (`tools/nightly-smoke/prompts/{fixer,summary}.md` name `crates/ti4-engine` as the Rust engine: after the flip fixers would edit dead code) | none (ships before the flip) | 10-16 |
| **8b1** | `generated` shadow, wire/format types copy, `ts` parity, `cargo wire-web` alias fix, parity test | none | 4-6 |
| **8b2** | glue copy + cut 81 ties (Appendix A), tests that go with the glue (`render` unit tests, `core_sync` tests, C 38), `dual` hooks removed from copies | 8b1 (same stack) | 8-12 |
| **8b3** | `Observed`/`SeatObservation` natively (class O), `Decider::wants_view` | 8b1; queries from 8d | 6-10 |
| **8c** | setup native: `setup`, `seating`, `deck`, `opening`, `starting_tech`, `map_feasibility`, `fixtures`, position builders (class M); refuse dropped content; `host::setup::create` calls them | none (pure `GameState` code) | 8-13 |
| **8d** | surface sweep: 34 constants, ~70 queries, previews; regenerate Appendix B as a script (`boundary_inventory.py --paths`) | none (module by module) | 10-16 |
| **8e** | the `Game` wrapper (2.1-2.3, 1.3), `try_*` constructors, fork/snapshot, `run`, `legal_options`, `state()` cache, `rolls`, `Stop::Asker` | 8b stack | 12-20 |
| **8f** | side channels (2.4), per-step hook parity (the list of `D2-prereqs.md` D2-8 item 5, verify none still needs the string), `ExecutionObservation`, hypotheticals | 8e | 6-10 |
| **8g** | bridge retarget, `native_play` on the wrapper, oracle subject B = wrapper, `golden check --subject wrapper`, rv35 re-record of `bench`/`bench-v1..v5`, widened Z-gate matrix, wire parity runs, server/wasm recovery tests, perf A/B | 8b, grows with 8e | 10-16 |
| **D2-9** | strip + gates (section 7) | all | 8-12 + heavy runs |

### 6.2 Parallel lanes and file ownership (to limit conflicts)

`crates/ti4-engine-next/src/lib.rs` is the conflict magnet: pre-create it in the first commit with one section per lane, `// ---- lane A ----` ... and have each lane touch only its own section; `Cargo.toml` of the
thin crate belongs to lane A (others ask for a feature). The `ti4-engine-core*` crates are owned by their existing lanes; the wrapper lanes only *read* them (a needed core entry is a request to the core owner).

| lane | packages | owns (write) | reads |
|---|---|---|---|
| **A foundation** | 8b1 -> 8b2 -> 8b3 | `crates/ti4-engine-next/{Cargo.toml, src/lib.rs, src/generated, src/wire/*, src/core_sync, src/kernel_sync, src/render, src/live, src/resolver_sync.rs, src/product.rs, src/seen_load.rs, src/observed.rs}`, `.cargo/config.toml` | legacy (copy source), core |
| **B surface** | 8d | `crates/ti4-engine-next/src/{kinds.rs, queries/*, preview*.rs, fleet_strength.rs, valuation.rs, ...}` (one file per module of Appendix B) | core `query::*` |
| **C setup** | 8c | `crates/ti4-engine-next/src/{setup, seating, deck, opening, starting_tech, map_feasibility, fixtures, builders}.rs`, `crates/ti4-host/src/{setup.rs, preset.rs, scenario}` call sites | |
| **D wrapper** | 8e -> 8f | `crates/ti4-engine-next/src/game/*` (new directory), the call-site edits in `ti4-host/{batch,replay,step_snapshot,planning,history}`, `ti4-server/src/session/{worker,undo,preview,draft_basis}`, `ti4-wasm/src/{lib,host_session,host_api}` for `state()`/`try_*` | lane A's glue (after the swap) |
| **E product edges** | P1 | `ti4-host/{disabled,setup,replay}`, `ti4-server/{storage,http,session/registry}`, `ti4-wasm/host_api`, `ti4-model/{state,view}`, `web/`, `deploy/public`, `tools/nightly-smoke` | |
| **F gates/bridge** | 8g | `crates/ti4-engine-core-bridge/**`, `tools/engine_core/**`, `tools/rulebot/{baseline*.json, golden*.json}` (new rv35 baselines), `crates/ti4-server/tests/it` (recovery tests) | |

Start order: **E and C, B start on day 0** (no dependency on the swap; E ships first). **A** starts on day 0 and is the critical path; **D** starts as soon as 8b1 + the glue copy compile (it can write
against `crate::*` paths that resolve through the facade until the swap flips them); **F** starts with the bridge retarget in the same stack as the swap. Merge order: E -> (B, C in any order) -> A -> D ->
F-gates -> 8f -> D2-9. A lane never edits another lane's directory; a needed change is a request (comment in the lane's branch notes).

### 6.3 Verification, in order

1. **Per commit (cheap):** `cargo check --workspace --all-targets` minus the torch crates (INTEGRATION15 §1); `cargo c-web`; `python3 -I tools/engine_core/check_layers.py core`; `tools/engine_core/clippy_core.sh` (the thin
   crate is zero-tolerance; extend `core_re` if the layout changes); `cargo xtask gen-cards --check`; `cargo wire-web` leaves `git status` clean (after the alias fix); the wire parity test.
2. **Swap done (8b):** `native_play.sh --quick` and `--quick --twice` print the **same fingerprint and step count as the base line** (int-15: 1 198 steps `2e93df9fa00fa982`; int-16: 1 165 steps
   `74d5675ac754ac81`; use the number of the merged base), now driven through the wrapper (a `Decider` adapter around the bot instead of `Bot: Asker`); `native_play.sh --factions all --players 3,4,5,6 --seeds 1`
   same finish/gap table.
3. **Wrapper done (8e/8f):** the existing wire tests against the wrapper unchanged: `ti4-host` 306 tests, `ti4-server` 416 (73 `tests/it` files), `ti4-wasm` 59, `ti4-rule-bot` 191, `ti4-selfplay` 12, `ti4-model` 94,
   `ti4-calc` 18 (`cargo t-web`, measured counts of `#[test]`); `tests/fork_equivalence.rs` and `GameSnapshot` continue-equals-original; checkpoint/restore around a nested question; the decider-kind tests of 2.3;
   `host_session`/`HostSave` resume tests; `protocol_fixtures` / `wire_types` / `catalog_fixture` byte-identical.
4. **Equivalence (8g, heavy, END-OF-PHASE ONLY, `flock /root/.ti4-heavy.lock`):**
   * Z-gate `native_play --factions all --players 3,4,5,6 --seeds 8 [--shadow]` AND the widened matrix of 6.4, zero gaps, same fingerprint twice;
   * `--shadow` against the legacy game: **0 unexplained differences**; explained ones are an allow-list keyed by `rules_version` (S1 of `STRING_ENGINE_BUGS.md`, any core-only rule fix) with a reason, as in
     `allow_list.rs`; this is the gate other lanes are finishing (the 96-game table of INTEGRATION15 §6.2: PB 65, AC 10, NE 9, TG 4, SC 3, S1 3, OT 2 at that time);
   * `bench`/`bench-v1..v5` re-recorded at rv35 by the frozen legacy engine (`verify_benches.sh` after changing the golden configs' `rules_version` to the latest and `--record` with a legacy-built
     `selfplay`), then **200/200 identical x6 on the wrapper**: this is the only gate that runs the real rule bot through `Observed` over whole games, so it is the proof of 8b3;
   * golden sets replayed by recorded answers through the wrapper (`v35-std`, ladder at 35); `decision_corpus` replay;
   * `TI4_CORE_STRICT=1` over golden 20 + the Z-gate, 0 assertions.
5. **Product (live-flip needs):** (a) `cd web && npm run test:e2e:smoke` (`TI4_SMOKE=1 playwright test e2e/smoke_playthrough.spec.ts`) against a server built from the flipped line (`b-web`), per card set
   (`TI4_SMOKE_CARD_SET=pok|te`), 3-8 seats; (b) the offline sweep (`web/e2e/offline-sweep.config.ts`, `tools/nightly-smoke/offline-run.sh`) against a fresh `wasm-release` build; (c) the resume specs
   (`test:e2e:resume`, `resume_real.spec.ts`); (d) the server recovery test of 4.3; (e) **one dry night** of the nightly sweep on a slot copy of the flipped line before it is merged into live (CLAUDE.md:
   "the sweep tests the LATEST version", so the merge into live is itself the start of the real test; do it at least a day before a night one wants to be quiet, and never in the last hour before 20:30
   Berlin, `TZ=Europe/Berlin date`); (f) update `tools/nightly-smoke/prompts/{fixer,summary}.md` and `AGENTS.md` so fixers edit `crates/ti4-engine-core*` / `ti4-engine-next`, not the frozen legacy
   (`ENGINE_CORE_STATE.md` 6.5); CLAUDE.md is the user's.
6. **Perf and size (G7 rows 7, 8, 10, 11):** `tools/engine_profile/ab_time.sh` (wrapper vs the int-7 selfplay, budget +1.5 % CPU as written; I expect it to be tight, see 7.2), `state_size`, `wasm-release` raw/gzip/brotli
   against `size-budget.json` (2.4 MB gzip budget, ratchet), release build time of the thin crate.

### 6.4 Widened Z-gate matrix (measured gap between product and gate)

| axis | product | `native_play` today | add |
|---|---|---|---|
| seats | 2..=8 | 3,4,5,6 | **2, 7, 8** (alpha had 8-seat games: 4 of 14); maps for 7/8: `random`/template |
| card set | `te`, `pok`, `base_game_codex1` | `te` | **`pok`, `base_game_codex1`** (nightly plays `pok` in 25 %) |
| starting tech | on/off | on | **off** |
| speaker | random/seat | random | **seat** |
| presets / scenarios | `preset.rs`, `scenario::start` | none | the presets the nightly picks (`smokePreset.ts`), minus the TE ones |
| `thunders_edge` | refused after DECISION 4 | false | n/a |
| bots | rule bot (smart), random bot | seeded random chooser | **the real rule bot via `Observed`** (wrapper + `ti4-rule-bot` decider): covers 8b3 |

---

## 7. Effort, risks, go/no-go

### 7.1 Estimates (agent-hours, *estimated*; calibration: D2-7 split 3-5 h, window work 0.2-0.35 of document estimates)

| lane | h | critical path? |
|---|---:|---|
| E (P1) | 10-16 | no (ships first) |
| A (8b1+b2+b3) | 18-28 | **yes** |
| B (8d) | 10-16 | no |
| C (8c) | 8-13 | no |
| D (8e+8f) | 18-30 | yes (after A) |
| F (8g) | 10-16 | yes (gates) |
| D2-9 | 8-12 + heavy runs (wall: ~8-10 h) | yes |
| **total** | **82-131** (the sum of the ranges) | critical path ~45-70 h (A, then D overlapping, then F and D2-9) |

### 7.2 Risks

| # | risk | evidence | answer |
|---|---|---|---|
| R1 | **silent gaps**: with no string fallback a missing ability/subject/leader is a wrong game, not a bail | `BAIL_MAP.md` 207 agreed silent gaps; shadow can compare only what plays | go/no-go gate 7.4 (3): the shadow + the widened matrix + a per-faction ability audit (`every_ability_the_string_registers_is_ported_or_accounted_for`) before the merge into live |
| R2 | **performance**: core vs string. No whole-game timing of the native loop exists (M7 timed gap-stopped games). Known per-step costs: two `CoreState` clones (`NativeGame::step` + `run_live`), `env_with_plays` rebuilt per step, `Asking.at: Box<CoreState>` clones for nested questions, `render` code sync, one materialisation per bot ask. String reference: 0.9-2.4 ms/decision, 4p game 1.6 s, bot 55-60 % of wall (`ENGINE_OPTIMIZATION.md` scaling table) | cache the `Env`; take the rollback copy once per step (the wrapper's, drop `run_live`'s); `wants_view`; measure first (8g item: `native_play` wall for 4p/6p whole games + `ab_time.sh` selfplay A/B). `ENGINE_CORE_STATE.md` 6.6 expects the checkpoint bucket (18.7 %) to fall to ~2 %: the gain is real only if clones are cheap |
| R3 | **wasm size** | core + factions already compile into wasm (default `typed-core`); the legacy rule bodies (300 k lines) leave; the glue (20 k) is already in. Facade measured identical (D27). No wasm-release number exists after the split | expect a net decrease; measure once at G7 (`wasm-release` is LTO fat, heavy); budget 2.4 MB gzip with the ratchet |
| R4 | `Rc`/`!Send` in the wrapper vs the server's worker threads | `fork_for_worker` returns a `Send` closure | T-S1: plain-data snapshot, rebuild `Rc`s on instantiate |
| R5 | bench hashes change for a legitimate reason (core-only rule fixes, e.g. Heart of Ixth) | `D2-remaining.md` T-F | record the baselines from legacy, list each intended difference in the allow-list with `rules_version_min`, never edit a hash |
| R6 | the swap breaks the bridge and the 30+ bridge bins at once | the bridge names thin `ti4_engine::{render, live, core_sync, choice}` 233 times in 42 files | retarget in the same stack; `choice` compared via serde; budget in 8g |
| R7 | wire/TS or save format drift between the two copies | 23 `ts` sites; V1/V2 hashes | 1.4 parity test in CI; `cargo wire-web` clean |
| R8 | `Panic` policy in a long-lived server/wasm page | wasm is `panic = "abort"` | 5.1: `Err` for everything refusable; panics only for impossible states |
| R9 | live-branch drift: nightly fixes after the flip hit legacy (dead code in production) | CLAUDE.md: sweep tests the latest; fixer prompts name `crates/ti4-engine` | merge live into the line right before and right after (T11); update prompts; tell the sweeper |
| R10 | `RULES_VERSION` collision (live 27 vs line 35) | 4.1 | freeze version bumps on live; the flip merge is one commit with `MIN = 35` |
| R11 | player-visible features silently lost (undo latch, toasts, wait banner, secondary watch) | 2.4 | one regression test per item taken from the string behaviour; the server/host tests cover undo and the wait banner |
| R12 | hidden information through `state()` / `Observed` | `view_for`, `conceal_piles` live in `ti4-model` (unchanged); `Seen` is the typed contract for D3 | the existing sentinel tests (`hidden_piles.rs`, `view` tests) run against the wrapper; `seen_parity` perturbation test stays |
| R13 | dev-tool tail (102 paths, 11 crates) | 3 | `lend-string` surface; compile-only gate (`cargo check --workspace` minus torch crates); no behaviour gate |
| R14 | disk (45 GB free) and heavy-run contention | `df` | private `CARGO_TARGET_DIR` per lane, delete after, `flock /root/.ti4-heavy.lock` for gates |

### 7.3 What D2-9 must do for the flip (reduced by this document)

1. Delete from the thin crate's `Cargo.toml` everything that is a dual switch: `typed-core` (the core is a plain dependency), the eight `string-*`, `native-subject`; `AreaMask`, `Backend`, `dual*`, `*_dual`, `*_core`, `d1_*`,
   `churn`, `TI4_AREAS`, `TI4_DUAL_FAIL_FAST`, `TI4_CHURN`, `TI4_BAIL_PROBE`, `TI4_LEND_PROOF` are *never copied* into the thin crate (they stay in legacy); so the "strip" is a verification: `grep` for them in
   `crates/ti4-engine-next` finds nothing.
2. `lend-string` per section 3 (dev surface only); the CI check `cargo tree -p ti4-server/-p ti4-wasm -e normal,features` has no `ti4-engine-legacy`.
3. Aliases: `c-web, l-web, t-web, b-web, wire-web` keep `-p ti4-engine-legacy`? No: legacy leaves the web set (it is no longer a dependency of anything in it); add a `t-legacy` alias (legacy tests + `content_ids_resolve`) and keep
   `-p ti4-engine-legacy` in `t-web-slow`'s `profile.dev.package` keys only if legacy is built there. `tools/engine_core/{verify_package,verify_stage,end_of_d1}.sh` package lists, `clippy_core.sh` (thin crate zero-tolerance).
4. `decision_delivery_inventory` for the new crate (T10): a copy of the scanner test over `ti4-engine-next/src` for the `Choice` producers (`render/*`, `live/*`).
5. L2 (cut legacy's cord to the core, D2-7b): **not needed for the flip**; do it only if a core API change breaks legacy's glue (R7 of D2-7). Under A the oracle compares the *legacy-with-core-wiring* at `AreaMask::STRING`
   with the wrapper; T-G (legacy compiled with its own wiring must run `STRING`) is checked by `AreaMask::build_default()` in the bridge's subject constructor.
6. G7 rows rewritten: row 3 = shadow + rv35 oracle (`golden` through the wrapper), row 5 = `bench` + `bench-v1..v5` **at rv35** (6.3 item 4), row 12 = the server recovery test instead of the alpha preflight, new
   row 13 = `TI4_CORE_STRICT=1`, new row 14 = the widened matrix, new row 15 = production gap counter 0 over one nightly. Row 2 ("immutable surfaces") excepts the P1 `web/` diff.
7. Docs: `README` of both crate directories, `docs/ENGINE_CORE_STATE.md` 9 (final structure), `docs/RULES_VERSION.md` ("versions below MIN are refused"), `docs/D27_RESUME.md` leftovers (clippy baseline keyed to
   `crates/ti4-engine/`, ~40 doc comments naming `ti4-engine/src/render/...` in the core: fix when the glue copy lands).

### 7.4 Go / no-go

**GO to start** (lanes E, A, B, C immediately; D when A's glue compiles). The loop exists, the core plays complete games, and the remaining work is bounded and mostly mechanical (copy + cut 81 ties,
207 paths, 4.7 k lines of setup, one wrapper type).

**NO-GO for merging into live until all five hold:**
1. Z-gate green on the **widened** matrix (6.4) through the wrapper, same fingerprint twice, with the real rule bot in at least one column;
2. `--shadow` against legacy has zero unexplained differences over 96 + 288 games and the widened set (the other lanes' work, S1 ruling pending);
3. the silent-gap list of `BAIL_MAP.md` (high and medium, agreed content) is closed or each row has a user-accepted waiver; a per-faction ability audit is green;
4. rv35 `bench`/`bench-v1..v5` 200/200 on the wrapper; wire/TS/fixtures byte-identical; server and wasm recovery tests green; perf within budget (or a stated, accepted delta);
5. P1 is merged and was a nightly old; the dry night of 6.3 (5e) is clean.

**NO-GO triggers that should stop the work:** the first whole-game wall measurement shows the wrapper more than 1.5x slower than the string engine per game with the same bot and the cheap fixes (cached `Env`,
single rollback copy, `wants_view`) are in; or the shadow matrix does not converge to zero explained differences.

---

## 8. Open questions for you (each with a default; ask once)

1. **Order of P1 relative to the engine work.** Default: P1 first and independent (it needs no new engine and removes the 5-conflict merge from the flip). Also default: the live branch is merged into the
   line now, and again immediately before and after the flip.
2. **Wire types: copy + parity test (default) or a shared leaf crate** (`ti4-engine-wire`, one definition of `Choice`, `DecisionRecord`, `DecisionContext`, `Roll`, hashes; costs a 118-module import edit in the
   frozen legacy crate and a third engine-named crate; removes drift by construction).
3. **`lend-string` as in section 3 (default: dev-tool surface only, no lend code in the thin crate)**, instead of the D2-7 feature that wraps `reverse_lend`. Consequence: D2-5r is cancelled and
   `reverse_lend`/`lend_table` stay in legacy.
4. **Thunders Edge content (`thunders_edge = true`, expedition, breakthroughs): refused at creation and load** with a message (default yes; the strategy card set `te` stays). Needs the lobby switch removed
   and the nightly `NIGHTLY_TE_PROBABILITY=0`.
5. **Replay endpoint** (4.4): only after the game is finished (default) / owner only / unchanged.
6. **What happens to saves the new engine refuses** (14 alpha games, any v25-27 game on live, offline browser saves): default = keep the files, list them as "older version", offer deletion only for offline
   saves (as the format reset did), tell the testers once. Alternative: ship a read-only legacy viewer (not in production, a standalone tool).
7. **Dev tools** (policy, training, sim, mlp, review, replayer, advisor, cli): default = they stay on the thin crate and get the 102 extra paths through `lend-string`; the diplomacy and wide-roster options keep
   working only through legacy (as `D2-remaining.md` 7 Q3 decided).
8. **Wider gate** (6.4): is 2-8 seats x 3 card sets x tech off acceptable as the zero-bail definition for the flip, or must the Z-gate stay at the current 3-6 seats `te` for the first flip night (default: widen; the
   alpha had 8-seat games and the nightly plays `pok`)?

---

## Appendix A. The 81 items outside the glue that `live/`, `render/`, `core_sync/`, `kernel_sync/`, `resolver_sync/`, `product`, `seen_load` name (measured: `crate::<module>::<item>` in non-test text, excluding the wire modules)

Copy or cut each in 8b2; "cut" = the dual/lend-only reference disappears in the copy. Counts are references.

| module | items |
|---|---|
| `dual_window` (cut) | `NestedPos::of_core` 3, `tables` 3 (tests), `complete_nested` 1, `nested_position` 1, `Pushed` 1, `lend` 1, `ResolverCfg` 1 -> move `NestedPos`, `complete_nested`, `nested_position`, `Pushed` into `render/` (they are render code) |
| `action_cards` | `name_of` 8, `DISCARD_KIND`, `TAKE_REVEALED_KIND`, `OWN_HAND_KIND` -> `kinds` |
| `factions` | `firmament_plots::PRIMARY_INSTEAD_ID` 2, `crimson::MECH` 2 (cut: dropped), `MODULES` 1, `hooks_strategy::waiver_module_index`, `mahact::AGENT_WAIVER_PREFIX`, `mahact::system_name_of`, `naalu::MINDSIEVE_PREFIX`, `borrowed_round_agents::RoundBonus` |
| `strategy_cards` | `agenda_details` 5, `card_name` 2, `RESEARCH_KIND`, `leader_card`, `commander_card` |
| `strategy` | `STRATEGY_KIND` 6, `STRATEGIC_ACTION_ID` 2, `WAIVED_SECONDARY_PREFIX` 2 |
| `technology` | `name` 7, `colour_type`, `prerequisites` (core `query::tech`) |
| `draft` | `STRATEGY_CARD_KIND` 4, `strategy_card_label` 4 |
| `transactions` | `fragment_key`, `Offer`, `planet_value`, `FREE_FORM_PREFIX`, `offer_from_with`, `OPEN_KIND` |
| `fixtures` | `real_game` 6 (tests) |
| `core_adapter` (cut) | `typed` 2, `kernel_for`, `typed_map`, `scope` -> the live `kernel` already replaces it (`live/kernel.rs`); `kernel_for(sources)`'s `base.sources` needs the `Sources` struct only |
| `dual` (cut) | `note_bail` 2, `DualReport`, `bail_difference` |
| `production` | `PAY_KIND` 3, `PRODUCE_KIND` |
| `invasion` | `GROUND_CASUALTY_KIND` 3, `breakthrough_card` |
| `combat_live` | `combat_texts`, `after_nested`, `complete` -> glue (move to `live/`) |
| `technology_core` | `roster_for` 3 -> `live/` |
| `reactions` / `reactions_core` | `ACTION_CARD_KIND` 2, `arm`; `table`, `set_table_codes`, `id_text` (the shared registry builders, 737 lines) -> `live/` |
| `combat` | `RETREAT_KIND` 2, `RETREAT_TO_KIND` |
| `diplomacy` (cut: dropped) | `builder`, `window`, `candidates::built_bundle` |
| `production_view` | `kernel_for_core`, `kernel` -> `live/kernel.rs` |
| `tactical`, `promissory`, `trade_write`, `exploring_dual`, `faction_abilities`, `seating`, `neutral_units`, `breakthroughs`, `action_cards_view`, `tactical_live`, `strategy_cards_core`, `exploration`, `starting_tech`, `timing`, `laws` | 1-2 items each: `route_suffix`, `ACTIVATE_KIND`, `SUPPORT_PREFIX`, `note_key`, `codes`/`words`, `ability_card`/`MUNITIONS_COST`, `LOBBY_FACTIONS`, `NEUTRAL`, `for_faction`, `reveal_set_of_lines`, `complete`, `pool_details`, `pay_offer`, `KIND`, `ability_label`, `blanked` |

Plus `choice` (87 references) and `decision_context`, `event`, `rng`, `dice`, `fingerprint`, `preview`, `observation`, `generated` (the wire set copied in 8b1).
Larger than they look: `exploring_dual` (398 lines) and `reactions_core` (737) are the shared builders; verify each copy is the part the live path uses, not the dual half.

## Appendix B. The surface by module (union of host, server, wasm, rule-bot, rule-bot-typed, selfplay; production code; paths / legacy non-test lines)

Disposition: **W** wire copy (8b1) - **O** Observed (8b3) - **Q** query (delegate to core `query::*` or copy the pure function) - **K** constant (`kinds`) - **M** position builder (8c) - **S** setup (8c) - **X** wrapper (8e). "dev" = also used by dev tools.

| module | paths | lines | disposition and notes |
|---|---:|---:|---|
| `choice` | 19 | 3 176 | W + O + `Table`/`Decider` (copy); `ask_private`, `HIDDEN`, `redacted_full_state` -> O |
| `objectives` | 19 | 2 729 | Q (`leader, points_for, cost_of, family_token, remaining_position_progress, counting_progress, bought_progress_at, neighbour_planet_counts, home_planets_not_controlled`: core `query::objectives`), types `Imagined, Cost, Position, CardProgress` copy, `VICTORY_TARGET` K, `adjust_victory_points` M; dev |
| `production` | 17 | 4 584 | K (6 kinds), `Spend` copy, Q (`placements, capacity, buildable_for, available, price_of_under, native_payment_planets, trade_good_worth, planet_value_boosts`: core `rules::production`/`query::economy`); dev |
| `technology` | 10 | 2 161 | Q (core `query::tech`), `apply_unit_upgrades` M |
| `decision_context` | 9 | 574 | W |
| `tactical` | 9 | 1 346 | K, `Movable` copy, Q (`ship_routes, routes_of, movable_into, effective_move_value_for_ship, preview_moves`: core `query::reach`/`movement`) |
| `promissory` | 8 | 1 093 | Q (`deal, faction_name, owner_of, note_id, held_by, may_receive`), M (`take, receive, support`) |
| `game` | 6 | 2 331 | X + K (`CONTINUE_ACTION_ID/KIND`, `SECONDARY_PREVIEW_COMPLETE`) |
| `exploration` | 6 | 2 121 | Q (`traits_of, frontier_systems, attachment_for, resolution`), M (`explore, gain_fragment, mark_legacy_device_ids`) |
| `seating` | 6 | 966 (+2 172 incl. all) | S + K (`LOBBY_FACTIONS`, `seat_faction`, `MECATOL`, `SeatingError`) |
| `combat`, `invasion`, `transit`, `secrets`, `action_cards`, `planets`, `fleet`, `supply`, `reactions`, `strategy_cards`, `strategy`, `timing`, `relics`, `reach_query`, `laws`, `legendary`, `entropic_scars`, `faction_abilities`, `fracture`, `thunders_edge`, `diplomacy::candidates`, `factions::{assets, hooks_economy, mahact}` | 2-5 each | 295-6 545 | K (option kinds) + Q (delegate); `laws::apply_to_galaxy` M; `thunders_edge::breakthrough_for` and `diplomacy::candidates` are TE/dropped: delete the call sites in host (`DECISION 4`) |
| `movement` | 5 | 1 209 | `MapEdit`, `recorded_map_edits`, `MovementRules`, `Board` copy (types of the model/host map reconstruction), `replay_map_edits` pure on a `Galaxy` |
| `fleet_strength`, `valuation`, `preview`, `secondary_preview`, `dice`, `fingerprint`, `rng`, `event`, `observation` | 1-5 | 56-828 | copy (pure) / W; `secondary_preview` needs the core twin (`live/strategic.rs`): N |
| `setup`, `deck`, `starting_tech`, `map_feasibility`, `seeded_speaker`, `fixtures` | 1-3 | 204-420 | S |
| `core_sync::guard` | 2 | | glue (A) |
| rule-bot extras (`transactions::OPEN_KIND`, `promissory::SUPPORT_PREFIX`, ...) | | | K |

Regenerate with the script in the scratchpad of this document's author (`boundary_inventory.py` helpers `use_paths` over `split_prod_test`; 25 lines): the 8d agent commits it as `tools/engine_core/surface_paths.py`
with `--check`, so the table cannot rot.

## Appendix C. Method

* API usage: `tools/engine_core/boundary_inventory.py` (`rs_files`, `split_prod_test`, `use_paths`) over `crates/ti4-{host,server,wasm,rule-bot,rule-bot-typed,selfplay,policy,...}`; method counts by `grep -rn "\.name("`
  over non-test `src/`. Upper bounds where the name is generic.
* Glue ties: `crate::<module>::<item>` of the non-test text of the 8 glue modules (text before the first `#[cfg(test)]`), modules outside the glue and the wire set.
* Alpha saves: `/var/lib/ti4-public/game_*/init.json` read-only (`payload.initial_state.rules_version`, absent = 0); no data copied or printed beyond counts.
* Merge facts: `git merge-tree --write-tree --name-only` (`core-d27-split` + `disable-xf123`, `f472fd4e1` alone with `--merge-base=f472fd4e1^`, `core-d27-split` + the live branch).
* Cited documents: `D27_RESUME.md`, `todo/D2-7-legacy-split.md`, `todo/D2-remaining.md`, `todo/D2-prereqs.md`, `D2_8_0_AUDIT.md`, `D2_LF_RESUME.md`, `D2_LF2_RESUME.md`, `M7_RESUME.md`, `INTEGRATION15.md`,
  `ENGINE_CORE_STATE.md`, `RULES_VERSION.md`, `OFFLINE_SAVE_RESET_NOTE.md`; from other branches: `BAIL_MAP.md` (`core-bailmap`), `DISABLE_XF123.md` (`disable-xf123`), `HIDDEN_INFORMATION_AUDIT.md` (`redact-hidden-decks`).
* Not done: any cargo build or test, any timing, any heavy run; therefore no performance or wasm-size number is measured here (R2, R3).
