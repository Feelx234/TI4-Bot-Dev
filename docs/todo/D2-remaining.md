# TODO D2-remaining: what is left of D2, against the tree of `core-integration-8` (2026-10-31)

Theory document (read-only analysis: no cargo, no builds, no tests were run). Branch `core-todo-d2b`, cut from `core-todo-d2` (`b84e7228`, the int-3 plan
`docs/todo/D2-prereqs.md`, which this file refreshes; read that file only for the parts this one points to). Tree read: `core-integration-8` @ `14593a23`
(through `git show` and a detached scratch worktree; line numbers are from that tree: re-grep before editing). Only this file changed.

**Labels.** *Measured* = I counted it in the tree for this document. *Cited* = from a document of the tree. *Estimated* = my judgement (agent-hours, calibration of
`D2-prereqs.md` 2.2: window/stage work 0.2-0.35 of document estimates, leaf work 0.5-0.7). **VERIFY** = a 10-minute check the agent does first.
**END-OF-PHASE ONLY** = a heavy run (`/root/heavy2.sh`, `SHARDS=3`, private `CARGO_TARGET_DIR`): never inside a package, only in the gate named for it.

---

## 0. Summary and findings that change the plan

1. **Done on the int-8 line (do not redo):** D2-1 (fixtures, `core_sync::unmapped`, `guard`, `alpha_roundtrip`, `marks_census.py`), D2-2 (disabled-feature policy), D2-3 (text
   tables in `CoreState`), D2-4a/4b (resolver image, `ResolverState` in `Pending`), D2-5 *mechanism* (`core_sync::materialize`, `reverse_lend::with_string`, `StringTape`, footprint
   audit, `lend_table` probe + `bail_table` bin, `TI4_CHURN=state`), D2-6a (`allow_list.rs`, bin `allow_list_check`, `tools/engine_core/allow_list.json` is `{"entries": []}`). Their gate
   is the heavy run `tools/engine_core/end_of_d1.sh` (20 steps), which has **not** been run yet; it is the first thing that happens (section 2, step X0).
2. **`lend_table::BAILS` is empty** (`lend_table.rs:38`) and the two table tests (`missing_entries`, `stale_entries`) run only on synthetic input. The probe records
   **only bails of `dual_window!` windows** (`dual.rs:1283`, inside `run_dual_window`). It does **not** see: the `[gate: ..]` rows (`note_gate`, `dual.rs:1352`: a position the call site
   runs bare, no window opened), a bail of a window nested in another window (the outer label carries it), the `None` fallbacks of `dual_typed!` (164 sites), `dual::write` (17 + 48
   `dual_write` sites) and `dual!` (87), the string slots of `dual_hook!` (4), and the question half `run_dual_ask` (3). All of these are string code the flip must still reach. So the
   table from the probe is a **lower bound of the lent set**, not the set (section 3.1, R3).
3. **The core half of a window is a replay adapter, not a driver.** `dual_window!` with the `Core` backend runs the *string* closure (`dual.rs:1479`: `String | Core => ($string)($state).value`);
   the core closure is called with the string side's recording (`Recording`) and answers from it (`dual_window.rs` header, `drive`). The window *logic* is core code, but the *construction*
   of each window from the call site's string arguments, and the composition (who opens whom), are string. `driver::step::Session::step` (D1b2) runs a handful of arms and returns
   `Step::Lend(..)` / `Ran{lent}` for the rest; `Window::Invasion` / `Window::Production` are `Untyped` placeholders (`state/pending.rs:36-39,122`); `docs/ENGINE_CORE_DRIVER.md` 10-11 and
   `D1b2-bail-01..04` list what is not run. **D2-8 as written in `D2-prereqs.md` ("mostly mechanical once D1b has moved the window fields") underestimates the flip.** Section 3.4 splits it
   and adds a one-hour audit (D2-8.0) whose result picks the architecture; section 7 asks you.
4. **`disable-xf123` (product edges, `docs/DISABLE_XF123.md`) is NOT in int-8** (`git merge-base --is-ancestor disable-xf123 core-integration-8` fails; a trial merge conflicts in
   `ti4-host/src/view/scenario/mod.rs`, `ti4-server/src/session/{mod,registry,worker}.rs`, `ti4-server/tests/it/main.rs`; measured). The engine-side policy exists
   (`core_sync::unsupported`, `ti4_engine_core::unsupported`), the **refusal at game creation and at recovery does not**. The flip needs it merged first (D2-8 prerequisite P1).
5. **Deviations from `D2-prereqs.md` that already landed:** the default of `core_sync::unsupported::Policy` is `Off`, not `Bail` (`D2_1_RESUME.md`: a Bail default would zero `compared >= 1` of lockstep tests);
   the disabled list has **nine** families (`Diplomacy, DeckReserve, LastBastion, Crimson, Keleres, Deepwrought, Firmament, Obsidian, RalNel`, `ti4-engine-core/src/unsupported.rs:23`), not four; the
   `Driver::new` count is **176** (`grep "Driver::new("` over `crates`), of which **100 are production code of the engine adapters** (33 in `dual_window.rs`, 11 `leaders_core.rs`, 7 `faction_dual.rs`, 6 each
   `technology_dual.rs`/`hero_dual.rs`, 5 each `strategy_cards_core.rs`/`production_window_dual.rs`/`cannon_dual.rs`, 4 `tactical_dual.rs`, ...) and 76 are in tests or test modules (the "~190" of the plan).
