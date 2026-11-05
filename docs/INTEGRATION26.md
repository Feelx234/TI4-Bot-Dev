# core-integration-26 (2026-11-05)

Branch `core-integration-26`, cut from `core-integration-25` @ `3e7a099ab` (deployed to alpha; `RULES_VERSION` 36). Not pushed, not deployed. The live branch `nightly-fixes-2026-10-26` was not touched; the nightly cron stays disabled.
Purpose: one line for the re-run of heavy run A ("round 2", `docs/HEAVY_RUN_PLAN.md` section 12.4): the fixes for the four steps that failed in run A (`dual-lent`, `zero-bail`, `soak`, `d4gate`), the native game setup, and the kernel speed-ups.

## 1. Merges (in this order)

| # | Branch | Tip | Conflicts and joins |
|---|---|---|---|
| a | `core-duallent-fix` | `e8fab2758` | none. The strict panic of `dual-lent` (a deck text that names no exploration deck is a probe, not an unmapped value: Crown of Emphidia offers `MR` / `FACTION` planet types). |
| b | `core-lent-gaps` | `f7549b6d9` (on a) | none. The families the lent set and the soak showed behind the panic (Pillage chain in windows lent no map, Decks and Marks of nested windows, neutral owner, TE faction menu gap, ...); lane `old` out of the default soak plan (`REFUSED = {"old"}`). |
| c | `core-zerobail` | `d51153049` (on a) | none by git. `zero_bail.py` / `end_of_d1.sh` (zero-bail step: native evidence + classes table) and the D4 stage probe `driver/probe.rs`. Joined textually with b in `end_of_d1.sh` (b touches the `census`/`corpus` area, c the `zero-bail` step) and `test_soak.sh` / `soak_corpus.py` (b only). |
| d | `core-setup-native` | `973a8aa56` (3 commits) | none by git. New crate `ti4-engine-core-setup` (layer L7: `start_game(spec, galaxy) -> CoreStart`, no string `GameState`); `Cargo.toml` members/dependency, `.cargo/config.toml` sets (`c-web`, `l-web`, `t-web`), `check_layers.py setup` mode, `ENGINE_LAYOUT.md` row, the live kernel tables moved into the crate (`ti4_engine_core_setup::kernel`). Joined cleanly with b/c (`check_layers.py` and `end_of_d1.sh` hunks apart). |
| e | `core-perf-impl` | `f0e108bcb` (7 commits, contains `core-perf-kernel` `a21a6a240`) | **2 conflicts, both resolved without dropping a side.** (1) `crates/ti4-engine-next/src/live/kernel.rs`: d moved the whole file's content into the setup crate and left a one-line re-export; e added `exact_static` (the kept `Env` and the lazy play facts read it) to the old file. Resolution: `exact_static` moved into `crates/ti4-engine-core-setup/src/kernel.rs` (next to `static_of`, same `KERNELS` table), the re-export lists it. (2) `crates/ti4-engine/src/reactions_core.rs` (`follow`): d adds the `table.facts.roster = roster_for(..)` line, e changes `plays = play_facts(..)` to `.into()` (lazy `Plays`). Both lines kept. Semantic join checked: `live/env.rs` (e) reads `facts.roster` set by d's `ties::technology_core::roster_for` and builds `PlaysSource { kernel: exact_static(kernel), roster }`; the live `static_of` / `exact_static` pair compare pointers into the one `KERNELS` table of the setup crate, so a kernel built by the live loop is still recognised. Proof: the suite hash and the 24 perf games below. |
| f | `core-perf-flags` | `e6432285b` | none. Docs and scratch profiles only (`docs/PERF_FLAGS.md`, `docs/perf_flags_data/`). |

Not merged: nothing pending.

## 2. Own commits on the line

1. Merge commit of e with the two resolutions above.
2. `ti4-wasm` test import error fixed (B9 below).
3. `end_of_d1.sh`: the `lint` base is tried as `heavy-base-int26` first (then int25, int24, core-integration-24, origin); `docs/HEAVY_RUN_PLAN.md` header, preflight, 12.3 (fresh target `/root/deploytmp/eod26`, out `-r2`, detached) and the new 12.4. `web/`, `state.rs` and `docs/RULES_VERSION.md` of the tip equal `core-integration-25` (checked, empty diff), so the lint step is green against either tag.
4. This document, `docs/I26_RESUME.md`. Local tag `heavy-base-int26` = the tip.

## 3. Bugs found

