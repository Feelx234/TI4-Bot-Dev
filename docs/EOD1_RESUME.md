# End-of-D1 combined run: state and how to resume (branch `core-integration-8`, tip see `git log`)

Written by the end-of-D1 agent. Rule from the user: the heavy gate runs ONCE, when a full playthrough is possible on the new engine with zero
bails on the agreed content (all but Thunder's Edge and the dropped factions). Nothing heavy is running for this branch except the
FIRST run's mutation ratchet (see 4), which is left to finish.

## 1. What the first run (`/root/deploytmp/eod1-out`, tree `8bafe070`, `SHARDS=3`) found

| step | result | cause / fix |
|---|---|---|
| layers, lint, build | ok | |
| clippy | ok (gate rewritten: `clippy_core.sh`, 42 old engine warnings in `clippy_engine_baseline.txt`) | |
| tests | ok, 12438 passed, 0 failed (packages, lib all=dual fail-fast, D4 crate, gen-cards, xtask, wasm) | |
| dual-all | FAIL, 3 shards panic at once | strict-mode refusals at seating: `empyrean::deal_notes` dual loads a table whose seats/notes are still `generic` (fixed, `ce.rs`), then `Map` slice without galaxy in the `cc`/`ca` call-site duals (fixed), then golden games 1/3/11/13/17 `invasion_seq` (fix `core-eod1-b`), game 3 `combat::settle` (fix `core-eod1-a`, round_seq `core-eod1-d`), game 16 Transit Diodes planet order (fixed here), game 18 barrage label (fix `core-eod1-d`) |
| dual-drv | FAIL | games 1, 3, 18 (the fixes above) |
| dual-wide | FAIL | `invasion_seq` (games 1, 11, 13, 17), as above |
| board-t, board-g, sync, alpha | ok | |
| bench-off, bench-resolver | ok, 200/200 identical x6 | |
| bench-state, bench-resolver+state | FAIL: bench-v5 seed 85 "mark not typed private:#staged:ground:00000001" | staged ground destruction of a galvanized unit had no typed shape (fixed, `mark_values.rs` + new word `g`) |
| census | FAIL (no `bail-probe.tsv`: dual-all failed) | dependency now explicit |
| soak | "ok" with INCOMPLETE slices | script now fails on INCOMPLETE |
| d4gate | FAIL: step 4 coverage (`d4-covers.txt` was 172 MB of torn lines from parallel appends) | `core-eod1-c` / `core-d4a4` |
| ratchet | running on the old tree, see 4 | |

Not run in the first run: `dual-lent`, `strict`, `corpus`, `zero-bail` (added afterwards).

## 2. What is on the branch now

* the four weak spots of `end_of_d1.sh` fixed; new steps `dual-lent`, `corpus` (decision corpus replay), `zero-bail` (ZERO_BAIL=report|enforce),
  `strict` (TI4_CORE_STRICT=1); soak INCOMPLETE fails; 24 steps in all (`tools/engine_core/end_of_d1.sh --dry-run`).
* `core-eod1-d` (a, b, c, and the round_seq/barrage fixes) and `core-d4a4` merged; own fixes with regression tests (each verified to FAIL on the
  pre-fix code, except where noted): round_seq re-read per round (`combat_machine.rs` test; the engine test passes without the fix),
  seating strict refusals (`ce.rs` x2), Map-slice refusals (`cc.rs`, `ca.rs`; the two tests were verified failing before their fixture lost the generic
  notes, not re-verified after), galvanized staged mark (`keys/tests.rs`), Transit Diodes planet order (`technology_dual.rs`), word prefix table.
* cheap chain on the merged tree (`/root/deploytmp/eod1-cheap`): layers 0 violations, lint 0 hits, `cargo check` of the 11 crates all targets ok, `clippy_core.sh` ok,
  engine lib 4871 passed with and without `TI4_AREAS=all=dual TI4_DUAL_FAIL_FAST=1`, D4 crate (D4 flags) 1268 passed, core + factions + bridge + typed bot lib 623 passed.
* unchanged versus `core-integration-6` (the lint step's `git diff --exit-code`): `RULES_VERSION` doc, `ti4-model/src/state.rs`, `web/`; also `fingerprint.rs` and `ti4-model`.

## 3. The starting list of the zero-bail rule

`docs/EOD1_ZERO_BAIL_START.txt`: 41113 bails/gates in 118 causes over the 20 golden games (`all=dual`, single-process runs of the pre-merge binary, one
file per game). The biggest are the driver's lent arms (`d1::step`: tactical 7815, aftermath 5438, turn_prelude 3344, ask.closing 3137, voting 2380, trade 1293,
tokens 1142, ask.draft 863; `turn::hooks` minister_of_war / legendary::end_turn / technology::end_turn 3344 each; `d1::reconcile` announce_staged_ground_events 1996),
then the string-run first-claim exploration gate (697), Titans Coalescence (558), Sardakk agent, Scanlink Drone Network, and 100+ rare ones.

## 4. The ratchet of the first run

`/root/deploytmp/eod1-out/mutation` (started by the first run on tree `8bafe070`, floors in `tools/engine_core/mutation_floor.json`). Scores so far are in the report.
The main worktree `/root/TI4-Bot-Dev/.claude/worktrees/core-integration-8` still holds that old tree; when the ratchet is done run
`git -C /root/TI4-Bot-Dev/.claude/worktrees/core-integration-8 reset --hard core-integration-8` (the branch ref points at the merged tip; the fix worktree
`core-integration-8-fix` is the same commit and may be removed).

## 5. Command for the combined run (after lanes 9/10 are merged)

```
cd /root/TI4-Bot-Dev/.claude/worktrees/core-integration-8           # after the reset above
CARGO_TARGET_DIR=/root/deploytmp/eod1 ZERO_BAIL=enforce tools/engine_core/end_of_d1.sh --out /root/deploytmp/eod1-final
# resume after a stop:  ... --out /root/deploytmp/eod1-final --from <step>      (census, zero-bail need dual-all / dual-lent in the same --out)
```
It holds one lane of `/root/heavy2.sh` for the whole run (SHARDS=3); keep the target under ~8 GB (delete `eod1-fix` first: `rm -rf /root/deploytmp/eod1-fix`).
Known weak spots left: the `bc` syntax error line in `verify_benches.sh` output when a bench fails (cosmetic); `dual-lent` and the zero-bail corpus are the
20 golden + the lent set, not yet every lobby-seatable faction (soak lane `factions`); the probe does not see gate rows of `dual_typed!`/`dual!`/`dual::write`
(the `lent_sites.py` inventory of `docs/todo/D2-remaining.md` R3).