6. **Three process-wide switches, all default off, none set by any product binary:** `product::set_strict` / `TI4_CORE_STRICT` (`product.rs:39-60`; W17 product assertions), `core_sync::unmapped::set_strict` /
   `TI4_UNMAPPED=strict` (`unmapped.rs:81-98`), `core_sync::unsupported::set_disabled_policy` (`unsupported.rs`; also sets the core's `unsupported::set_policy`). The gate bins call the `_for_gate` forms of the last two;
   **no gate sets `product::set_strict`**, and `end_of_d1.sh` has **no `TI4_CORE_STRICT=1` step** although `INTEGRATION7.md` 6.5/7.3/8.2 list it under "not run" (section 3.4, step X0b).
7. **The git state:** the integration line (int-8) shares `93d1b266` with the live branch `nightly-fixes-2026-10-26`; the live branch has 8 commits since, of which exactly **one touches `crates/ti4-engine/src`**
   (`promissory.rs`, +16/-1; measured). The move (D2-7) must be preceded by merging the then-current live branch into the integration line, or those nightly fixes conflict with the rename.

---

## 1. Facts the plan stands on (measured on `core-integration-8`)

| Fact | Evidence |
|---|---|
| Merged: `core-d2-1`, `core-d2-3` (carries 4a, 4b, 5-mechanism), `core-d2-6a`, `core-d1a`, `core-d1b`, `core-d1b2`, `core-p1`, `core-d3-0`. Not merged: `disable-xf123`, anything named `core-d2-2`/`d2-4a`/`d2-5`/`d2-6b` (they exist as commits inside `core-d2-1`/`core-d2-3`) | `git merge-base --is-ancestor` |
| `crates/ti4-engine/src/` has `resolver_sync.rs` (741 lines), `reverse_lend.rs` (346), `lend_table.rs` (233), `churn.rs` (37), `core_sync/{materialize,unmapped,unsupported,guard,texts,footprint}.rs`, `product.rs`; **no** `Game::with_string`, **no** `Game::materialize`, **no** `GameError::Unmapped` | listing; `grep "pub enum GameError"` (`game.rs:101`) |
| `reverse_lend::with_string<T>(core: &mut CoreState, cx: &mut Ctx, base: &GameState, galaxy: Option<&Galaxy>, fp: Footprint, label: &'static str, tape: &[String], f: impl FnOnce(&mut StringCtx<'_>) -> T) -> R<Lent<T>>` (`reverse_lend.rs:115`); `StringCtx { state, galaxy, timing, rng, dice, seq, events }`; `Lent::{Done(LentOut{value, log, events, rolls}), Asking(Box<Choice>)}`; a body that writes outside `fp` is refused with `Unmapped("lent body wrote outside its footprint")` and nothing is stored | code |
| `Bail { label, writes, reads, ids, test }`, `BAILS: &[Bail] = &[]`, `probe(label, pre, post)` (env `TI4_BAIL_PROBE=FILE`, append mode, one `label<TAB>Slice,Slice` line per bail), `merge_probe`, `render_table`, `missing_entries`, `stale_entries`, `changed_slices` | `lend_table.rs` |
| `slice_of_field` maps a top-level `GameState` JSON key to a slice and **falls back to `Transient`** for an unknown key | `materialize.rs:~17` |
| 126 `dual_window!` sites, 113 free functions taking `&mut GameState` (+ context) and **13 methods on `self`** (window structs: `combat::announce_end`, `assault_cannon`, `barrage_step`, `begin_round`; `game::committee_formation`; `tactical::finish`, `tactical::free`; `turn::minister_of_war`; `invasion::advance_fighting`, `bombard_step`, `roll_ground`, `roll_ground_tail`, `space_cannon_defense`) | scripted grep |
| Most window bodies already exist as separate `*_string` functions (W-lanes renamed them: `trade_primary_string`, `production_biomes_string`, `roll_round_string`, ...) with signatures like `(state, content, sources, galaxy, table: &mut Table, player)` | `strategy_cards.rs:2459`, `combat.rs:6403` |
| Game: `pub struct Game<'a>` (`game.rs:1157`) has `pub state`, `pub table`, `pub events`, `pub timing` and ~25 private fields (windows: `secondary`, `scoring`, `tokens`, `voting`, `tactical`, `aftermath`, `trade`, `diplomacy`, `agenda_talks`, ...); `GameSnapshot` (`:1266`) and its FORK RULE (`snapshot`/`instantiate`/`into_game` destructure without `..`); `step` at `:2356` (hooks `churn::resolver`/`churn::state`, `d1_pending::check`); constructors `new :1659`, `with_seeded_random :1669`, `with_table :1700` | code |
| `AreaMask::build_default()` (`dual.rs:172`): `Core` for Movement, Objectives, TradeDiplomacy, ExplorationRelics, ActionCards, Strategy, AgendaLaws (view/write areas); **everything else (Combat, Invasion, Production, Kernel, LeadersAbilities, Factions, Driver, Technology, BoardQueries) is `String`** in a production build. `typed-core` is a default feature, so core + factions are already in the server and wasm | code |
| `driver::step::Session::step` runs: turn prelude (Spec Ops return, mark prepared), end of the action phase, strategy-phase flip, the question; **lent by name** (`Lent::{Arm, TurnHooks, StrategyHooks, PhaseEvents}`): the windows of the finished areas, tactical, aftermath chain, status/agenda halves | `driver/step.rs` |
| Where the host starts a game: `ti4_host::setup::create` (`setup.rs:504`, already calls `core_sync::guard::ids_supported`/`factions_known`); the engine constructors above are called from `ti4-server/src/session/worker.rs:1440`, `ti4-host/src/{batch.rs:1544,1760, replay.rs:189, step_snapshot.rs, history/*.rs}`, `ti4-wasm/src/lib.rs:110,1827`, `ti4-wasm/src/host_api.rs:456` (`HostSession::new`), `ti4-policy/src/bot.rs:2437`, training/sim/selfplay; the server recovers through `storage.rs:1034 recover_session`, `:1088 recover_from_init`. There is no `#[wasm_bindgen(start)]` in `ti4-wasm` | grep |
| `core-bridge` already has `golden` (frozen per-`RULES_VERSION` recordings, `tools/engine_core/golden/`; `golden check --mask all=core|all=string` replays recorded answers and compares state and V2 hashes: a legacy-independent check), `dual_run` (+ `dual_shard`), `core_sync_gate`, `board_equivalence`, `decision_corpus`, `alpha_roundtrip`, `bail_table`, `allow_list_check`, `state_size` | `ls src/bin` |
| Dependents of `ti4-engine` (Cargo): host, server, wasm, rule-bot, rule-bot-typed, selfplay, policy, training, sim, mlp, review, replayer, advisor, calc, bridge, legacy-stub `ti4-legacy` (the Python converter, unrelated to `ti4-engine-legacy`), cli, core-bridge. The bridge also depends on selfplay, rule-bot, host, server and wasm | `grep ti4-engine crates/*/Cargo.toml` |
| Path-bound artifacts naming `crates/ti4-engine/` outside `docs/`, `plans/`: `xtask/src/content_gen/mod.rs:56`, `ti4-engine-core/Cargo.toml:13`, `ti4-engine-core/tests/adapter.rs:3`, `tools/engine_core/{clippy_core.sh:25-26, check_layers.py:186, d4/coverage.py, d4/test_inventory.py:26, marks_census.py, verify_package.sh:61}`, `tools/engine_core/clippy_engine_baseline.txt` (42 lines keyed by file path), `tools/engine_core/d4/test_disposition.tsv` (4 544 lines, column 2 is a file path), `tools/engine_profile/module_graph.py`, `tools/card_audit/{card_audit.py,curation.json}` (145 refs), `scripts/make_faction_reference.py`, `crates/ti4-engine/tests/{decision_delivery_inventory,content_ids_resolve}.rs` (scan `CARGO_MANIFEST_DIR/src`), `tests/{decision_replay,reaction_corpus}.rs` (`../../tools/engine_core/decision_corpus`), `.cargo/config.toml` aliases (`c-web`, `l-web`, `t-web`, `t-web-slow`, `wire-web`, `b-web`: all `-p ti4-engine`). Also web/ comments and two screenshot scenes name engine files (comments only, do not edit generated manifests) | `git grep` |
| `end_of_d1.sh` writes (default `--out /root/end-of-d1-<UTC date>`): `summary.txt`, `<step>.log`, `bail-probe.tsv` (step `dual-all`), `dual-all.log` (the merged dual report), `dual-all-shards/shard-<i>.json|.log`, `bails.rs.txt` (step `census`), `bench-*.txt`, `soak*`, `mutation/` | `end_of_d1.sh` |

---

## 2. Order, dependencies, lanes

```text
X0   end_of_d1.sh (heavy, one run, the coordinator)  -- produces bail-probe.tsv + the lent census + proves D2-1..6a
X0b  strict-product run (TI4_CORE_STRICT=1) -- missing from end_of_d1.sh; folded into X0 or run right after
 |
 +--> D2-5r  (R1 consume the probe, R2 BAILS rows, R3 lent-site inventory, R4 wrappers, R5 lend-proof mode)       needs X0 for R1/R2; R3 and R4's builders start now
 +--> D2-6b  (a: harness + subject trait + allow-list wiring, startable NOW; b: corpora + shards + legacy subject, after D2-7)
 +--> P1: merge disable-xf123 into the line (a decision of yours: question 4)
 |
D2-7 legacy split (needs: freeze; live branch merged in; nobody editing crates/ti4-engine/src)  --> D2-8.0 audit --> D2-8a..8g  --> D2-9 (+ G7)
```

| Pkg | Goal | Needs | Est. agent-h | Heavy |
|---|---|---|---:|---|
| **X0** | run `end_of_d1.sh` once; hand back `OUT` | quiet window (nightly rule: not 20:30-24:00 Berlin; `TZ=Europe/Berlin date`) | 0.5 + ~6-8 h wall | the run itself |
| **D2-5r** | real `BAILS`, lent-site inventory, wrappers, footprint proof | X0 (R1, R2); R3, R4 builders independent | 16-24 | R5 proof run: END-OF-PHASE ONLY |
| **D2-6b** | `oracle` bin (a: now, b: after D2-7) | D2-6a (done); b needs D2-7 | 7-10 | G-flip / G7 only |
| **D2-7** | `ti4-engine-legacy` + facade | P1 done, live merged, freeze | 8-12 | none (cheap checks + one bench hash run) |
| **D2-8** | the flip (8.0 audit, 8a..8g) | D2-5r, D2-7, P1, X0 result | 30-45 (**estimated, range wide: section 3.4**) | G-flip: END-OF-PHASE ONLY |
| **D2-9** | production strip + G7 | D2-8, D2-6b | 6-8 + gate | G7: END-OF-PHASE ONLY |

**Lanes (section 6 has the briefs):**

* **Lane L1 (now, no heavy):** D2-6b(a) oracle harness; D2-5r R3 lent-site inventory tool; D2-8a `Driver::new` unification behind a "registry off" flag (it changes nothing while the flag is off).
* **Lane L2 (after X0):** D2-5r R1/R2/R4/R5 (two agents on wrappers split by file group).
* **Lane L3 (freeze):** D2-7, then D2-8.0..8g sequentially (one owner; 8c/8e can be a second agent), D2-6b(b) in parallel with D2-8, D2-9.
* **Coordinator:** X0, X0b, P1 (with your decision), the merge of the live branch before D2-7, G-flip, G7.

---

## 3. Packages

### 3.1 D2-5r `core-d2-5r-bails-and-wrappers` (the rest of D2-5)

**Definition of done.** `lend_table::BAILS` holds one entry per lent site of the golden/lent/bench corpora with `{label, writes, reads, kind, args, ids, test}`; a wrapper runs the string body of every
entry through `reverse_lend::with_string`; the **lend-proof mode** (R5) shows that running each labelled body through its wrapper gives the byte-identical state of the plain string run on every call of the corpus;
the two table tests run against a checked-in label list; the census of `dual-all` and the table agree (`bail_table --check`).

**R1. Produce and consume the probe output (X0 is the producer).**

Produce (coordinator, from a worktree at the end-of-D1 commit; the script re-executes itself under `/root/heavy2.sh` and holds it):

```text
export CARGO_TARGET_DIR=/root/deploytmp/eod          # private, not the live checkout's
tools/engine_core/end_of_d1.sh --out /root/end-of-d1-2026-10-31                 # all 20 steps
# only what the table needs (census re-reads the probe and runs golden check; build builds every bridge bin):
tools/engine_core/end_of_d1.sh --out OUT --only build,dual-all,census
```

`dual-all` runs `TI4_BAIL_PROBE=$OUT/bail-probe.tsv TI4_DUAL_FAIL_FAST=1 SHARDS=3 tools/engine_core/dual_sharded.sh $T/gate/dual_run tools/engine_profile/profile-20.json 20 all=dual $OUT/dual-all-shards`.
Outputs that matter: `OUT/bail-probe.tsv` (lines `label<TAB>Slice,Slice` or `label<TAB>-`, interleaved from the three shards: append mode, the order is meaningless), `OUT/dual-all.log` (the merged report:
`<label>: N compared, M bailed`, `<label> [bail: why]: 0 compared, M bailed`, `<label> [gate: why]: ...`), `OUT/dual-all-shards/shard-i.json`, `OUT/bails.rs.txt` (step `census` ran `bail_table OUT/bail-probe.tsv`),
`OUT/summary.txt`.

Consume (D2-5r agent, cheap, on a copy of `OUT`; the tools are python and the `bail_table` bin):

1. **Validity.** `summary.txt` must say `ok dual-all`. `TI4_DUAL_FAIL_FAST=1` makes a shard stop at its first difference: a failed `dual-all` leaves a *truncated* probe. Then fix the difference first (regression test first)
   and re-run the step; do not freeze a table from a truncated file. `grep -c . bail-probe.tsv` must be > 0 (the `census` step refuses an empty file).
2. **Census text.** `tools/engine_core/lent_census.py OUT/dual-all.log --games 20 --md > OUT/lent-census.md` (exit 1 means a silent bail: `NO REASON RECORDED`, or reasons that add up to less than `bailed`). `end_of_d1.sh`
   does not run this; run it. It is also the source of the `[gate: ..]` rows (R3). For the nested detail add `TI4_NESTED_LOG=1` in a second, cheaper dual run if a label shows `nested (..)` reasons you cannot map.
3. **Rows.** `bail_table OUT/bail-probe.tsv` prints `pub const BAILS: &[Bail] = &[ Bail { label, writes: SliceSet::of(&[..]), reads: SliceSet::ALL, ids: &[], test: "" }, .. ]`. Paste into `lend_table.rs:38`.
   Observed writes are a **lower bound**, and two systematic gaps: (a) `changed_slices` compares `pre = the copy the core would have run on` with the string result (`dual.rs:1272,1283`); if a core half mutated its copy before returning `None`
   the diff is wrong (**VERIFY** with the label list: expect none); (b) an unknown top-level `GameState` field lands in `Transient` (`slice_of_field` fallback) - **VERIFY** that every key of `serde_json::to_value(GameState)` is named in the match, else add it.
   Widen by hand with the string body's writes (read the `*_string` function) and then let R5 prove it.
4. **ids, test.** `ids`: the issue ids of `docs/ENGINE_KNOWN_ISSUES.md` (`B12-bail-*`, `W*-bail-*`, `D1b2-bail-*`, `W2-bail-02`, ...). `test`: the test that pins the bail; `tools/engine_core/d4/coverage.py` already maps `dual_window!` labels
   to bail tests (`--check` is step 4 of `d4_gate.sh`): reuse its map rather than searching by hand.
5. **Cross-check tool.** Extend the `bail_table` bin with `--report OUT/dual-all.log` (new, small): for every plain row with `bailed > 0` there must be a probe line, except the labels the report marks as nested-only; print `label, calls, bailed, share, writes` and exit 1 on a bail without a probe line.
6. **Corpus width.** The probe saw 20 games of `profile-20.json` at `rules_version` 23, no `sources`. A label that bails only elsewhere is missing. Before freezing, run the probe again, **END-OF-PHASE ONLY**, on `tools/rulebot/golden-lent.json`
   (19 games, six factions per table chosen for guards), the wide mask (`$WIDE` of `end_of_d1.sh`), and `soak.sh run` (its `dual_run` calls inherit `TI4_BAIL_PROBE` from the environment; use a different file per corpus and merge:
   `bail_table A.tsv B.tsv C.tsv`). Add them as one extra step `dual-lent` to `end_of_d1.sh` (same lock) so the next X0 has them.
7. **Table tests.** Replace the synthetic test data with a checked-in `tools/engine_core/bail_labels.txt` (label per line, from the census) and two tests: `every_bailing_label_has_an_entry` (file vs `BAILS`) and `no_entry_is_stale`
   (reverse). The live comparison against a real `DualReport` is `bail_table --check` in the gate (item 5); a unit test cannot afford a golden game.

**R2. Extend `Bail` (small, mechanical).** Add `kind: Kind { Window, Gate, Fallback, Question }` (where the string runs: a bailing window, a `[gate:]` position, a `dual_typed!`/`write` `None` fallback, a `run_dual_ask` question half),
`args: Args { Plain, Typed, Island }` (how the body's window struct crosses: plain data in the call's arguments; a typed `Window` with a round trip, e.g. `Window::Combat(CombatFields)`, `d1::pending.roundtrip`; not portable) and
`when: When { Start, Mid }` (the core bails before it asked the player anything, or after). `Mid` needs the tape (R4). Derive `when` from the census reasons: `gate:` and "the typed state could not be lent", "a seat the production windows do not carry" are `Start`;
`nested (..)`, "the core window refused the recorded answer or stage", "the machine: a position it does not carry" are `Mid`.

**R3. The inventory beyond windows (the part the probe cannot give).** New python tool `tools/engine_core/lent_sites.py` (text-based like `module_graph.py`): for every `dual_window!(`, `dual_typed!(`, `dual!(`, `dual::write(`/`dual_write(`, `dual_hook!(`,
`run_dual_ask(`, `note_gate(` in `crates/ti4-engine/src` print `file:line, macro, label, enclosing fn, receiver (free fn / method on self), has a *_string body (the closure calls one) y/n`. Output `tools/engine_core/lent_sites.tsv` (committed). Join with the census by label to
give `calls per 20 games` and the class of each site. This is the **work list of the flip** and the source of `Bail.kind/args`. Measured sizes today: 126 window sites (13 on `self`), 164 typed, 87 plain, 65 write, 4 hook, 3 ask, 8 gate notes. Startable now.

**R4. Wrappers.** One per `BAILS` entry with `Kind::Window | Gate | Fallback`. Shape (new file `crates/ti4-engine/src/lent.rs`, `#[cfg(feature = "typed-core")]`; moves to the wrapper crate at D2-8):

```rust
pub fn trade_primary(game: &mut LentGame<'_>, player: &PlayerId) -> R<Lent<Result<(), IllegalChoice>>> {
    reverse_lend::with_string(game.core, game.cx, game.base, game.galaxy, FP_TRADE_PRIMARY, "strategy_cards::trade_primary", game.tape,
        |s| strategy_cards::trade_primary_string(s.state, game.content, game.sources, s.galaxy, &mut s.timing_table(), player))
}
```

Rules: the wrapper calls the existing `*_string` body, never the enclosing `dual_window!` function (no `with_mask` trick needed once the body is separate; where no `*_string` body exists, extract it as a pure move: one commit per file, no logic change,
`decision_delivery_inventory` rows renamed in the same commit as in the W-rounds). Questions: the table inside `StringCtx.timing` answers from the `StringTape`; a dry tape returns `Lent::Asking(choice)` after rolling back (already implemented and tested).
`Args::Plain`: ~10-15 min each (estimated). `Args::Typed`: the converter pair `X_of(&Game)->Window` / `X_back(&Window)->string struct` exists for voting, tokens, trade, leader followers (`d1_windows.rs`), combat fields (`d1_pending`); the missing ones are written on demand.
`Args::Island`: **no wrapper**, listed for D2-8.0 (invasion machine, production step, aftermath chain; section 3.4). The 13 `self` methods are `Typed` or `Island`: decide per method in R3, do not build a window-kind codec.
Footprint audit stays on (debug and lend-proof runs). Determinism requirement (cited `ENGINE_AREA_STATUS` 2.2 test 3): the body reads nothing outside (state, window struct, tape): `rng_force`, `aftermath_observer`, `strategic_observer`, `blocked` are unreachable in a `StringCtx`; if a body touches one, the label is not lendable - report it.

**R5. Lend-proof mode (the strongest test, END-OF-PHASE ONLY).** Env `TI4_LEND_PROOF=1` (native only): in `run_dual_window`, after the plain string run, run the same body through its wrapper on a `CoreState` loaded from the *pre* state and compare the materialised result
with the plain string state (JSON, then bytes), the returned value, the events text and the rng positions; any difference is a `DualReport` difference named `<label>.lend`. Covers **every call** of a labelled window (e.g. `invasion::commit_one` 2 030 calls per 20 games), not just the 2-50 that bail, and so
validates the footprints and the tape protocol on the whole corpus. Cheap check first: 1 game with `--game I`; the heavy run is part of G-flip.

**Cheap checks (runners):** `cargo test --profile lockstep -p ti4-engine --lib lend_table:: reverse_lend:: core_sync::`; `cargo test -p ti4-engine-core-bridge --lib`; the new tool's unit test (`python3 -I -m unittest` style like `d4/coverage.py`'s self-test);
`cargo clippy -p ti4-engine -p ti4-engine-core-bridge --all-targets` (baseline `clippy_engine_baseline.txt`, may only shrink); `check_layers.py core`, `lint_core.sh`.
**Traps:** T-A the table is a union over runs: a stale entry hides a landed port - keep `no_entry_is_stale` honest; T-B `Bail.writes` too small is not a correctness bug (the audit refuses and stores nothing) but a production error: err wide; T-C a label can bail *before* the user answered (Start) or *after* (Mid): after, the string body re-asks from the start and the tape must hold all earlier answers of this window, in order;
T-D `ProductionWindow::step` is a window (label without `::`): treat as any other; T-E labels with `?` in the scripted listing (`diplomacy/window.rs`, `fleet.rs`, `game.rs`, `transactions.rs`) are `dual_window!` calls whose label sits in a variable or on a continuation line: read them by hand, do not skip them.