| # | Where | What | Fix |
|---|---|---|---|
| B9 | `crates/ti4-wasm/tests/it/main.rs` | the test module `wire_types` uses `ti4_wasm::wire_ts`, which exists only with the `ts` feature: `cargo test -p ti4-wasm` (and the aliases `c-web`, `t-web`, `l-web`, which name the package without the feature) failed to compile with `unresolved import ti4_wasm::wire_ts`. Reported by core-perf-impl as "pre-existing and unrelated": **real, and pre-existing** (`lib.rs` and the test are unchanged against int-25, int-24: workspace builds hide it, because another package turns `ts` on by feature unification). The heavy run was never hit because its step runs `cargo test -p ti4-wasm --features ts`. | `#[cfg(feature = "ts")] mod wire_types;` with a comment. The test still runs (and passes) with `--features ts`; without it the other 47 tests of the package run. |
| - | the six joins | no new engine bug at the joins; the two conflicts were API-location moves, not behaviour. | - |

Observation (not a bug): the wasm grew from 6.45 MB (int-25) to 6.60 MB raw (+0.15 MB: the setup crate, the zero-bail probe, the lent fixes, the perf code); gzip 2.10 / brotli 1.51 MB. All three are inside the budget 6.63 / 2.11 / 1.52 MB but only by 0.03 MB / 0.01 / 0.01. The next wasm-touching change needs a budget decision.

## 4. Matrix (private target `/root/deploytmp/eod26`, `CARGO_INCREMENTAL=0`, run on the merged tip; logs `/root/deploytmp/i26-logs/`)

| check | result |
|---|---|
| `cargo check --workspace` (without review, replayer, mlp, tensor, advisor) `--all-targets`; `-p ti4-engine-core-tests --features wrapper --all-targets`; `-p ti4-host --features ts -p ti4-wasm` | green |
| `clippy_core.sh` | ok, engine warnings 41 = baseline; `cargo l-web` (web crates `--all-targets`) green |
| `check_layers.py core` / `thin` / `setup` | 576 / 192 / 14 files, 0 violations each |
| `check_production_graph.sh` (strict) | 0 violations, 0 failed controls |
| wasm (`web-shared/scripts/build-wasm.mjs`, wasm-release + binaryen 123) + `scripts/wasm-size-gate.mjs` | 6,596,536 B (6.60 MB) raw / 2.10 gzip / 1.51 brotli against 6.63 / 2.11 / 1.52: ok (tight, see above) |
| `check_output_probes.sh` on the wasm | ok: no legacy literal |
| `native_play --quick --shadow --twice` | 1,165 steps, fingerprint `538dc68f23dddcc9`, shadow compared 808, ZERO GAP, deterministic |
| `perf_kernel suite 1` (gate) | `suite_hash 775f4136e60a99d5` (14 games, 14,738 steps); with `PERF_KERNEL_CHECK=1` (board indexes, lazy facts, kept `Env` verified at every step) the same hash |
| perf_ab `core smart` 3p/4p/5p/6p x seeds 1-6 | 24 of 24 finished, `errors` 0, `bad` none; chain, steps, decisions equal to the int-25 log (`/root/deploytmp/i25-logs/perf-ab.log`) in all 24 |
| tests (gate profile): core crates (`ti4-engine-core`, `-factions`, `-tests`) | 2,708 passed, 0 failed, 1 ignored |
| setup crate (`ti4-engine-core-setup`) | 7 passed |
| bridge (all targets, incl. `wrapper_native`, `kernel_env`, `setup_native_parity`, `host_create_golden`) | 239 passed, 0 failed; `host_create.golden` unchanged against int-25 |
| thin crates (`ti4-engine`, model, content, rule-bot, ti4-bridge) | 959 passed, 0 failed, 2 ignored |
| host / wasm host (`--features ts`) / server (`--test-threads=2`) | 320 / 58 (3 ignored) / 415 (8 ignored), 0 failed; `ti4-wasm` without features (after B9): 47 + 9 passed |
| `state_basics` in the dev profile (debug assertions on) | 14 passed |
| web `tsc --noEmit`, `vitest run` | clean; 278 files passed, 2 skipped; 2,750 tests passed, 4 skipped |
| `golden check` (200 games, default mask) | 200 games, 147,781 decisions, 0 failed (40.6 s) |

## 5. Not run (heavy run, by order)
Golden dual (the lane evidence: 0 differences on the golden 20 and the lent set in `LENT_GAPS_RESUME.md`), Z-gate, mutation ratchet, dual-drv / dual-wide / dual-lent / strict, the six bench verifies, soak, census, zero-bail, D4 gate. They run in round 2 of run A from the tip tagged `heavy-base-int26` (see `docs/I26_RESUME.md`).
