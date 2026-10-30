# D2 lane A (D2-1 fixtures/totality/alpha round trip, D2-2 disabled-feature panics): resume note

Branch `core-d2-1` (cut from `core-integration-5` @ `670963aa`). **Gate deferred to the end of D1** (user directive 2026-10-30:
no heavy work until then). Everything below was checked with cheap runs only, plus ONE partial heavy run before the directive
(see "Heavy run already made").

## State

Both packages are implemented and committed; nothing is half done. What is left is only the combined heavy gate on the final
commit (and a clippy re-check of one test line, see below).

Next steps, in order:
1. Merge `core-d2-1` into the integration line (conflicts to expect: `core_sync/lend.rs`, `core_sync/codec/mod.rs`, `core_sync/mod.rs`
   with lane B / D2-3; `ti4-host/src/setup.rs` with `disable-xf123`, see below).
2. Gate (once, after D1): `flock`/`/root/heavy2.sh`, `SHARDS=3` or `4`, `SKIP_AB=1 SKIP_WASM=1`, private `CARGO_TARGET_DIR`:
   `EXTRA_DUAL=movement=dual,board=dual,technology=dual,objectives=dual,kernel=dual,combat=dual,invasion=dual,production=dual,trade=dual,exploration=dual,action_cards=dual,leaders=dual,strategy=dual,agenda=dual,factions=dual tools/engine_core/verify_package.sh driver OUT none 6`
   (**`EXTRA_DUAL` entries must be `name=dual`**: my first run wrote bare names and the golden dual refused them with "want area=backend")
   then `board_equivalence templates` and `board_equivalence golden tools/engine_profile/profile-20.json 20 10`.
   The script now also runs `core_sync_gate --strict`, `marks_census.py --strict` and `alpha_roundtrip --strict` (on a fresh copy of `/var/lib/ti4-public`).
3. Re-run only: `dual-suite` and `dual-golden` with correct `EXTRA_DUAL`, and `clippy`.

## Heavy run already made (commit 6ceb98f8, `driver` + wrong EXTRA_DUAL, SHARDS=3, 04:50-05:34 UTC)

| Step | Result |
|---|---|
| layers / lint_core | 0 violations / 0 hits (402 / 404 files) |
| immutable surfaces | ok |
| clippy | 1 warning in my files (`is_faction` unused result in a test), fixed in `574099f8`; NOT re-run |
| build selfplay, bridge | ok |
| roundtrip (`core_sync_gate --strict`) | 3 102 positions (templates 100, fixtures 3, golden 2 999), fully loaded 3 102, 0 mismatches, 0 core diffs, `unmapped: 0` |
| marks census | 0 unknown templates, 13 unresolved sites (all reviewed) |
| alpha_roundtrip --strict (copy) | 14 games listed, 11 checked, 3 skipped (turn-redo), 0 failed; 1 030 positions; 0 differences; unmapped 0; max owners 8, texts 32, blobs 3 |
| tests (44 suites), gen-cards-check, tests-xtask, tests-wasm | ok |
| dual-suite | "6 suites ok" but with malformed `EXTRA_DUAL`: re-run |
| dual-golden | FAILED to start (malformed `EXTRA_DUAL`): re-run |
| benches `bench`, `bench-v1..v5` | 200/200 identical x6; `bench` with driver on String 200/200 |
| board_equivalence templates / golden | 0 mismatches, unmapped 0 (100 / 2 999 positions) |

Cancelled by the directive: the re-run of clippy, dual-suite and dual-golden.

## Cheap checks done (Haiku runners, `--profile lockstep`)

* `ti4-engine --lib core_sync::` 65 passed; `dual::` 14; `content_ids_resolve` 4; `ti4-host --test creation_guard` 3;
  `ti4-engine-core-bridge --test alpha_roundtrip` 3; `ti4-engine-core --lib unsupported` 10, `driver::` 39, `reserve` 5;
  `ti4-engine-core-factions --lib unsupported_tests` 5.
* Smoke on the bins: `core_sync_gate profile-20 3 10 --strict`, `alpha_roundtrip` on a copy, `board_equivalence templates`,
  `dual_run ... --game 0` with `driver,combat,invasion=dual`: all exit 0.
* Mutation (single-file, `tools/engine_core/mutants.sh`): `ti4-engine-core/src/unsupported.rs` 10/10 killed; `core_sync/unmapped.rs` 37/37;
  `core_sync/unsupported.rs` 10/10; `core_sync/guard.rs` 14/15, the survivor (`a.len() < b.len()` -> `<=`) is equivalent (equal lengths are refused
  earlier as "used twice") and marked `// mutants:skip`. The process-wide switches (`set_policy`, `set_strict`, `set_disabled_policy`, their
  `_for_gate` variants) are marked `// mutants:skip` with the reason (flipping them in a test process would trip every parallel test); their pure decision
  helpers (`encode/decode`, `env_is`, `gate_strict`, `core_policy`, `gate_policy`) are tested and killed.

## What the code does (short)

* `core_sync::unmapped`: census of every refusal by (slice, reason); strict mode; `DualReport.unmapped` (+ shard JSON).
* `fixtures::{seated_dealt, real_game, redeal_notes}`; the workarounds of `explore_write`, `vote`, `action_cards` use them.
* `core_sync::guard` (`ids_supported`, `factions_known`) called at the top of `ti4_host::setup::create`.
* `ti4-engine-core-bridge`: `roundtrip` (shared per-position check), `alpha` + bin `alpha_roundtrip`, `core_sync_gate` on the shared check.
* `tools/engine_core/marks_census.py`, `verify_package.sh` steps `roundtrip`, `marks-census`, `alpha-roundtrip`.
* D2-2: `ti4_engine_core::unsupported` (`Disabled`, `Policy::{Off, Panic}`, `guard`, `hit`, `faction_in_play`) with hits at the sites; `core_sync::unsupported`
  (`Policy::{Off, Bail, Panic}`, `check`) at `Codec::load`, `Lend::{with_core, region, view}`, `CoreView::of`, `dual_window::tables_with`. **Default is `Off`, not `Bail`**
  (deviation from `docs/todo/D2-prereqs.md`: a Bail default would zero the `compared >= 1` of lockstep tests that follow Bastion/Crimson tables). Gate binaries set `Panic`.