### 3.2 D2-6b `core-d2-6b-oracle` (legacy vs new, permanent)

**Definition of done.** Bin `oracle` in `ti4-engine-core-bridge` plays each scenario on subject A (legacy) and subject B (the new engine), feeding B's chosen option ids to A, and compares after every step: the choice (ids, prompt, `DecisionContext::canonical`), the materialised `GameState` JSON, new events, rng positions and V1/V2 decision hashes. Output goes through the `DualReport` text and the shard JSON of `dual_shard.rs` (`--shard i/N --out FILE`, `--merge`), the allow-list (`allow_list::AllowList::judge`, `Tally::check_stale`) is applied, a skipped-scenario count is printed, exit 1 on an unallowed difference or (full run) a stale entry.

* **(a) Startable now, no dependency on the flip.** Trait `Subject { fn new(spec) -> Self; fn options(&self) -> Option<Choice>; fn answer(&mut self, option_id: &str); fn step(&mut self); fn state_json(&self) -> Value; fn events_since(&mut self) -> Vec<String>; fn rng_positions(&self); fn finished(&self) -> bool }`.
  Two impls today over the *same* crate: `MaskSubject(AreaMask::STRING)` (= legacy behaviour) and `MaskSubject(core mask)`, switched per call with `dual::with_mask` (thread-local, `dual.rs:295`). At D2-8 the second impl becomes the wrapper `Game`, at D2-7 the first becomes `ti4_engine_legacy::game::Game`. Tests (D2-6b item 5 of `D2-prereqs.md`): subject A vs A reports 0; an injected one-field difference is reported with its path; an allow-listed one passes; a stale entry fails the full run. Scenario filter: skip + count scenarios for which `core_sync::unsupported::find(&state).is_some()` (not allow-listed).
* **(b) After D2-7:** `ti4-engine-legacy` as a normal dependency of the bridge; corpora: golden 20 (`profile-20.json`), `tools/rulebot/golden-lent.json`, bench configs v1..v5 (`tools/rulebot/golden*.json`), template openings (`workload::template_openings`), `decision_corpus` replay, the **golden sets** of `tools/engine_core/golden` (replayed by recorded answers: independent of legacy, keep both checks), alpha replays on a COPY (`alpha::list_games`, `replay_session` on both subjects; 11 of 14 replayable, 3 turn-redo skipped).
* The existing `dual_run` (inline dual) stays until D2-9 removes the dual wiring from the production crate; it moves with legacy. The oracle replaces it in `verify_package.sh`/`end_of_d1.sh` (`AREAS` argument dropped).

**Traps:** T-F the oracle is valid per `RULES_VERSION`: a rule fix in the core only (Heart of Ixth B1, Harrow B2-K7, Xxekir Grom B7-copy-01) needs a `RULES_VERSION` bump and an allow-list entry with `rules_version_min`; an entry without a reason or a bound is refused by the parser. T-G legacy compiled with its own `typed-core` code must run `AreaMask::STRING` (D2-7 item 5) or it compares the core with itself. T-H the oracle subject for B drives the bot through `ti4-rule-bot`, which reads `Observed` views (`obs()` 500 call sites): the views must come from B, not A.
**Cheap checks:** `cargo test -p ti4-engine-core-bridge --lib`; a 2-game smoke of `oracle` (`--game 0`); no heavy run until G-flip.

### 3.3 D2-7 `core-d2-7-legacy-split` (freeze; after P1 and the live merge)

`D2-prereqs.md` D2-7 stands (whole-crate `git mv` to `crates/ti4-engine-legacy`, package and lib rename, new facade `crates/ti4-engine` = `pub use ti4_engine_legacy::*;` plus forwarded features, path fixes in separate commits, `git grep -n "crates/ti4-engine/"` empty outside docs/plans at the end). What changes against the tree of int-8:

0. **Before the move:** merge the current live branch into the integration line (today one engine file differs, `promissory.rs`); run `boundary_inventory.py --json $SCRATCH/inv.json` and record the classification (a/b/c) as a table comment in the facade `lib.rs`.
1. **The path list is longer than in `D2-prereqs.md`** (table in section 1): add `tools/engine_core/clippy_engine_baseline.txt` + `clippy_core.sh:25-26` (the baseline keys are `crates/ti4-engine/src/x.rs<TAB>message`: rewrite with `sed` in the same commit and re-check that the count does not grow), `tools/engine_core/d4/{test_disposition.tsv, test_inventory.py:26, coverage.py}`, `tools/card_audit/*`, `scripts/make_faction_reference.py`, `.cargo/config.toml` (`b-web` names `--test content_ids_resolve`: that test moves with the crate: add `-p ti4-engine-legacy`). One commit per file group, each listed in the report (rename-only edits of `decision_delivery_inventory.rs`/`content_ids_resolve.rs` need your yes: question 5).
2. **The facade owns nothing yet and the `Game` types are the same type.** Until D2-8 the oracle (b) has nothing to compare; that is why (a) of D2-6b uses masks. Do not put wire types (`choice`, `decision_context`, `fingerprint`, `event`, `dice`, `rng`, `observation`) in the facade: they stay in legacy and are re-exported, because 118 string modules import them; "frozen" means **no rule-body edits**, not no edits.
3. **The glue stays in legacy.** `core_sync`, `render/*`, `resolver_sync`, `reverse_lend`, `lend_table`, `churn`, `product`, `d1_*`, `*_dual.rs`, `*_core.rs` (measured: 17.2 k lines in `dual.rs`, `dual_window.rs`, `*_core.rs`, `*_dual.rs`; 7.3 k in `core_sync`; 5.5 k in `render`; 1 k in `kernel_sync`) are called by 440 macro sites in the string modules; stripping them from legacy means editing every site. Keep them compiled in legacy (runtime mask `STRING`, `oracle` feature as in `D2-prereqs.md` D2-7 item 5). The wrapper (D2-8) needs `core_sync`, `render`, `resolver_sync`, `reverse_lend`, `lend_table`: it **copies** them out of legacy at D2-8b (a `cp`, history is in git; the two copies diverge on purpose) and the facade stops re-exporting them. Plan that copy as its own commit pair so the diff is reviewable.
4. **Names:** the workspace already has `crates/ti4-legacy` (package `ti4-legacy`, the Python converter stub); write `ti4-engine-legacy` in full everywhere.
5. **Cheap checks (runners):** `cargo check --workspace --all-targets` with the alias package sets; `cargo test -p ti4-engine-legacy -p ti4-engine-core-bridge --no-run`; `cargo test -p ti4-engine-legacy --test decision_delivery_inventory --test content_ids_resolve`; `cargo xtask gen-cards --check`; `check_layers.py core`, `lint_core.sh`; `python3 -I tools/engine_core/check_layers.py engine` (path argument). One bench-hash run (`verify_benches.sh`, 200/200 x6) on the facade build is **END-OF-PHASE ONLY** (the coordinator, in G-flip).
6. **Not in the last hour before 20:30 Berlin** (nightly rule), and announce the new path to nightly fixers the moment it merges: their fixes target `crates/ti4-engine-legacy/src` afterwards and the package owner re-applies them to the core (`ENGINE_CORE_STATE.md` 6.5).
**Trap T-I:** the facade's `Cargo.toml` and `src/lib.rs` sit at the paths of the old crate's files; a later merge of a nightly branch that edited the old `lib.rs`/`Cargo.toml` conflicts there (git follows the other files by rename). Merge the live branch immediately before the move and again right after, in the same hour, and tell the sweeper.

### 3.4 D2-8 `core-d2-8-flip` (split; the unit of work is bigger than the plan said)

**Definition of done (unchanged):** `ti4_engine::Game` is a wrapper that owns a `CoreState`; `step`/`answer`/`legal_options` come from the core; `Game::state()` is the cached materialisation; checkpoint = `CoreState::clone` + the small wrapper fields; leftovers run through `with_string`; the
disabled features and `Unmapped` are errors/panics in the product; the bots, host, server and wasm see the same API and the same bytes (`GameSnapshot`, fingerprints, saves, wire/TS unchanged).

**D2-8.0 (1-2 h, read-only, first): the completeness audit; its table decides the architecture.** Produce `docs/D2_8_AUDIT.md` (the one allowed doc of the package) with, per row:
every private window field of `Game` (`secondary`, `secondary_after_tactical`, `leader_strategy`, `leader_followers`, `scoring`, `event_scoring`, `tokens`, `voting`, `executive_order`, `agenda_queue_after_event_scoring`, `tactical`, `aftermath`, `trade`, `diplomacy`, `agenda_talks`, ... `game.rs:1157-1250`) against `state::WindowKind` /
`Window` (typed with round trip / `Untyped`), and every `driver::step::Lent` / `Arm` variant and every `D1b*-bail-*` row against "core runs it / lent". Three classes: **Core** (the core opens, drives and closes it), **Portable** (string window struct <-> typed `Window` round trip exists: lend by label with `Args::Typed`), **Island** (no machine state in the core: invasion, production step, aftermath chain, ...).
Decision rule: if the Islands are at most three and each has a clear open and close site, take **Architecture A**; otherwise **B**:

* **A (target, matches `ENGINE_CORE_STATE.md` 9.2): the core owns the loop, windows and state; leftovers are label lends.** Islands are held by the wrapper as opaque string window structs next to the `CoreState` in a small `Lent` field (`Option<InvasionWindow>`, ...), cloned in checkpoints (they are `Clone` already: `GameSnapshot` clones them), entered at a labelled opening site and left when the window closes; during an island every `answer()` is a `with_string` that moves the struct in and out. Needs the core to own the *composition* (D1b2-bail-01..04: the chain after the head, hooks between turn steps, status/agenda halves) - if the audit shows those are still lent arms, they are islands too.
* **B (transitional scaffold): the wrapper embeds one legacy `Game` as the lender.** Authority per step: core-run arms operate on `CoreState`; for a lent arm the wrapper materialises (`materialize`), runs the legacy `Game::step_flow` arm on the persistent embedded `Game` (its private window fields are the truth of string windows), and loads the written slices back (`TI4_CHURN=state` is exactly this at step granularity, already proven on benches). No argument builders, no islands, quick; but the string windows never leave `Game`, checkpoints must clone them (cost), and the string engine stays in the binary until the composition is ported.
* Either way the invariant is "the lent code sees exactly what the string engine would have seen" and is enforced by the lend-proof mode (R5) and `TI4_CHURN=state` benches.
The audit's output also yields the corrected estimate for D2-8; **stop and report after 8.0**, do not start 8a+ before you have read it (question 1).

**Prerequisites (before 8a):** P1 `disable-xf123` merged (question 4); D2-5r R1-R4 merged; D2-7 merged.

**D2-8a. One driver constructor (the ~190 `Driver::new` sites; mechanical, behaviour-neutral behind a flag; can start before D2-7 as lane L1).**
New `crates/ti4-engine/src/window_driver.rs`: `pub(crate) fn driver_for<W: Stage>(window: W, core: &CoreState, reg: &RegistryCache, table: AbilityTable) -> Driver<W>` = `Driver::from_state(window, core, reg.get(), table)` (`ti4-engine-core/src/driver/mod.rs:149`), and the 100 production sites
(counts in section 0 item 5) call it instead of `Driver::new(window, Resolver::new(&[seat], Some(seat)), AbilityTable::default(), 1)`. `RegistryCache`: the registry derived once per game by `AbilityTable::registry(state, order, name)` (`kernel/ability.rs:~425`) (the adapters today build `reactions_core::linked_table()` or `AbilityTable::default()` and an empty `Registry`, so **no ability opens a reaction window inside an adapter's window**: that is the origin of the counted `nested (unported ability reaction:..)` bails). A global flag `window_driver::armed()` (default **off**) selects the old behaviour (empty registry, `next_id = 1`, `used` from the recorded `ResolverCfg`). With the flag off the refactor changes nothing; with it on, the golden dual
turns bailing nested rows into compared windows or into differences: **that run is END-OF-PHASE ONLY** (it is the first half of G-flip) and each new difference gets a regression test before its fix. The 76 test-side sites are not touched (they build isolated windows on purpose) except where a test goes through a production constructor.
Cheap checks: the lockstep suite with `TI4_AREAS=all=dual TI4_DUAL_FAIL_FAST=1` (4 860 tests today; flag off) and a 2-game dual smoke with the flag on (`--game 0`).

**D2-8b. The wrapper crate content.** Copy `core_sync`, `render`, `resolver_sync`, `reverse_lend`, `lend_table`, `product` from legacy into the facade crate (D2-7 item 3); `Render.codes` borrows `CoreState::texts.codes` (D2_3_RESUME item 3 of "Not done"). The facade stops re-exporting those paths.

**D2-8c. Entry and strict mode.** `Game::new / with_seeded_random / with_table / with_state(state: GameState, ..)` (`game.rs:1659,1669,1700`) run once: `guard::ids_supported`, `guard::factions_known`, `unsupported::check` (Policy `Panic` for products), `Codec::load(ALL)` strict, `Err` -> new `GameError::Unmapped(Unmapped)` (add the variant, `game.rs:101`; the server marks the game unrecoverable and keeps its files like `StorageError::Disabled` of `disable-xf123`). Setup, draft, opening and starting tech stay string and end in this `load` (N4 of `ENGINE_AREA_STATUS`).
**The three switches** (section 0 item 6) get one entry function, `ti4_engine::product_init()` = `product::set_strict(true); core_sync::unmapped::set_strict(true) (or: Unmapped becomes Err and the strict panic is for gates only); core_sync::unsupported::set_disabled_policy(Policy::Panic)`, called from `ti4-server/src/bin/server.rs:10 main` and from the wasm entry (`HostSession::new`, `host_api.rs:456`, once per module; there is no start function), **not** from library code: the dev tools (`ti4-training --diplomacy`, `ti4-review`, `--roster wide`, selfplay, sim, mlp) keep the library default `Off`/non-strict. Decision for you (question 3): the defaults of `Policy` and `strict` stay `Off`/off in the library until D4 deletes the gated core code; dev tools needing disabled features then need `ti4-engine-legacy` directly.
Env `TI4_CORE_STRICT=1` remains the gate/nightly spelling of `product::set_strict`; add it to `end_of_d1.sh`/G7 as a step (X0b): `dual-all` itself must stay un-strict (strict turns a counted bail into a panic and would empty the probe).

**D2-8d. State, checkpoint, surface.** `Game { core: CoreState, cx: Ctx, materialized: OnceLock<Arc<GameState>> (invalidated by every &mut self method), table, content, sources, strategy_cards, events (P1), knobs, lent (islands, Architecture A) | legacy (Architecture B) }`; `Game::state(&self) -> &GameState` fills lazily (`OnceLock`: the server shares `&Game` across a worker). Public field `state` (reads: host 45, server 42, wasm 29, sim 29, training 67, mlp 90, review 24, selfplay 20 per `D2-prereqs.md` measurements; **re-count**) -> `state()`; the 2 writes (host/wasm) -> `Game::edit(|gs| ..)` (materialise, edit, `Codec::load` back; setup tools only). `game.timing` readers (host 2, server 4, review 4, training 1) -> `game.resolver_image()`. `checkpoint()`/`restore()`/`GameSnapshot` = `(CoreState::clone(), DecisionLog, events len, lent windows)`; keep the FORK RULE destructure-without-`..`. Measure with `state_size` (checkpoint bucket target 18.7 % -> ~2 % of a game, `ENGINE_CORE_STATE.md` 6.6).

**D2-8e. Flip the windows.** In dependency order of D1: for each window family switch from "string runs, core compares" to "core runs live, string lent on bail": `dual_window!` gets a real `Core` arm: construct the window, `driver.run`, return `Status::Asking(choice)` to the host, and resume from `Pending` on `answer()` instead of replaying a `Recording`. Bail inside the window (a gate at start: `Lent` immediately; mid-window: roll back to the window start `CoreState::clone`, re-run through the wrapper with the tape of this window's earlier answers (`Bail.when = Mid`, T-C). Order: finished areas first (strategy, agenda, trade, exploration/relics, objectives: they are already `Core` in `build_default()`), then combat/invasion/production machines, leaders/abilities, factions, last the driver arms (`Session::step` exhaustive). One package per family, own lockstep tests with the family on `Dual` and on `Core`. `dual_typed!`/`dual!`/`write` `None` fallbacks become `lent::<label>` calls or a hard `Err(Unmapped{label})` (no wrapper = refusal with the label named).

**D2-8f. Reconcile hooks.** Verify none of the per-step hooks of `Game::step` (`starting_tech::settle`, `announce_staged_ground_events`, `announce_gains`, `space_stations::reconcile_all`, `nexus_is_triggered`, `legendary::settle_control_points`, `keleres::reconcile` (dropped with Keleres), `crimson::enforce_sundered` (dropped), `laws::apply_to_galaxy`, `movement::replay_map_edits`) is still called on the string state unlent. `mirror_timing_log` (`game.rs:~3345`) is replaced by P1's rendering from the typed journal.

**D2-8g. Flip tests (cheap, runners):** `fork_equivalence` (tests dir) and `GameSnapshot` instantiate/continue equals the original; checkpoint/restore around a nested question; the lockstep lib suite with every area `Core` and with `Dual`; `golden check --smoke --mask all=core` (cheap, 4 smallest games per set); `alpha_roundtrip` on a copy. **G-flip (END-OF-PHASE ONLY, once):** `end_of_d1.sh` steps `dual-all` (with `window_driver::armed()` on), `dual-wide`, `board-*`, `sync`, `alpha`, the four bench modes, `census`, `soak`, plus the lend-proof run (R5), `TI4_CORE_STRICT=1` over golden 20 and the lent set, `golden check --mask all=core` and `all=string`, `d4gate`, `ratchet`.

**Traps:** T-J `core.texts.codes` minted during a lent body must be adopted before the slices load (`reverse_lend` does; keep the order if you wrap it). T-K the string resolver's `initiative_order` is fixed at `Game::new`: the image carries it (D2-4a; keep I3 passing). T-L every `&mut self` method of the wrapper invalidates the `OnceLock`; a read of `state()` held across a call is a borrow error by design - do not clone to dodge it. T-M `Window::Untyped` carries `data: [u32;3]` only: never use it to round-trip a string window. T-N the lend cost is per step: measure `with_string` calls per game from the census (the `invasion::finish_committing` / `finish_control_gain` gate rows are in the thousands per 20 games, `LENT_CENSUS.md`, pre-W numbers): if the island/step lend costs more than the checkpoint saving, the CPU A/B (G7 row 7) fails; decide with data from R3 before 8e.
**Estimated:** 8.0 1-2 h; 8a 8-12; 8b 2-3; 8c 4-6; 8d 8-12; 8e 12-25 (Architecture A) or 4-8 (B); 8f/8g 4-6. Total **30-45 h (A)** or **22-30 h (B)**. The earlier 8-12 h only covered 8c/8d.

### 3.5 D2-9 `core-d2-9-prod-strip-final-gate`

`D2-prereqs.md` D2-9 stands with these edits.

1. **Strip** from the production `ti4-engine`: feature `typed-core` (core and factions become plain dependencies), the eight `string-*` features (`string-movement, -objectives, -factions, -trade, -exploration, -strategy, -action-cards, -agenda`), `AreaMask`/`Backend`/`dual*.rs`/`kernel_sync/`/`*_dual.rs`/`*_core.rs` adapters, `set_default_mask`, `TI4_AREAS`, `TI4_DUAL_FAIL_FAST`, `TI4_CHURN`, `TI4_BAIL_PROBE`, `TI4_LEND_PROOF`; `ti4-engine-core-bridge/Cargo.toml:12` loses `features = ["typed-core"]` and gains `ti4-engine-legacy`. Their home after D2-7 is legacy (frozen) and the bridge.
2. **`lend-string` feature** on `ti4-engine` (`dep:ti4-engine-legacy`), enabled by `ti4-server` and `ti4-wasm` in **phase a**. Without the feature `lent::<label>` returns `Err(Unmapped{label})` and the build contains no string engine. `Lent` counters (label, count) exported next to `unmapped`; **phase b gate:** over G7's whole corpus every production-reachable label has count 0; any nonzero label stays in phase a and is a named follow-up port. The user's final structure (9.1/9.4) is reached exactly when the census is empty. Expected remaining string-lent sites are in section 4.
3. **wasm size:** `scripts/wasm-size-gate.mjs` + `build-wasm.mjs`; raw/gzip/brotli before/after; budget 2.4 MB gzip with a ratchet (`ENGINE_CORE_STATE.md` 6.3). In phase a the size stays up (legacy is in); the first number without legacy is phase b.
4. `.cargo/config.toml` aliases and `verify_package.sh`/`end_of_d1.sh` package lists: add `-p ti4-engine-legacy`; `AREA` modes removed; `oracle` replaces `dual-golden`.
5. **G7 (END-OF-PHASE ONLY, one run, `SHARDS=3`/`4`, heavy lane):** rows 1-12 of `D2-prereqs.md` D2-9 item 5 with the updates: row 3 `oracle` (D2-6b(b) corpora) **plus** `golden check --mask all=core` and `all=string` (legacy-independent), row 4 `core_sync_gate profile-20.json 20 10 --strict` and `alpha_roundtrip --strict` on a fresh copy, row 5 the six bench modes (`bench`, `bench-v1..v5`) x `TI4_CHURN` off/resolver/state/resolver+state *on the pre-flip build once*, row 9 adds the production `cargo tree` check (`cargo tree -p ti4-wasm -e normal | grep -c ti4-engine-legacy` prints 0 in phase b, the feature-gated count in phase a), new row 13 `TI4_CORE_STRICT=1` over golden 20 + lent set + bench (0 assertions), row 12 the alpha preflight (`deploy/public/deploy.sh --preflight-only --ref <head> --allow-failed <ids>`; expected failures: `game_a36f40db3b5cc50fabaabcd66a6bad01` (known, canonical hashes), the turn-redo saves `game_935f5618...`, `game_d26c07f1...`; any other failure blocks).
6. After G7 delete nothing. `ti4-engine-legacy` stays (decision 9.1); D3 starts one nightly later; the deletion of the gated diplomacy/Bastion/Crimson/Keleres/... core code is D4.

---

## 4. Lent-body gates (`INTEGRATION7.md` 10.3) and how the flip handles each

All remaining string code is reached **only** through `reverse_lend::with_string` (an in-step label lend, the "reverse lend") and is linked into the production binary **only** with the Cargo feature `lend-string` (`dep:ti4-engine-legacy`, on in server and wasm in phase a). Numbers from `LENT_CENSUS.md` predate the W-lanes: **use the fresh census of X0** (`lent-census.md`).

| Gate / site (10.3) | Label / site | Kind, args, when | Flip handling | Exit from lend-string |
|---|---|---|---|---|
| Action leaders: `empyreanhero` (frontier exploration under whole-state rollback), `yinhero` (eventful ground combat in the timing resolver, `finish_control_gain`), `mahacthero` (fleet move + `combat::resolve_resolving`) | `leaders::use_leader [gate: <id>]`, `leaders::string_only_leader` | Gate, Plain args, Start | wrapper tests the same predicate before choosing the core hero window; whole `use_leader` body through `with_string`, `Footprint::ALL`, tape for questions; needs resolver in/out (supported) | port (W follow-up) or keep; rare |
| Dropped by decision: Last Bastion, Crimson, Keleres, Deepwrought, Firmament, Obsidian, Ral Nel | many `unported_economy`, `crimson::enforce_sundered`, `keleres::reconcile`, ... | not lent | refused at creation and at recovery (P1) and by `Policy::Panic`; no wrapper; the string branches inside shared functions are legacy-only | none (feature removed) |
| Warfare / Imperial inside Naaz / Winnu heroes (`fx_perform::begin -> Began::Unsupported`), Politics draw with a hook, research with a faction waiver, Leadership payment, Cabal/Firmament/Trade-Agreement replenishment, Mahact-held token | `factions::naaz::hero`, `factions::winnu::hero`, `leaders::hacanagent`, `strategy_cards::{trade,leadership,politics}_*` | Window, Typed/Plain, Start (mostly) | whole label through `with_string` (a hero is one window; the nested production window runs inside the string body) | port per census share |
| Thunder's Edge: Saar Deorbit Barrage, Arborec Psychospore, Heart of Ixth in the combat opening, staged Yin breakthrough in `objectives::award_tail`, TE Construction/Warfare primaries | `faction_abilities::perform_component [gate]`, `combat::begin_round`, `objectives::award_tail`, `strategy_cards::te4_construction`, `te6_warfare` | Gate/Window, Typed | lent; **TE is an opt-in content set, 0 of 14 alpha games** (`lent-bodies.md` question 2): alternative is to refuse TE at creation in phase b (your decision, question 6) | port W16b or refuse TE |
| Combat openings pinned by lane 1 (`W2-bail-02`): Jol-Nar commander reroll in barrage, Waylay in barrage, Heart of Ixth, Assault Cannon vs neutral, Munitions Reserves < v8, hits cancelled by a card, staged destructions, a roll that ends the fight, X-89, Hacan flagship / Keleres agent goods | `combat::{barrage_step, begin_round, roll_round, settle, answer}` (**methods on the combat window**) | Window, **Typed** (`Window::Combat(CombatFields)` + `*_back`), Mid for `settle`/`answer` | converter `CombatFields -> string window`, lend the method; Mid needs the tape; Last Bastion / diplomacy rows disappear with the features | port rare ones; Munitions < v8 is a rules-version branch (`rules_version >= 8` games never need it: census) |
| Counted bails inside hero windows: Nekro galvanized force, Yssaril blank laws/hand limit, Cabal test faces | `factions::{nekro,yssaril,cabal}::hero` | Window, Plain | Cabal test faces: product assertion XF-7, unreachable in the product (`product::reproducible`); the others lent whole | port |
| Component actions: Inferno / Fires placement over supply, Nekro or Valefar copies, Naaz mech staged, Hacan breakthrough, Prophecy of Ixth | `faction_abilities::perform_component [gate]`, `factions::muaat::{inferno,fires}` | Gate, Plain | lent whole component action | port |
| **Invasion tail** `invasion::finish_committing` (5b Research Team + forced combats), `invasion::finish_control_gain` (5a control-gain chain, Assimilate, exploration, Fracture) | gate rows, ~1 100-1 500 per 20 games pre-W | **Island** (invasion has no machine state) | the single biggest lend by frequency; decide in D2-8.0 whether it is an island (A) or a typed window (port the invasion machine state, D1b2-bail-02) | port = the lever for phase b |
| `ProductionWindow::step`, `strategy_cards::construction_*`, `te4_construction` (seat the production windows do not carry; production nested in a card) | production labels | Window, Island (production step) | as invasion | port |
| `game::close_vote`, `game::reveal_agenda`, `agenda_effects::resolve_with` | agenda labels | Window, Typed (`Window::Voting`) | lend by label; many `nested (unported ability ..)` reasons were removed by int-7 (Future Sight, Galactic Threat ported) | likely zero after X0 |
| Question halves and typed queries with a string fallback (`run_dual_ask`, `dual_typed!` None) | R3 inventory | Question / Fallback | read-only lends: `Footprint` with empty writes, no tape | port per census |

**Rule for the flip:** a lent site with no wrapper is a hard `Err(Unmapped{label})` in the product (never a silent string fallback, never a panic in release), counted; a site is only allowed to stay lent if the census shows its production-reachable count and a named owner.

---

## 5. Gates: what is cheap, what is END-OF-PHASE ONLY

| Gate | What | Who | When |
|---|---|---|---|
| per package, cheap | `cargo check`/`clippy` of the touched crates (`clippy_core.sh`: bridge/core clean, engine baseline `clippy_engine_baseline.txt` may only shrink), the unit tests of the touched module (`--profile lockstep`), `check_layers.py core`, `lint_core.sh`, `d4_gate.sh 2 3` | the package's Haiku runner | every commit batch |
| lockstep suite with all areas `Dual` | `TI4_AREAS=all=dual TI4_DUAL_FAIL_FAST=1 cargo test --profile lockstep -p ti4-engine --lib` (4 860 tests at int-7) | runner | once per package end (it is in `end_of_d1.sh` `tests` too) |
| **X0** (`end_of_d1.sh`, all 20 steps) | proves D2-1..6a and produces the probe + census | coordinator | **END-OF-PHASE ONLY**, first |
| **G-flip** | D2-8a `armed()` dual, wide dual, lend-proof (R5), `TI4_CORE_STRICT=1` run, golden checks, benches x4 churn modes, soak, ratchet | coordinator | **END-OF-PHASE ONLY**, after D2-8g |
| **G7** | section 3.5 item 5 | coordinator | **END-OF-PHASE ONLY**, last |
| mutation | `mutants.sh` single-file on new core-owned code (equivalent mutants marked `// mutants:skip` with a reason); ratchet only in G-flip/G7 | runner / coordinator | per package / gate |

Every failure of a gate gets a regression test before its fix (standing rule). Standing rules, the runner brief template and the workflow rule of `D2-prereqs.md` section 6 apply unchanged to every brief below.

---

## 6. Traps (summary; package-local traps are in the sections)

| # | Trap | Where | What to do |
|---|---|---|---|
| T1 | The probe sees `run_dual_window` bails only (finding 2) | R1, R3 | `lent_sites.py` + `bail_table --check` |
| T2 | `TI4_DUAL_FAIL_FAST=1` in `dual-all` truncates the probe on the first difference | R1.1 | fix, re-run `dual-all`, never freeze a partial table |
| T3 | Footprint too small is a production error, not a silent corruption (audit refuses and stores nothing) | R4 | widen; prove with R5 |
| T4 | Mid-window bail after the player answered | R2, 8e | tape of the window's earlier answers; roll back to the window start |
| T5 | `Window::Untyped` cannot carry a string window | 8.0 | island (opaque struct in the wrapper) |
| T6 | `dual_window!` `Core` arm today runs the string (finding 3) | 8e | write the live arm; do not assume `Backend::Core` runs the core for windows |
| T7 | Arming the registry in the adapters changes which nested windows open | 8a | flag default off; golden dual with it on is part of G-flip |
| T8 | Strict product assertions turn a counted bail into a panic | 8c, X0b | never combine `TI4_CORE_STRICT=1` with the probe run |
| T9 | `disable-xf123` conflicts in 5 files; it also edits `setup.rs` next to D2-1's `guard` calls | P1 | resolve keeping both guards; the creation refusal and `guard` are different checks |
| T10 | Path-bound artifacts (section 1 table, 145 refs in `tools/card_audit`, 4 544 rows in `test_disposition.tsv`) | D2-7 | one commit per group, `git grep` empty at the end |
| T11 | Live-branch drift (1 engine file today) | D2-7 | merge live into the line right before the move and right after |
| T12 | Nightly rule: whatever lands before 20:30 Berlin is in that night's sweep | all merges | check `TZ=Europe/Berlin date`; the move never in the last hour; the integration line is not the live branch, so the sweep is unaffected until the line is merged |
| T13 | Process-wide switches in tests trip parallel tests | 8c | thread-local `with_*` forms in tests; `// mutants:skip` on the setters (as done) |
| T14 | Same `CARGO_TARGET_DIR`, `/tmp` names or alpha copy in parallel runners | all | own scratch worktree and target dir per runner; `mktemp -d` for the alpha copy; `alpha_roundtrip` refuses the live directory |

---

## 7. Open questions for you

1. **Architecture of the flip (A: core owns the loop, islands for invasion/production/aftermath; B: embedded legacy `Game` as the lender).** Needs the 1-2 h audit D2-8.0 first. *Recommend:* run the audit, then A if there are at most three islands, else B as a scaffold for one nightly and A afterwards; either way the string engine stays in server/wasm in phase a.
2. **Is a flip estimate of 30-45 h (A) / 22-30 h (B) acceptable, instead of 8-12 h?** The difference is the core-side composition and the live window driving (finding 3). *Recommend:* yes; the alternative is to stop at "state authority only" (B without any core-run arm) and keep D3 waiting.
3. **Defaults of `Policy` and strict in the library: `Off`/off until D4, product binaries call `product_init()`?** *Recommend:* yes (dev tools with diplomacy/wide rosters keep working on the core's still-present rules); at D4, tools needing the disabled features depend on `ti4-engine-legacy` directly.
4. **Merge `disable-xf123` into the integration line now (5 conflicts, product edges for diplomacy, deck reserve, Last Bastion, Crimson), and add Keleres/Deepwrought/Firmament/Obsidian/Ral Nel to its refusals (the engine already lists them as disabled)?** *Recommend:* yes, before D2-8; it is the creation/recovery refusal the flip relies on.
5. **May the move/flip commits edit `decision_delivery_inventory.rs` rows and `content_ids_resolve.rs` scan roots (rename/path-only, no assertion change) and the `.tsv`/baseline path columns?** *Recommend:* yes, each listed in the commit message and report.
6. **Thunder's Edge: keep lent through phase a, or refuse the content set at creation in phase b?** 0 of 14 alpha games use it; its remaining gates are Saar/Arborec breakthroughs, Heart of Ixth, TE Construction/Warfare. *Recommend:* decide after X0's census; refuse TE at creation if its lent count is nonzero and porting is not scheduled.
7. **Add the `dual-lent` step (golden-lent set) and a `TI4_CORE_STRICT=1` step to `end_of_d1.sh` now, before the first X0?** *Recommend:* yes; the run is expensive and these two are the cheapest additions that widen the table and the strict proof.

---

## 8. Ready-to-paste briefs (paste the STANDING RULES + WORKFLOW RULE blocks of `D2-prereqs.md` section 6 above each)

**Brief X0 (coordinator).** > Run `CARGO_TARGET_DIR=/root/deploytmp/eod tools/engine_core/end_of_d1.sh --out /root/end-of-d1-<date>` from a worktree at the newest end-of-D1 commit in a quiet window (check `TZ=Europe/Berlin date`; not 20:30-24:00). Afterwards run `tools/engine_core/lent_census.py OUT/dual-all.log --games 20 --md > OUT/lent-census.md`. Hand back `summary.txt`, the failed steps (each with its first error), `bail-probe.tsv`, `dual-all.log`, `bails.rs.txt`, `lent-census.md`. A failed `dual-all` means the probe is truncated: say so.

**Brief D2-5r.** > Package D2-5r, branch `core-d2-5r-bails-and-wrappers`, worktree `.claude/worktrees/core-d2-5r`, cut from the newest `core-integration-N`. Read `docs/todo/D2-remaining.md` 3.1 and 4, `crates/ti4-engine/src/{lend_table.rs,reverse_lend.rs,core_sync/materialize.rs,dual.rs (run_dual_window, note_gate)}`. Start with R3 (`tools/engine_core/lent_sites.py`, no dependency); when X0's `OUT` exists do R1 (consume the probe), R2, R4 (wrappers, plain-args first, two agents split by file group), R5. No behaviour change, no rule-body edit except pure moves to extract a `*_string` body. Hand back: `lent_sites.tsv` summary (sites per macro and class), the `BAILS` table (label, calls, bails, share, writes, kind/args/when), the labels that need a typed converter or are islands (with minutes), the lend-proof result on 1 game.

**Brief D2-6b(a).** > Package D2-6b(a), branch `core-d2-6b-oracle`, worktree `.claude/worktrees/core-d2-6b`. Read `docs/todo/D2-remaining.md` 3.2, `crates/ti4-engine-core-bridge/src/{allow_list.rs,dual_shard.rs,workload.rs,bin/dual_run.rs}`. Build the `oracle` bin with the `Subject` trait and the two mask-based subjects, shards/merge reusing `dual_shard`, the allow-list judge and stale check, the skip filter, and the four tests of 3.2. Do not touch the engine crate. Hand back a sample report and the per-corpus step counts.

**Brief D2-7.** > as `D2-prereqs.md` section 7 (D2-7) with the changes of `D2-remaining.md` 3.3 (longer path list, merge live first, glue stays in legacy, wire types re-exported, announce the new path). Start only when the coordinator announces the freeze and P1 is merged; not in the last hour before 20:30 Berlin.

**Brief D2-8.0.** > Package D2-8.0, branch `core-d2-8-audit`, read-only plus one doc. Produce `docs/D2_8_AUDIT.md` as specified in `D2-remaining.md` 3.4 (window fields x `WindowKind`/`Window`, `Session::step` lent arms, `D1b*-bail-*` rows; classes Core/Portable/Island; the A/B recommendation with counts; a corrected estimate of 8a-8g). Stop after the doc and report.

**Brief D2-8a.** > Package D2-8a, branch `core-d2-8a-driver-for`, worktree `.claude/worktrees/core-d2-8a`. Read `D2-remaining.md` 3.4 (8a) and `crates/ti4-engine-core/src/driver/mod.rs`, `crates/ti4-engine/src/{dual_window.rs,reactions_core.rs,resolver_sync.rs}`. Replace the 100 production `Driver::new` sites by `window_driver::driver_for` with the flag `armed()` default off; with the flag off nothing may change (lockstep suite with all areas `Dual`, 2-game smoke). Do not run the armed dual; hand back the site list and the smoke result with the flag on for one game.

**Briefs D2-8b..g, D2-9:** written after the D2-8.0 audit (they depend on A/B); the content of 3.4 and 3.5 is the checklist.
